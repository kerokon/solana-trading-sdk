use crate::common::lamports::Lamports;
use crate::instruction::builder::build_legacy_transaction;
use crate::swqos::SWQoSRuntime;
use crate::{
    common::transaction::Transaction,
    errors::trading_endpoint_error::TradingEndpointError,
    instruction::builder::{build_transaction, PriorityFee, TipFee},
    swqos::SWQoSTrait,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::signature::Signer;
use solana_sdk::{
    hash::Hash,
    instruction::Instruction,
    signature::{Keypair, Signature},
};
use std::ops::Add;
use std::sync::Arc;
use tracing::debug;

type Tip = u64;

#[derive(Debug, Clone, Copy)]
pub enum TransactionType {
    Buy,
    Sell,
    Create,
}

pub struct TradingEndpoint {
    pub rpc: Arc<RpcClient>,
    pub swqos: Vec<Arc<SWQoSRuntime>>,
}

pub struct BatchTxItem {
    pub payer: Keypair,
    pub instructions: Vec<Instruction>,
}

impl TradingEndpoint {
    pub fn new(rpc: Arc<RpcClient>, swqos: Vec<Arc<SWQoSRuntime>>) -> Self {
        Self { rpc, swqos }
    }

    pub async fn get_latest_blockhash(&self) -> Result<Hash, TradingEndpointError> {
        let blockhash = self.rpc.get_latest_blockhash().await?;
        Ok(blockhash)
    }

    /// Get the appropriate tip configuration based on transaction type
    fn get_tip_config(&self, swqos: &SWQoSRuntime, tx_type: TransactionType, additional_tip: u64) -> Result<Option<TipFee>, TradingEndpointError> {
        let tip_account = match swqos.get_tip_account() {
            Some(account) => account,
            None => return Ok(None),
        };

        let tip_lamports = self.get_default_tip_for_tx_type(swqos, tx_type)?.0.add(additional_tip);

        Ok(Some(TipFee { tip_account, tip_lamports }))
    }

    /// Get the default tip amount based on transaction type
    fn get_default_tip_for_tx_type(&self, swqos: &SWQoSRuntime, tx_type: TransactionType) -> Result<Lamports, TradingEndpointError> {
        let tip = match tx_type {
            TransactionType::Buy => swqos.config.buy_tip,
            TransactionType::Sell => swqos.config.sell_tip.or(swqos.config.buy_tip), // Fallback to buy_tip if sell_tip not configured
            TransactionType::Create => swqos.config.buy_tip,                         // Fallback to buy_tip if create_tip not configured
        };

        tip.ok_or_else(|| {
            TradingEndpointError::TransactionError(format!(
                "No tip configured for transaction type {:?} in SWQoS: {}",
                tx_type,
                swqos.client.get_name()
            ))
        })
    }

    /// Get the appropriate fee configuration based on transaction type
    fn get_fee_config(&self, swqos: &SWQoSRuntime, tx_type: TransactionType, additional_fee: Option<PriorityFee>) -> Option<PriorityFee> {
        let base_fee = match tx_type {
            TransactionType::Buy => swqos.config.buy_fee,
            TransactionType::Sell => swqos.config.sell_fee.or(swqos.config.buy_fee),
            TransactionType::Create => swqos.config.buy_fee,
        };

        match (base_fee, additional_fee) {
            (Some(base), Some(additional)) => Some(base + additional),
            (Some(base), None) => Some(base),
            (None, Some(additional)) => Some(additional),
            (None, None) => None,
        }
    }

    /// Build fee instructions for the transaction
    fn build_fee_instructions(&self, swqos: &SWQoSRuntime, tx_type: TransactionType, custom_fee: Option<PriorityFee>) -> Vec<Instruction> {
        if let Some(fee) = self.get_fee_config(swqos, tx_type, custom_fee) {
            vec![
                ComputeBudgetInstruction::set_compute_unit_price(fee.unit_price),
                ComputeBudgetInstruction::set_compute_unit_limit(fee.unit_limit),
            ]
        } else {
            vec![]
        }
    }

    /// Build tip instruction for the transaction
    fn build_tip_instruction(&self, payer: &Keypair, tip_config: Option<TipFee>) -> Option<Instruction> {
        tip_config.map(|tip| solana_sdk::system_instruction::transfer(&payer.pubkey(), &tip.tip_account, tip.tip_lamports))
    }

    pub async fn build_and_broadcast_tx(
        &self,
        tx_type: TransactionType,
        payer: &Keypair,
        instructions: Vec<Instruction>,
        nonce_ix: Option<Instruction>,
        blockhashes: Vec<Hash>,
        additional_fee: Option<PriorityFee>,
        additional_tip: u64,
        other_signers: Option<Vec<&Keypair>>,
    ) -> Result<Vec<Signature>, TradingEndpointError> {
        let mut signatures = vec![];
        let mut txs_to_send = Vec::new();

        for (index, swqos) in self.swqos.iter().enumerate() {
            let mut transaction_instructions = vec![];

            // Add nonce instruction if provided
            if let Some(ix) = nonce_ix.as_ref() {
                transaction_instructions.push(ix.clone());
            }

            // Add fee instructions
            let fee_instructions = self.build_fee_instructions(swqos, tx_type, additional_fee);
            transaction_instructions.extend(fee_instructions);

            // Add tip instruction if configured
            let tip_config = self.get_tip_config(swqos, tx_type, additional_tip)?;
            if let Some(tip_instruction) = self.build_tip_instruction(payer, tip_config) {
                transaction_instructions.push(tip_instruction);
            }

            // Add main instructions
            transaction_instructions.extend(instructions.clone());

            // Get blockhash for this transaction, cycling through available hashes
            let blockhash = blockhashes[index % blockhashes.len()];

            let tx = build_legacy_transaction(
                payer,
                transaction_instructions,
                blockhash,
                other_signers.as_ref().map(|v| v.to_vec())
            )
                .map_err(|e| TradingEndpointError::TransactionError(e.to_string()))?;

            let signature = match tx {
                Transaction::Legacy(ref tx) => tx.signatures[0],
                Transaction::Versioned(ref tx) => tx.signatures[0],
            };

            signatures.push(signature);
            txs_to_send.push((swqos, tx));
        }

        // Send all transactions concurrently
        let tasks: Vec<_> = txs_to_send
            .into_iter()
            .map(|(swqos, tx)| async move { swqos.send_transaction(tx).await })
            .collect();

        let results = futures::future::join_all(tasks).await;
        debug!("Transaction results: {:?}", results);

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
        tx_type: TransactionType,
        items: Vec<BatchTxItem>,
        blockhash: Hash,
        custom_fee: Option<PriorityFee>,
        custom_tip: u64,
    ) -> Result<Vec<Signature>, TradingEndpointError> {
        let mut tasks = vec![];
        let mut signatures = vec![];

        for swqos in self.swqos.iter() {
            let tip_config = self.get_tip_config(swqos, tx_type, custom_tip)?;
            let fee_instructions = self.build_fee_instructions(swqos, tx_type, custom_fee);

            let txs = items
                .iter()
                .map(|item| {
                    let mut transaction_instructions = vec![];

                    // Add fee instructions
                    transaction_instructions.extend(fee_instructions.clone());

                    // Add tip instruction if configured
                    if let Some(tip_instruction) = self.build_tip_instruction(&item.payer, tip_config) {
                        transaction_instructions.push(tip_instruction);
                    }

                    // Add main instructions
                    transaction_instructions.extend(item.instructions.clone());

                    build_transaction(&item.payer, transaction_instructions, blockhash, None)
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| TradingEndpointError::TransactionError(e.to_string()))?;

            signatures.extend(txs.iter().map(|tx| match tx {
                Transaction::Legacy(ref tx) => tx.signatures[0],
                Transaction::Versioned(ref tx) => tx.signatures[0],
            }));

            tasks.push(swqos.send_transactions(txs));
        }

        let results = futures::future::join_all(tasks).await;
        let errors: Vec<_> = results.into_iter().filter_map(Result::err).collect();

        if !errors.is_empty() {
            return Err(TradingEndpointError::CustomError(format!(
                "Errors occurred while sending batch transactions: {:?}",
                errors
            )));
        }

        Ok(signatures)
    }
}
