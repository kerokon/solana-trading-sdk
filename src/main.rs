use anyhow::Ok;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{native_token::sol_str_to_lamports, pubkey::Pubkey, signature::Keypair};
use solana_trading_sdk::{
    common::{trading_endpoint::TradingEndpoint, TradingClient, TradingConfig},
    dex::{
        dex_traits::DexTrait,
        pumpfun::Pumpfun,
        types::{Create, DexType},
    },
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
async fn main() -> anyhow::Result<()> {
    Ok(())
}

pub fn get_solana_client() -> Arc<RpcClient> {
    Arc::new(RpcClient::new(RPC_ENDPOINT.to_string()))
}

pub fn get_swqos_client() -> DefaultSWQoSClient {

    let swqos_client = DefaultSWQoSClient::new("default", get_solana_client(), RPC_ENDPOINT.to_string(), None, vec![]);
    swqos_client
}

pub async fn transfer_sol() -> anyhow::Result<()> {
    let rpc_url = "https://solana-rpc.publicnode.com".to_string();
    let from = Keypair::from_base58_string("your_payer_pubkey");
    let to = Pubkey::from_str("recipient_pubkey")?;
    let amount = sol_str_to_lamports("0.1").unwrap();
    let fee = PriorityFee {
        unit_limit: 100000,
        unit_price: 10000000,
    };
    let swqos_client = DefaultSWQoSClient::new("default", Arc::new(RpcClient::new(rpc_url.clone())), rpc_url.to_string(), None, vec![]);
    swqos_client.transfer(&from, &to, amount, Some(fee)).await?;
    Ok(())
}

pub async fn transfer_token() -> anyhow::Result<()> {
    let from = Keypair::from_base58_string("your_payer_pubkey");
    let to = Pubkey::from_str("recipient_pubkey")?;
    let mint = Pubkey::from_str("token_mint_pubkey")?;
    let amount = 1000;
    let fee = PriorityFee {
        unit_limit: 100000,
        unit_price: 10000000,
    };
    let swqos_client = get_swqos_client();
    swqos_client.spl_transfer(&from, &to, &mint, amount, Some(fee)).await?;
    Ok(())
}

pub async fn get_trading_client() -> anyhow::Result<TradingClient> {
    let rpc_url = "https://solana-rpc.publicnode.com".to_string();
    let client = TradingClient::new(TradingConfig {
        rpc_url: rpc_url.to_string(),
        swqos: vec![
            SWQoSType::Default("https://solana-rpc.publicnode.com".to_string(), None),
            SWQoSType::Jito(JITO_ENDPOINT_MAINNET.to_string()),
            SWQoSType::NextBlock(NEXTBLOCK_ENDPOINT_FRA.to_string(), "your_api_key".to_string()),
            SWQoSType::Blox(BLOX_ENDPOINT_FRA.to_string(), "your_api_key".to_string()),
            SWQoSType::ZeroSlot(ZEROSLOT_ENDPOINT_FRA.to_string(), "your_api_key".to_string()),
            SWQoSType::Temporal(TEMPORAL_ENDPOINT_FRA.to_string(), "your_api_key".to_string()),
        ],
    })?;

    client.initialize().await?;
    Ok(client)
}

pub async fn swap() -> anyhow::Result<()> {
    let client = get_trading_client().await?;
    let payer = Keypair::from_base58_string("your_payer_pubkey");
    let mint = Pubkey::from_str("token_mint_pubkey")?;
    let sol_amount = sol_str_to_lamports("1.0").unwrap();
    let slippage_basis_points = 3000; // 30%
    let fee = PriorityFee {
        unit_limit: 100000,
        unit_price: 10000000,
    };
    let tip = sol_str_to_lamports("0.001").unwrap();

    client.dexs[&DexType::Pumpfun]
        .buy(&payer, &mint, sol_amount, slippage_basis_points, Some(fee), Some(tip))
        .await?;

    client.dexs[&DexType::PumpSwap]
        .buy(&payer, &mint, sol_amount, slippage_basis_points, Some(fee), Some(tip))
        .await?;

    Ok(())
}

pub async fn create() -> anyhow::Result<()> {
    let jwt_token = "jpinata.cloud jwt_token";
    let token_info = CreateTokenMetadata {
        name: "TokenName".to_string(),
        symbol: "TOKEN".to_string(),
        description: "Token description".to_string(),
        file: "data:image/png;base64,base64_image_string".to_string(),
        twitter: Some("twitter".to_string()),
        telegram: Some("telegram".to_string()),
        website: Some("https://example.com".to_string()),
        metadata_uri: None,
    };
    let metadata = create_token_metadata(token_info, jwt_token).await?;

    let swqos_client = get_swqos_client();
    let trading_endpoint = TradingEndpoint::new(get_solana_client(), vec![Arc::new(swqos_client)]);
    let pumpfun_client = Pumpfun::new(Arc::new(trading_endpoint));

    let payer = Keypair::from_base58_string("your_payer_keypair");
    let mint_key = Keypair::from_base58_string("your_mint_keypair");
    let buy_sol_amount = Some(sol_str_to_lamports("0.1").unwrap());
    let slippage_basis_points = 3000; // 30%
    let fee = PriorityFee {
        unit_limit: 100000,
        unit_price: 10000000,
    };
    let tip = sol_str_to_lamports("0.001").unwrap();

    let create = Create {
        mint_private_key: mint_key,
        name: metadata.metadata.name,
        symbol: metadata.metadata.symbol,
        uri: metadata.metadata_uri,
        buy_sol_amount,
        slippage_basis_points: Some(slippage_basis_points),
    };
    pumpfun_client.create(payer, create, Some(fee), Some(tip)).await?;

    Ok(())
}
