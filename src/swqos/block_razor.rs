use crate::common::Transaction;
use crate::errors::swqos_error::SWQoSError;
use crate::swqos::swqos_rpc::{SWQoSClientTrait, SWQoSRequest};
use crate::swqos::SWQoSTrait;
use rand::prelude::IndexedRandom;
use serde::Serialize;
use solana_sdk::{pubkey, pubkey::Pubkey};
use std::sync::Arc;

pub const BLOCK_RAZOR_ENDPOINT_FRA: &str = "http://frankfurt.solana.blockrazor.xyz:443";
pub const BLOCK_RAZOR_ENDPOINT_NY: &str = "http://newyork.solana.blockrazor.xyz:443";
pub const BLOCK_RAZOR_ENDPOINT_TOKYO: &str = "http://tokyo.solana.blockrazor.xyz:443";
pub const BLOCK_RAZOR_ENDPOINT_AMS: &str = "http://amsterdam.solana.blockrazor.xyz:443";

pub const BLOCK_RAZOR_TIP_ACCOUNTS: &[Pubkey] = &[
    pubkey!("FjmZZrFvhnqqb9ThCuMVnENaM3JGVuGWNyCAxRJcFpg9"),
    pubkey!("6No2i3aawzHsjtThw81iq1EXPJN6rh8eSJCLaYZfKDTG"),
    pubkey!("A9cWowVAiHe9pJfKAj3TJiN9VpbzMUq6E4kEvf5mUT22"),
    pubkey!("Gywj98ophM7GmkDdaWs4isqZnDdFCW7B46TXmKfvyqSm"),
    pubkey!("68Pwb4jS7eZATjDfhmTXgRJjCiZmw1L7Huy4HNpnxJ3o"),
    pubkey!("4ABhJh5rZPjv63RBJBuyWzBK3g9gWMUQdTZP2kiW31V9"),
    pubkey!("B2M4NG5eyZp5SBQrSdtemzk5TqVuaWGQnowGaCBt8GyM"),
    pubkey!("5jA59cXMKQqZAVdtopv8q3yyw9SYfiE3vUCbt7p8MfVf"),
    pubkey!("5YktoWygr1Bp9wiS1xtMtUki1PeYuuzuCF98tqwYxf61"),
    pubkey!("295Avbam4qGShBYK7E9H5Ldew4B3WyJGmgmXfiWdeeyV"),
    pubkey!("EDi4rSy2LZgKJX74mbLTFk4mxoTgT6F7HxxzG2HBAFyK"),
    pubkey!("BnGKHAC386n4Qmv9xtpBVbRaUTKixjBe3oagkPFKtoy6"),
    pubkey!("Dd7K2Fp7AtoN8xCghKDRmyqr5U169t48Tw5fEd3wT9mq"),
    pubkey!("AP6qExwrbRgBAVaehg4b5xHENX815sMabtBzUzVB4v8S"),
];

#[derive(Clone)]
pub struct BlockRazorClient {
    pub swqos_endpoint: String,
    pub swqos_header: Option<(String, String)>,
    pub swqos_client: Arc<reqwest::Client>,
}

#[derive(Clone, Serialize)]
pub enum Mode {
    #[serde(rename = "fast")]
    Fast,
}
#[async_trait::async_trait]
impl SWQoSTrait for BlockRazorClient {
    async fn send_transaction(&self, transaction: Transaction) -> Result<(), SWQoSError> {
        let tx_base64 = transaction.to_base64_string();
        let body = serde_json::json!({
            "transaction": tx_base64,
        });

        let url = format!("{}/sendTransaction", self.swqos_endpoint);
        self.swqos_client
            .swqos_json_post(
                SWQoSRequest {
                    name: self.get_name().to_string(),
                    url: url.clone(),
                    auth_header: self.swqos_header.clone(),
                    transactions: vec![transaction],
                },
                body,
            )
            .await
    }

    async fn send_transactions(&self, transactions: Vec<Transaction>) -> Result<(), SWQoSError> {
        let body = serde_json::json!({
            "entries":  transactions
                .iter()
                .map(|tx| {

                    let tx_base64 = tx.to_base64_string();
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
                    transactions,
                },
                body,
            )
            .await
    }

    fn get_tip_account(&self) -> Option<Pubkey> {
        Some(*BLOCK_RAZOR_TIP_ACCOUNTS.choose(&mut rand::rng())?)
    }

    fn get_name(&self) -> &str {
        "blockrazor"
    }
}

impl BlockRazorClient {
    pub fn new(endpoint: String, auth_token: String) -> Self {
        let swqos_client = reqwest::Client::new_swqos_client();

        Self {
            swqos_endpoint: endpoint,
            swqos_header: Some(("apikey".to_string(), auth_token)),
            swqos_client: Arc::new(swqos_client),
        }
    }
}
