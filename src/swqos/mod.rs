use crate::common::transaction::Transaction;
pub mod blox;
pub mod default;
pub mod jito;
pub mod nextblock;
pub mod swqos_rpc;
pub mod temporal;
pub mod zeroslot;

use blox::BloxClient;
use default::DefaultSWQoSClient;
use jito::JitoClient;
use nextblock::NextBlockClient;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::{any::Any, sync::Arc};
use temporal::TEMPORAL_TIP_ACCOUNTS;
use zeroslot::ZEROSLOT_TIP_ACCOUNTS;

// (endpoint, auth_token)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SWQoSType {
    Default(String, Option<(String, String)>),
    Jito(String),
    NextBlock(String, String),
    Blox(String, String),
    Temporal(String, String),
    ZeroSlot(String, String),
}

#[async_trait::async_trait]

pub trait SWQoSTrait: Send + Sync + Any {
    async fn send_transaction(&self, transaction: Transaction) -> anyhow::Result<()>;
    async fn send_transactions(&self, transactions: Vec<Transaction>) -> anyhow::Result<()>;
    fn get_tip_account(&self) -> Option<Pubkey>;
    fn get_name(&self) -> &str;
}

impl SWQoSType {
    pub fn instantiate(&self, rpc_client: Arc<RpcClient>) -> Arc<dyn SWQoSTrait> {
        match self {
            SWQoSType::Default(endpoint, header) => Arc::new(DefaultSWQoSClient::new("default", rpc_client, endpoint.to_string(), header.clone(), vec![])),
            SWQoSType::Jito(endpoint) => Arc::new(JitoClient::new(rpc_client, endpoint.to_string())),
            SWQoSType::NextBlock(endpoint, auth_token) => Arc::new(NextBlockClient::new(rpc_client, endpoint.to_string(), auth_token.to_string())),
            SWQoSType::Blox(endpoint, auth_token) => Arc::new(BloxClient::new(rpc_client, endpoint.to_string(), auth_token.to_string())),
            SWQoSType::ZeroSlot(endpoint, auth_token) => Arc::new(DefaultSWQoSClient::new(
                "0slot",
                rpc_client,
                format!("{}?api-key={}", endpoint, auth_token),
                None,
                ZEROSLOT_TIP_ACCOUNTS.into(),
            )),

            SWQoSType::Temporal(endpoint, auth_token) => Arc::new(DefaultSWQoSClient::new(
                "temporal",
                rpc_client,
                format!("{}?c={}", endpoint, auth_token),
                None,
                TEMPORAL_TIP_ACCOUNTS.into(),
            )),
        }
    }
}
