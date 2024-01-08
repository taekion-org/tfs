use std::path::Path;
use protobuf::Message;
use uuid::Uuid;
use async_trait::async_trait;

use redb::{Database,ReadableTable, ReadableMultimapTable, TableDefinition, MultimapTableDefinition, TransactionError, TableError, StorageError, CommitError};
use libtfslite::protos::transaction::Transaction;
use crate::state::{LocalStateStore, LocalStateStoreError, TransactionId, TransactionInfo, TransactionStatus, TransactionSubmitId};

const FILES_TABLE: TableDefinition<u128, u64> = TableDefinition::new("files");
const FILE_TXS_TABLE: MultimapTableDefinition<u128, &str> = MultimapTableDefinition::new("file_txs");
const TX_INFO_TABLE: TableDefinition<&str, (u64, &str, &str)> = TableDefinition::new("tx_info");
const TX_BYTES_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("tx_bytes");

impl From<TransactionError> for LocalStateStoreError {
    fn from(value: TransactionError) -> Self {
        LocalStateStoreError::ImplementationError(format!("TransactionError: {}", value))
    }
}

impl From<TableError> for LocalStateStoreError {
    fn from(value: TableError) -> Self {
        LocalStateStoreError::ImplementationError(format!("TableError: {}", value))
    }
}

impl From<StorageError> for LocalStateStoreError {
    fn from(value: StorageError) -> Self {
        LocalStateStoreError::ImplementationError(format!("StorageError: {}", value))
    }
}

impl From<CommitError> for LocalStateStoreError {
    fn from(value: CommitError) -> Self {
        LocalStateStoreError::ImplementationError(format!("CommitError: {}", value))
    }
}

pub struct RedbLocalStateStore {
    db: Database
}

impl RedbLocalStateStore {
    pub async fn new(path: impl AsRef<Path>) -> Result<Self, LocalStateStoreError> {
        let db = Database::create(&path).unwrap();

        let write_txn = db.begin_write()?;
        {
            let _table_files = write_txn.open_table(FILES_TABLE)?;
            let _table_file_txs = write_txn.open_multimap_table(FILE_TXS_TABLE)?;
            let _table_info = write_txn.open_table(TX_INFO_TABLE)?;
            let _table_tx_bytes = write_txn.open_table(TX_BYTES_TABLE)?;
        }
        write_txn.commit()?;

        let result = RedbLocalStateStore{
            db,
        };

        Ok(result)
    }

    pub async fn set_has_file(&self, file_id: &uuid::Uuid) -> Result<(), LocalStateStoreError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(FILES_TABLE)?;
            let _ = table.insert(file_id.as_u128(), 1);
        }
        write_txn.commit()?;

        Ok(())
    }

    pub async fn check_has_file(&self, file_id: &uuid::Uuid) -> Result<(), LocalStateStoreError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(FILES_TABLE)?;

        let value = table.get(file_id.as_u128())?;
        if value.is_none() {
            return Err(LocalStateStoreError::NoSuchFile)
        }

        Ok(())
    }
}

#[async_trait(?Send)]
impl LocalStateStore for RedbLocalStateStore {
    async fn get_files(&self) -> Result<Vec<Uuid>, LocalStateStoreError> {
        let read_txn = self.db.begin_read()?;
        let table_files = read_txn.open_table(FILES_TABLE)?;
        let results: Vec<Uuid> = table_files.iter()?.map(|v| Uuid::from_u128(v.unwrap().0.value())).collect();

        Ok(results)
    }

    async fn get_txs(&self, file_id: &Uuid) -> Result<Vec<TransactionInfo>, LocalStateStoreError> {
        self.check_has_file(file_id).await?;

        let read_txn = self.db.begin_read()?;

        let table_file_txs = read_txn.open_multimap_table(FILE_TXS_TABLE)?;
        let table_tx_info = read_txn.open_table(TX_INFO_TABLE)?;

        let mut results = Vec::<TransactionInfo>::new();
        for file_tx in table_file_txs.get(file_id.as_u128())? {
            let file_tx = file_tx?;
            let file_tx_id = file_tx.value();

            let tx_info = table_tx_info.get(file_tx_id)?.unwrap();
            let (order, submit_id, status) =  tx_info.value();

            results.push(TransactionInfo{
                order,
                tx_id: file_tx_id.to_string(),
                submit_id: match submit_id {
                    "" => None,
                    other => Some(other.to_string()),
                },
                status: TransactionStatus::from(status.to_string())
            });
        }

        results.sort_by(|a,b| a.order.cmp(&b.order));

        Ok(results)
    }

    async fn get_tx_bytes(&self, tx_id: &TransactionId) -> Result<Vec<u8>, LocalStateStoreError> {
        let read_txn = self.db.begin_read()?;

        let table_bytes = read_txn.open_table(TX_BYTES_TABLE)?;
        let value = table_bytes.get(tx_id.as_str())?;

        match value {
            None => Err(LocalStateStoreError::NoSuchTransaction),
            Some(bytes) => Ok(Vec::from(bytes.value()))
        }
    }

    async fn update_tx(&self, tx_id: &TransactionId, submit_id: Option<TransactionSubmitId>, status: Option<TransactionStatus>) -> Result<(), LocalStateStoreError> {
        let order_db: u64;
        let mut submit_id_db: String;
        let mut status_db: String;

        let mut need_commit = false;

        let write_txn = self.db.begin_write()?;
        {
            let table_tx_info = write_txn.open_table(TX_INFO_TABLE)?;

            let value = table_tx_info.get(tx_id.as_str())?;
            match value {
                None => {
                    return Err(LocalStateStoreError::NoSuchTransaction);
                },
                Some(tx_info) => {
                    let value = tx_info.value();
                    (order_db, submit_id_db, status_db) = (value.0, value.1.to_string(), value.2.to_string());
                }
            }
        }
        {
            let mut table_tx_info = write_txn.open_table(TX_INFO_TABLE)?;

            if let Some(submit_id) = submit_id {
                submit_id_db = submit_id;
                need_commit = true;
            }

            if let Some(status) = status {
                status_db = status.into();
                need_commit = true;
            }

            table_tx_info.insert(tx_id.as_str(), (order_db, submit_id_db.as_str(), status_db.as_str()))?;
        }

        if need_commit {
            write_txn.commit()?;
        }

        Ok(())
    }

    async fn flush_txs(&self, file_id: &Uuid) -> Result<(), LocalStateStoreError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table_files = write_txn.open_table(FILES_TABLE)?;
            let mut table_file_txs = write_txn.open_multimap_table(FILE_TXS_TABLE)?;
            let mut table_info = write_txn.open_table(TX_INFO_TABLE)?;
            let mut table_bytes = write_txn.open_table(TX_BYTES_TABLE)?;

            for file_tx in table_file_txs.get(file_id.as_u128())? {
                let file_tx = file_tx?;
                let tx_id = file_tx.value();

                let _ = table_info.remove(tx_id)?;
                let _ = table_bytes.remove(tx_id)?;
            }
            let _ = table_files.remove(file_id.as_u128())?;
            let _ = table_file_txs.remove_all(file_id.as_u128())?;
        }
        write_txn.commit()?;

        Ok(())
    }


    async fn add_tx(&self, file_id: &Uuid, transaction: &Transaction) -> Result<(), LocalStateStoreError> {
        let next_order: u64;

        let write_txn = self.db.begin_write()?;
        {
            let table_files = write_txn.open_table(FILES_TABLE)?;
            next_order = match table_files.get(file_id.as_u128())? {
                None => 0,
                Some(next_order) => next_order.value()
            };
        }
        {
            let mut table_files = write_txn.open_table(FILES_TABLE)?;
            let _ = table_files.insert(file_id.as_u128(), next_order + 1)?;

            let mut table_file_txs = write_txn.open_multimap_table(FILE_TXS_TABLE)?;
            let _ = table_file_txs.insert(file_id.as_u128(), transaction.get_header_signature())?;

            let mut table_info = write_txn.open_table(TX_INFO_TABLE)?;
            let _ = table_info.insert(transaction.get_header_signature(), (next_order, "", String::from(TransactionStatus::Local).as_str()))?;

            let mut table_bytes = write_txn.open_table(TX_BYTES_TABLE)?;
            let _ = table_bytes.insert(transaction.get_header_signature(), transaction.write_to_bytes().unwrap().as_slice());
        }
        write_txn.commit()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::state::LocalStateStoreError;
    use crate::state_redb::RedbLocalStateStore;
    use crate::tests::test_local_state_store_common;

    #[tokio::test]
    async fn test_local_state_store() -> Result<(), LocalStateStoreError> {
        let store = Box::new(RedbLocalStateStore::new("/tmp/redb-test.db").await?);
        test_local_state_store_common(store).await
    }
}
