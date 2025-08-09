use solana_sdk::transaction::{Transaction as LegacyTransaction, VersionedTransaction};


#[derive(Debug,Clone)]
pub enum Transaction {
    Legacy(LegacyTransaction),
    Versioned(VersionedTransaction),
}
