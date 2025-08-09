use super::{swqos_rpc::SWQoSRequest, SWQoSTrait};
use crate::{common::transaction::Transaction, swqos::swqos_rpc::SWQoSClientTrait};
use base64::{engine::general_purpose, Engine};
use rand::seq::IndexedRandom;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey, pubkey::Pubkey, transaction::VersionedTransaction};
use std::sync::Arc;

pub const NEXTBLOCK_TIP_ACCOUNTS: &[Pubkey] = &[
    pubkey!("NextbLoCkVtMGcV47JzewQdvBpLqT9TxQFozQkN98pE"),
    pubkey!("NexTbLoCkWykbLuB1NkjXgFWkX9oAtcoagQegygXXA2"),
    pubkey!("NeXTBLoCKs9F1y5PJS9CKrFNNLU1keHW71rfh7KgA1X"),
    pubkey!("NexTBLockJYZ7QD7p2byrUa6df8ndV2WSd8GkbWqfbb"),
    pubkey!("neXtBLock1LeC67jYd1QdAa32kbVeubsfPNTJC1V5At"),
    pubkey!("nEXTBLockYgngeRmRrjDV31mGSekVPqZoMGhQEZtPVG"),
    pubkey!("NEXTbLoCkB51HpLBLojQfpyVAMorm3zzKg7w9NFdqid"),
    pubkey!("nextBLoCkPMgmG8ZgJtABeScP35qLa2AMCNKntAP7Xc"),
];

pub const NEXTBLOCK_ENDPOINT_FRA: &str = "https://fra.nextblock.io";
pub const NEXTBLOCK_ENDPOINT_NY: &str = "https://ny.nextblock.io";

#[derive(Clone)]
pub struct NextBlockClient {
    pub rpc_client: Arc<RpcClient>,
    pub swqos_endpoint: String,
    pub swqos_header: Option<(String, String)>,
    pub swqos_client: Arc<reqwest::Client>,
}

#[async_trait::async_trait]
impl SWQoSTrait for NextBlockClient {
    async fn send_transaction(&self, transaction: Transaction) -> anyhow::Result<()> {
        let versioned_tx = match transaction {
            Transaction::Legacy(tx) => VersionedTransaction::from(tx),
            Transaction::Versioned(tx) => tx,
        };

        let tx_bytes = bincode::serialize(&versioned_tx)?;
        let tx_base64 = general_purpose::STANDARD.encode(tx_bytes);
        let body = serde_json::json!({
            "transaction": {
                "content": tx_base64,
            },
            "frontRunningProtection": false,
        });

        let url = format!("{}/api/v2/submit", self.swqos_endpoint);
        self.swqos_client
            .swqos_json_post(
                SWQoSRequest {
                    name: self.get_name().to_string(),
                    url: url.clone(),
                    auth_header: self.swqos_header.clone(),
                    transactions: vec![versioned_tx],
                },
                body,
            )
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

        let body = serde_json::json!({
            "entries":  versioned_txs
                .iter()
                .map(|tx| {
                    let tx_bytes = bincode::serialize(tx).unwrap();
                    let tx_base64 = general_purpose::STANDARD.encode(tx_bytes);
                    serde_json::json!({
                        "transaction": {
                            "content": tx_base64,
                        },
                    })
                })
                .collect::<Vec<_>>(),
        });

        let url = format!("{}/api/v2/submit-batch", self.swqos_endpoint);
        self.swqos_client
            .swqos_json_post(
                SWQoSRequest {
                    name: self.get_name().to_string(),
                    url: url.clone(),
                    auth_header: self.swqos_header.clone(),
                    transactions: versioned_txs,
                },
                body,
            )
            .await
    }

    fn get_tip_account(&self) -> Option<Pubkey> {
        Some(*NEXTBLOCK_TIP_ACCOUNTS.choose(&mut rand::rng())?)
    }

    fn get_name(&self) -> &str {
        "nextblock"
    }
}

impl NextBlockClient {
    pub fn new(rpc_client: Arc<RpcClient>, endpoint: String, auth_token: String) -> Self {
        let swqos_client = reqwest::Client::new_swqos_client();

        Self {
            rpc_client,
            swqos_endpoint: endpoint,
            swqos_header: Some(("Authorization".to_string(), auth_token)),
            swqos_client: Arc::new(swqos_client),
        }
    }
}
