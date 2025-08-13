use super::{
    dex_traits::DexTrait,
    raydium_bonk_types::*,
    types::{Create, PoolInfo, SwapInfo},
};
use crate::{
    common::{accounts::PUBKEY_WSOL, trading_endpoint::TradingEndpoint},
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

pub struct RaydiumBonk {
    pub endpoint: Arc<TradingEndpoint>,
}

#[async_trait::async_trait]
impl DexTrait for RaydiumBonk {
    async fn initialize(&self) -> Result<(), TradingEndpointError> {
        Ok(())
    }

    fn initialized(&self) -> Result<(), TradingEndpointError> {
        Ok(())
    }

    fn use_wsol(&self) -> bool {
        true
    }

    fn get_trading_endpoint(&self) -> Arc<TradingEndpoint> {
        self.endpoint.clone()
    }

    async fn get_pool(&self, mint: &Pubkey) -> Result<PoolInfo, TradingEndpointError> {
        let pool = Self::get_pool_pda(mint)?;
        let account = self.endpoint.rpc.get_account(&pool).await?;
        if account.data.is_empty() {
            return Err(TradingEndpointError::CustomError(format!("Bonding curve not found: {}", pool.to_string())));
        }

        let bonding_curve = bincode::deserialize::<PoolState>(&account.data).map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;

        Ok(PoolInfo {
            pool,
            creator: Some(bonding_curve.creator),
            creator_vault: None,
            config: None,
            token_reserves: bonding_curve.virtual_base,
            sol_reserves: bonding_curve.virtual_quote,
        })
    }

    async fn create(&self, _: Keypair, _: Create, _: Option<PriorityFee>, _: Option<u64>) -> Result<Vec<Signature>, TradingEndpointError> {
        Err(TradingEndpointError::CustomError("Not supported".to_string()))
    }

    fn build_buy_instruction(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        _: Option<&Pubkey>,
        token_program_account: &Pubkey,
        buy: SwapInfo,
    ) -> Result<Instruction, TradingEndpointError> {
        self.initialized()?;

        let buy_info: BuyInfo = buy.into();
        let buffer = buy_info.to_buffer().map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;
        let pool_address = Self::get_pool_pda(mint)?;
        let pool_base_vault = Self::get_pool_mint_vault(mint, &pool_address)?;
        let pool_quote_vault = Self::get_pool_quote_vault(&PUBKEY_WSOL, &pool_address)?;

        Ok(Instruction::new_with_bytes(
            PUBKEY_RAYDIUM_BONK,
            &buffer,
            vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(PUBKEY_RAYDIUM_BONK_AUTHORITY, false),
                AccountMeta::new_readonly(PUBKEY_RAYDIUM_BONK_GLOBAL_CONFIG, false),
                AccountMeta::new_readonly(PUBKEY_RAYDIUM_BONK_PLATFORM_CONFIG, false),
                AccountMeta::new(pool_address, false),
                AccountMeta::new(get_associated_token_address(&payer.pubkey(), mint), false),
                AccountMeta::new(get_associated_token_address(&payer.pubkey(), &PUBKEY_WSOL), false),
                AccountMeta::new(pool_base_vault, false),
                AccountMeta::new(pool_quote_vault, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new_readonly(PUBKEY_WSOL, false),
                AccountMeta::new_readonly(*token_program_account, false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(PUBKEY_RAYDIUM_BONK_EVENT_AUTHORITY, false),
                AccountMeta::new_readonly(PUBKEY_RAYDIUM_BONK, false),
            ],
        ))
    }

    fn build_sell_instruction(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        custom_ata: Option<&Pubkey>,
        _: Option<&Pubkey>,
        sell: SwapInfo,
    ) -> Result<Instruction, TradingEndpointError> {
        self.initialized()?;
        let ata = match custom_ata {
            None => get_associated_token_address(&payer.pubkey(), mint),
            Some(t) => *t,
        };
        let sell_info: SellInfo = sell.into();
        let buffer = sell_info.to_buffer().map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;
        let pool_address = Self::get_pool_pda(mint)?;
        let pool_base_vault = Self::get_pool_mint_vault(mint, &pool_address)?;
        let pool_quote_vault = Self::get_pool_quote_vault(&PUBKEY_WSOL, &pool_address)?;

        Ok(Instruction::new_with_bytes(
            PUBKEY_RAYDIUM_BONK,
            &buffer,
            vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(PUBKEY_RAYDIUM_BONK_AUTHORITY, false),
                AccountMeta::new_readonly(PUBKEY_RAYDIUM_BONK_GLOBAL_CONFIG, false),
                AccountMeta::new_readonly(PUBKEY_RAYDIUM_BONK_PLATFORM_CONFIG, false),
                AccountMeta::new(pool_address, false),
                AccountMeta::new(ata, false),
                AccountMeta::new(get_associated_token_address(&payer.pubkey(), &PUBKEY_WSOL), false),
                AccountMeta::new(pool_base_vault, false),
                AccountMeta::new(pool_quote_vault, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new_readonly(PUBKEY_WSOL, false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(PUBKEY_RAYDIUM_BONK_EVENT_AUTHORITY, false),
                AccountMeta::new_readonly(PUBKEY_RAYDIUM_BONK, false),
            ],
        ))
    }
}

impl RaydiumBonk {
    pub fn new(endpoint: Arc<TradingEndpoint>) -> Self {
        Self { endpoint }
    }

    pub fn get_pool_pda(mint: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let seeds: &[&[u8]; 3] = &[b"pool", mint.as_ref(), PUBKEY_WSOL.as_ref()];
        let pda = Pubkey::try_find_program_address(seeds, &PUBKEY_RAYDIUM_BONK)
            .ok_or(TradingEndpointError::CustomError("Failed to find program address".to_string()))?;
        Ok(pda.0)
    }

    pub fn get_pool_mint_vault(mint: &Pubkey, pool: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let seeds: &[&[u8]; 3] = &[b"pool_vault", pool.as_ref(), mint.as_ref()];
        let pda = Pubkey::try_find_program_address(seeds, &PUBKEY_RAYDIUM_BONK)
            .ok_or(TradingEndpointError::CustomError("Failed to find pool mint vault PDA".to_string()))?;
        Ok(pda.0)
    }

    pub fn get_pool_quote_vault(quote: &Pubkey, pool: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let seeds: &[&[u8]; 3] = &[b"pool_vault", pool.as_ref(), quote.as_ref()];
        let pda = Pubkey::try_find_program_address(seeds, &PUBKEY_RAYDIUM_BONK)
            .ok_or(TradingEndpointError::CustomError("Failed to find pool quote vault PDA".to_string()))?;
        Ok(pda.0)
    }
}
