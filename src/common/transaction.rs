use base64::Engine;
use base64::engine::general_purpose;
use solana_sdk::transaction::{Transaction as LegacyTransaction, VersionedTransaction};


#[derive(Debug,Clone)]
pub enum Transaction {
    Legacy(LegacyTransaction),
    Versioned(VersionedTransaction),
}
impl Transaction{
    pub fn to_base64_string(&self) -> String {
        match self{
            Transaction::Legacy(t) => {
                let tx_bytes = bincode::serialize(t).unwrap();
                general_purpose::STANDARD.encode(tx_bytes)
            }
            Transaction::Versioned(t) => {
                let tx_bytes = bincode::serialize(t).unwrap();
                general_purpose::STANDARD.encode(tx_bytes)
            }
        }

    }
}