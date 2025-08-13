use super::{dex_traits::DexTrait, pumpfun, pumpswap};
use crate::{
    common::trading_endpoint::TradingEndpoint,
    dex::{believe, boopfun, raydium_bonk},
};
use serde::{Deserialize, Serialize};
use solana_client::client_error::ClientError;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use spl_associated_token_account::get_associated_token_address;
use std::sync::Arc;

pub struct PoolInfo {
    pub pool: Pubkey,
    pub creator: Option<Pubkey>,
    pub creator_vault: Option<Pubkey>,
    pub config: Option<Pubkey>,
    pub token_reserves: u64,
    pub sol_reserves: u64,
}

pub struct SwapInfo {
    pub token_amount: u64,
    pub sol_amount: u64,
}

pub struct Create {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub mint_private_key: Keypair,
    pub buy_sol_amount: Option<u64>,
    pub slippage_basis_points: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DexType {
    Pumpfun,
    PumpSwap,
    RayBonk,
    Boopfun,
    Believe,
}

impl DexType {
    pub fn all() -> Vec<DexType> {
        vec![DexType::Pumpfun, DexType::PumpSwap, DexType::RayBonk, DexType::Boopfun, DexType::Believe]
    }

    pub fn instantiate(&self, endpoint: Arc<TradingEndpoint>) -> Arc<dyn DexTrait> {
        match self {
            DexType::Pumpfun => Arc::new(pumpfun::Pumpfun::new(endpoint)),
            DexType::PumpSwap => Arc::new(pumpswap::PumpSwap::new(endpoint)),
            DexType::RayBonk => Arc::new(raydium_bonk::RaydiumBonk::new(endpoint)),
            DexType::Boopfun => Arc::new(boopfun::Boopfun::new(endpoint)),
            DexType::Believe => Arc::new(believe::Believe::new(endpoint)),
        }
    }
}

pub enum TokenAmountType {
    Percent(u64),
    Amount(u64),
}

impl TokenAmountType {
    pub async fn to_amount(&self, rpc: Arc<RpcClient>, payer: &Pubkey, mint: &Pubkey) -> Result<u64, ClientError> {
        match self {
            TokenAmountType::Percent(percent) => {
                let ata = get_associated_token_address(payer, mint);
                let balance = rpc.get_token_account_balance(&ata).await?;
                let balance_u64 = balance.amount.parse::<u64>().unwrap_or(0);
                Ok((balance_u64 * percent) / 100)
            }
            TokenAmountType::Amount(amount) => Ok(*amount),
        }
    }
}

pub enum CreateATA {
    Create,
    None,
    Idempotent,
    CreateWithSeed(String),
}

pub struct BatchBuyParam {
    pub payer: Keypair,
    pub sol_amount: u64,
}

pub struct BatchSellParam {
    pub payer: Keypair,
    pub token_amount: u64,
    pub close_mint_ata: bool,
    pub custom_ata: Option<Pubkey>,
}
