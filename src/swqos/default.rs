use super::{
    swqos_rpc::{SWQoSClientTrait, SWQoSRequest},
    SWQoSTrait,
};
use crate::{common::transaction::Transaction, instruction::builder::{build_transaction, PriorityFee}};
use rand::seq::IndexedRandom;
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcTransactionConfig};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::VersionedTransaction,
};
use solana_transaction_status::UiTransactionEncoding;
use spl_associated_token_account::{get_associated_token_address, instruction::create_associated_token_account_idempotent};
use std::sync::Arc;

#[derive(Clone)]
pub struct DefaultSWQoSClient {
    pub name: String,
    pub rpc_client: Arc<RpcClient>,
    pub tip_accounts: Vec<Pubkey>,
    pub swqos_endpoint: String,
    pub swqos_header: Option<(String, String)>,
    pub swqos_client: Arc<reqwest::Client>,
}

#[async_trait::async_trait]
impl SWQoSTrait for DefaultSWQoSClient {
    async fn send_transaction(&self, transaction: Transaction) -> anyhow::Result<()> {

        self.swqos_client
            .swqos_send_transaction(SWQoSRequest {
                name: self.name.clone(),
                url: self.swqos_endpoint.clone(),
                auth_header: self.swqos_header.clone(),
                transactions: vec![transaction],
            })
            .await
    }

    async fn send_transactions(&self, transactions: Vec<Transaction>) -> anyhow::Result<()> {

        self.swqos_client
            .swqos_send_transactions(SWQoSRequest {
                name: self.name.clone(),
                url: self.swqos_endpoint.clone(),
                auth_header: self.swqos_header.clone(),
                transactions,
            })
            .await
    }

    fn get_tip_account(&self) -> Option<Pubkey> {
        Some(*self.tip_accounts.choose(&mut rand::rng())?)
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}

pub struct TransferInfo {
    pub to: Pubkey,
    pub amount: u64,
}

impl DefaultSWQoSClient {
    pub fn new(name: &str, rpc_client: Arc<RpcClient>, endpoint: String, header: Option<(String, String)>, tip_accounts: Vec<Pubkey>) -> Self {
        let swqos_client = reqwest::Client::new_swqos_client();

        Self {
            name: name.to_string(),
            rpc_client,
            tip_accounts,
            swqos_endpoint: endpoint,
            swqos_header: header,
            swqos_client: Arc::new(swqos_client),
        }
    }

    pub async fn transfer(&self, from: &Keypair, to: &Pubkey, amount: u64, fee: Option<PriorityFee>) -> anyhow::Result<Signature> {
        let blockhash = self.rpc_client.get_latest_blockhash().await?;
        let instruction = solana_sdk::system_instruction::transfer(&from.pubkey(), to, amount);
        let transaction = build_transaction(from, vec![instruction], blockhash, fee, None, None)?;
        let signature = match transaction {
            Transaction::Legacy(ref tx) => tx.signatures[0],
            Transaction::Versioned(ref tx) => tx.signatures[0],
        };
        self.send_transaction(transaction).await?;
        Ok(signature)
    }

    pub async fn batch_transfer(&self, from: &Keypair, to: Vec<TransferInfo>, fee: Option<PriorityFee>) -> anyhow::Result<Signature> {
        let blockhash = self.rpc_client.get_latest_blockhash().await?;
        let instructions = to
            .iter()
            .map(|transfer| solana_sdk::system_instruction::transfer(&from.pubkey(), &transfer.to, transfer.amount))
            .collect::<Vec<_>>();
        let transaction = build_transaction(from, instructions, blockhash, fee, None, None)?;
        let signature = match transaction {
            Transaction::Legacy(ref tx) => tx.signatures[0],
            Transaction::Versioned(ref tx) => tx.signatures[0],
        };
        self.send_transaction(transaction).await?;
        Ok(signature)
    }

    pub async fn spl_transfer(&self, from: &Keypair, to: &Pubkey, mint: &Pubkey, amount: u64, fee: Option<PriorityFee>) -> anyhow::Result<Signature> {
        let blockhash = self.rpc_client.get_latest_blockhash().await?;
        let from_ata = get_associated_token_address(&from.pubkey(), mint);
        let to_ata = get_associated_token_address(to, mint);
        let create_ata = create_associated_token_account_idempotent(&from.pubkey(), to, &mint, &spl_token::ID);
        let instruction = spl_token::instruction::transfer(&spl_token::ID, &from_ata, &to_ata, &from.pubkey(), &[], amount)?;
        let transaction = build_transaction(from, vec![create_ata, instruction], blockhash, fee, None, None)?;
        let signature = match transaction {
            Transaction::Legacy(ref tx) => tx.signatures[0],
            Transaction::Versioned(ref tx) => tx.signatures[0],
        };
        self.send_transaction(transaction).await?;
        Ok(signature)
    }

    pub async fn spl_batch_transfer(&self, from: &Keypair, to: Vec<TransferInfo>, mint: &Pubkey, fee: Option<PriorityFee>) -> anyhow::Result<Signature> {
        let blockhash = self.rpc_client.get_latest_blockhash().await?;
        let from_ata = get_associated_token_address(&from.pubkey(), mint);
        let mut instructions = Vec::new();

        for transfer in &to {
            let to_ata = get_associated_token_address(&transfer.to, mint);
            let create_ata = create_associated_token_account_idempotent(&from.pubkey(), &transfer.to, &mint, &spl_token::ID);
            let instruction = spl_token::instruction::transfer(&spl_token::ID, &from_ata, &to_ata, &from.pubkey(), &[], transfer.amount)?;
            instructions.push(create_ata);
            instructions.push(instruction);
        }

        let transaction = build_transaction(from, instructions, blockhash, fee, None, None)?;
        let signature = match transaction {
            Transaction::Legacy(ref tx) => tx.signatures[0],
            Transaction::Versioned(ref tx) => tx.signatures[0],
        };
        self.send_transaction(transaction).await?;
        Ok(signature)
    }

    pub async fn wait_for_confirm(&self, signature: &Signature) -> anyhow::Result<()> {
        const MAX_WAIT_SECONDS: u64 = 10;
        let ts = std::time::SystemTime::now();
        loop {
            if let Ok(tx) = self
                .rpc_client
                .get_transaction_with_config(
                    &signature,
                    RpcTransactionConfig {
                        encoding: Some(UiTransactionEncoding::Json),
                        commitment: Some(CommitmentConfig::confirmed()),
                        max_supported_transaction_version: Some(0),
                    },
                )
                .await
            {
                if tx.slot > 0 {
                    break;
                }
            }
            if ts.elapsed().unwrap().as_secs() > MAX_WAIT_SECONDS {
                return Err(anyhow::anyhow!("Transaction confirmation timedout: {:?}", signature));
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        Ok(())
    }
}
