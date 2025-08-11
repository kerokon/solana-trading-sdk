use super::{
    amm_calc::{amm_buy_get_token_out, amm_sell_get_sol_out, calculate_with_slippage_buy, calculate_with_slippage_sell},
    types::{BatchBuyParam, BatchSellParam, Create, CreateATA, PoolInfo, SwapInfo, TokenAmountType},
};
use crate::{
    common::trading_endpoint::{BatchTxItem, TradingEndpoint},
    instruction::builder::{build_sol_buy_instructions, build_sol_sell_instructions, build_wsol_buy_instructions, build_wsol_sell_instructions, PriorityFee},
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
    async fn initialize(&self) -> anyhow::Result<()>;
    fn initialized(&self) -> anyhow::Result<()>;
    fn use_wsol(&self) -> bool;
    fn get_trading_endpoint(&self) -> Arc<TradingEndpoint>;
    async fn get_pool(&self, mint: &Pubkey) -> anyhow::Result<PoolInfo>;
    async fn create(&self, payer: Keypair, create: Create, fee: Option<PriorityFee>, tip: Option<u64>) -> anyhow::Result<Vec<Signature>>;
    fn build_buy_instruction(&self, payer: &Keypair, mint: &Pubkey, creator_vault: Option<&Pubkey>, buy: SwapInfo) -> anyhow::Result<Instruction>;
    fn build_sell_instruction(&self, payer: &Keypair, mint: &Pubkey, creator_vault: Option<&Pubkey>, sell: SwapInfo) -> anyhow::Result<Instruction>;
    async fn buy(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        sol_amount: u64,
        slippage_basis_points: u64,
        fee: Option<PriorityFee>,
        tip: Option<u64>,
    ) -> anyhow::Result<Vec<Signature>> {
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
            blockhash,
            CreateATA::Idempotent,
            fee,
            tip,
        ).await
    }
    async fn buy_immediately(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        extra_address: Option<&Pubkey>,
        sol_amount: u64,
        token_amount: u64,
        blockhash: Hash,
        create_ata: CreateATA,
        fee: Option<PriorityFee>,
        tip: Option<u64>,
    ) -> anyhow::Result<Vec<Signature>> {
        let instruction = self.build_buy_instruction(payer, mint, extra_address, SwapInfo { token_amount, sol_amount })?;
        let instructions = if self.use_wsol() {
            build_wsol_buy_instructions(payer, mint, sol_amount, instruction, create_ata)?
        } else {
            build_sol_buy_instructions(payer, mint, instruction, create_ata)?
        };
        let signatures = self
            .get_trading_endpoint()
            .build_and_broadcast_tx(payer, instructions, blockhash, fee, tip, None).await?;

        Ok(signatures)
    }
    async fn sell(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        token_amount: TokenAmountType,
        slippage_basis_points: u64,
        close_mint_ata: bool,
        fee: Option<PriorityFee>,
        tip: Option<u64>,
    ) -> anyhow::Result<Vec<Signature>> {
        let trading_endpoint = self.get_trading_endpoint();
        let payer_pubkey = payer.pubkey();
        let (pool_info, blockhash, token_amount) = tokio::try_join!(
            self.get_pool(&mint),
            trading_endpoint.get_latest_blockhash(),
            token_amount.to_amount(trading_endpoint.rpc.clone(), &payer_pubkey, mint)
        )?;
        let sol_lamports = amm_sell_get_sol_out(pool_info.sol_reserves, pool_info.token_reserves, token_amount);
        let sol_lamports_with_slippage = calculate_with_slippage_sell(sol_lamports, slippage_basis_points);

        self.sell_immediately(
            payer,
            mint,
            pool_info.creator_vault.as_ref(),
            token_amount,
            sol_lamports_with_slippage,
            close_mint_ata,
            blockhash,
            fee,
            tip,
        ).await
    }
    async fn sell_immediately(
        &self,
        payer: &Keypair,
        mint: &Pubkey,
        extra_address: Option<&Pubkey>,
        token_amount: u64,
        sol_amount: u64,
        close_mint_ata: bool,
        blockhash: Hash,
        fee: Option<PriorityFee>,
        tip: Option<u64>,
    ) -> anyhow::Result<Vec<Signature>> {
        let instruction = self.build_sell_instruction(payer, mint, extra_address, SwapInfo { token_amount, sol_amount })?;
        let instructions = if self.use_wsol() {
            build_wsol_sell_instructions(payer, mint, instruction, close_mint_ata)?
        } else {
            build_sol_sell_instructions(payer, mint, instruction, close_mint_ata)?
        };
        let signatures = self
            .get_trading_endpoint()
            .build_and_broadcast_tx(payer, instructions, blockhash, fee, tip, None).await?;

        Ok(signatures)
    }
    async fn batch_buy(
        &self,
        mint: &Pubkey,
        slippage_basis_points: u64,
        fee: PriorityFee,
        tip: u64,
        items: Vec<BatchBuyParam>,
    ) -> anyhow::Result<Vec<Signature>> {
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
                SwapInfo {
                    token_amount: buy_token_amount,
                    sol_amount: sol_lamports_with_slippage,
                },
            )?;
            let instructions = if self.use_wsol() {
                build_wsol_buy_instructions(&item.payer, mint, sol_lamports_with_slippage, instruction, CreateATA::Idempotent)?
            } else {
                build_sol_buy_instructions(&item.payer, mint, instruction, CreateATA::Idempotent)?
            };
            batch_items.push(BatchTxItem {
                payer: item.payer,
                instructions,
            });
            pool_sol_amount += item.sol_amount;
            pool_token_amount -= buy_token_amount;
        }

        let signatures = trading_endpoint.build_and_broadcast_batch_txs(batch_items, blockhash, fee, tip).await?;

        Ok(signatures)
    }
    async fn batch_sell(
        &self,
        mint: &Pubkey,
        slippage_basis_points: u64,
        fee: PriorityFee,
        tip: u64,
        items: Vec<BatchSellParam>,
    ) -> anyhow::Result<Vec<Signature>> {
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
                pool_info.creator_vault.as_ref(),
                SwapInfo {
                    token_amount: sol_amount,
                    sol_amount: sol_lamports_with_slippage,
                },
            )?;
            let instructions = if self.use_wsol() {
                build_wsol_sell_instructions(&item.payer, mint, instruction, item.close_mint_ata)?
            } else {
                build_sol_sell_instructions(&item.payer, mint, instruction, item.close_mint_ata)?
            };
            batch_items.push(BatchTxItem {
                payer: item.payer,
                instructions,
            });
            pool_sol_amount -= sol_amount;
            pool_token_amount += item.token_amount;
        }

        let signatures = trading_endpoint.build_and_broadcast_batch_txs(batch_items, blockhash, fee, tip).await?;

        Ok(signatures)
    }
}
