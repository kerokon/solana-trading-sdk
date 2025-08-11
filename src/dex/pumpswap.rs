use super::{
    dex_traits::DexTrait,
    pumpfun_common_types::{BuyInfo, SellInfo},
    pumpfun_types::PUMPFUN_PROGRAM,
    pumpswap_types::*,
    types::{Create, SwapInfo},
};
use crate::{
    common::{accounts::PUBKEY_WSOL, trading_endpoint::TradingEndpoint},
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
    async fn initialize(&self) -> anyhow::Result<()> {
        let account = self.endpoint.rpc.get_account(&PUBKEY_GLOBAL_ACCOUNT).await?;
        let global_account = bincode::deserialize::<GlobalAccount>(&account.data)?;
        let global_account = Arc::new(global_account);

        self.global_account.set(global_account).unwrap();
        Ok(())
    }

    fn initialized(&self) -> anyhow::Result<()> {
        if self.global_account.get().is_none() {
            return Err(anyhow::anyhow!("PumpSwap not initialized"));
        }
        Ok(())
    }

    fn get_trading_endpoint(&self) -> Arc<TradingEndpoint> {
        self.endpoint.clone()
    }

    fn use_wsol(&self) -> bool {
        true
    }

    async fn get_pool(&self, mint: &Pubkey) -> anyhow::Result<super::types::PoolInfo> {
        let pool = Self::get_pool_address(mint)?;
        let pool_base = get_associated_token_address(&pool, &mint);
        let pool_quote = get_associated_token_address(&pool, &PUBKEY_WSOL);
        let (pool_account, pool_base_account, pool_quote_account) = tokio::try_join!(
            self.endpoint.rpc.get_account(&pool),
            self.endpoint.rpc.get_token_account(&pool_base),
            self.endpoint.rpc.get_token_account(&pool_quote),
        )?;

        if pool_account.data.is_empty() {
            return Err(anyhow::anyhow!("Pool account not found: {}", mint.to_string()));
        }

        let pool_account = bincode::deserialize::<PoolAccount>(&pool_account.data)?;
        let pool_base_account = pool_base_account.ok_or_else(|| anyhow::anyhow!("Pool base account not found: {}", mint.to_string()))?;
        let pool_quote_account = pool_quote_account.ok_or_else(|| anyhow::anyhow!("Pool quote account not found: {}", mint.to_string()))?;

        let pool_base_reserve = u64::from_str(&pool_base_account.token_amount.amount)?;
        let pool_quote_reserve = u64::from_str(&pool_quote_account.token_amount.amount)?;

        Ok(super::types::PoolInfo {
            pool,
            creator: Some(pool_account.coin_creator),
            creator_vault: Some(Self::get_creator_vault(&pool_account.coin_creator)?),
            config: None,
            token_reserves: pool_base_reserve,
            sol_reserves: pool_quote_reserve,
        })
    }

    async fn create(&self, _: Keypair, _: Create, _: Option<PriorityFee>, _: Option<u64>) -> anyhow::Result<Vec<Signature>> {
        Err(anyhow::anyhow!("Not supported"))
    }

    fn build_buy_instruction(&self, payer: &Keypair, mint: &Pubkey, creator_vault: Option<&Pubkey>, buy: SwapInfo) -> anyhow::Result<Instruction> {
        self.initialized()?;

        let buy_info: BuyInfo = buy.into();
        let buffer = buy_info.to_buffer()?;
        let pool = Self::get_pool_address(&mint)?;
        let creator_vault = creator_vault.ok_or(anyhow::anyhow!("Creator vault is required for buy instruction"))?;
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

    fn build_sell_instruction(&self, payer: &Keypair, mint: &Pubkey, creator_vault: Option<&Pubkey>, sell: SwapInfo) -> anyhow::Result<Instruction> {
        self.initialized()?;

        let sell_info: SellInfo = sell.into();
        let buffer = sell_info.to_buffer()?;
        let pool = Self::get_pool_address(&mint)?;
        let creator_vault = creator_vault.ok_or(anyhow::anyhow!("Creator vault is required for buy instruction"))?;
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

    pub fn get_creator_vault(creator: &Pubkey) -> anyhow::Result<Pubkey> {
        let pda = Pubkey::try_find_program_address(&[b"creator_vault", creator.as_ref()], &PUBKEY_PUMPSWAP)
            .ok_or_else(|| anyhow::anyhow!("Failed to find creator vault PDA"))?;
        Ok(pda.0)
    }

    pub fn get_pool_authority_pda(mint: &Pubkey) -> anyhow::Result<Pubkey> {
        let pda = Pubkey::try_find_program_address(&[b"pool-authority", mint.as_ref()], &PUMPFUN_PROGRAM)
            .ok_or_else(|| anyhow::anyhow!("Failed to find pool authority PDA"))?;
        Ok(pda.0)
    }

    pub fn get_pool_address(mint: &Pubkey) -> anyhow::Result<Pubkey> {
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
        .ok_or_else(|| anyhow::anyhow!("Failed to find pool address PDA"))?;
        Ok(pda.0)
    }
}
