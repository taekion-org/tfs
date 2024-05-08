use std::fmt::{Display, Formatter};
use std::error::Error;
use protobuf::{Message, RepeatedField};
use rand::{Rng, thread_rng};
use sha2::{Digest, Sha512};
use crate::common::get_tfslite_prefix;
use crate::common::{FAMILY_NAME, FAMILY_VERSION};
use crate::client::keys::{PublicKey, Signature, Signer, SigningError, Verifier};
use crate::protos::transaction::{Transaction, TransactionHeader};
use crate::protos::payload::Payload;

#[derive(Debug)]
pub enum TransactionBuildError {
    SerializationError(String),
    MissingField(String),
    SigningError(String),
}

impl Error for TransactionBuildError {}

impl Display for TransactionBuildError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            TransactionBuildError::SerializationError(ref s) => write!(f, "SerializationError: {}", s),
            TransactionBuildError::MissingField(ref s) => write!(f, "MissingField: {}", s),
            TransactionBuildError::SigningError(ref s) => write!(f, "SigningError: {}", s),
        }
    }
}

impl From<SigningError> for TransactionBuildError {
    fn from(value: SigningError) -> Self {
        TransactionBuildError::SigningError(format!("{}", value))
    }
}

#[derive(Clone)]
pub struct TransactionBuilder {
    batcher_public_key: Option<Vec<u8>>,
    dependencies: Option<Vec<String>>,
    family_name: Option<String>,
    family_version: Option<String>,
    nonce: Option<Vec<u8>>,
    payload: Option<Payload>
}

impl Default for TransactionBuilder {
    fn default() -> Self {
        TransactionBuilder {
            batcher_public_key: None,
            dependencies: None,
            family_name: Some(FAMILY_NAME.to_string()),
            family_version: Some(FAMILY_VERSION.to_string()),
            nonce: None,
            payload: None,
        }
    }
}
impl TransactionBuilder {
    pub fn new() -> Self {
        TransactionBuilder::default()
    }

    pub fn with_batcher_public_key(mut self, batcher_public_key: Vec<u8>) -> Self {
        self.batcher_public_key = Some(batcher_public_key);
        self
    }

    pub fn with_dependencies(mut self, dependencies: Vec<String>) -> Self {
        self.dependencies = Some(dependencies);
        self
    }

    pub fn with_family_name(mut self, family_name: String) -> Self {
        self.family_name = Some(family_name);
        self
    }

    pub fn with_family_version(mut self, family_version: String) -> Self {
        self.family_version = Some(family_version);
        self
    }

    pub fn with_nonce(mut self, nonce: Vec<u8>) -> Self {
        self.nonce = Some(nonce);
        self
    }

    pub fn with_payload(mut self, payload: Payload) -> Self {
        self.payload = Some(payload);
        self
    }

    pub fn build(self, signer: &dyn Signer) -> Result<Transaction, TransactionBuildError> {
        let mut tx_header = TransactionHeader::new();

        // Signer public key
        let signer_public_key = signer.public_key()?;
        tx_header.set_signer_public_key(signer_public_key.as_hex());

        // Batcher public key
        let batcher_public_key = match self.batcher_public_key {
            Some(key_bytes) => PublicKey::load_from_bytes(key_bytes.as_slice()),
            None => signer_public_key
        };
        tx_header.set_batcher_public_key(batcher_public_key.as_hex());

        // Dependencies
        let dependencies = self.dependencies.unwrap_or_default();
        tx_header.set_dependencies(RepeatedField::from_vec(dependencies));

        // Family name
        let family_name = self.family_name.ok_or_else(|| {
            TransactionBuildError::MissingField("Field 'family_name' is required".to_string())
        })?;
        tx_header.set_family_name(family_name);

        // Family version
        let family_version = self.family_version.ok_or_else(|| {
            TransactionBuildError::MissingField("Field 'family_version' is required".to_string())
        })?;
        tx_header.set_family_version(family_version);

        // Inputs
        let inputs = vec![get_tfslite_prefix()];
        tx_header.set_inputs(RepeatedField::from_vec(inputs));

        // Outputs
        let outputs = vec![get_tfslite_prefix()];
        tx_header.set_outputs(RepeatedField::from(outputs));

        // Nonce
        let nonce = self.nonce.unwrap_or_else(|| {
            let mut nonce = [0u8; 32];
            thread_rng()
                .fill(&mut nonce[..]);
            nonce.to_vec()
        });
        tx_header.set_nonce(hex::encode(nonce));

        let payload = self.payload.ok_or_else(|| {
            TransactionBuildError::MissingField("Field 'payload' is required".to_string())
        })?;

        let payload_bytes = payload.write_to_bytes().map_err(|err| {
            TransactionBuildError::SerializationError(format!("Unable to serialize payload: {}", err))
        })?;

        let payload_hash = Sha512::digest(&payload_bytes).to_vec();
        tx_header.set_payload_sha512(hex::encode(payload_hash));

        let tx_header_bytes = tx_header
            .write_to_bytes()
            .map_err(|err| {
            TransactionBuildError::SerializationError(format!("Unable to serialize tx header: {}", err))
        })?;

        let signature = signer
            .sign(&tx_header_bytes)
            .map_err(|err| {
                TransactionBuildError::SigningError(format!("Unable to sign tx: {}", err))
            })?;

        let mut tx = Transaction::new();

        tx.set_header(tx_header_bytes.to_vec());
        tx.set_header_signature(signature.as_hex());
        tx.set_payload(payload_bytes);

        Ok(tx)
    }
}

#[derive(Debug)]
pub struct TransactionValidationError(String);

impl Display for TransactionValidationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ValidateTransactionError: {}", self.0)
    }
}

impl Error for TransactionValidationError {}

pub trait TransactionExt {
    fn validate(&self) -> Result<(), TransactionValidationError>;
}

impl TransactionExt for Transaction {
    fn validate(&self) -> Result<(), TransactionValidationError> {
        let header = TransactionHeader::parse_from_bytes(self.get_header())
            .map_err(|_err| TransactionValidationError(String::from("Transaction header could not be parsed")))?;

        let public_key = PublicKey::load_from_hex(header.get_signer_public_key())
            .map_err(|_err| TransactionValidationError(String::from("Transaction signer public key could not be loaded")))?;

        let signature = Signature::try_from(self.get_header_signature())
            .map_err(|err| TransactionValidationError(format!("Error loading Transaction signature: {}", err)))?;

        let verified = public_key.verify(self.get_header(), &signature)
            .map_err(|err| TransactionValidationError(format!("Error during signature verification: {}", err)))?;

        if !verified {
            return Err(TransactionValidationError("Transaction signature is invalid".to_string()));
        }

        let payload_hash = hex::encode(Sha512::digest(self.get_payload()).to_vec());
        if payload_hash.as_str() != header.get_payload_sha512() {
            return Err(TransactionValidationError("Transaction payload hash does not match header".to_string()));
        }

        Ok(())
    }
}
