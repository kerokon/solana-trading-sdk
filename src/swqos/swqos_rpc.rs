use crate::{common::Transaction, errors::swqos_error::SWQoSError};
use base64::{engine::general_purpose, Engine};
use solana_sdk::transaction::VersionedTransaction;
use std::time::Duration;
use tracing::{error, info};

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
        reqwest::Client::builder()
            .http1_only()
            .tcp_keepalive(Some(Duration::from_secs(1_000_000)))
            .pool_idle_timeout(None)
            .timeout(SWQOS_RPC_TIMEOUT)
            .build()
            .unwrap()
    }

    async fn swqos_send_transaction(&self, request: SWQoSRequest) -> Result<(), SWQoSError>;
    async fn swqos_send_transactions(&self, request: SWQoSRequest) -> Result<(), SWQoSError>;
    async fn swqos_json_post(&self, request: SWQoSRequest, body: serde_json::Value) -> Result<(), SWQoSError>;
}

#[async_trait::async_trait]
impl SWQoSClientTrait for reqwest::Client {
    async fn swqos_send_transaction(&self, request: SWQoSRequest) -> Result<(), SWQoSError> {
        let base64_tx = match &request.transactions[0] {
            Transaction::Legacy(t) => t.to_base64_string(),
            Transaction::Versioned(t) => t.to_base64_string(),
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

    async fn swqos_send_transactions(&self, request: SWQoSRequest) -> Result<(), SWQoSError> {
        let txs_base64: Vec<String> = request
            .transactions
            .iter()
            .map(|tx| match tx {
                Transaction::Legacy(t) => t.to_base64_string(),
                Transaction::Versioned(t) => t.to_base64_string(),
            })
            .collect();

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

    async fn swqos_json_post(&self, request: SWQoSRequest, body: serde_json::Value) -> Result<(), SWQoSError> {
        let signature = match &request.transactions[0] {
            Transaction::Legacy(t) => t.signatures[0],
            Transaction::Versioned(t) => t.signatures[0],
        };

        let txs_hash = request.transactions.iter().map(|_| signature.to_string()).collect::<Vec<_>>().join(", ");

        let mut req_builder = self.post(&request.url).json(&body);

        if let Some((key, value)) = request.auth_header {
            req_builder = req_builder.header(key, value);
        }

        let response = req_builder.send().await?;

        let http_status = response.status();
        let response_body = response.text().await?;

        let response_json: serde_json::Value = serde_json::from_str(&response_body)?;
        if let Some(error_value) = response_json.get("error").filter(|e| !e.to_string().is_empty() && e.to_string() != "\"\"") {
            let error_msg = format!(
                "swqos_json_post error: {} {} {} error: {}",
                request.name,
                txs_hash,
                http_status,
                error_value.to_string()
            );
            error!("{}", error_msg);
            return Err(SWQoSError::Custom(error_msg));
        }

        info!("swqos_json_post success: {} {} {:#?}", request.name, txs_hash, response_json);

        Ok(())
    }
}
