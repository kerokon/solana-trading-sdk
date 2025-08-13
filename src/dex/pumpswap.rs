use super::{
    dex_traits::DexTrait,
    pumpfun_common_types::{BuyInfo, SellInfo},
    pumpfun_types::PUMPFUN_PROGRAM,
    pumpswap_types::*,
    types::{Create, SwapInfo},
};
use crate::{
    common::{accounts::PUBKEY_WSOL, trading_endpoint::TradingEndpoint},
    errors::trading_endpoint_error::TradingEndpointError,
    instruction::builder::PriorityFee,
};
use once_cell::sync::OnceCell;
use rand::seq::IndexedRandom;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
};
use spl_associated_token_account::get_associated_token_address;
use std::{str::FromStr, sync::Arc};

pub struct PumpSwap {
    pub endpoint: Arc<TradingEndpoint>,
    pub global_account: OnceCell<Arc<GlobalAccount>>,
}

#[async_trait::async_trait]
impl DexTrait for PumpSwap {
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
            return Err(TradingEndpointError::CustomError("PumpSwap not initialized".to_string()));
        }
        Ok(())
    }

    fn get_trading_endpoint(&self) -> Arc<TradingEndpoint> {
        self.endpoint.clone()
    }

    fn use_wsol(&self) -> bool {
        true
    }

    async fn get_pool(&self, mint: &Pubkey) -> Result<super::types::PoolInfo, TradingEndpointError> {
        let pool = Self::get_pool_address(mint)?;
        let pool_base = get_associated_token_address(&pool, &mint);
        let pool_quote = get_associated_token_address(&pool, &PUBKEY_WSOL);
        let (pool_account, pool_base_account, pool_quote_account) = tokio::try_join!(
            self.endpoint.rpc.get_account(&pool),
            self.endpoint.rpc.get_token_account(&pool_base),
            self.endpoint.rpc.get_token_account(&pool_quote),
        )?;

        if pool_account.data.is_empty() {
            return Err(TradingEndpointError::CustomError(format!("Pool account not found: {}", mint.to_string())));
        }

        let pool_account = bincode::deserialize::<PoolAccount>(&pool_account.data).map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;
        let pool_base_account =
            pool_base_account.ok_or_else(|| TradingEndpointError::CustomError(format!("Pool base account not found: {}", mint.to_string())))?;
        let pool_quote_account =
            pool_quote_account.ok_or_else(|| TradingEndpointError::CustomError(format!("Pool quote account not found: {}", mint.to_string())))?;

        let pool_base_reserve = u64::from_str(&pool_base_account.token_amount.amount).map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;
        let pool_quote_reserve = u64::from_str(&pool_quote_account.token_amount.amount).map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;

        Ok(super::types::PoolInfo {
            pool,
            creator: Some(pool_account.coin_creator),
            creator_vault: Some(Self::get_creator_vault(&pool_account.coin_creator)?),
            config: None,
            token_reserves: pool_base_reserve,
            sol_reserves: pool_quote_reserve,
        })
    }

    async fn create(&self, _: Keypair, _: Create, _: Option<PriorityFee>, _: Option<u64>) -> Result<Vec<Signature>, TradingEndpointError> {
        Err(TradingEndpointError::CustomError("Not supported".to_string()))
    }

    fn build_buy_instruction(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        creator_vault: Option<&Pubkey>,
        token_program_account: &Pubkey,
        buy: SwapInfo,
    ) -> Result<Instruction, TradingEndpointError> {
        self.initialized()?;

        let buy_info: BuyInfo = buy.into();
        let buffer = buy_info.to_buffer().map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;
        let pool = Self::get_pool_address(&mint)?;
        let creator_vault = creator_vault.ok_or(TradingEndpointError::CustomError("Creator vault is required for buy instruction".to_string()))?;
        let creator_vault_ata = get_associated_token_address(creator_vault, &PUBKEY_WSOL);
        let fee_recipient = self.global_account.get().unwrap().protocol_fee_recipients.choose(&mut rand::rng()).unwrap();

        Ok(Instruction::new_with_bytes(
            PUBKEY_PUMPSWAP,
            &buffer,
            vec![
                AccountMeta::new_readonly(pool, false),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(PUBKEY_GLOBAL_ACCOUNT, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new_readonly(PUBKEY_WSOL, false),
                AccountMeta::new(get_associated_token_address(&payer.pubkey(), mint), false),
                AccountMeta::new(get_associated_token_address(&payer.pubkey(), &PUBKEY_WSOL), false),
                AccountMeta::new(get_associated_token_address(&pool, mint), false),
                AccountMeta::new(get_associated_token_address(&pool, &PUBKEY_WSOL), false),
                AccountMeta::new_readonly(*fee_recipient, false),
                AccountMeta::new(get_associated_token_address(fee_recipient, &PUBKEY_WSOL), false),
                AccountMeta::new_readonly(*token_program_account, false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(solana_program::system_program::ID, false),
                AccountMeta::new_readonly(spl_associated_token_account::ID, false),
                AccountMeta::new_readonly(PUBKEY_EVENT_AUTHORITY, false),
                AccountMeta::new_readonly(PUBKEY_PUMPSWAP, false),
                AccountMeta::new(creator_vault_ata, false),
                AccountMeta::new_readonly(*creator_vault, false),
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
        let pool = Self::get_pool_address(&mint)?;
        let creator_vault = creator_vault.ok_or(TradingEndpointError::CustomError("Creator vault is required for buy instruction".to_string()))?;
        let creator_vault_ata = get_associated_token_address(creator_vault, &PUBKEY_WSOL);
        let fee_recipient = self.global_account.get().unwrap().protocol_fee_recipients.choose(&mut rand::rng()).unwrap();
        let ata = match custom_ata {
            None => get_associated_token_address(&payer.pubkey(), mint),
            Some(t) => *t,
        };
        Ok(Instruction::new_with_bytes(
            PUBKEY_PUMPSWAP,
            &buffer,
            vec![
                AccountMeta::new_readonly(pool, false),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(PUBKEY_GLOBAL_ACCOUNT, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new_readonly(PUBKEY_WSOL, false),
                AccountMeta::new(ata, false),
                AccountMeta::new(get_associated_token_address(&payer.pubkey(), &PUBKEY_WSOL), false),
                AccountMeta::new(get_associated_token_address(&pool, mint), false),
                AccountMeta::new(get_associated_token_address(&pool, &PUBKEY_WSOL), false),
                AccountMeta::new_readonly(*fee_recipient, false),
                AccountMeta::new(get_associated_token_address(fee_recipient, &PUBKEY_WSOL), false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(solana_program::system_program::ID, false),
                AccountMeta::new_readonly(spl_associated_token_account::ID, false),
                AccountMeta::new_readonly(PUBKEY_EVENT_AUTHORITY, false),
                AccountMeta::new_readonly(PUBKEY_PUMPSWAP, false),
                AccountMeta::new(creator_vault_ata, false),
                AccountMeta::new_readonly(*creator_vault, false),
            ],
        ))
    }
}

impl PumpSwap {
    pub fn new(endpoint: Arc<TradingEndpoint>) -> Self {
        Self {
            endpoint,
            global_account: OnceCell::new(),
        }
    }

    pub fn get_creator_vault(creator: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let pda = Pubkey::try_find_program_address(&[b"creator_vault", creator.as_ref()], &PUBKEY_PUMPSWAP)
            .ok_or_else(|| TradingEndpointError::CustomError("Failed to find creator vault PDA".to_string()))?;
        Ok(pda.0)
    }

    pub fn get_pool_authority_pda(mint: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let pda = Pubkey::try_find_program_address(&[b"pool-authority", mint.as_ref()], &PUMPFUN_PROGRAM)
            .ok_or_else(|| TradingEndpointError::CustomError("Failed to find pool authority PDA".to_string()))?;
        Ok(pda.0)
    }

    pub fn get_pool_address(mint: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let pda = Pubkey::try_find_program_address(
            &[
                b"pool",
                &0u16.to_le_bytes(),
                Self::get_pool_authority_pda(mint)?.as_ref(),
                mint.as_ref(),
                PUBKEY_WSOL.as_ref(),
            ],
            &PUBKEY_PUMPSWAP,
        )
        .ok_or_else(|| TradingEndpointError::CustomError("Failed to find pool address PDA".to_string()))?;
        Ok(pda.0)
    }
}
