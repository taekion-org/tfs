use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use async_stream::stream;
use futures::stream::StreamExt;
use futures_util::pin_mut;
use reqwest::Response;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;
use libtfslite::client::keys::{PublicKey, Signer};
use libtfslite::client::payload::*;
use libtfslite::client::transaction::*;
use libtfslite::types::FileMode;
use crate::state::{LocalStateStore, TransactionId, TransactionStatus, TransactionSubmitId};
use crate::types::{BuildInfo, FileList, FileListEntry, FileListResponse, AccountBalance};
use crate::debug::debug_println;
use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        use std::thread;
        use std::path::{Path, PathBuf};
        use tokio::fs::File;
        use tokio::io::AsyncReadExt;

    } else if #[cfg(target_arch = "wasm32")] {
        use wasm_bindgen::prelude::*;
        use futures::AsyncReadExt;
        use crate::signing::JsSigner;
    }
}

const DEFAULT_CHUNK_SIZE: usize = 131072;

#[derive(Debug)]
pub enum TFSLiteClientErrorType {
    InvalidAccount,
    TransportError,
    DecodeError,
}

#[derive(Debug)]
pub struct TFSLiteClientError {
    error_type: TFSLiteClientErrorType,
    error_msg: Option<String>,
}

impl Error for TFSLiteClientError {}

impl Display for TFSLiteClientError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.error_type {
            TFSLiteClientErrorType::InvalidAccount => write!(f, "InvalidAccountError"),
            TFSLiteClientErrorType::TransportError => write!(f, "TransportError: {}", self.error_msg.clone().unwrap_or("<no msg>".to_string())),
            TFSLiteClientErrorType::DecodeError => write!(f, "DecodeError: {}", self.error_msg.clone().unwrap_or("<no msg>".to_string())),
        }
    }
}

impl TFSLiteClientError {
    pub fn new(error_type: TFSLiteClientErrorType, error_msg: Option<String>) -> Self {
        Self {
            error_type,
            error_msg,
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl From<TFSLiteClientError> for JsValue {
    fn from(value: TFSLiteClientError) -> Self {
        JsValue::from_str(value.to_string().as_str())
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct TFSLiteClient {
    url: String,
    account: Option<PublicKey>,
    store: Arc<Mutex<dyn LocalStateStore>>,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl TFSLiteClient {
    pub async fn new(url: String) -> TFSLiteClient {
        TFSLiteClient {
            url,
            account: None,
            store: Self::init_state_store().await
        }
    }

    // TODO: Figure out a standard file path for this database.
    #[cfg(not(target_arch = "wasm32"))]
    async fn init_state_store() -> Arc<Mutex<dyn LocalStateStore>> {
        use crate::state_redb;
        Arc::new(Mutex::new(state_redb::RedbLocalStateStore::new("/tmp/redb-client.db").await.unwrap()))
    }

    #[cfg(target_arch = "wasm32")]
    async fn init_state_store() -> Arc<Mutex<dyn LocalStateStore>> {
        console_error_panic_hook::set_once();

        use crate::state_indexeddb;
        Arc::new(Mutex::new(state_indexeddb::IndexedDBLocalStateStore::new().await.unwrap()))
    }

    pub fn set_account(&mut self, account: PublicKey) {
        self.account = Some(account);
    }

    async fn fetch_url(&self, url: String) -> Result<Response, TFSLiteClientError> {
        let result = reqwest::get(url)
            .await
            .map_err(|err|TFSLiteClientError::new(TFSLiteClientErrorType::TransportError, Some(format!("{}", err))))?;

        Ok(result)
    }

    async fn fetch_url_json<T: DeserializeOwned>(&self, url: String) -> Result<T, TFSLiteClientError> {
        let result = self.fetch_url(url)
            .await?
            .json::<T>()
            .await
            .map_err(|err|TFSLiteClientError::new(TFSLiteClientErrorType::DecodeError, Some(format!("{}", err))))?;

        Ok(result)
    }

    async fn fetch_url_object(&self, url: String) -> Result<serde_json::Map<String, serde_json::Value>, TFSLiteClientError> {
        let result = self.fetch_url_json::<serde_json::Value>(url)
            .await?
            .as_object()
            .unwrap()
            .clone();

        Ok(result)
    }

    pub async fn get_build_info(&self) -> Result<BuildInfo, TFSLiteClientError> {
        let url = format!("{}/build-info", self.url);

        self.fetch_url_json(url).await
    }

    pub async fn get_batcher_public_key(&self) -> Result<PublicKey, TFSLiteClientError> {
        let url = format!("{}/batcher-public-key", self.url);
        let data = self.fetch_url_object(url)
            .await?;

        let key_string = data.get("batcher_public_key")
            .unwrap()
            .as_str()
            .unwrap();

        let result = hex::decode(key_string)
            .map_err(|err| TFSLiteClientError::new(TFSLiteClientErrorType::DecodeError, Some(format!("{}", err))))?;

        let public_key = PublicKey::load_from_bytes(result.as_slice());

        Ok(public_key)
    }

    pub async fn get_account_balance(&self) -> Result<AccountBalance, TFSLiteClientError> {
        let account = match &self.account {
            Some(account) => hex::encode(account.as_slice()),
            None => {
                return Err(TFSLiteClientError::new(TFSLiteClientErrorType::InvalidAccount, None));
            },
        };

        let url = format!("{}/account/balance/{}", self.url, account);

        let data = self.fetch_url_object(url)
            .await?;

        let balance = data.get("balance")
            .unwrap()
            .as_u64()
            .unwrap();

        Ok(AccountBalance(balance))
    }

    pub async fn get_account_files(&self) -> Result<FileList, TFSLiteClientError> {
        let account = match &self.account {
            Some(account) => hex::encode(account.as_slice()),
            None => {
                return Err(TFSLiteClientError::new(TFSLiteClientErrorType::InvalidAccount, None));
            },
        };

        let url = format!("{}/account/files/{}", self.url, account);
        let response: FileListResponse = self.fetch_url_json(url).await?;

        let result: Vec<FileListEntry> = response.files.iter().map(|e| e.try_into().unwrap()).collect();

        #[cfg(not(target_arch = "wasm32"))]
        return Ok(result);

        #[cfg(target_arch = "wasm32")]
        return Ok(result.into_iter().map(JsValue::from).collect());
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn upload_file(&self, file: &Path) -> Result<FileUpload, TFSLiteClientError> {
        let batcher_public_key = PublicKey::load_from_bytes(
            self.get_batcher_public_key().await?.as_slice()
        );

        let file_upload = FileUpload {
            file: file.to_path_buf(),
            url: self.url.clone(),
            store: self.store.clone(),

            signer: None,
            batcher_public_key,
            uuid: Uuid::new_v4(),
            chunk_size: DEFAULT_CHUNK_SIZE,
            filename: None,
        };

        Ok(file_upload)
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn upload_file(&self, file: web_sys::File) -> Result<FileUpload, TFSLiteClientError> {
        let batcher_public_key = PublicKey::load_from_bytes(
            self.get_batcher_public_key().await?.as_slice()
        );

        let file_upload = FileUpload {
            file: file,
            url: self.url.clone(),
            store: self.store.clone(),

            signer: None,
            batcher_public_key,
            uuid: Uuid::new_v4(),
            chunk_size: DEFAULT_CHUNK_SIZE,
            filename: None,
        };

        Ok(file_upload)
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct FileUpload {
    #[cfg(not(target_arch = "wasm32"))]
    file: PathBuf,

    #[cfg(target_arch = "wasm32")]
    file: web_sys::File,

    url: String,
    store: Arc<Mutex<dyn LocalStateStore>>,

    signer: Option<Box<dyn Signer>>,
    batcher_public_key: PublicKey,
    uuid: Uuid,
    chunk_size: usize,
    filename: Option<String>,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl FileUpload {

    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_signer(&mut self, signer: &dyn Signer) {
        self.signer = Some(signer.clone_box());
    }

    #[cfg(target_arch = "wasm32")]
    pub fn set_signer(&mut self, signer: JsSigner) {
        self.signer = Some(Box::new(signer));
    }

    pub fn set_chunk_size(&mut self, chunk_size: usize) {
        self.chunk_size = chunk_size;
    }

    pub fn set_filename(&mut self, filename: &str) {
        self.filename = Some(filename.to_string());
    }

    pub async fn prepare_transactions(&self) -> Result<(), TFSLiteClientError> {
        let mut filename: Option<String> = self.filename.clone();

        #[cfg(not(target_arch = "wasm32"))]
        let mut f = {
            if filename.is_none() {
                filename = Some(self.file.file_name().unwrap().to_str().unwrap().to_string());
            }

            File::open(self.file.as_path()).await.unwrap()
        };

        #[cfg(target_arch = "wasm32")]
        let mut f = {
            if filename.is_none() {
                filename = Some(self.file.name());
            }
            let readable_stream = wasm_streams::ReadableStream::from_raw(self.file.stream());
            readable_stream.into_async_read()
        };

        let stream = stream ! {
            let mut buffer: Vec<u8> = vec![0; self.chunk_size];
            let slice = buffer.as_mut_slice();

            while let Ok(bytes_read) = f.read(slice).await {
                if bytes_read == 0 {
                    break;
                }

                yield slice[0..bytes_read].to_vec();
            }
        };

        pin_mut!(stream);
        debug_println!("Uuid: {}", self.uuid);

        use libtfslite::common::FILE_CREATE_COST;
        let public_key = self.signer.as_ref().unwrap().public_key().unwrap();
        let mut tx_id_prev: String;

        let store = self.store.lock().unwrap();

        let payload = PayloadBuilder::new(PayloadOperation::AccountDeposit)
            .with_address(public_key.as_slice().to_vec())
            .with_amount(FILE_CREATE_COST*10)
            .build()
            .unwrap();

        let tx = TransactionBuilder::new()
            .with_payload(payload)
            .with_batcher_public_key(self.batcher_public_key.as_slice().to_vec())
            .build(self.signer.as_ref().unwrap().as_ref())
            .unwrap();

        let _ = store.add_tx(&self.uuid, &tx)
            .await;

        tx_id_prev = tx.get_header_signature().to_string();

        let payload = PayloadBuilder::new(PayloadOperation::FileCreate)
            .with_uuid(self.uuid)
            .with_mode(FileMode::Immutable)
            .with_filename(filename.unwrap())
            .build()
            .unwrap();
        let tx = TransactionBuilder::new()
            .with_payload(payload)
            .with_batcher_public_key(self.batcher_public_key.as_slice().to_vec())
            .with_dependencies(vec![tx_id_prev])
            .build(self.signer.as_ref().unwrap().as_ref())
            .unwrap();

        let _ = store.add_tx(&self.uuid, &tx)
            .await;

        tx_id_prev = tx.get_header_signature().to_string();

        while let Some(data) = stream.next().await {
            debug_println!("Len: {}", data.len());

            let payload = PayloadBuilder::new(PayloadOperation::FileAppend)
                .with_uuid(self.uuid)
                .with_block(data)
                .build()
                .unwrap();
            let tx = TransactionBuilder::new()
                .with_payload(payload)
                .with_batcher_public_key(self.batcher_public_key.as_slice().to_vec())
                .with_dependencies(vec![tx_id_prev])
                .build(self.signer.as_ref().unwrap().as_ref())
                .unwrap();

            let _ = store.add_tx(&self.uuid, &tx)
                .await;

            tx_id_prev = tx.get_header_signature().to_string();
        }

        let payload = PayloadBuilder::new(PayloadOperation::FileSeal)
            .with_uuid(self.uuid)
            .build()
            .unwrap();
        let tx = TransactionBuilder::new()
            .with_payload(payload)
            .with_batcher_public_key(self.batcher_public_key.as_slice().to_vec())
            .with_dependencies(vec![tx_id_prev])
            .build(self.signer.as_ref().unwrap().as_ref())
            .unwrap();

        let _ = store.add_tx(&self.uuid, &tx)
            .await;

        Ok(())
    }

    async fn submit_transaction(&self, tx_id: &TransactionId) -> Result<TransactionSubmitId, TFSLiteClientError> {
        #[derive(Deserialize)]
        struct SubmitResponse {
            submit_id: String,
        }

        let store = self.store.lock().unwrap();

        let tx_bytes = store.get_tx_bytes(tx_id)
            .await.unwrap();

        let http_client = reqwest::Client::new();

        let response = http_client
            .post(format!("{}/transaction/submit", self.url.as_str()))
            .header("Content-Type", "application/octet-stream")
            .body(tx_bytes)
            .send()
            .await
            .map_err(|err| TFSLiteClientError::new(TFSLiteClientErrorType::TransportError, Some(format!("{}", err))))?;

        if response.status().is_success() {
            let response_data = response
                .json::<SubmitResponse>()
                .await
                .unwrap();

            Ok(response_data.submit_id)
        } else {
            let status = response.status();
            let msg = response
                .text()
                .await
                .unwrap_or(String::from("(No Message Found)"));

            Err(TFSLiteClientError::new(TFSLiteClientErrorType::TransportError, Some(format!("Response Code: {}, Message: {}", status, msg))))
        }
    }

    async fn get_transaction_statuses(&self, submit_ids: Vec<TransactionSubmitId>) -> Result<HashMap<TransactionSubmitId, TransactionStatus>, TFSLiteClientError> {
        let http_client = reqwest::Client::new();

        let mut request: HashMap<&str, Vec<String>> = HashMap::new();
        request.insert("submit_ids", submit_ids);
        debug_println!("{:?}", request);

        let response = http_client
            .post(format!("{}/transaction/status/multiple", self.url.as_str()))
            .json(&request)
            .send()
            .await
            .map_err(|err| TFSLiteClientError::new(TFSLiteClientErrorType::TransportError, Some(format!("{}", err))))?;

        if response.status().is_success() {
            let response_data = response
                .json::<HashMap<String, String>>()
                .await
                .unwrap();

            let mut response: HashMap<TransactionSubmitId, TransactionStatus> = HashMap::new();
            response_data.iter().for_each(|(k,v)| {
               response.insert(k.clone(), v.clone().into());
            });

            Ok(response)
        } else {
            let status = response.status();
            let msg = response
                .text()
                .await
                .unwrap_or(String::from("(No Message Found)"));

            Err(TFSLiteClientError::new(TFSLiteClientErrorType::TransportError, Some(format!("Response Code: {}, Message: {}", status, msg))))
        }
    }

    pub async fn send_transactions(&self) -> Result<(), TFSLiteClientError> {
        debug_println!("send_transactions({})", self.uuid);

        let store = self.store.lock().unwrap();

        let tx_infos = store.get_txs(&self.uuid)
            .await
            .unwrap();

        drop(store);

        for tx_info in tx_infos {
            debug_println!("tx_info: {:?}", tx_info);
            let tx_submit_id = self.submit_transaction(&tx_info.tx_id).await?;

            let store = self.store.lock().unwrap();
            store.update_tx(&tx_info.tx_id, Some(tx_submit_id), None)
                .await.unwrap();
        }

        Ok(())
    }

    async fn update_tx_statuses(&self) -> Result<(), TFSLiteClientError> {
        debug_println!("update_tx_status({})", self.uuid);

        let store = self.store.lock().unwrap();

        let tx_infos = store.get_txs(&self.uuid)
            .await
            .unwrap();

        let tx_map: HashMap<TransactionSubmitId, TransactionId> = tx_infos.iter().map(|tx_info| {
            let submit_id = tx_info.submit_id.clone().unwrap();
            let tx_id = tx_info.tx_id.clone();
            (submit_id, tx_id)
        }).collect();
        let submit_ids_check: Vec<TransactionSubmitId> = tx_infos.iter().map(|tx_info| tx_info.submit_id.clone().unwrap()).collect();

        let tx_statuses = self.get_transaction_statuses(submit_ids_check)
            .await?;

        for (submit_id, mut status) in tx_statuses {
            let tx_id = tx_map.get(&submit_id).unwrap();
            if status == TransactionStatus::Unknown {
                status = TransactionStatus::Local
            }
            debug_println!("{} -> {:?}", tx_id, status);
            let _ = store.update_tx(tx_id, Some(submit_id), Some(status))
                .await;
        }

        Ok(())
    }

    pub async fn wait_transactions(&self) -> Result<(), TFSLiteClientError> {
        debug_println!("wait_transactions({})", self.uuid);

        loop {
            let mut uncommited_count = 0;

            self.update_tx_statuses()
                .await?;

            let store = self.store.lock().unwrap();

            let tx_infos = store.get_txs(&self.uuid)
                .await
                .unwrap();

            drop(store);

            for tx_info in tx_infos {
                debug_println!("tx_info: {:?}", tx_info);
                if tx_info.status != TransactionStatus::Committed {
                    uncommited_count += 1;
                }

                if tx_info.status == TransactionStatus::Local {
                    debug_println!("Resubmitting tx: {:?}", tx_info.tx_id);
                    let tx_submit_id = self.submit_transaction(&tx_info.tx_id)
                        .await?;

                    let store = self.store.lock().unwrap();
                    store.update_tx(&tx_info.tx_id, Some(tx_submit_id), None)
                        .await.unwrap();
                }
            }

            if uncommited_count == 0 {
                break;
            }

            debug_println!("Sleeping...");
            #[cfg(not(target_arch = "wasm32"))]
            thread::sleep(Duration::from_millis(500));
            #[cfg(target_arch = "wasm32")]
            gloo_timers::future::sleep(Duration::from_millis(500)).await;
            debug_println!("Done sleeping...");
        }

        let store = self.store.lock().unwrap();
        let _ = store.flush_txs(&self.uuid)
            .await;

        Ok(())
    }
}

impl FileUpload {
    pub(crate) fn _set_signer(&mut self, signer: &dyn Signer) {
        self.signer = Some(signer.clone_box());
    }
}

#[cfg(test)]
mod tests {
    use crate::client::TFSLiteClientError;
    use crate::tests::test_client_common;

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_client() -> Result<(), TFSLiteClientError> {
        test_client_common().await
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test::wasm_bindgen_test]
    async fn test_client() -> Result<(), TFSLiteClientError> {
        test_client_common().await
    }
}
