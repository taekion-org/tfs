use libtfslite::protos::transaction::Transaction;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum TransactionStatus {
    Local = 0,
    Queued = 1,
    Pending = 2,
    Committed = 3,
    Unknown = 4,
    InvalidStatus = 5,
}

impl From<TransactionStatus> for String {
    fn from(value: TransactionStatus) -> Self {
        match value {
            TransactionStatus::Local => String::from("LOCAL"),
            TransactionStatus::Queued => String::from("QUEUED"),
            TransactionStatus::Pending => String::from("PENDING"),
            TransactionStatus::Committed => String::from("COMMITTED"),
            TransactionStatus::Unknown => String::from("UNKNOWN"),
            TransactionStatus::InvalidStatus => String::from("INVALID_STATUS"),
        }
    }
}

impl From<String> for TransactionStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "LOCAL" => TransactionStatus::Local,
            "QUEUED" => TransactionStatus::Queued,
            "PENDING" => TransactionStatus::Pending,
            "COMMITTED" => TransactionStatus::Committed,
            "UNKNOWN" => TransactionStatus::Unknown,
            "INVALID_STATUS" => TransactionStatus::InvalidStatus,
            &_ => TransactionStatus::InvalidStatus,
        }

    }
}

pub type TransactionId = String;
pub type TransactionSubmitId = String;

#[derive(Debug)]
pub struct TransactionInfo {
    pub order: u64,
    pub tx_id: TransactionId,
    pub submit_id: Option<TransactionSubmitId>,
    pub status: TransactionStatus,
}

#[derive(Debug)]
pub enum LocalStateStoreError {
    NoSuchFile,
    NoSuchTransaction,
    ImplementationError(String),
}

#[async_trait(?Send)]
pub trait LocalStateStore {
    async fn get_files(&self) -> Result<Vec<uuid::Uuid>, LocalStateStoreError>;
    async fn get_txs(&self, file_id: &uuid::Uuid) -> Result<Vec<TransactionInfo>, LocalStateStoreError>;
    async fn get_tx_bytes(&self, tx_id: &TransactionId) -> Result<Vec<u8>, LocalStateStoreError>;
    async fn update_tx(&self, tx_id: &TransactionId, submit_id: Option<TransactionSubmitId>, status: Option<TransactionStatus>) -> Result<(), LocalStateStoreError>;
    async fn flush_txs(&self, file_id: &uuid::Uuid) -> Result<(), LocalStateStoreError>;
    async fn add_tx(&self, file_id: &uuid::Uuid, transaction: &Transaction) -> Result<(), LocalStateStoreError>;
}
