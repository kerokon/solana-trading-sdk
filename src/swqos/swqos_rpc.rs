use base64::{engine::general_purpose, Engine};
use solana_sdk::transaction::VersionedTransaction;
use std::{str::FromStr, time::Duration};
use tokio::time::timeout;
use crate::common::Transaction;

pub const SWQOS_RPC_TIMEOUT: std::time::Duration = Duration::from_secs(10);

pub struct SWQoSRequest {
    pub name: String,
    pub url: String,
    pub auth_header: Option<(String, String)>,
    pub transactions: Vec<Transaction>,
}

pub trait FormatBase64VersionedTransaction {
    fn to_base64_string(&self) -> String;
}

impl FormatBase64VersionedTransaction for VersionedTransaction {
    fn to_base64_string(&self) -> String {
        let tx_bytes = bincode::serialize(self).unwrap();
        general_purpose::STANDARD.encode(tx_bytes)
    }
}
impl FormatBase64VersionedTransaction for solana_sdk::transaction::Transaction {
    fn to_base64_string(&self) -> String {
        let tx_bytes = bincode::serialize(self).unwrap();
        general_purpose::STANDARD.encode(tx_bytes)
    }
}
#[async_trait::async_trait]
pub trait SWQoSClientTrait {
    fn new_swqos_client() -> reqwest::Client {
        reqwest::Client::builder().timeout(SWQOS_RPC_TIMEOUT).build().unwrap()
    }
    async fn swqos_send_transaction(&self, request: SWQoSRequest) -> anyhow::Result<()>;
    async fn swqos_send_transactions(&self, request: SWQoSRequest) -> anyhow::Result<()>;
    async fn swqos_json_post(&self, request: SWQoSRequest, body: serde_json::Value) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
impl SWQoSClientTrait for reqwest::Client {
    async fn swqos_send_transaction(&self, request: SWQoSRequest) -> anyhow::Result<()> {
        let base64_tx = match &request.transactions[0]{
            Transaction::Legacy(t) => {t.to_base64_string()}
            Transaction::Versioned(t) => {t.to_base64_string() }
        };

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "sendTransaction",
            "params": [
                base64_tx,
                { "encoding": "base64" }
            ],
            "id": 1,
        });

        self.swqos_json_post(request, body).await
    }

    async fn swqos_send_transactions(&self, request: SWQoSRequest) -> anyhow::Result<()> {
        let txs_base64:Vec<String> = request.transactions.iter().map(|tx| {
            match &tx{
                Transaction::Legacy(t) => {t.to_base64_string()}
                Transaction::Versioned(t) => {t.to_base64_string() }
            }
        }).collect();
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "sendTransactions",
            "params": [
                txs_base64,
                { "encoding": "base64" }
            ],
            "id": 1,
        });

        self.swqos_json_post(request, body).await
    }

    async fn swqos_json_post(&self, request: SWQoSRequest, body: serde_json::Value) -> anyhow::Result<()> {
        let signature = match &request.transactions[0]{
            Transaction::Legacy(t) => {t.signatures[0]}
            Transaction::Versioned(t) => {t.signatures[0] }
        };
        let txs_hash = request
            .transactions
            .iter()
            .map(|tx| signature.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let response = if let Some((key, value)) = request.auth_header {
            timeout(SWQOS_RPC_TIMEOUT, self.post(request.url).header(key, value).json(&body).send()).await??
        } else {
            timeout(SWQOS_RPC_TIMEOUT, self.post(request.url).json(&body).send()).await??
        };
        let http_status = response.status();
        let response_body = timeout(SWQOS_RPC_TIMEOUT, response.text()).await??;

        if !http_status.is_success() {
            let error = format!("swqos_json_post error: {} {} {} {}", request.name, txs_hash, http_status, response_body);
            eprintln!("{}", error);
            return Err(anyhow::anyhow!(error));
        }

        let response_json = serde_json::Value::from_str(&response_body)?;
        if let Some(error) = response_json.get("error") {
            let error = format!("swqos_json_post error: {} {} {} {}", request.name, txs_hash, http_status, error.to_string());
            eprintln!("{}", error);
            return Err(anyhow::anyhow!(error));
        }

        println!("swqos_json_post success: {} {} {:#?}", request.name, txs_hash, response_json);

        Ok(())
    }
}
