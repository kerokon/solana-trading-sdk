use super::{
    amm_calc::{amm_buy_get_token_out, calculate_with_slippage_buy},
    dex_traits::DexTrait,
    pumpfun_common_types::{BuyInfo, SellInfo},
    pumpfun_types::*,
    types::{Create, PoolInfo, SwapInfo},
};
use crate::{common::trading_endpoint::TradingEndpoint, instruction::builder::PriorityFee};
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
    async fn initialize(&self) -> anyhow::Result<()> {
        let account = self.endpoint.rpc.get_account(&PUBKEY_GLOBAL_ACCOUNT).await?;
        let global_account = bincode::deserialize::<GlobalAccount>(&account.data)?;
        let global_account = Arc::new(global_account);

        self.global_account.set(global_account).unwrap();
        Ok(())
    }

    fn initialized(&self) -> anyhow::Result<()> {
        if self.global_account.get().is_none() {
            return Err(anyhow::anyhow!("Pumpfun not initialized"));
        }
        Ok(())
    }

    fn get_trading_endpoint(&self) -> Arc<TradingEndpoint> {
        self.endpoint.clone()
    }

    fn use_wsol(&self) -> bool {
        false
    }

    async fn get_pool(&self, mint: &Pubkey) -> anyhow::Result<PoolInfo> {
        let bonding_curve_pda = Self::get_bonding_curve_pda(mint).unwrap();
        let account = self.endpoint.rpc.get_account(&bonding_curve_pda).await?;
        if account.data.is_empty() {
            return Err(anyhow::anyhow!("Bonding curve not found: {}", mint.to_string()));
        }

        let bonding_curve = bincode::deserialize::<BondingCurveAccount>(&account.data)?;

        Ok(PoolInfo {
            pool: bonding_curve_pda,
            creator: Some(bonding_curve.creator),
            creator_vault: Some(Self::get_creator_vault_pda(&bonding_curve.creator)?),
            config: None,
            token_reserves: bonding_curve.virtual_token_reserves,
            sol_reserves: bonding_curve.virtual_sol_reserves,
        })
    }

    async fn create(&self, payer: Keypair, create: Create, fee: Option<PriorityFee>, tip: Option<u64>) -> anyhow::Result<Vec<Signature>> {
        let mint = create.mint_private_key.pubkey();
        let buy_sol_amount = create.buy_sol_amount;
        let slippage_basis_points = create.slippage_basis_points.unwrap_or(0);

        let create_info = CreateInfo::from_create(&create, payer.pubkey());
        let mut buffer = Vec::new();
        create_info.serialize(&mut buffer)?;

        let blockhash = self.endpoint.rpc.get_latest_blockhash().await?;
        let bonding_curve = Self::get_bonding_curve_pda(&mint)?;

        let mut instructions = vec![];
        let create_instruction = Instruction::new_with_bytes(
            PUBKEY_PUMPFUN,
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
                AccountMeta::new_readonly(PUBKEY_PUMPFUN, false),
            ],
        );

        instructions.push(create_instruction);

        if let Some(buy_sol_amount) = buy_sol_amount {
            let create_ata = create_associated_token_account(&payer.pubkey(), &payer.pubkey(), &mint, &spl_token::ID);
            instructions.push(create_ata);

            let buy_token_amount = amm_buy_get_token_out(INITIAL_VIRTUAL_SOL_RESERVES, INITIAL_VIRTUAL_TOKEN_RESERVES, buy_sol_amount);
            let sol_lamports_with_slippage = calculate_with_slippage_buy(buy_sol_amount, slippage_basis_points);
            let creator_vault = Self::get_creator_vault_pda(&payer.pubkey()).unwrap();
            let buy_instruction = self.build_buy_instruction(
                &payer,
                &mint,
                Some(&creator_vault),
                SwapInfo {
                    token_amount: buy_token_amount,
                    sol_amount: sol_lamports_with_slippage,
                },
            )?;
            instructions.push(buy_instruction);
        }

        let signatures = self
            .endpoint
            .build_and_broadcast_tx(&payer, instructions, blockhash, fee, tip, Some(vec![&create.mint_private_key])).await?;

        Ok(signatures)
    }

    fn build_buy_instruction(&self, payer: &Keypair, mint: &Pubkey, creator_vault: Option<&Pubkey>, buy: SwapInfo) -> anyhow::Result<Instruction> {
        self.initialized()?;

        let buy_info: BuyInfo = buy.into();
        let buffer = buy_info.to_buffer()?;
        let bonding_curve = Self::get_bonding_curve_pda(mint)?;

        Ok(Instruction::new_with_bytes(
            PUBKEY_PUMPFUN,
            &buffer,
            vec![
                AccountMeta::new_readonly(PUBKEY_GLOBAL_ACCOUNT, false),
                AccountMeta::new(PUBKEY_FEE_RECIPIENT, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new(bonding_curve, false),
                AccountMeta::new(get_associated_token_address(&bonding_curve, mint), false),
                AccountMeta::new(get_associated_token_address(&payer.pubkey(), mint), false),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(solana_program::system_program::ID, false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new(*creator_vault.ok_or(anyhow::anyhow!("Creator vault not provided"))?, false),
                AccountMeta::new_readonly(PUBKEY_EVENT_AUTHORITY, false),
                AccountMeta::new_readonly(PUBKEY_PUMPFUN, false),
            ],
        ))
    }

    fn build_sell_instruction(&self, payer: &Keypair, mint: &Pubkey, creator_vault: Option<&Pubkey>, sell: SwapInfo) -> anyhow::Result<Instruction> {
        self.initialized()?;

        let sell_info: SellInfo = sell.into();
        let buffer = sell_info.to_buffer()?;
        let bonding_curve = Self::get_bonding_curve_pda(mint)?;

        Ok(Instruction::new_with_bytes(
            PUBKEY_PUMPFUN,
            &buffer,
            vec![
                AccountMeta::new_readonly(PUBKEY_GLOBAL_ACCOUNT, false),
                AccountMeta::new(PUBKEY_FEE_RECIPIENT, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new(bonding_curve, false),
                AccountMeta::new(get_associated_token_address(&bonding_curve, mint), false),
                AccountMeta::new(get_associated_token_address(&payer.pubkey(), mint), false),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(solana_program::system_program::ID, false),
                AccountMeta::new(*creator_vault.ok_or(anyhow::anyhow!("Creator vault not provided"))?, false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(PUBKEY_EVENT_AUTHORITY, false),
                AccountMeta::new_readonly(PUBKEY_PUMPFUN, false),
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

    pub fn get_bonding_curve_pda(mint: &Pubkey) -> anyhow::Result<Pubkey> {
        let seeds: &[&[u8]; 2] = &[BONDING_CURVE_SEED, mint.as_ref()];
        let program_id: &Pubkey = &PUBKEY_PUMPFUN;
        let pda = Pubkey::try_find_program_address(seeds, program_id).ok_or_else(|| anyhow::anyhow!("Failed to find bonding curve PDA"))?;
        Ok(pda.0)
    }

    pub fn get_creator_vault_pda(creator: &Pubkey) -> anyhow::Result<Pubkey> {
        let seeds: &[&[u8]; 2] = &[CREATOR_VAULT_SEED, creator.as_ref()];
        let program_id: &Pubkey = &PUBKEY_PUMPFUN;
        let pda = Pubkey::try_find_program_address(seeds, program_id).ok_or_else(|| anyhow::anyhow!("Failed to find creator vault PDA"))?;
        Ok(pda.0)
    }
}
