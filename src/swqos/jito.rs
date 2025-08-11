use super::{
    swqos_rpc::{SWQoSClientTrait, SWQoSRequest},
    SWQoSTrait,
};
use crate::{common::transaction::Transaction, swqos::swqos_rpc::FormatBase64VersionedTransaction};
use rand::seq::IndexedRandom;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey, pubkey::Pubkey, transaction::VersionedTransaction};
use std::sync::Arc;

pub const JITO_TIP_ACCOUNTS: &[Pubkey] = &[
    pubkey!("96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5"),
    pubkey!("HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe"),
    pubkey!("Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY"),
    pubkey!("ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49"),
    pubkey!("DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh"),
    pubkey!("ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt"),
    pubkey!("DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL"),
    pubkey!("3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT"),
];

pub const JITO_ENDPOINT_MAINNET: &str = "https://mainnet.block-engine.jito.wtf";
pub const JITO_ENDPOINT_MAS: &str = "https://amsterdam.mainnet.block-engine.jito.wtf";
pub const JITO_ENDPOINT_FRA: &str = "https://frankfurt.mainnet.block-engine.jito.wtf";
pub const JITO_ENDPOINT_LONDON: &str = "https://london.mainnet.block-engine.jito.wtf";
pub const JITO_ENDPOINT_NY: &str = "https://ny.mainnet.block-engine.jito.wtf";
pub const JITO_ENDPOINT_TOKYO: &str = "https://tokyo.mainnet.block-engine.jito.wtf";
pub const JITO_ENDPOINT_SLC: &str = "https://slc.mainnet.block-engine.jito.wtf";

pub const JITO_RELAYER_AMS: &str = "http://amsterdam.mainnet.relayer.jito.wtf:8100";
pub const JITO_RELAYER_TOKYO: &str = "http://tokyo.mainnet.relayer.jito.wtf:8100";
pub const JITO_RELAYER_NY: &str = "http://ny.mainnet.relayer.jito.wtf:8100";
pub const JITO_RELAYER_FRA: &str = "http://frankfurt.mainnet.relayer.jito.wtf:8100";
pub const JITO_RELAYER_LONDON: &str = "http://london.mainnet.relayer.jito.wtf:8100";

#[derive(Clone)]
pub struct JitoClient {
    pub rpc_client: Arc<RpcClient>,
    pub swqos_endpoint: String,
    pub swqos_client: Arc<reqwest::Client>,
}

#[async_trait::async_trait]
impl SWQoSTrait for JitoClient {
    async fn send_transaction(&self, transaction: Transaction) -> anyhow::Result<()> {
        let versioned_tx = match transaction {
            Transaction::Legacy(tx) => VersionedTransaction::from(tx),
            Transaction::Versioned(tx) => tx,
        };
        
        
        self.swqos_client
            .swqos_send_transaction(SWQoSRequest {
                name: self.get_name().to_string(),
                url: format!("{}/api/v1/transactions", self.swqos_endpoint),
                auth_header: None,
                transactions: vec![versioned_tx],
            })
            .await
    }

    async fn send_transactions(&self, transactions: Vec<Transaction>) -> anyhow::Result<()> {
        let versioned_txs = transactions
            .into_iter()
            .map(|tx| match tx {
                Transaction::Legacy(tx) => VersionedTransaction::from(tx),
                Transaction::Versioned(tx) => tx,
            })
            .collect::<Vec<_>>();

        let txs_base64 = versioned_txs.iter().map(|tx| tx.to_base64_string()).collect::<Vec<String>>();
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "sendBundle",
            "params": [
                txs_base64,
                { "encoding": "base64" }
            ],
            "id": 1,
        });

        self.swqos_client
            .swqos_json_post(
                SWQoSRequest {
                    name: self.get_name().to_string(),
                    url: format!("{}/api/v1/bundles", self.swqos_endpoint),
                    auth_header: None,
                    transactions: versioned_txs,
                },
                body,
            )
            .await
    }

    fn get_tip_account(&self) -> Option<Pubkey> {
        Some(*JITO_TIP_ACCOUNTS.choose(&mut rand::rng())?)
    }

    fn get_name(&self) -> &str {
        "jito"
    }
}

impl JitoClient {
    pub fn new(rpc_client: Arc<RpcClient>, endpoint: String) -> Self {
        let swqos_client = reqwest::Client::new_swqos_client();

        Self {
            rpc_client,
            swqos_endpoint: endpoint,
            swqos_client: Arc::new(swqos_client),
        }
    }
}
