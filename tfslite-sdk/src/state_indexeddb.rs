use uuid::Uuid;
use async_trait::async_trait;

use rexie::{Rexie, Error, ObjectStore, Index, TransactionMode, KeyRange};

use wasm_bindgen::JsValue;
use gloo_utils::format::JsValueSerdeExt;
use protobuf::Message;

use libtfslite::protos::transaction::Transaction;
use crate::state::{LocalStateStore, LocalStateStoreError, TransactionId, TransactionInfo, TransactionStatus, TransactionSubmitId};
use crate::debug::debug_println;

use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
struct FileInfo {
    file_id: String,
    next_order: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct TxInfo {
    //id: Option<u64>,
    order: u64,
    file_id: String,
    tx_id: String,
    submit_id: Option<String>,
    status: String,
}

impl From<TxInfo> for TransactionInfo {
    fn from(value: TxInfo) -> Self {
        TransactionInfo {
            order: value.order,
            tx_id: value.tx_id,
            submit_id: value.submit_id,
            status: value.status.into(),
        }
    }
}
impl From<Error> for LocalStateStoreError {
    fn from(value: Error) -> Self {
        LocalStateStoreError::ImplementationError(format!("rexie::Error: {}", value))
    }
}

pub struct IndexedDBLocalStateStore {
    db: Rexie,
}

impl IndexedDBLocalStateStore {
    pub async fn new() -> Result<Self, LocalStateStoreError> {
        let db = Rexie::builder("tfslite")
            .version(3)
            .add_object_store(
                ObjectStore::new("files")
                    .key_path("file_id")
            )
            .add_object_store(
                ObjectStore::new("tx_info")
                    .key_path("tx_id")
                    .add_index(Index::new("file_id", "file_id"))
                    .add_index(Index::new("order", "order"))
            )
            .add_object_store(
                ObjectStore::new("tx_bytes")
            )
            .build().await?;

        let result = IndexedDBLocalStateStore{
            db
        };

        Ok(result)
    }

    pub async fn set_has_file(&self, file_id: &uuid::Uuid) -> Result<(), LocalStateStoreError> {
        let tx = self.db.transaction(&["files"], TransactionMode::ReadWrite)?;
        let files = tx.store("files")?;

        let entry = FileInfo {
            file_id: file_id.to_string(),
            next_order: 1,
        };

        let entry = JsValue::from_serde(&entry).unwrap();

        let _ = files.add(&entry, None).await?;
        tx.done().await?;

        Ok(())
    }

    pub async fn check_has_file(&self, file_id: &uuid::Uuid) -> Result<(), LocalStateStoreError> {
        let tx = self.db.transaction(&["files"], TransactionMode::ReadOnly)?;
        let store = tx.store("files")?;

        let key = JsValue::from_serde(&file_id.to_string()).unwrap();
        debug_println!("Key: {:?}", key);

        let entry = store.get(&key).await?;
        debug_println!("Entry: {:?}", entry);
        if entry.is_undefined() {
            return Err(LocalStateStoreError::NoSuchFile)
        }

        Ok(())
    }
}

#[async_trait(?Send)]
impl LocalStateStore for IndexedDBLocalStateStore {
    async fn get_files(&self) -> Result<Vec<Uuid>, LocalStateStoreError> {
        let tx = self.db.transaction(&["files"], TransactionMode::ReadOnly)?;
        let store = tx.store("files")?;

        let files: Vec<Uuid> = store.get_all(None, None, None, None)
            .await?
            .into_iter()
            .map(|(k, _v)| k.into_serde().unwrap())
            .collect();

        Ok(files)
    }

    async fn get_txs(&self, file_id: &Uuid) -> Result<Vec<TransactionInfo>, LocalStateStoreError> {
        self.check_has_file(file_id).await?;

        let tx = self.db.transaction(&["tx_info"], TransactionMode::ReadOnly)?;
        let store = tx.store("tx_info")?;
        let index = store.index("file_id")?;

        let key = JsValue::from_serde(&file_id.to_string()).unwrap();
        let range = KeyRange::only(&key)?;

        let tx_infos: Vec<TxInfo> = index.get_all(Some(&range), None, None, None)
            .await?
            .into_iter()
            .map(|(_k,v)| v.into_serde().unwrap())
            .collect();

        let mut results: Vec<TransactionInfo> = tx_infos.into_iter().map(|e|e.into()).collect();
        results.sort_by(|a, b| a.order.cmp(&b.order));

        Ok(results)
    }

    async fn get_tx_bytes(&self, tx_id: &TransactionId) -> Result<Vec<u8>, LocalStateStoreError> {
        let tx = self.db.transaction(&["tx_bytes"], TransactionMode::ReadOnly)?;
        let store = tx.store("tx_bytes")?;

        let key = JsValue::from_serde(&tx_id).unwrap();
        let value = store.get(&key).await?;
        if value.is_undefined() {
            return Err(LocalStateStoreError::NoSuchTransaction);
        }

        let bytes: Vec<u8> = value.into_serde().unwrap();

        Ok(bytes)
    }

    async fn update_tx(&self, tx_id: &TransactionId, submit_id: Option<TransactionSubmitId>, status: Option<TransactionStatus>) -> Result<(), LocalStateStoreError> {
        let tx = self.db.transaction(&["tx_info"], TransactionMode::ReadWrite)?;
        let store = tx.store("tx_info")?;

        let key = JsValue::from_serde(&tx_id).unwrap();
        let value = store.get(&key).await?;
        if value.is_undefined() {
            return Err(LocalStateStoreError::NoSuchTransaction);
        }

        let mut tx_info: TxInfo = value.into_serde().unwrap();
        let mut need_update = false;

        if let Some(submit_id) = submit_id {
            tx_info.submit_id = Some(submit_id);
            need_update = true;
        }

        if let Some(status) = status {
            tx_info.status = status.into();
            need_update = true;
        }

        if need_update {
            let value_updated = JsValue::from_serde(&tx_info).unwrap();
            store.put(&value_updated, None).await?;
        }
        tx.done().await?;


        Ok(())
    }

    async fn flush_txs(&self, file_id: &Uuid) -> Result<(), LocalStateStoreError> {
        let tx = self.db.transaction(&["files", "tx_info", "tx_bytes"], TransactionMode::ReadWrite)?;
        let files_store = tx.store("files")?;
        let tx_info_store = tx.store("tx_info")?;
        let tx_bytes_store = tx.store("tx_bytes")?;

        let key = JsValue::from_serde(&file_id.to_string()).unwrap();
        files_store.delete(&key).await?;

        let range = KeyRange::only(&key)?;
        let tx_info_index = tx_info_store.index("file_id")?;
        let tx_infos: Vec<TxInfo> = tx_info_index.get_all(Some(&range), None, None, None)
            .await?
            .into_iter()
            .map(|(_k,v)| v.into_serde().unwrap())
            .collect();

        for tx_info in tx_infos {
            let key = JsValue::from_serde(&tx_info.tx_id).unwrap();
            tx_info_store.delete(&key).await?;
            tx_bytes_store.delete(&key).await?;
        }

        tx.done().await?;

        Ok(())
    }

    async fn add_tx(&self, file_id: &Uuid, transaction: &Transaction) -> Result<(), LocalStateStoreError> {
        let tx = self.db.transaction(&["files", "tx_info", "tx_bytes"], TransactionMode::ReadWrite)?;

        let store_files = tx.store("files")?;
        let key: JsValue = file_id.to_string().into();
        let value: JsValue = store_files.get(&key).await?;

        let mut file_info: FileInfo;
        if value.is_undefined() {
            file_info = FileInfo{
                file_id: file_id.to_string(),
                next_order: 0
            }
        } else {
            file_info = value.into_serde().unwrap();
        }

        // Add tx info
        let store_tx_info = tx.store("tx_info")?;
        let tx_info = TxInfo {
            file_id: file_id.to_string(),
            tx_id: transaction.get_header_signature().to_string(),
            submit_id: None,
            status: TransactionStatus::Local.into(),
            order: file_info.next_order,
        };
        let value = JsValue::from_serde(&tx_info).unwrap();
        store_tx_info.add(&value, None).await?;

        // Add tx bytes
        let store_tx_bytes = tx.store("tx_bytes")?;
        let bytes = transaction.write_to_bytes().unwrap();
        let key = JsValue::from_serde(&transaction.get_header_signature().to_string()).unwrap();
        let value = JsValue::from_serde(bytes.as_slice()).unwrap();
        debug_println!("Bytes: {:?}", value);
        store_tx_bytes.add(&value, Some(&key)).await?;

        // Update file info
        file_info.next_order += 1;
        let value = JsValue::from_serde(&file_info).unwrap();
        store_files.put(&value, None).await?;

        tx.done().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::state::LocalStateStoreError;
    use crate::state_indexeddb::IndexedDBLocalStateStore;
    use crate::tests::test_local_state_store_common;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    async fn test_local_state_store() -> Result<(), LocalStateStoreError> {
        use crate::state::LocalStateStore;
        let store = Box::new(IndexedDBLocalStateStore::new().await?);
        test_local_state_store_common(store).await
    }
}
