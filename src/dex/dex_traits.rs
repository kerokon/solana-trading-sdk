use super::{
    amm_calc::{amm_buy_get_token_out, amm_sell_get_sol_out, calculate_with_slippage_buy, calculate_with_slippage_sell},
    types::{BatchBuyParam, BatchSellParam, Create, CreateATA, PoolInfo, SwapInfo, TokenAmountType},
};
use crate::common::trading_endpoint::TransactionType;
use crate::{
    common::trading_endpoint::{BatchTxItem, TradingEndpoint},
    errors::trading_endpoint_error::TradingEndpointError,
    instruction::builder::{build_sol_sell_instructions, build_token_account_instructions, build_wsol_sell_instructions, PriorityFee},
};
use solana_sdk::{
    hash::Hash,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
};
use std::{any::Any, sync::Arc};

#[async_trait::async_trait]
pub trait DexTrait: Send + Sync + Any {
    async fn initialize(&self) -> Result<(), TradingEndpointError>;
    fn initialized(&self) -> Result<(), TradingEndpointError>;
    fn get_swqos_quantity(&self) ->usize{
        self.get_trading_endpoint().swqos.len()
    }
    fn use_wsol(&self) -> bool;
    fn get_trading_endpoint(&self) -> Arc<TradingEndpoint>;
    async fn get_pool(&self, mint: &Pubkey) -> Result<PoolInfo, TradingEndpointError>;
    async fn create(&self, payer: Keypair, create: Create, fee: Option<PriorityFee>, tip: Option<u64>) -> Result<Vec<Signature>, TradingEndpointError>;
    fn build_buy_instruction(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        creator_vault: Option<&Pubkey>,
        token_program_account: &Pubkey,
        buy: SwapInfo,
    ) -> Result<Instruction, TradingEndpointError>;
    fn build_sell_instruction(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        custom_ata: Option<&Pubkey>,
        creator_vault: Option<&Pubkey>,
        sell: SwapInfo,
    ) -> Result<Instruction, TradingEndpointError>;
    async fn buy(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        sol_amount: u64,
        slippage_basis_points: u64,
        fee: Option<PriorityFee>,
        tip: Option<u64>,
    ) -> Result<Vec<Signature>, TradingEndpointError> {
        let trading_endpoint = self.get_trading_endpoint();
        let (pool_info, blockhash) = tokio::try_join!(self.get_pool(mint), trading_endpoint.get_latest_blockhash(),)?;
        let buy_token_amount = amm_buy_get_token_out(pool_info.sol_reserves, pool_info.token_reserves, sol_amount);
        let sol_lamports_with_slippage = calculate_with_slippage_buy(sol_amount, slippage_basis_points);

        self.buy_immediately(
            payer,
            mint,
            pool_info.creator_vault.as_ref(),
            sol_lamports_with_slippage,
            buy_token_amount,
            vec![blockhash],
            None,
            CreateATA::Create,
            fee,
            tip.unwrap_or_default(),
        )
        .await
    }
    async fn buy_immediately(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        extra_address: Option<&Pubkey>,
        sol_amount: u64,
        token_amount: u64,
        blockhashes: Vec<Hash>,
        nonce_ix: Option<Instruction>,
        create_ata: CreateATA,
        additional_fee: Option<PriorityFee>,
        additional_tip: u64,
    ) -> Result<Vec<Signature>, TradingEndpointError> {
        let (token_account, mut instructions) =
            build_token_account_instructions(payer, mint, create_ata).map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;

        let instruction = self.build_buy_instruction(payer, mint, extra_address, &token_account, SwapInfo { token_amount, sol_amount })?;

        instructions.push(instruction);
        let signatures = self
            .get_trading_endpoint()
            .build_and_broadcast_tx(
                TransactionType::Buy,
                payer,
                instructions,
                nonce_ix,
                blockhashes,
                additional_fee,
                additional_tip,
                None,
            )
            .await?;

        Ok(signatures)
    }
    async fn sell(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        token_amount: TokenAmountType,
        slippage_basis_points: u64,
        custom_ata: Option<&Pubkey>,
        close_mint_ata: bool,
        additional_fee: Option<PriorityFee>,
        additional_tip: u64,
    ) -> Result<Vec<Signature>, TradingEndpointError> {
        let trading_endpoint = self.get_trading_endpoint();
        let payer_pubkey = payer.pubkey();
        let get_amount = async || {
            token_amount
                .to_amount(trading_endpoint.rpc.clone(), &payer_pubkey, mint)
                .await
                .map_err(|w| TradingEndpointError::SolanaClientError(w))
        };
        let (pool_info, blockhash, token_amount) = tokio::try_join!(self.get_pool(&mint), trading_endpoint.get_latest_blockhash(), get_amount())?;
        let sol_lamports = amm_sell_get_sol_out(pool_info.sol_reserves, pool_info.token_reserves, token_amount);
        let sol_lamports_with_slippage = calculate_with_slippage_sell(sol_lamports, slippage_basis_points);

        self.sell_immediately(
            payer,
            mint,
            custom_ata,
            pool_info.creator_vault.as_ref(),
            token_amount,
            sol_lamports_with_slippage,
            close_mint_ata,
            vec![blockhash],
            None,
            additional_fee,
            additional_tip,
        )
        .await
    }
    async fn sell_immediately(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        custom_ata: Option<&Pubkey>,
        extra_address: Option<&Pubkey>,
        token_amount: u64,
        sol_amount: u64,
        close_mint_ata: bool,
        blockhashes: Vec<Hash>,
        nonce_ix: Option<Instruction>,
        additional_fee: Option<PriorityFee>,
        additional_tip: u64,
    ) -> Result<Vec<Signature>, TradingEndpointError> {
        let instruction = self.build_sell_instruction(payer, mint, custom_ata, extra_address, SwapInfo { token_amount, sol_amount })?;
        let instructions = if self.use_wsol() {
            build_wsol_sell_instructions(payer, mint, instruction, close_mint_ata).map_err(|e| TradingEndpointError::CustomError(e.to_string()))?
        } else {
            build_sol_sell_instructions(payer, mint, instruction, close_mint_ata).map_err(|e| TradingEndpointError::CustomError(e.to_string()))?
        };
        let signatures = self
            .get_trading_endpoint()
            .build_and_broadcast_tx(
                TransactionType::Sell,
                payer,
                instructions,
                nonce_ix,
                blockhashes,
                additional_fee,
                additional_tip,
                None,
            )
            .await?;

        Ok(signatures)
    }
    async fn batch_buy(
        &self,
        mint: &Pubkey,
        slippage_basis_points: u64,
        fee: PriorityFee,
        tip: u64,
        items: Vec<BatchBuyParam>,
    ) -> Result<Vec<Signature>, TradingEndpointError> {
        let trading_endpoint = self.get_trading_endpoint();
        let (pool_info, blockhash) = tokio::try_join!(self.get_pool(&mint), trading_endpoint.get_latest_blockhash(),)?;
        let mut pool_token_amount = pool_info.token_reserves;
        let mut pool_sol_amount = pool_info.sol_reserves;
        let mut batch_items = vec![];

        for item in items {
            let sol_lamports_with_slippage = calculate_with_slippage_buy(item.sol_amount, slippage_basis_points);
            let buy_token_amount = amm_buy_get_token_out(pool_sol_amount, pool_token_amount, item.sol_amount);
            let instruction = self.build_buy_instruction(
                &item.payer,
                &mint,
                pool_info.creator_vault.as_ref(),
                &spl_token::ID,
                SwapInfo {
                    token_amount: buy_token_amount,
                    sol_amount: sol_lamports_with_slippage,
                },
            )?;

            let (_, mut instructions) =
                build_token_account_instructions(&item.payer, mint, CreateATA::Idempotent).map_err(|e| TradingEndpointError::CustomError(e.to_string()))?;
            instructions.push(instruction);
            batch_items.push(BatchTxItem {
                payer: item.payer,
                instructions,
            });
            pool_sol_amount += item.sol_amount;
            pool_token_amount -= buy_token_amount;
        }

        let signatures = trading_endpoint
            .build_and_broadcast_batch_txs(TransactionType::Buy, batch_items, blockhash, Some(fee), tip)
            .await?;

        Ok(signatures)
    }
    async fn batch_sell(
        &self,
        mint: &Pubkey,
        slippage_basis_points: u64,
        fee: PriorityFee,
        tip: u64,
        items: Vec<BatchSellParam>,
    ) -> Result<Vec<Signature>, TradingEndpointError> {
        let trading_endpoint = self.get_trading_endpoint();
        let (pool_info, blockhash) = tokio::try_join!(self.get_pool(&mint), trading_endpoint.get_latest_blockhash(),)?;
        let mut pool_token_amount = pool_info.token_reserves;
        let mut pool_sol_amount = pool_info.sol_reserves;
        let mut batch_items = vec![];

        for item in items {
            let sol_amount = amm_sell_get_sol_out(pool_sol_amount, pool_token_amount, item.token_amount);
            let sol_lamports_with_slippage = calculate_with_slippage_sell(sol_amount, slippage_basis_points);
            let instruction = self.build_sell_instruction(
                &item.payer,
                &mint,
                item.custom_ata.as_ref(),
                pool_info.creator_vault.as_ref(),
                SwapInfo {
                    token_amount: sol_amount,
                    sol_amount: sol_lamports_with_slippage,
                },
            )?;
            let instructions = if self.use_wsol() {
                build_wsol_sell_instructions(&item.payer, mint, instruction, item.close_mint_ata)
                    .map_err(|e| TradingEndpointError::CustomError(e.to_string()))?
            } else {
                build_sol_sell_instructions(&item.payer, mint, instruction, item.close_mint_ata)
                    .map_err(|e| TradingEndpointError::CustomError(e.to_string()))?
            };
            batch_items.push(BatchTxItem {
                payer: item.payer,
                instructions,
            });
            pool_sol_amount -= sol_amount;
            pool_token_amount += item.token_amount;
        }

        let signatures = trading_endpoint
            .build_and_broadcast_batch_txs(TransactionType::Sell, batch_items, blockhash, Some(fee), tip)
            .await?;

        Ok(signatures)
    }
}
