use super::{
    swqos_rpc::{SWQoSClientTrait, SWQoSRequest},
    SWQoSTrait,
};
use crate::{common::transaction::Transaction, swqos::swqos_rpc::FormatBase64VersionedTransaction};
use rand::seq::IndexedRandom;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey, pubkey::Pubkey, transaction::VersionedTransaction};
use std::sync::Arc;

pub const BLOX_TIP_ACCOUNTS: &[Pubkey] = &[
    pubkey!("HWEoBxYs7ssKuudEjzjmpfJVX7Dvi7wescFsVx2L5yoY"),
    pubkey!("95cfoy472fcQHaw4tPGBTKpn6ZQnfEPfBgDQx6gcRmRg"),
    pubkey!("3UQUKjhMKaY2S6bjcQD6yHB7utcZt5bfarRCmctpRtUd"),
    pubkey!("FogxVNs6Mm2w9rnGL1vkARSwJxvLE8mujTv3LK8RnUhF"),
];

pub const BLOX_ENDPOINT_FRA: &str = "https://germany.solana.dex.blxrbdn.com";
pub const BLOX_ENDPOINT_AMS: &str = "https://amsterdam.solana.dex.blxrbdn.com";
pub const BLOX_ENDPOINT_NY: &str = "https://ny.solana.dex.blxrbdn.com";
pub const BLOX_ENDPOINT_UK: &str = "https://uk.solana.dex.blxrbdn.com";
pub const BLOX_ENDPOINT_LA: &str = "https://la.solana.dex.blxrbdn.com";
pub const BLOX_ENDPOINT_TOKYO: &str = "https://tokyo.solana.dex.blxrbdn.com";

#[derive(Clone)]
pub struct BloxClient {
    pub rpc_client: Arc<RpcClient>,
    pub swqos_endpoint: String,
    pub swqos_header: Option<(String, String)>,
    pub swqos_client: Arc<reqwest::Client>,
}

#[async_trait::async_trait]
impl SWQoSTrait for BloxClient {
    async fn send_transaction(&self, transaction: Transaction) -> anyhow::Result<()> {

        let body = serde_json::json!({
            "transaction": {
                "content": transaction.to_base64_string(),
            },
            "frontRunningProtection": false,
            "useStakedRPCs": true,
        });

        self.swqos_client
            .swqos_json_post(
                SWQoSRequest {
                    name: self.get_name().to_string(),
                    url: format!("{}/api/v2/submit", self.swqos_endpoint),
                    auth_header: self.swqos_header.clone(),
                    transactions: vec![transaction],
                },
                body,
            )
            .await
    }

    async fn send_transactions(&self, transactions: Vec<Transaction>) -> anyhow::Result<()> {

        let body = serde_json::json!({
            "entries":  transactions
                .iter()
                .map(|tx| {
                    serde_json::json!({
                        "transaction": {
                            "content": tx.to_base64_string(),
                        },
                    })
                })
                .collect::<Vec<_>>(),
        });

        self.swqos_client
            .swqos_json_post(
                SWQoSRequest {
                    name: self.get_name().to_string(),
                    url: format!("{}/api/v2/submit-batch", self.swqos_endpoint),
                    auth_header: self.swqos_header.clone(),
                    transactions,
                },
                body,
            )
            .await
    }

    fn get_tip_account(&self) -> Option<Pubkey> {
        Some(*BLOX_TIP_ACCOUNTS.choose(&mut rand::rng())?)
    }

    fn get_name(&self) -> &str {
        "blox"
    }
}

impl BloxClient {
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
