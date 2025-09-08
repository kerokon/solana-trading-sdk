use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{native_token::sol_str_to_lamports, pubkey::Pubkey, signature::Keypair};
use solana_trading_sdk::{
    common::{trading_endpoint::TradingEndpoint, TradingClient, TradingConfig},
    dex::{
        dex_traits::DexTrait,
        pumpfun::Pumpfun,
        types::{Create, DexType},
    },
    errors::trading_endpoint_error::TradingEndpointError,
    instruction::builder::PriorityFee,
    ipfs::{metadata::create_token_metadata, types::CreateTokenMetadata},
    swqos::{
        blox::BLOX_ENDPOINT_FRA, default::DefaultSWQoSClient, jito::JITO_ENDPOINT_MAINNET, nextblock::NEXTBLOCK_ENDPOINT_FRA, temporal::TEMPORAL_ENDPOINT_FRA,
        zeroslot::ZEROSLOT_ENDPOINT_FRA, SWQoSType,
    },
};
use std::{str::FromStr, sync::Arc};

const RPC_ENDPOINT: &str = "https://solana-rpc.publicnode.com";

#[tokio::main]
async fn main() -> Result<(), TradingEndpointError> {
    Ok(())
}

pub fn get_solana_client() -> Arc<RpcClient> {
    Arc::new(RpcClient::new(RPC_ENDPOINT.to_string()))
}

pub fn get_swqos_client() -> DefaultSWQoSClient {
    let swqos_client = DefaultSWQoSClient::new("default", get_solana_client(), RPC_ENDPOINT.to_string(), None, vec![]);
    swqos_client
}

pub async fn transfer_sol() -> Result<(), TradingEndpointError> {
    let rpc_url = "https://solana-rpc.publicnode.com".to_string();
    let from = Keypair::from_base58_string("your_payer_pubkey");
    let to = Pubkey::from_str("recipient_pubkey").unwrap();
    let amount = sol_str_to_lamports("0.1").unwrap();
    let fee = PriorityFee {
        unit_limit: 100000,
        unit_price: 10000000,
    };
    let swqos_client = DefaultSWQoSClient::new("default", Arc::new(RpcClient::new(rpc_url.clone())), rpc_url.to_string(), None, vec![]);
    swqos_client.transfer(&from, &to, amount, Some(fee)).await?;
    Ok(())
}

pub async fn transfer_token() -> Result<(), TradingEndpointError> {
    let from = Keypair::from_base58_string("your_payer_pubkey");
    let to = Pubkey::from_str("recipient_pubkey").unwrap();
    let mint = Pubkey::from_str("token_mint_pubkey").unwrap();
    let amount = 1000;
    let fee = PriorityFee {
        unit_limit: 100000,
        unit_price: 10000000,
    };
    let swqos_client = get_swqos_client();
    swqos_client.spl_transfer(&from, &to, &mint, amount, Some(fee)).await?;
    Ok(())
}

pub async fn get_trading_client() -> Result<TradingClient, TradingEndpointError> {
    let rpc_url = "https://solana-rpc.publicnode.com".to_string();

    let client = TradingClient::new(&TradingConfig {
        rpc_url: rpc_url.to_string(),
        swqos: vec![],
    })
    .map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;

    client.initialize().await?;
    Ok(client)
}

pub async fn swap() -> Result<(), TradingEndpointError> {
    let client = get_trading_client().await?;
    let payer = Keypair::from_base58_string("your_payer_pubkey");
    let mint = Pubkey::from_str("token_mint_pubkey").unwrap();
    let sol_amount = sol_str_to_lamports("1.0").unwrap();
    let slippage_basis_points = 3000;
    let fee = PriorityFee {
        unit_limit: 100000,
        unit_price: 10000000,
    };

    let tip = sol_str_to_lamports("0.001").unwrap();

    client.dexs[&DexType::PumpSwap]
        .buy(&payer, &mint, sol_amount, slippage_basis_points, Some(fee), Some(tip))
        .await?;

    Ok(())
}
