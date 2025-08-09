use crate::{
    common::transaction::Transaction,
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

    pub async fn get_latest_blockhash(&self) -> anyhow::Result<Hash> {
        let blockhash = self.rpc.get_latest_blockhash().await?;
        Ok(blockhash)
    }

    pub fn build_and_broadcast_tx(
        &self,
        payer: &Keypair,
        instructions: Vec<Instruction>,
        blockhash: Hash,
        fee: Option<PriorityFee>,
        tip: Option<u64>,
        other_signers: Option<Vec<&Keypair>>,
    ) -> anyhow::Result<Vec<Signature>> {
        let mut signatures = vec![];
        let mut txs = Vec::new();

        for swqos in self.swqos.iter() {
            let tip = if let Some(tip_account) = swqos.get_tip_account() {
                if let Some(tip) = tip {
                    Some(TipFee {
                        tip_account,
                        tip_lamports: tip,
                    })
                } else {
                    // If no tip is provided, skip this Tip-SWQoS
                    eprintln!("No tip provided for SWQoS: {}", swqos.get_name());
                    txs.push(None);
                    continue;
                }
            } else {
                None
            };

            let tx = build_transaction(payer, instructions.clone(), blockhash, fee, tip, other_signers.clone())?;
            let signature = match tx {
                Transaction::Legacy(ref tx) => tx.signatures[0],
                Transaction::Versioned(ref tx) => tx.signatures[0],
            };
            signatures.push(signature);
            txs.push(Some(tx));
        }

        let all_swqos = self.swqos.clone();
        tokio::spawn(async move {
            let mut tasks = vec![];
            for (swqos, tx) in all_swqos.iter().zip(txs.iter()) {
                if let Some(tx) = tx {
                    tasks.push(swqos.send_transaction(tx.clone()));
                }
            }
            let result = futures::future::join_all(tasks).await;
            let errors = result.into_iter().filter_map(|res| res.err()).collect::<Vec<_>>();
            if errors.len() > 0 {
                eprintln!("Errors occurred while sending transactions: {:?}", errors);
            }
        });

        Ok(signatures)
    }

    pub async fn build_and_broadcast_batch_txs(&self, items: Vec<BatchTxItem>, blockhash: Hash, fee: PriorityFee, tip: u64) -> anyhow::Result<Vec<Signature>> {
        let mut tasks = vec![];
        let mut signatures = vec![];
        for swqos in self.swqos.iter() {
            let tip_account = swqos
                .get_tip_account()
                .ok_or(anyhow::anyhow!("No tip account provided for SWQoS: {}", swqos.get_name()))?;
            let mut tip = Some(TipFee {
                tip_account,
                tip_lamports: tip,
            });

            let txs = items
                .iter()
                .map(|item| build_transaction(&item.payer, item.instructions.clone(), blockhash, Some(fee), tip.take(), None))
                .collect::<Result<Vec<_>, _>>()?;

            signatures.extend(txs.iter().map(|tx| match tx {
                Transaction::Legacy(ref tx) => tx.signatures[0],
                Transaction::Versioned(ref tx) => tx.signatures[0],
            }));
            tasks.push(swqos.send_transactions(txs));
        }

        let result = futures::future::join_all(tasks).await;
        let errors = result.into_iter().filter_map(|res| res.err()).collect::<Vec<_>>();
        if errors.len() > 0 {
            return Err(anyhow::anyhow!("{:?}", errors));
        }

        Ok(signatures)
    }
}
