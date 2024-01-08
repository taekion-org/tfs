use cfg_if::cfg_if;
use crate::debug::debug_println;
use uuid::Uuid;

use crate::state::{LocalStateStore, LocalStateStoreError, TransactionId};
pub async fn test_local_state_store_common(store: Box<dyn LocalStateStore>) -> Result<(), LocalStateStoreError> {
    use libtfslite::types::FileMode;
    use libtfslite::client::payload::{PayloadBuilder, PayloadOperation};
    use libtfslite::client::transaction::TransactionBuilder;
    use libtfslite::client::keys::PrivateKey;

    let key = PrivateKey::generate_random_key();
    let pubkey = key.public_key().unwrap();

    let uuid = Uuid::new_v4();
    let mut tx_ids: Vec<TransactionId> = Vec::new();

    let payload1 = PayloadBuilder::new(PayloadOperation::FileCreate)
        .with_uuid(uuid)
        .with_mode(FileMode::Immutable)
        .build()
        .unwrap();

    let tx1 = TransactionBuilder::new()
        .with_payload(payload1)
        .build(&key)
        .expect("Couldn't build tx1");

    debug_println!("tx1 {}", tx1.get_header_signature());

    tx_ids.push(tx1.get_header_signature().to_string());
    store.add_tx(&uuid, &tx1)
        .await?;

    let payload2 = PayloadBuilder::new(PayloadOperation::FileAppend)
        .with_uuid(uuid)
        .with_block(Vec::new())
        .build()
        .unwrap();

    let tx2 = TransactionBuilder::new()
        .with_payload(payload2)
        .build(&key)
        .expect("Couldn't build tx2");

    debug_println!("tx2 {}", tx2.get_header_signature());
    tx_ids.push(tx2.get_header_signature().to_string());
    store.add_tx(&uuid, &tx2)
        .await?;

    let payload3 = PayloadBuilder::new(PayloadOperation::FileSeal)
        .with_uuid(uuid)
        .build()
        .unwrap();

    let tx3 = TransactionBuilder::new()
        .with_payload(payload3)
        .build(&key)
        .expect("Couldn't build tx3");

    debug_println!("tx3 {}", tx3.get_header_signature());
    tx_ids.push(tx3.get_header_signature().to_string());
    store.add_tx(&uuid, &tx3)
        .await?;

    let pending = store.get_txs(&uuid)
        .await.unwrap();
    for ti in pending {
        debug_println!("{:?}", ti);
        let bytes = store.get_tx_bytes(&ti.tx_id)
            .await?;
        debug_println!("\tsize of tx: {}", bytes.len());
    }

    let files = store.get_files()
        .await?;
    debug_println!("{:?}", files);

    store.flush_txs(&uuid)
        .await?;

    store.get_txs(&uuid)
        .await
        .expect_err("Should be no txs for this uuid");

    Ok(())
}

use crate::client::{FileUpload, TFSLiteClient, TFSLiteClientError};
pub async fn test_client_common() -> Result<(), TFSLiteClientError> {
    use rand::{Rng, thread_rng};
    use libtfslite::client::keys::PrivateKey;

    let private_key = PrivateKey::generate_random_key();
    let public_key = private_key.public_key().unwrap();

    let mut client = TFSLiteClient::new("http://localhost:3455".to_string()).await;
    client.set_account(public_key);

    let build_info = client.get_build_info().await?;
    debug_println!("Build Info: {:?}", build_info);

    let files = client.get_account_files().await?;
    debug_println!("{:?}", files);

    let mut data = [0u8; 131072 + 1024];
    thread_rng()
        .try_fill(&mut data[..]).unwrap();
    let mut upload = {
        cfg_if! {
            if #[cfg(not(target_arch = "wasm32"))] {
                use tokio::io::AsyncWriteExt;

                let mut f = tokio::fs::File::create("/tmp/random").await.unwrap();
                f.write_all(&data[..]).await.unwrap();
                let _ = f.flush().await;

                let upload = client
                    .upload_file(std::path::Path::new("/tmp/random"))
                    .await?;

                upload
            } else if #[cfg(target_arch = "wasm32")] {
                use web_sys::File;

                let js_array = js_sys::Uint8Array::new_with_length(data.len() as u32);
                js_array.copy_from(&data[..]);
                let file = File::new_with_buffer_source_sequence(&js_array, "test_file").unwrap();

                let upload = client
                    .upload_file(file)
                    .await?;

                upload
            }
        }
    };

    upload._set_signer(&private_key);
    upload.set_chunk_size(32768);
    upload.set_filename("test-file");

    upload
        .prepare_transactions()
        .await?;

    upload
        .send_transactions()
        .await?;

    upload.wait_transactions()
        .await?;

    let files = client.get_account_files().await?;
    debug_println!("{:?}", files);

    Ok(())
}

pub fn test_signing_common() {
    use rand::{Rng, thread_rng};
    use libtfslite::client::keys::{PrivateKey, Verifier};

    let key = PrivateKey::generate_random_key();
    let mut data = [0u8; 131072];
    thread_rng()
        .try_fill(&mut data[..]).unwrap();

    let data2 = [0u8; 131072];
    thread_rng()
        .try_fill(&mut data[..]).unwrap();

    let signature = key.sign(data.as_slice()).expect("Signing error!");
    debug_println!("signature {}", signature.as_hex());

    let public_key = key.public_key().expect("Signing error!");

    assert!(public_key.verify(data.as_slice(), &signature).expect("Verification error!"));
    debug_println!("signature passed!");

    assert!(!public_key.verify(data2.as_slice(), &signature).expect("Verification error!"));
    debug_println!("signature did not pass, as expected!");
}
