use super::{
    amm_calc::{amm_buy_get_token_out, calculate_with_slippage_buy},
    dex_traits::DexTrait,
    pumpfun_common_types::{BuyInfo, SellInfo},
    pumpfun_types::*,
    types::{Create, PoolInfo, SwapInfo},
};
use crate::{common::trading_endpoint::TradingEndpoint, errors::trading_endpoint_error::TradingEndpointError, instruction::builder::PriorityFee};
use borsh::BorshSerialize;
use once_cell::sync::OnceCell;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
};
use spl_associated_token_account::{get_associated_token_address, instruction::create_associated_token_account};
use std::sync::Arc;

pub struct Pumpfun {
    pub endpoint: Arc<TradingEndpoint>,
    pub global_account: OnceCell<Arc<GlobalAccount>>,
}

#[async_trait::async_trait]
impl DexTrait for Pumpfun {
    async fn initialize(&self) -> Result<(), TradingEndpointError> {
        let account = self.endpoint.rpc.get_account(&PUBKEY_GLOBAL_ACCOUNT).await?;
        let global_account = bincode::deserialize::<GlobalAccount>(&account.data).map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;
        let global_account = Arc::new(global_account);

        self.global_account
            .set(global_account)
            .map_err(|_| TradingEndpointError::CustomError("OnceCell already set".to_string()))?;
        Ok(())
    }

    fn initialized(&self) -> Result<(), TradingEndpointError> {
        if self.global_account.get().is_none() {
            return Err(TradingEndpointError::CustomError("Pumpfun not initialized".to_string()));
        }
        Ok(())
    }

    fn get_trading_endpoint(&self) -> Arc<TradingEndpoint> {
        self.endpoint.clone()
    }

    fn use_wsol(&self) -> bool {
        false
    }

    async fn get_pool(&self, mint: &Pubkey) -> Result<PoolInfo, TradingEndpointError> {
        let bonding_curve_pda = Self::get_bonding_curve_pda(mint)?;
        let account = self.endpoint.rpc.get_account(&bonding_curve_pda).await?;
        if account.data.is_empty() {
            return Err(TradingEndpointError::CustomError(format!("Bonding curve not found: {}", mint.to_string())));
        }

        let bonding_curve = bincode::deserialize::<BondingCurveAccount>(&account.data).map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;

        Ok(PoolInfo {
            pool: bonding_curve_pda,
            creator: Some(bonding_curve.creator),
            creator_vault: Some(Self::get_creator_vault_pda(&bonding_curve.creator)?),
            config: None,
            token_reserves: bonding_curve.virtual_token_reserves,
            sol_reserves: bonding_curve.virtual_sol_reserves,
        })
    }

    async fn create(&self, payer: Keypair, create: Create, fee: Option<PriorityFee>, tip: Option<u64>) -> Result<Vec<Signature>, TradingEndpointError> {
        let mint = create.mint_private_key.pubkey();
        let buy_sol_amount = create.buy_sol_amount;
        let slippage_basis_points = create.slippage_basis_points.unwrap_or(0);

        let create_info = CreateInfo::from_create(&create, payer.pubkey());
        let mut buffer = Vec::new();
        create_info
            .serialize(&mut buffer)
            .map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;

        let blockhash = self.endpoint.rpc.get_latest_blockhash().await?;
        let bonding_curve = Self::get_bonding_curve_pda(&mint)?;

        let mut instructions = vec![];
        let create_instruction = Instruction::new_with_bytes(
            PUMPFUN_PROGRAM,
            &buffer,
            vec![
                AccountMeta::new(mint, true),
                AccountMeta::new(*PUBKEY_MINT_AUTHORITY_PDA, false),
                AccountMeta::new(bonding_curve, false),
                AccountMeta::new(get_associated_token_address(&bonding_curve, &mint), false),
                AccountMeta::new_readonly(*PUBKEY_GLOBAL_PDA, false),
                AccountMeta::new_readonly(mpl_token_metadata::ID, false),
                AccountMeta::new(mpl_token_metadata::accounts::Metadata::find_pda(&mint).0, false),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(solana_program::system_program::ID, false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(spl_associated_token_account::ID, false),
                AccountMeta::new_readonly(solana_program::sysvar::rent::ID, false),
                AccountMeta::new_readonly(PUBKEY_EVENT_AUTHORITY, false),
                AccountMeta::new_readonly(PUMPFUN_PROGRAM, false),
                AccountMeta::new_readonly(PUMPFUN_PROGRAM, false),
                AccountMeta::new_readonly(PUMPFUN_PROGRAM, false),
            ],
        );

        instructions.push(create_instruction);

        if let Some(buy_sol_amount) = buy_sol_amount {
            let create_ata = create_associated_token_account(&payer.pubkey(), &payer.pubkey(), &mint, &spl_token::ID);
            instructions.push(create_ata);

            let buy_token_amount = amm_buy_get_token_out(INITIAL_VIRTUAL_SOL_RESERVES, INITIAL_VIRTUAL_TOKEN_RESERVES, buy_sol_amount);
            let sol_lamports_with_slippage = calculate_with_slippage_buy(buy_sol_amount, slippage_basis_points);
            let creator_vault = Self::get_creator_vault_pda(&payer.pubkey())?;
            let buy_instruction = self.build_buy_instruction(
                &payer,
                &mint,
                Some(&creator_vault),
                &get_associated_token_address(&payer.pubkey(), &mint),
                SwapInfo {
                    token_amount: buy_token_amount,
                    sol_amount: sol_lamports_with_slippage,
                },
            )?;
            instructions.push(buy_instruction);
        }

        let signatures = self
            .endpoint
            .build_and_broadcast_tx(&payer, instructions, blockhash, fee, tip, Some(vec![&create.mint_private_key]))
            .await?;

        Ok(signatures)
    }

    fn build_buy_instruction(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        creator_vault: Option<&Pubkey>,
        token_account: &Pubkey,
        buy: SwapInfo,
    ) -> Result<Instruction, TradingEndpointError> {
        self.initialized()?;

        let buy_info: BuyInfo = buy.into();
        let buffer = buy_info.to_buffer().map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;
        let bonding_curve = Self::get_bonding_curve_pda(mint)?;

        Ok(Instruction::new_with_bytes(
            PUMPFUN_PROGRAM,
            &buffer,
            vec![
                AccountMeta::new_readonly(PUBKEY_GLOBAL_ACCOUNT, false),
                AccountMeta::new(PUBKEY_FEE_RECIPIENT, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new(bonding_curve, false),
                AccountMeta::new(get_associated_token_address(&bonding_curve, mint), false),
                AccountMeta::new(*token_account, false),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(solana_program::system_program::ID, false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new(
                    *creator_vault.ok_or(TradingEndpointError::CustomError("Creator vault not provided".to_string()))?,
                    false,
                ),
                AccountMeta::new_readonly(PUBKEY_EVENT_AUTHORITY, false),
                AccountMeta::new_readonly(PUMPFUN_PROGRAM, false),
                AccountMeta::new(Self::get_global_volume_accumulator_pda()?, false),
                AccountMeta::new(Self::get_user_volume_accumulator_pda(&payer.pubkey())?, false),
            ],
        ))
    }

    fn build_sell_instruction(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        custom_ata: Option<&Pubkey>,
        creator_vault: Option<&Pubkey>,
        sell: SwapInfo,
    ) -> Result<Instruction, TradingEndpointError> {
        self.initialized()?;

        let sell_info: SellInfo = sell.into();
        let buffer = sell_info.to_buffer().map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;
        let bonding_curve = Self::get_bonding_curve_pda(mint)?;

        let ata = match custom_ata {
            None => get_associated_token_address(&payer.pubkey(), mint),
            Some(t) => *t,
        };

        Ok(Instruction::new_with_bytes(
            PUMPFUN_PROGRAM,
            &buffer,
            vec![
                AccountMeta::new_readonly(PUBKEY_GLOBAL_ACCOUNT, false),
                AccountMeta::new(PUBKEY_FEE_RECIPIENT, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new(bonding_curve, false),
                AccountMeta::new(get_associated_token_address(&bonding_curve, mint), false),
                AccountMeta::new(ata, false),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(solana_program::system_program::ID, false),
                AccountMeta::new(
                    *creator_vault.ok_or(TradingEndpointError::CustomError("Creator vault not provided".to_string()))?,
                    false,
                ),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(PUBKEY_EVENT_AUTHORITY, false),
                AccountMeta::new_readonly(PUMPFUN_PROGRAM, false),
                AccountMeta::new(Self::get_global_volume_accumulator_pda()?, false),
                AccountMeta::new(Self::get_user_volume_accumulator_pda(&payer.pubkey())?, false),
            ],
        ))
    }
}

impl Pumpfun {
    pub fn new(endpoint: Arc<TradingEndpoint>) -> Self {
        Self {
            endpoint,
            global_account: OnceCell::new(),
        }
    }

    pub fn get_bonding_curve_pda(mint: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let seeds: &[&[u8]; 2] = &[BONDING_CURVE_SEED, mint.as_ref()];
        let program_id: &Pubkey = &PUMPFUN_PROGRAM;
        let pda = Pubkey::try_find_program_address(seeds, program_id)
            .ok_or_else(|| TradingEndpointError::CustomError("Failed to find bonding curve PDA".to_string()))?;
        Ok(pda.0)
    }

    pub fn get_creator_vault_pda(creator: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let seeds: &[&[u8]; 2] = &[CREATOR_VAULT_SEED, creator.as_ref()];
        let program_id: &Pubkey = &PUMPFUN_PROGRAM;
        let pda = Pubkey::try_find_program_address(seeds, program_id)
            .ok_or_else(|| TradingEndpointError::CustomError("Failed to find creator vault PDA".to_string()))?;
        Ok(pda.0)
    }

    pub fn get_user_volume_accumulator_pda(user: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let seeds: &[&[u8]; 2] = &[&USER_VOLUME_ACCUMULATOR_SEED, user.as_ref()];
        let program_id: &Pubkey = &PUMPFUN_PROGRAM;
        let pda = Pubkey::try_find_program_address(seeds, program_id)
            .ok_or_else(|| TradingEndpointError::CustomError("Failed to find user volume accumulator PDA".to_string()))?;
        Ok(pda.0)
    }

    pub fn get_global_volume_accumulator_pda() -> Result<Pubkey, TradingEndpointError> {
        let seeds: &[&[u8]; 1] = &[&GLOBAL_VOLUME_ACCUMULATOR_SEED];
        let program_id: &Pubkey = &PUMPFUN_PROGRAM;
        let pda = Pubkey::try_find_program_address(seeds, program_id)
            .ok_or_else(|| TradingEndpointError::CustomError("Failed to find global volume accumulator PDA".to_string()))?;
        Ok(pda.0)
    }
}
