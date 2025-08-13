use super::{boopfun_types::*, dex_traits::DexTrait, types::Create};
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

pub struct Boopfun {
    pub endpoint: Arc<TradingEndpoint>,
}

#[async_trait::async_trait]
impl DexTrait for Boopfun {
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
        false
    }

    async fn get_pool(&self, mint: &Pubkey) -> Result<PoolInfo, TradingEndpointError> {
        let pool = Self::get_bonding_curve_pda(mint)?;
        let account = self.endpoint.rpc.get_account(&pool).await?;
        if account.data.is_empty() {
            return Err(TradingEndpointError::CustomError(format!("Bonding curve not found: {}", mint.to_string())));
        }

        let bonding_curve = bincode::deserialize::<BondingCurveAccount>(&account.data).map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;

        Ok(PoolInfo {
            pool,
            creator: Some(bonding_curve.creator),
            creator_vault: None,
            config: None,
            token_reserves: bonding_curve.virtual_token_reserves,
            sol_reserves: bonding_curve.virtual_sol_reserves,
        })
    }

    async fn create(&self, _: Keypair, _: Create, _: Option<PriorityFee>, _: Option<u64>) -> Result<Vec<Signature>, TradingEndpointError> {
        Err(TradingEndpointError::CustomError("Not supported".to_string()))
    }

    fn build_buy_instruction(
        &self,
        _payer: &Keypair,
        mint: &Pubkey,
        _: Option<&Pubkey>,
        _token_program_account: &Pubkey,
        buy: SwapInfo,
    ) -> Result<Instruction, TradingEndpointError> {
        self.initialized()?;

        let buy_info: BuyInfo = buy.into();
        let buffer = buy_info.to_buffer().map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;
        let bonding_curve = Self::get_bonding_curve_pda(mint)?;
        let bonding_curve_vault = Self::get_bonding_curve_vault(mint)?;
        let bonding_curve_sol_vault = Self::get_bonding_curve_sol_vault(mint)?;
        let trading_fee_vault = Self::get_trading_fee_vault(mint)?;

        Ok(Instruction::new_with_bytes(
            PUBKEY_BOOPFUN,
            &buffer,
            vec![
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new(bonding_curve, false),
                AccountMeta::new(trading_fee_vault, false),
                AccountMeta::new(bonding_curve_vault, false),
                AccountMeta::new(bonding_curve_sol_vault, false),
                AccountMeta::new(get_associated_token_address(&_payer.pubkey(), mint), false),
                AccountMeta::new(_payer.pubkey(), true),
                AccountMeta::new_readonly(PUBKEY_BOOPFUN_CONFIG, false),
                AccountMeta::new_readonly(PUBKEY_BOOPFUN_VAULT_AUTHORITY, false),
                AccountMeta::new_readonly(PUBKEY_WSOL, false),
                AccountMeta::new_readonly(solana_program::system_program::ID, false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(spl_associated_token_account::ID, false),
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
        let bonding_curve = Self::get_bonding_curve_pda(mint)?;
        let bonding_curve_vault = Self::get_bonding_curve_vault(mint)?;
        let bonding_curve_sol_vault = Self::get_bonding_curve_sol_vault(mint)?;
        let trading_fee_vault = Self::get_trading_fee_vault(mint)?;

        Ok(Instruction::new_with_bytes(
            PUBKEY_BOOPFUN,
            &buffer,
            vec![
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new(bonding_curve, false),
                AccountMeta::new(trading_fee_vault, false),
                AccountMeta::new(bonding_curve_vault, false),
                AccountMeta::new(bonding_curve_sol_vault, false),
                AccountMeta::new(ata, false),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(PUBKEY_BOOPFUN_CONFIG, false),
                AccountMeta::new_readonly(solana_program::system_program::ID, false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(spl_associated_token_account::ID, false),
            ],
        ))
    }
}

impl Boopfun {
    pub fn new(endpoint: Arc<TradingEndpoint>) -> Self {
        Self { endpoint }
    }

    pub fn get_bonding_curve_pda(mint: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let seeds: &[&[u8]; 2] = &[BONDING_CURVE_SEED, mint.as_ref()];
        let pda = Pubkey::try_find_program_address(seeds, &PUBKEY_BOOPFUN)
            .ok_or_else(|| TradingEndpointError::CustomError("Failed to find bonding curve PDA".to_string()))?;
        Ok(pda.0)
    }

    pub fn get_bonding_curve_vault(mint: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let seeds: &[&[u8]; 2] = &[BONDING_CURVE_VAULT_SEED, mint.as_ref()];
        let pda = Pubkey::try_find_program_address(seeds, &PUBKEY_BOOPFUN)
            .ok_or_else(|| TradingEndpointError::CustomError("Failed to find bonding curve vault PDA".to_string()))?;
        Ok(pda.0)
    }

    pub fn get_bonding_curve_sol_vault(mint: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let seeds: &[&[u8]; 2] = &[BONDING_CURVE_SOL_VAULT_SEED, mint.as_ref()];
        let pda = Pubkey::try_find_program_address(seeds, &PUBKEY_BOOPFUN)
            .ok_or_else(|| TradingEndpointError::CustomError("Failed to find bonding curve sol vault PDA".to_string()))?;
        Ok(pda.0)
    }

    pub fn get_trading_fee_vault(mint: &Pubkey) -> Result<Pubkey, TradingEndpointError> {
        let seeds: &[&[u8]; 2] = &[TRADING_FEE_VAULT_SEED, mint.as_ref()];
        let pda = Pubkey::try_find_program_address(seeds, &PUBKEY_BOOPFUN)
            .ok_or_else(|| TradingEndpointError::CustomError("Failed to find trading fee vault PDA".to_string()))?;
        Ok(pda.0)
    }
}
