use anyhow::{anyhow, Error};
use base64ct::{Base64, Encoding};
use js_sys::JsString;
use log::warn;
use serde::{Deserialize, Serialize};

use gloo_storage::{LocalStorage, Storage};
use indexed_db_futures::prelude::*;
use wasm_bindgen::{JsCast, JsValue};

use crate::crypto::KeyData;

const AWS_CREDS_KEY_NAME: &'static str = "aws_credentials";
const IDB_DB_NAME: &'static str = "aws_worker";
const IDB_STORE_NAME: &'static str = "aws";

// this should only be stored in memory,
// so we intentionally do not impl Serialize
#[derive(Debug, Clone)]
pub struct AwsCreds {
    pub lambda_host: String,
    pub key_id: String,
    pub access_key: String,
}

impl AwsCreds {
    pub(crate) fn create_and_save(
        key_data: &KeyData,
        lambda_host: String,
        key_id: String,
        access_key: String,
    ) -> Result<Self, Error> {
        let aws_creds = AwsCreds {
            lambda_host,
            key_id,
            access_key,
        };
        //aws_creds
        //.try_encrypt(key_data)
        //.map(|encrypted| encrypted.save())?;
        Ok(aws_creds)
    }

    // returns newly allocated encrypted aws data or None if anything went wrong
    pub(crate) fn try_encrypt(&self, key_data: &KeyData) -> Result<AwsCredsEncrypted, Error> {
        let key_id = key_data.encrypt_string(&self.key_id)?;
        let access_key = key_data.encrypt_string(&self.access_key)?;
        let lambda_host = key_data.encrypt_string(&self.lambda_host)?;
        let b64salt = Base64::encode_string(&key_data.salt);
        Ok(AwsCredsEncrypted {
            salt: b64salt,
            lambda_host,
            key_id,
            access_key,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AwsCredsEncrypted {
    pub salt: String,
    lambda_host: String,
    key_id: String,
    access_key: String,
}

impl AwsCredsEncrypted {
    pub fn load_local_storage_blob() -> Option<Vec<u8>> {
        let maybe_creds: Result<String, _> = LocalStorage::get(AWS_CREDS_KEY_NAME);
        if let Ok(creds) = maybe_creds {
            let decoded = Base64::decode_vec(&creds).expect("decoding base64");
            // WARNING: do not print creds in production
            //warn!("decoded creds: {}", String::from_utf8_lossy(&decoded));
            Some(decoded)
        } else {
            None
        }
    }

    pub fn save_local_storage_blob(data: &[u8]) {
        warn!("encoding creds: {}", String::from_utf8_lossy(&data));
        let b64data = Base64::encode_string(data);
        //let data = serde_json::to_vec(&self).expect("serializing encrypted creds");
        LocalStorage::set(AWS_CREDS_KEY_NAME, b64data).unwrap();
    }

    //pub fn load() -> Option<Self> {
    //let maybe_creds: Result<Self, _> = LocalStorage::get(AWS_CREDS_KEY_NAME);
    //maybe_creds.ok()
    //}

    //pub fn save(&self) {
    //let data = serde_json::to_vec(&self).expect();
    //LocalStorage::set(AWS_CREDS_KEY_NAME, self).unwrap();
    //}

    //pub(crate) fn decrypt_from_slice(data: &[u8], key_data: &KeyData) -> Result<AwsCreds, Error> {
    //let encrypted: Self = serde_json::from_slice(data)?;
    //encrypted.try_decrypt(key_data)
    //}

    // returns newly allocated decrypted aws data or None if anything went wrong
    pub(crate) fn try_decrypt(&self, key_data: &KeyData) -> Result<AwsCreds, Error> {
        let key_id = key_data.decrypt_string(&self.key_id)?;
        let access_key = key_data.decrypt_string(&self.access_key)?;
        let lambda_host = key_data.decrypt_string(&self.lambda_host)?;
        Ok(AwsCreds {
            key_id,
            access_key,
            lambda_host,
        })
    }
}

pub(crate) async fn save_indexed_db_blob(data: String) -> Result<(), Error> {
    // Open database
    let mut db_req: OpenDbRequest = IdbDatabase::open(IDB_DB_NAME)
        .map_err(|e| anyhow!("creating database open request: {:?}", e))?;
    db_req.set_on_upgrade_needed(Some(|evt: &IdbVersionChangeEvent| -> Result<(), JsValue> {
        // Check if the object store exists; create it if it doesn't
        if let None = evt.db().object_store_names().find(|n| n == IDB_STORE_NAME) {
            // this error will be lifted up
            evt.db().create_object_store(IDB_STORE_NAME)?;
        }
        Ok(())
    }));

    let db: IdbDatabase = db_req
        .into_future()
        .await
        .map_err(|e| anyhow!("opening database: {:?}", e))?;

    // Insert/overwrite a record
    let tx: IdbTransaction = db
        .transaction_on_one_with_mode(IDB_STORE_NAME, IdbTransactionMode::Readwrite)
        .map_err(|e| anyhow!("creating transaction: {:?}", e))?;
    let store: IdbObjectStore = tx
        .object_store(IDB_STORE_NAME)
        .map_err(|e| anyhow!("opening store: {:?}", e))?;

    store
        .put_key_val_owned(AWS_CREDS_KEY_NAME, &JsValue::from_str(&data))
        .map_err(|e| anyhow!("putting data: {:?}", e))?;

    // IDBTransactions can have an Error or an Abort event; into_result() turns both into a
    // DOMException
    tx.await
        .into_result()
        .map_err(|e| anyhow!("completing transaction: {:?}", e))?;

    Ok(())
}

pub(crate) async fn load_indexed_db_blob() -> Result<Option<String>, Error> {
    // Open database
    let mut db_req: OpenDbRequest = IdbDatabase::open(IDB_DB_NAME)
        .map_err(|e| anyhow!("creating database open request: {:?}", e))?;
    db_req.set_on_upgrade_needed(Some(|evt: &IdbVersionChangeEvent| -> Result<(), JsValue> {
        // Check if the object store exists; create it if it doesn't
        if let None = evt.db().object_store_names().find(|n| n == IDB_STORE_NAME) {
            // this error will be lifted up
            evt.db().create_object_store(IDB_STORE_NAME)?;
        }
        Ok(())
    }));

    let db: IdbDatabase = db_req
        .into_future()
        .await
        .map_err(|e| anyhow!("opening database: {:?}", e))?;

    // Get a record
    let tx = db
        .transaction_on_one(IDB_STORE_NAME)
        .map_err(|e| anyhow!("creating transaction: {:?}", e))?;
    let store = tx
        .object_store(IDB_STORE_NAME)
        .map_err(|e| anyhow!("opening store: {:?}", e))?;

    let value: Option<JsValue> = store
        .get_owned(IDB_STORE_NAME)
        .map_err(|e| anyhow!("acquiring value: {:?}", e))?
        .await
        .map_err(|e| anyhow!("acquiring value after await: {:?}", e))?;

    if let Some(value) = &value {
        if !value.is_string() {
            return Err(anyhow!("returned value is not string"));
        }
    }

    Ok(value.map(|v| v.as_string().unwrap()))
}
