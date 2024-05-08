use std::error::Error;
use std::fmt::{Display, Formatter};
use uuid::Uuid;
use sha2::Digest;
use crate::types::{FileMode, Permission};
use crate::protos::payload::{Payload, Payload_DataBlock, Payload_Operation, Payload_FileMode, Payload_Permission};

#[derive(Debug)]
pub enum PayloadBuildError {
    SerializationError(String),
    MissingField(String),
}

impl Error for PayloadBuildError {}

impl Display for PayloadBuildError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            PayloadBuildError::SerializationError(ref s) => write!(f, "SerializationError: {}", s),
            PayloadBuildError::MissingField(ref s) => write!(f, "MissingField: {}", s),
        }
    }
}

#[derive(Clone)]
pub struct PayloadBuilder {
    operation: Payload_Operation,
    uuid: Option<Uuid>,
    mode: Option<Payload_FileMode>,
    block: Option<Payload_DataBlock>,
    filename: Option<String>,
    address: Option<Vec<u8>>,
    amount: Option<u64>,
    permission: Option<Payload_Permission>,
    permission_public_key: Option<Vec<u8>>,
    timestamp_create: Option<i64>,
    timestamp_append: Option<i64>,
    timestamp_seal: Option<i64>,
}

pub enum PayloadOperation {
    FileCreate,
    FileAppend,
    FileSeal,
    FileDestroy,
    AccountDeposit,
    AccountTransfer,
    PermissionSet,
    PermissionClear,
    TimestampSet,
}

impl From<PayloadOperation> for Payload_Operation {
    fn from(value: PayloadOperation) -> Self {
        match value {
            PayloadOperation::FileCreate => Payload_Operation::FILE_CREATE,
            PayloadOperation::FileAppend => Payload_Operation::FILE_APPEND,
            PayloadOperation::FileSeal => Payload_Operation::FILE_SEAL,
            PayloadOperation::FileDestroy => Payload_Operation::FILE_DESTROY,
            PayloadOperation::AccountDeposit => Payload_Operation::ACCOUNT_DEPOSIT,
            PayloadOperation::AccountTransfer => Payload_Operation::ACCOUNT_TRANSFER,
            PayloadOperation::PermissionSet => Payload_Operation::PERMISSION_SET,
            PayloadOperation::PermissionClear => Payload_Operation::PERMISSION_CLEAR,
            PayloadOperation::TimestampSet => Payload_Operation::TIMESTAMP_SET,
        }
    }
}

impl PayloadBuilder {
    pub fn new(operation: PayloadOperation) -> PayloadBuilder {
        PayloadBuilder {
            operation: operation.into(),
            uuid: None,
            mode: None,
            block: None,
            filename: None,
            address: None,
            amount: None,
            permission: None,
            permission_public_key: None,
            timestamp_create: None,
            timestamp_append: None,
            timestamp_seal: None,
        }
    }

    pub fn with_uuid(mut self, uuid: uuid::Uuid) -> Self {
        self.uuid = Some(uuid);
        self
    }

    pub fn with_mode(mut self, mode: FileMode) -> Self {
        self.mode = Some(mode.into());
        self
    }

    pub fn with_block(mut self, data: Vec<u8>) -> Self {
        let mut block = Payload_DataBlock::new();
        let sha224 = sha2::Sha224::digest(&data).to_vec();
        block.set_sha224(sha224);
        block.set_data(data);

        self.block = Some(block);
        self
    }

    pub fn with_filename(mut self, filename: String) -> Self {
        self.filename = Some(filename);
        self
    }

    pub fn with_address(mut self, address: Vec<u8>) -> Self {
        self.address = Some(address);
        self
    }

    pub fn with_amount(mut self, amount: u64) -> Self {
        self.amount = Some(amount);
        self
    }

    pub fn with_permission(mut self, perm: Permission) -> Self {
        self.permission = Some(perm.into());
        self
    }

    pub fn with_permission_public_key(mut self, public_key: Vec<u8>) -> Self {
        self.permission_public_key = Some(public_key);
        self
    }

    pub fn with_timestamp_create(mut self, timestamp: i64) -> Self {
        self.timestamp_create = Some(timestamp);
        self
    }

    pub fn with_timestamp_append(mut self, timestamp: i64) -> Self {
        self.timestamp_append = Some(timestamp);
        self
    }

    pub fn with_timestamp_seal(mut self, timestamp: i64) -> Self {
        self.timestamp_seal = Some(timestamp);
        self
    }

    pub fn build(self) -> Result<Payload, PayloadBuildError> {
        let mut payload = Payload::new();
        payload.set_operation(self.operation);

        match self.operation {
            Payload_Operation::FILE_CREATE => {
                let uuid = self.uuid.ok_or_else(|| {
                    PayloadBuildError::MissingField("Field 'uuid' is required".to_string())
                })?;
                let uuid_ref: &[u8] = uuid.as_ref();
                payload.set_uuid(uuid_ref.to_vec());

                let mode = self.mode.ok_or_else(|| {
                    PayloadBuildError::MissingField("Field 'mode' is required".to_string())
                })?;
                payload.set_mode(mode);

                if let Some(filename) = self.filename {
                    payload.set_filename(filename);
                }
            },
            Payload_Operation::FILE_APPEND => {
                let uuid = self.uuid.ok_or_else(|| {
                    PayloadBuildError::MissingField("Field 'uuid' is required".to_string())
                })?;
                let uuid_ref: &[u8] = uuid.as_ref();
                payload.set_uuid(uuid_ref.to_vec());

                let block = self.block.ok_or_else(|| {
                    PayloadBuildError::MissingField("Field 'block' is required".to_string())
                })?;
                payload.set_block(block);
            },
            Payload_Operation::FILE_SEAL | Payload_Operation::FILE_DESTROY => {
                let uuid = self.uuid.ok_or_else(|| {
                    PayloadBuildError::MissingField("Field 'uuid' is required".to_string())
                })?;
                let uuid_ref: &[u8] = uuid.as_ref();
                payload.set_uuid(uuid_ref.to_vec());
            },
            Payload_Operation::ACCOUNT_DEPOSIT | Payload_Operation::ACCOUNT_TRANSFER => {
                let address = self.address.ok_or_else(|| {
                    PayloadBuildError::MissingField("Field 'address' is required".to_string())
                })?;
                payload.set_address(address);

                let amount = self.amount.ok_or_else(|| {
                    PayloadBuildError::MissingField("Field 'amount' is required".to_string())
                })?;
                payload.set_amount(amount);
            },
            Payload_Operation::PERMISSION_SET => {
                let permission = self.permission.ok_or_else(|| {
                    PayloadBuildError::MissingField("Field 'permission' is required".to_string())
                })?;
                payload.set_permission(permission);

                let permission_public_key = self.permission_public_key.ok_or_else(|| {
                    PayloadBuildError::MissingField("Field 'permission_public_key' is required".to_string())
                })?;
                payload.set_permission_public_key(permission_public_key);
            },
            Payload_Operation::PERMISSION_CLEAR => {
                let permission = self.permission.ok_or_else(|| {
                    PayloadBuildError::MissingField("Field 'permission' is required".to_string())
                })?;
                payload.set_permission(permission);
            },
            Payload_Operation::TIMESTAMP_SET => {
                let uuid = self.uuid.ok_or_else(|| {
                    PayloadBuildError::MissingField("Field 'uuid' is required".to_string())
                })?;
                let uuid_ref: &[u8] = uuid.as_ref();
                payload.set_uuid(uuid_ref.to_vec());

                if self.timestamp_create.is_none() && self.timestamp_append.is_none() && self.timestamp_seal.is_none() {
                    return Err(PayloadBuildError::MissingField("At least one of the the fields 'timestamp_create', 'timestamp_append' or 'timestamp_seal' must be set".to_string()));
                }

                if let Some(timestamp) = self.timestamp_create {
                    payload.set_timestamp_create(timestamp);
                }

                if let Some(timestamp) = self.timestamp_append {
                    payload.set_timestamp_append(timestamp);
                }

                if let Some(timestamp) = self.timestamp_seal {
                    payload.set_timestamp_seal(timestamp)
                }
            }
        }

        Ok(payload)
    }
}
