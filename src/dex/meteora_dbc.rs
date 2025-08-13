use super::{dex_traits::DexTrait, meteora_dbc_types::*, types::Create};
use crate::{
    common::{accounts::PUBKEY_WSOL, trading_endpoint::TradingEndpoint},
    dex::types::{PoolInfo, SwapInfo},
    errors::trading_endpoint_error::TradingEndpointError,
    instruction::builder::PriorityFee,
};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
};
use spl_associated_token_account::get_associated_token_address;
use std::sync::Arc;

pub struct MemeoraDBC {
    pub endpoint: Arc<TradingEndpoint>,
}

#[async_trait::async_trait]
impl DexTrait for MemeoraDBC {
    async fn initialize(&self) -> Result<(), TradingEndpointError> {
        Ok(())
    }

    fn initialized(&self) -> Result<(), TradingEndpointError> {
        Ok(())
    }

    fn get_trading_endpoint(&self) -> Arc<TradingEndpoint> {
        self.endpoint.clone()
    }

    fn use_wsol(&self) -> bool {
        true
    }

    async fn get_pool(&self, mint: &Pubkey) -> Result<PoolInfo, TradingEndpointError> {
        let pool = self.get_pool_by_base_mint(mint).await?;
        let account = self.endpoint.rpc.get_account(&pool).await?;
        if account.data.is_empty() {
            return Err(TradingEndpointError::CustomError(format!("Bonding curve not found: {}", pool.to_string())));
        }

        let bonding_curve = bincode::deserialize::<VirtualPool>(&account.data).map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;

        Ok(PoolInfo {
            pool,
            creator: Some(bonding_curve.creator),
            creator_vault: None,
            config: Some(bonding_curve.config),
            token_reserves: bonding_curve.base_reserve,
            sol_reserves: bonding_curve.quote_reserve,
        })
    }

    async fn create(&self, _: Keypair, _: Create, _: Option<PriorityFee>, _: Option<u64>) -> Result<Vec<Signature>, TradingEndpointError> {
        Err(TradingEndpointError::CustomError("Not supported".to_string()))
    }

    fn build_buy_instruction(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        config: Option<&Pubkey>,
        token_program_account: &Pubkey,
        buy: SwapInfo,
    ) -> Result<Instruction, TradingEndpointError> {
        self.initialized()?;

        let buy_info = SwapInstruction::from_swap_info(&buy, true);
        let buffer = buy_info.to_buffer().map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;
        let config = config.ok_or_else(|| TradingEndpointError::CustomError("Config must be provided for buy instruction".to_string()))?;
        let bonding_curve = Self::get_virtual_pool_pda(mint, config)?;
        let bonding_curve_vault = Self::get_bonding_curve_vault(mint)?;
        let bonding_curve_sol_vault = Self::get_bonding_curve_sol_vault(mint)?;

        Ok(Instruction::new_with_bytes(
            PUBKEY_METEORA_DBC,
            &buffer,
            vec![
                AccountMeta::new_readonly(PUBKEY_METEORA_DBC_POOL_AUTHORITY, false),
                AccountMeta::new_readonly(*config, false),
                AccountMeta::new(bonding_curve, false),
                AccountMeta::new(get_associated_token_address(&payer.pubkey(), &PUBKEY_WSOL), false),
                AccountMeta::new(get_associated_token_address(&payer.pubkey(), mint), false),
                AccountMeta::new(bonding_curve_vault, false),
                AccountMeta::new(bonding_curve_sol_vault, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new_readonly(PUBKEY_WSOL, false),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(*token_program_account, false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(PUBKEY_METEORA_DBC, false),
                AccountMeta::new_readonly(PUBKEY_METEORA_DBC_EVENT_AUTHORITY, false),
                AccountMeta::new_readonly(PUBKEY_METEORA_DBC, false),
            ],
        ))
    }

    fn build_sell_instruction(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        custom_ata: Option<&Pubkey>,
        config: Option<&Pubkey>,
        sell: SwapInfo,
    ) -> Result<Instruction, TradingEndpointError> {
        self.initialized()?;
        let ata = match custom_ata {
            None => get_associated_token_address(&payer.pubkey(), mint),
            Some(t) => *t,
        };
        let sell_info = SwapInstruction::from_swap_info(&sell, false);
        let buffer = sell_info.to_buffer().map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;
        let config = config.ok_or_else(|| TradingEndpointError::CustomError("Config must be provided for sell instruction".to_string()))?;
        let bonding_curve = Self::get_virtual_pool_pda(mint, config)?;
        let bonding_curve_vault = Self::get_bonding_curve_vault(mint)?;
        let bonding_curve_sol_vault = Self::get_bonding_curve_sol_vault(mint)?;

        Ok(Instruction::new_with_bytes(
            PUBKEY_METEORA_DBC,
            &buffer,
            vec![
                AccountMeta::new_readonly(PUBKEY_METEORA_DBC_POOL_AUTHORITY, false),
                AccountMeta::new_readonly(*config, false),
                AccountMeta::new(bonding_curve, false),
                AccountMeta::new(ata, false),
                AccountMeta::new(get_associated_token_address(&payer.pubkey(), &PUBKEY_WSOL), false),
                AccountMeta::new(bonding_curve_vault, false),
                AccountMeta::new(bonding_curve_sol_vault, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new_readonly(PUBKEY_WSOL, false),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(PUBKEY_METEORA_DBC, false),
                AccountMeta::new_readonly(PUBKEY_METEORA_DBC_EVENT_AUTHORITY, false),
                AccountMeta::new_readonly(PUBKEY_METEORA_DBC, false),
            ],
        ))
    }
}

impl MemeoraDBC {
    pub fn new(endpoint: Arc<TradingEndpoint>) -> Self {
        Self { endpoint }
    }

    pub fn get_virtual_pool_pda(mint: &Pubkey, config: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let seeds: &[&[u8]; 4] = &[VIRTUAL_POOL_SEED, config.as_ref(), mint.as_ref(), PUBKEY_WSOL.as_ref()];
        let pda = Pubkey::try_find_program_address(seeds, &PUBKEY_METEORA_DBC)
            .ok_or_else(|| TradingEndpointError::CustomError("Failed to find virtual pool PDA".to_string()))?;
        Ok(pda.0)
    }

    pub fn get_bonding_curve_vault(mint: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let seeds: &[&[u8]; 2] = &[VIRTUAL_POOL_BASE_VAULT, mint.as_ref()];
        let pda = Pubkey::try_find_program_address(seeds, &PUBKEY_METEORA_DBC)
            .ok_or_else(|| TradingEndpointError::CustomError("Failed to find bonding curve vault PDA".to_string()))?;
        Ok(pda.0)
    }

    pub fn get_bonding_curve_sol_vault(mint: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let seeds: &[&[u8]; 2] = &[VIRTUAL_POOL_QUOTE_VAULT, mint.as_ref()];
        let pda = Pubkey::try_find_program_address(seeds, &PUBKEY_METEORA_DBC)
            .ok_or_else(|| TradingEndpointError::CustomError("Failed to find bonding curve SOL vault PDA".to_string()))?;
        Ok(pda.0)
    }

    pub async fn get_pool_by_base_mint(&self, base_mint: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let accounts = self
            .endpoint
            .rpc
            .get_program_accounts_with_config(
                &PUBKEY_METEORA_DBC,
                solana_client::rpc_config::RpcProgramAccountsConfig {
                    filters: Some(vec![
                        solana_client::rpc_filter::RpcFilterType::DataSize(136),
                        solana_client::rpc_filter::RpcFilterType::Memcmp(solana_client::rpc_filter::Memcmp::new_raw_bytes(8, base_mint.to_bytes().to_vec())),
                    ]),
                    account_config: solana_client::rpc_config::RpcAccountInfoConfig {
                        encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                        commitment: None,
                        data_slice: None,
                        min_context_slot: None,
                    },
                    with_context: None,
                    sort_results: None,
                },
            )
            .await?;

        if accounts.is_empty() {
            return Err(TradingEndpointError::CustomError(format!(
                "No bonding curve found for base mint: {}",
                base_mint.to_string()
            )));
        }

        Ok(accounts[0].0)
    }
}
