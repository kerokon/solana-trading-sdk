use crate::common::transaction::Transaction;
use crate::errors::swqos_error::SWQoSError;
pub mod block_razor;
pub mod blox;
pub mod default;
pub mod jito;
pub mod nextblock;
pub mod swqos_rpc;
pub mod temporal;
pub mod zeroslot;

use crate::common::lamports::Lamports;
use crate::instruction::builder::PriorityFee;
use crate::swqos::block_razor::{BlockRazorClient, BLOCK_RAZOR_TIP_ACCOUNTS};
use crate::swqos::blox::BLOX_TIP_ACCOUNTS;
use crate::swqos::jito::JITO_TIP_ACCOUNTS;
use crate::swqos::nextblock::NEXTBLOCK_TIP_ACCOUNTS;
use blox::BloxClient;
use default::DefaultSWQoSClient;
use jito::JitoClient;
use nextblock::NextBlockClient;
use serde::{Deserialize, Serialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::{any::Any, sync::Arc};
use temporal::TEMPORAL_TIP_ACCOUNTS;
use zeroslot::ZEROSLOT_TIP_ACCOUNTS;

// (endpoint, auth_token)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum SWQoSType {
    Default(String, Option<(String, String)>),
    Jito(String),
    NextBlock(String, String),
    Blox(String, String),
    Temporal(String, String),
    ZeroSlot(String, String),
    BlockRazor(String, String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SWQoSConfig {
    pub kind: SWQoSType,
    pub threads: u64,
    #[serde(default)]
    pub buy_tip: Option<Lamports>,
    #[serde(default)]
    pub buy_fee: Option<PriorityFee>,
    #[serde(default)]
    pub sell_tip: Option<Lamports>,
    #[serde(default)]
    pub sell_fee: Option<PriorityFee>,
}

pub struct SWQoSRuntime {
    pub config: SWQoSConfig,
    pub client: Arc<dyn SWQoSTrait>,
}

#[async_trait::async_trait]
pub trait SWQoSTrait: Send + Sync + Any {
    async fn send_transaction(&self, transaction: Transaction) -> Result<(), SWQoSError>;
    async fn send_transactions(&self, transactions: Vec<Transaction>) -> Result<(), SWQoSError>;
    fn get_tip_account(&self) -> Option<Pubkey>;
    fn get_name(&self) -> &str;
}

impl SWQoSConfig {
    pub fn new(kind: SWQoSType) -> Self {
        Self {
            kind,
            threads: 1,
            buy_tip: None,
            buy_fee: None,
            sell_tip: None,
            sell_fee: None,
        }
    }

    pub fn with_threads(mut self, threads: u64) -> Self {
        self.threads = threads;
        self
    }

    pub fn with_buy_config(mut self, tip: Option<Lamports>, fee: Option<PriorityFee>) -> Self {
        self.buy_tip = tip;
        self.buy_fee = fee;
        self
    }

    pub fn with_sell_config(mut self, tip: Option<Lamports>, fee: Option<PriorityFee>) -> Self {
        self.sell_tip = tip;
        self.sell_fee = fee;
        self
    }

    pub fn with_buy_tip(mut self, tip: Lamports) -> Self {
        self.buy_tip = Some(tip);
        self
    }

    pub fn with_buy_fee(mut self, fee: PriorityFee) -> Self {
        self.buy_fee = Some(fee);
        self
    }

    pub fn with_sell_tip(mut self, tip: Lamports) -> Self {
        self.sell_tip = Some(tip);
        self
    }

    pub fn with_sell_fee(mut self, fee: PriorityFee) -> Self {
        self.sell_fee = Some(fee);
        self
    }

    pub fn get_buy_config(&self) -> (Option<Lamports>, Option<PriorityFee>) {
        (self.buy_tip, self.buy_fee.clone())
    }

    pub fn get_sell_config(&self) -> (Option<Lamports>, Option<PriorityFee>) {
        (self.sell_tip, self.sell_fee.clone())
    }

    /// Create multiple SWQoSRuntime instances based on the threads configuration
    pub fn build_runtimes(self, rpc_client: Arc<RpcClient>) -> Vec<SWQoSRuntime> {
        let clients = self.kind.instantiate_many(rpc_client, self.threads);

        clients.into_iter().map(|client| SWQoSRuntime { config: self.clone(), client }).collect()
    }
}

impl SWQoSRuntime {
    pub fn new(config: SWQoSConfig, rpc_client: Arc<RpcClient>) -> Vec<Self> {
        config.build_runtimes(rpc_client)
    }

    /// Create a single SWQoSRuntime with one client
    pub fn new_single(config: SWQoSConfig, rpc_client: Arc<RpcClient>) -> Self {
        let client = config.kind.instantiate(rpc_client);
        Self { config, client }
    }

    pub fn get_buy_config(&self) -> (Option<Lamports>, Option<PriorityFee>) {
        self.config.get_buy_config()
    }

    pub fn get_sell_config(&self) -> (Option<Lamports>, Option<PriorityFee>) {
        self.config.get_sell_config()
    }

    pub fn get_client(&self) -> &Arc<dyn SWQoSTrait> {
        &self.client
    }

    pub async fn send_transaction(&self, transaction: Transaction) -> Result<(), SWQoSError> {
        self.client.send_transaction(transaction).await
    }

    pub async fn send_transactions(&self, transactions: Vec<Transaction>) -> Result<(), SWQoSError> {
        self.client.send_transactions(transactions).await
    }

    pub fn get_tip_account(&self) -> Option<Pubkey> {
        self.client.get_tip_account()
    }

    pub fn get_client_name(&self) -> &str {
        self.client.get_name()
    }
}

impl SWQoSType {
    fn instantiate(&self, rpc_client: Arc<RpcClient>) -> Arc<dyn SWQoSTrait> {
        match self {
            SWQoSType::Default(endpoint, header) => Arc::new(DefaultSWQoSClient::new("default", rpc_client, endpoint.to_string(), header.clone(), vec![])),

            SWQoSType::Jito(endpoint) => Arc::new(JitoClient::new(rpc_client, endpoint.to_string(), JITO_TIP_ACCOUNTS.into())),

            SWQoSType::NextBlock(endpoint, auth_token) => Arc::new(NextBlockClient::new(
                rpc_client,
                endpoint.to_string(),
                auth_token.to_string(),
                NEXTBLOCK_TIP_ACCOUNTS.into(),
            )),

            SWQoSType::Blox(endpoint, auth_token) => Arc::new(BloxClient::new(
                rpc_client,
                endpoint.to_string(),
                auth_token.to_string(),
                BLOX_TIP_ACCOUNTS.into(),
            )),

            SWQoSType::BlockRazor(endpoint, auth_token) => Arc::new(BlockRazorClient::new(
                endpoint.to_string(),
                auth_token.to_string(),
                BLOCK_RAZOR_TIP_ACCOUNTS.into(),
            )),

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

    fn instantiate_many(&self, rpc_client: Arc<RpcClient>, threads: u64) -> Vec<Arc<dyn SWQoSTrait>> {
        let threads = threads.max(1); // avoid zero threads
        fn chunk_accounts(accounts: &[Pubkey], threads: u64) -> Vec<Vec<Pubkey>> {
            let threads = threads.min(accounts.len() as u64).max(1) as usize;
            let chunk_size = (accounts.len() + threads - 1) / threads;
            accounts.chunks(chunk_size).map(|c| c.to_vec()).collect()
        }

        match self {
            SWQoSType::Default(endpoint, header) => (0..threads)
                .map(|_| {
                    Arc::new(DefaultSWQoSClient::new(
                        "default",
                        rpc_client.clone(),
                        endpoint.to_string(),
                        header.clone(),
                        vec![],
                    )) as Arc<dyn SWQoSTrait>
                })
                .collect(),

            SWQoSType::Jito(endpoint) => {
                let chunks = chunk_accounts(&JITO_TIP_ACCOUNTS, threads);
                chunks
                    .into_iter()
                    .map(|chunk| Arc::new(JitoClient::new(rpc_client.clone(), endpoint.to_string(), chunk)) as Arc<dyn SWQoSTrait>)
                    .collect()
            }

            SWQoSType::NextBlock(endpoint, auth_token) => {
                let chunks = chunk_accounts(&NEXTBLOCK_TIP_ACCOUNTS, threads);
                chunks
                    .into_iter()
                    .map(|chunk| Arc::new(NextBlockClient::new(rpc_client.clone(), endpoint.to_string(), auth_token.to_string(), chunk)) as Arc<dyn SWQoSTrait>)
                    .collect()
            }

            SWQoSType::Blox(endpoint, auth_token) => {
                let chunks = chunk_accounts(&BLOX_TIP_ACCOUNTS, threads);
                chunks
                    .into_iter()
                    .map(|chunk| Arc::new(BloxClient::new(rpc_client.clone(), endpoint.to_string(), auth_token.to_string(), chunk)) as Arc<dyn SWQoSTrait>)
                    .collect()
            }

            SWQoSType::BlockRazor(endpoint, auth_token) => {
                let chunks = chunk_accounts(&BLOCK_RAZOR_TIP_ACCOUNTS, threads);
                chunks
                    .into_iter()
                    .map(|chunk| Arc::new(BlockRazorClient::new(endpoint.to_string(), auth_token.to_string(), chunk)) as Arc<dyn SWQoSTrait>)
                    .collect()
            }

            SWQoSType::ZeroSlot(endpoint, auth_token) => {
                let chunks = chunk_accounts(&ZEROSLOT_TIP_ACCOUNTS, threads);
                chunks
                    .into_iter()
                    .map(|chunk| {
                        Arc::new(DefaultSWQoSClient::new(
                            "0slot",
                            rpc_client.clone(),
                            format!("{}?api-key={}", endpoint, auth_token),
                            None,
                            chunk,
                        )) as Arc<dyn SWQoSTrait>
                    })
                    .collect()
            }

            SWQoSType::Temporal(endpoint, auth_token) => {
                let chunks = chunk_accounts(&TEMPORAL_TIP_ACCOUNTS, threads);
                chunks
                    .into_iter()
                    .map(|chunk| {
                        Arc::new(DefaultSWQoSClient::new(
                            "temporal",
                            rpc_client.clone(),
                            format!("{}?c={}", endpoint, auth_token),
                            None,
                            chunk,
                        )) as Arc<dyn SWQoSTrait>
                    })
                    .collect()
            }
        }
    }
}
