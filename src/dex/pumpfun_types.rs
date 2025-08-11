use super::types::Create;
use borsh::{BorshDeserialize, BorshSerialize};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey, pubkey::Pubkey};

pub const PUMPFUN_PROGRAM: Pubkey = pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");
pub const PUBKEY_GLOBAL_ACCOUNT: Pubkey = pubkey!("4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf");
pub const PUBKEY_EVENT_AUTHORITY: Pubkey = pubkey!("Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1");
pub const PUBKEY_FEE_RECIPIENT: Pubkey = pubkey!("62qc2CNXwrYqQScmEdiZFFAnJR262PxWEuNQtxfafNgV");
pub const AMM_PROGRAM: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

pub const PUBKEY_PUMPFUN_GLOBAL_VOLUME_ACCUMULATOR: Pubkey = pubkey!("Hq2wp8uJ9jCPsYgNHex8RtqdvMPfVGoYwjvF1ATiwn2Y");
pub const PUBKEY_PUMPFUN_USER_VOLUME_ACCUMULATOR: Pubkey = pubkey!("8BWnAfvqm9BGtKCBk61Ern2eMT3SE192FDCVWTGibj1N");

pub const GLOBAL_SEED: &[u8] = b"global";
pub const MINT_AUTHORITY_SEED: &[u8] = b"mint-authority";
pub const BONDING_CURVE_SEED: &[u8] = b"bonding-curve";
pub const CREATOR_VAULT_SEED: &[u8] = b"creator-vault";
pub const USER_VOLUME_ACCUMULATOR_SEED: &[u8] = b"user_volume_accumulator";
pub const GLOBAL_VOLUME_ACCUMULATOR_SEED: &[u8] = b"global_volume_accumulator";
pub const METADATA_SEED: &[u8] = b"metadata";

pub const INITIAL_VIRTUAL_TOKEN_RESERVES: u64 = 1_073_000_000_000_000;
pub const INITIAL_VIRTUAL_SOL_RESERVES: u64 = 30_000_000_000;

pub static PUBKEY_MINT_AUTHORITY_PDA: Lazy<Pubkey> = Lazy::new(|| Pubkey::find_program_address(&[MINT_AUTHORITY_SEED], &PUMPFUN_PROGRAM).0);
pub static PUBKEY_GLOBAL_PDA: Lazy<Pubkey> = Lazy::new(|| Pubkey::find_program_address(&[GLOBAL_SEED], &PUMPFUN_PROGRAM).0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalAccount {
    pub discriminator: u64,
    pub initialized: bool,
    pub authority: Pubkey,
    pub fee_recipient: Pubkey,
    pub initial_virtual_token_reserves: u64,
    pub initial_virtual_sol_reserves: u64,
    pub initial_real_token_reserves: u64,
    pub token_total_supply: u64,
    pub fee_basis_points: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BondingCurveAccount {
    pub discriminator: u64,
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub token_total_supply: u64,
    pub complete: bool,
    pub creator: Pubkey,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct CreateInfo {
    pub discriminator: u64,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub creator: Pubkey,
}

impl CreateInfo {
    pub fn from_create(create: &Create, creator: Pubkey) -> Self {
        Self {
            discriminator: 8576854823835016728,
            name: create.name.to_string(),
            symbol: create.symbol.to_string(),
            uri: create.uri.to_string(),
            creator,
        }
    }
}
