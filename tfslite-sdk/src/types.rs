use std::error::Error;
use std::fmt::{Display, Formatter};
use chrono::prelude::*;
use serde::{Serialize, Deserialize};
use wasm_bindgen::prelude::wasm_bindgen;
use libtfslite::types::{FileMode, FileState};

#[wasm_bindgen]
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct BuildInfo {
    commit_hash: String,
}

//#[wasm_bindgen]
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct FileListResponse {
    account: String,
    pub files: Vec<FileListEntryIntermediate>,
}

#[wasm_bindgen]
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct FileListEntryIntermediate {
    id: uuid::Uuid,
    state: String,
    mode: String,
    last_updated: Option<DateTime<Utc>>,
    name: Option<String>,
}

#[wasm_bindgen]
#[derive(Serialize, Debug)]
#[allow(dead_code)]
pub struct FileListEntry {
    id: uuid::Uuid,
    state: FileState,
    mode: FileMode,
    last_updated: Option<DateTime<Utc>>,
    name: Option<String>,
}

impl FileListEntry {
    pub fn get_id(&self) -> uuid::Uuid {
        self.id
    }

    pub fn get_state(&self) -> FileState {
        self.state
    }

    pub fn get_mode(&self) -> FileMode {
        self.mode
    }

    pub fn get_last_updated(&self) -> Option<DateTime<Utc>> {
        self.last_updated
    }

    pub fn get_name(&self) -> Option<String> {
        self.name.clone()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub type FileList = Vec<FileListEntry>;
#[cfg(target_arch = "wasm32")]
pub type FileList = js_sys::Array;

#[derive(Debug)]
pub struct FileListParseError;

impl Display for FileListParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "FileListParseError")
    }
}

impl Error for FileListParseError {}

impl TryFrom<&FileListEntryIntermediate> for FileListEntry {
    type Error = FileListParseError;

    fn try_from(value: &FileListEntryIntermediate) -> Result<Self, Self::Error> {
        let entry = FileListEntry {
            id: value.id,
            state: match value.state.as_str() {
                "OPEN" => FileState::Open,
                "SEALED" => FileState::Sealed,
                _ => {
                    return Err(FileListParseError)
                },
            },
            mode: match value.mode.as_str() {
                "IMMUTABLE" => FileMode::Immutable,
                "DESTROYABLE" => FileMode::Destroyable,
                _ => {
                    return Err(FileListParseError)
                },
            },
            last_updated: value.last_updated,
            name: match &value.name {
                Some(name) => Some(name.clone()),
                None => None,
            },
        };
        Ok(entry)
    }
}

#[wasm_bindgen]
pub struct AccountBalance(pub u64);

#[wasm_bindgen]
impl AccountBalance {
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}
