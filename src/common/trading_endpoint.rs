use crate::{
    common::transaction::Transaction,
    errors::trading_endpoint_error::TradingEndpointError,
    instruction::builder::{build_transaction, PriorityFee, TipFee},
    swqos::SWQoSTrait,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    hash::Hash,
    instruction::Instruction,
    signature::{Keypair, Signature},
};
use std::sync::Arc;

pub struct TradingEndpoint {
    pub rpc: Arc<RpcClient>,
    pub swqos: Arc<Vec<Arc<dyn SWQoSTrait>>>,
}

pub struct BatchTxItem {
    pub payer: Keypair,
    pub instructions: Vec<Instruction>,
}

impl TradingEndpoint {
    pub fn new(rpc: Arc<RpcClient>, swqos: Vec<Arc<dyn SWQoSTrait>>) -> Self {
        Self { rpc, swqos: Arc::new(swqos) }
    }

    pub async fn get_latest_blockhash(&self) -> Result<Hash, TradingEndpointError> {
        let blockhash = self.rpc.get_latest_blockhash().await?;
        Ok(blockhash)
    }

    pub async fn build_and_broadcast_tx(
        &self,
        payer: &Keypair,
        instructions: Vec<Instruction>,
        blockhash: Hash,
        fee: Option<PriorityFee>,
        tip: Option<u64>,
        other_signers: Option<Vec<&Keypair>>,
    ) -> Result<Vec<Signature>, TradingEndpointError> {
        let mut signatures = vec![];
        let mut txs_to_send = Vec::new();

        for swqos in self.swqos.iter() {
            let tip = if let Some(tip_account) = swqos.get_tip_account() {
                if let Some(tip) = tip {
                    Some(TipFee {
                        tip_account,
                        tip_lamports: tip,
                    })
                } else {
                    return Err(TradingEndpointError::TransactionError(format!(
                        "Tip value not provided for SWQoS: {}",
                        swqos.get_name()
                    )));
                }
            } else {
                None
            };

            let tx = build_transaction(payer, instructions.clone(), blockhash, fee, tip, other_signers.as_ref().map(|v| v.to_vec()))
                .map_err(|e| TradingEndpointError::TransactionError(e.to_string()))?;

            let signature = match tx {
                Transaction::Legacy(ref tx) => tx.signatures[0],
                Transaction::Versioned(ref tx) => tx.signatures[0],
            };
            signatures.push(signature);
            txs_to_send.push((swqos.clone(), tx));
        }

        let tasks: Vec<_> = txs_to_send
            .into_iter()
            .map(|(swqos, tx)| async move { swqos.send_transaction(tx).await })
            .collect();
        let results = futures::future::join_all(tasks).await;

        let errors: Vec<_> = results.into_iter().filter_map(Result::err).collect();

        if !errors.is_empty() {
            return Err(TradingEndpointError::CustomError(format!(
                "Errors occurred while sending transactions: {:?}",
                errors
            )));
        }

        Ok(signatures)
    }
    pub async fn build_and_broadcast_batch_txs(
        &self,
        items: Vec<BatchTxItem>,
        blockhash: Hash,
        fee: PriorityFee,
        tip: u64,
    ) -> Result<Vec<Signature>, TradingEndpointError> {
        let mut tasks = vec![];
        let mut signatures = vec![];
        for swqos in self.swqos.iter() {
            let tip_account = swqos
                .get_tip_account()
                .ok_or_else(|| TradingEndpointError::TransactionError(format!("No tip account provided for SWQoS: {}", swqos.get_name())))?;
            let mut tip = Some(TipFee {
                tip_account,
                tip_lamports: tip,
            });

            let txs = items
                .iter()
                .map(|item| build_transaction(&item.payer, item.instructions.clone(), blockhash, Some(fee), tip.take(), None))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| TradingEndpointError::TransactionError(e.to_string()))?;

            signatures.extend(txs.iter().map(|tx| match tx {
                Transaction::Legacy(ref tx) => tx.signatures[0],
                Transaction::Versioned(ref tx) => tx.signatures[0],
            }));
            tasks.push(swqos.send_transactions(txs));
        }

        let result = futures::future::join_all(tasks).await;
        let errors = result.into_iter().filter_map(|res| res.err()).collect::<Vec<_>>();
        if !errors.is_empty() {
            return Err(TradingEndpointError::CustomError(format!("{:?}", errors)));
        }

        Ok(signatures)
    }
}
