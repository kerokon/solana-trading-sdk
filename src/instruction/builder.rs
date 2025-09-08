use crate::{
    common::{accounts::PUBKEY_WSOL, transaction::Transaction},
    dex::types::CreateATA,
};
use serde::{Deserialize, Serialize};
use solana_program::program_pack::Pack;
use solana_program::system_instruction;
use solana_sdk::{
    hash::Hash,
    instruction::Instruction,
    message::{v0, Message, VersionedMessage},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::{Transaction as LegacyTransaction, VersionedTransaction},
};
use spl_associated_token_account::{
    get_associated_token_address,
    instruction::{create_associated_token_account, create_associated_token_account_idempotent},
};
use spl_token::instruction::{close_account, initialize_account3, sync_native};
use std::ops::Add;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct PriorityFee {
    pub unit_limit: u32,
    pub unit_price: u64,
}

impl Add for PriorityFee {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            unit_limit: self.unit_limit.saturating_add(rhs.unit_limit),
            unit_price: self.unit_price.max(rhs.unit_price),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TipFee {
    pub tip_account: Pubkey,
    pub tip_lamports: u64,
}

pub fn build_transaction(
    payer: &Keypair,
    instructions: Vec<Instruction>,
    blockhash: Hash,
    other_signers: Option<Vec<&Keypair>>,
) -> anyhow::Result<Transaction> {
    let v0_message: v0::Message = v0::Message::try_compile(&payer.pubkey(), &instructions, &[], blockhash)?;
    let versioned_message: VersionedMessage = VersionedMessage::V0(v0_message);
    let signers = vec![payer].into_iter().chain(other_signers.unwrap_or_default().into_iter()).collect::<Vec<_>>();
    let transaction = VersionedTransaction::try_new(versioned_message, &signers)?;

    Ok(Transaction::Versioned(transaction))
}

pub fn build_legacy_transaction(
    payer: &Keypair,
    instructions: Vec<Instruction>,
    blockhash: Hash,
    other_signers: Option<Vec<&Keypair>>,
) -> anyhow::Result<Transaction> {
    let mut signers = vec![payer];
    if let Some(other_signers) = other_signers {
        signers.extend(other_signers);
    }

    let message = Message::new(&instructions, Some(&payer.pubkey()));
    let mut transaction = LegacyTransaction::new_unsigned(message);
    transaction.sign(&signers, blockhash);

    Ok(Transaction::Legacy(transaction))
}

pub fn build_token_account_instructions(payer: &Keypair, mint: &Pubkey, crate_ata: CreateATA) -> anyhow::Result<(Pubkey, Vec<Instruction>)> {
    let mut instructions = vec![];

    let (token_program, instructions) = match crate_ata {
        CreateATA::Create => {
            instructions.push(create_associated_token_account(&payer.pubkey(), &payer.pubkey(), &mint, &spl_token::ID));
            (get_associated_token_address(&payer.pubkey(), mint), instructions)
        }
        CreateATA::Idempotent => {
            instructions.push(create_associated_token_account_idempotent(
                &payer.pubkey(),
                &payer.pubkey(),
                &mint,
                &spl_token::ID,
            ));
            (get_associated_token_address(&payer.pubkey(), mint), instructions)
        }
        CreateATA::None => (get_associated_token_address(&payer.pubkey(), mint), vec![]),
        CreateATA::CreateWithSeed(seed) => {
            let (token_program, ixs) = build_seeded_token_address(&payer.pubkey(), &mint, &seed)?;
            instructions.extend_from_slice(&ixs);
            (token_program, ixs)
        }
    };

    Ok((token_program, instructions))
}

pub fn build_sol_sell_instructions(
    payer: &Keypair,
    mint: &Pubkey,
    sell_instruction: Instruction,
    close_mint_ata: bool,
) -> Result<Vec<Instruction>, anyhow::Error> {
    let mut instructions = vec![sell_instruction];

    if close_mint_ata {
        let mint_ata = get_associated_token_address(&payer.pubkey(), &mint);
        instructions.push(close_account(&spl_token::ID, &mint_ata, &payer.pubkey(), &payer.pubkey(), &[&payer.pubkey()])?);
    }

    Ok(instructions)
}

pub fn build_wsol_buy_instructions(
    payer: &Keypair,
    mint: &Pubkey,
    amount_sol: u64,
    buy_instruction: Instruction,
    crate_ata: CreateATA,
) -> anyhow::Result<Vec<Instruction>> {
    let mut instructions = vec![];

    match crate_ata {
        CreateATA::Create => {
            instructions.push(create_associated_token_account(&payer.pubkey(), &payer.pubkey(), &mint, &spl_token::ID));
        }
        CreateATA::Idempotent => {
            instructions.push(create_associated_token_account_idempotent(
                &payer.pubkey(),
                &payer.pubkey(),
                &mint,
                &spl_token::ID,
            ));
        }
        CreateATA::None => {}
        CreateATA::CreateWithSeed(seed) => {
            let (_, ixs) = build_seeded_token_address(&payer.pubkey(), &mint, &seed)?;
            instructions.extend_from_slice(&ixs);
        }
    }

    instructions.push(create_associated_token_account_idempotent(
        &payer.pubkey(),
        &payer.pubkey(),
        &PUBKEY_WSOL,
        &spl_token::ID,
    ));

    let wsol_ata = get_associated_token_address(&payer.pubkey(), &PUBKEY_WSOL);
    instructions.push(solana_sdk::system_instruction::transfer(&payer.pubkey(), &wsol_ata, amount_sol));

    instructions.push(sync_native(&spl_token::ID, &wsol_ata).unwrap());

    instructions.push(buy_instruction);

    instructions.push(close_account(&spl_token::ID, &wsol_ata, &payer.pubkey(), &payer.pubkey(), &[&payer.pubkey()]).unwrap());

    Ok(instructions)
}

pub fn build_wsol_sell_instructions(payer: &Keypair, mint: &Pubkey, sell_instruction: Instruction, close_mint_ata: bool) -> anyhow::Result<Vec<Instruction>> {
    let mint_ata = get_associated_token_address(&payer.pubkey(), &mint);
    let wsol_ata = get_associated_token_address(&payer.pubkey(), &PUBKEY_WSOL);

    let mut instructions = vec![];
    instructions.push(create_associated_token_account_idempotent(
        &payer.pubkey(),
        &payer.pubkey(),
        &PUBKEY_WSOL,
        &spl_token::ID,
    ));

    instructions.push(sell_instruction);

    instructions.push(close_account(&spl_token::ID, &wsol_ata, &payer.pubkey(), &payer.pubkey(), &[&payer.pubkey()]).unwrap());

    if close_mint_ata {
        instructions.push(close_account(&spl_token::ID, &mint_ata, &payer.pubkey(), &payer.pubkey(), &[&payer.pubkey()]).unwrap());
    }

    Ok(instructions)
}

fn build_seeded_token_address(payer: &Pubkey, mint: &Pubkey, seed: &str) -> anyhow::Result<(Pubkey, Vec<Instruction>)> {
    let base = payer;
    let token_program_id = spl_token::id();

    // 1. Derive the token account address (on-curve)
    let token_account = Pubkey::create_with_seed(&base, seed, &token_program_id)?;

    // 2. Calculate space & rent (works on-chain; for off-chain, hardcode)
    let account_size = spl_token::state::Account::LEN;
    let lamports = 2_139_280;

    // 3. Create account with seed
    let ix_create = system_instruction::create_account_with_seed(
        payer,          // from (funder)
        &token_account, // to (new account)
        base,           // base
        seed,
        lamports,
        account_size as u64,
        &token_program_id,
    );

    // 4. Init SPL Token account
    let ix_init = initialize_account3(
        &token_program_id,
        &token_account,
        mint,
        payer, // owner of token account
    )?;

    Ok((token_account, vec![ix_create, ix_init]))
}
