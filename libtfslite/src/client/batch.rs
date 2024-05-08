use std::fmt::{Display, Formatter, Debug};
use std::error::Error;
use protobuf::{Message, RepeatedField};
use crate::client::keys::{Signer, SigningError};
use crate::protos::transaction::Transaction;
use crate::protos::batch::{Batch, BatchHeader};

#[derive(Debug)]
pub enum BatchBuildError {
    SerializationError(String),
    MissingField(String),
    SigningError(String),
}

impl Error for BatchBuildError {}

impl Display for BatchBuildError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            BatchBuildError::SerializationError(ref s) => write!(f, "SerializationError: {}", s),
            BatchBuildError::MissingField(ref s) => write!(f, "MissingField: {}", s),
            BatchBuildError::SigningError(ref s) => write!(f, "SigningError: {}", s),
        }
    }
}

impl From<SigningError> for BatchBuildError {
    fn from(value: SigningError) -> Self {
        BatchBuildError::SigningError(format!("{}", value))
    }
}

#[derive(Clone, Default)]
pub struct BatchBuilder {
    transactions: Option<Vec<Transaction>>
}

impl BatchBuilder {
    pub fn new() -> Self {
        BatchBuilder::default()
    }

    pub fn with_transactions(mut self, transactions: Vec<Transaction>) -> Self {
        self.transactions = Some(transactions);
        self
    }

    pub fn build(self, signer: &dyn Signer) -> Result<Batch, BatchBuildError> {
        let mut batch_header = BatchHeader::new();

        let signer_public_key = signer.public_key()?.as_hex();
        batch_header.set_signer_public_key(signer_public_key);

        let transactions = self.transactions.ok_or_else(|| {
            BatchBuildError::MissingField("Field 'transactions' is required".to_string())
        })?;

        let transaction_ids = transactions
            .iter()
            .map(|tx| tx.get_header_signature().to_string())
            .collect();
        batch_header.set_transaction_ids(RepeatedField::from_vec(transaction_ids));

        let batch_header_bytes = batch_header
            .write_to_bytes()
            .map_err(|err| {
                BatchBuildError::SerializationError(format!("Unable to serialize batch header: {}", err))
            })?;

        let signature = signer
            .sign(&batch_header_bytes)
            .map_err(|err| {
                BatchBuildError::SigningError(format!("Unable to sign batch: {}", err))
            })?;

        let mut batch = Batch::new();

        batch.set_header(batch_header_bytes.to_vec());
        batch.set_header_signature(signature.as_hex());
        batch.set_transactions(RepeatedField::from_vec(transactions));

        Ok(batch)
    }
}
