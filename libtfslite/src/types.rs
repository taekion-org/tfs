use std::fmt::{Display, Formatter, Result};
use serde::{Deserialize, Serialize};
use serde_repr::{Serialize_repr, Deserialize_repr};
use uuid;
use uuid::serde::compact;
use crate::protos::payload::{Payload_FileMode, Payload_Permission};


#[derive(Serialize_repr, Deserialize_repr, Debug)]
#[repr(u8)]
pub enum FileState {
    Open = 1,
    Sealed = 2,
}

impl Display for FileState {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            FileState::Open => write!(f, "OPEN"),
            FileState::Sealed => write!(f, "SEALED"),
        }
    }
}

#[derive(Serialize_repr, Deserialize_repr, Debug)]
#[repr(u8)]
pub enum FileMode {
    Destroyable = 1,
    Immutable = 2,
}

impl Display for FileMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            FileMode::Destroyable => write!(f, "DESTROYABLE"),
            FileMode::Immutable => write!(f,"IMMUTABLE"),
        }
    }
}

impl From<Payload_FileMode> for FileMode {
    fn from(value: Payload_FileMode) -> Self {
        match value {
            Payload_FileMode::DESTROYABLE => FileMode::Destroyable,
            Payload_FileMode::IMMUTABLE => FileMode::Immutable,
        }
    }
}

impl From<FileMode> for Payload_FileMode {
    fn from(value: FileMode) -> Self {
        match value {
            FileMode::Destroyable => Payload_FileMode::DESTROYABLE,
            FileMode::Immutable => Payload_FileMode::IMMUTABLE,
        }
    }
}


#[derive(Serialize, Deserialize)]
pub struct DirectoryEntry {
    #[serde(with = "compact")]
    pub file_id: uuid::Uuid,
    pub file_name: String,
}

#[derive(Clone)]
pub enum Permission {
    Unset,
    SetPermission,
    Batcher,
    Deposit,
    Timestamp,
}

impl Permission {
    pub fn to_hex(&self) -> String {
        match self {
            Permission::Unset => String::from("00"),
            Permission::SetPermission => String::from("01"),
            Permission::Batcher => String::from("02"),
            Permission::Deposit => String::from("03"),
            Permission::Timestamp => String::from("04"),
        }
    }
}

impl Display for Permission {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match *self {
            Permission::Unset => write!(f, "Permission::Unset"),
            Permission::SetPermission => write!(f, "Permission::SetPermission"),
            Permission::Batcher => write!(f, "Permission::Batcher"),
            Permission::Deposit => write!(f, "Permission::Deposit"),
            Permission::Timestamp => write!(f, "Permission::Timestamp"),
        }
    }
}

impl From<Payload_Permission> for Permission {
    fn from(value: Payload_Permission) -> Self {
        match value {
            Payload_Permission::UNSET => Permission::Unset,
            Payload_Permission::SET_PERMISSION => Permission::SetPermission,
            Payload_Permission::BATCHER => Permission::Batcher,
            Payload_Permission::DEPOSIT => Permission::Deposit,
            Payload_Permission::TIMESTAMP => Permission::Timestamp,
        }
    }
}

impl From<Permission> for Payload_Permission {
    fn from(value: Permission) -> Self {
        match value {
            Permission::Unset => Payload_Permission::UNSET,
            Permission::SetPermission => Payload_Permission::SET_PERMISSION,
            Permission::Batcher => Payload_Permission::BATCHER,
            Permission::Deposit => Payload_Permission::DEPOSIT,
            Permission::Timestamp => Payload_Permission::TIMESTAMP,
        }
    }
}
