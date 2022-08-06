use aws_sdk_dynamodb::model::AttributeAction;
use aws_sdk_dynamodb::model::AttributeValue;
use aws_sdk_dynamodb::model::AttributeValueUpdate;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::spawn;

use anyhow::Error;
use tracing::{debug, warn};

use aws_sdk_dynamodb::{model::KeysAndAttributes, Client};
use serde_dynamo::{from_items, to_attribute_value, to_item};

use model::entry::*;
use model::events::*;

pub async fn all_entries(request: AllEntries) -> Result<Response, Error> {
    let shared_config = aws_config::load_from_env().await;
    let client = Client::new(&shared_config);
    let result = if request.unread_only {
        let mut query = client
            .query()
            .table_name("albibek-rss-entries")
            .index_name("entry-read-status")
            .key_condition_expression("readTs = :read");

        query = query
            //.filter_expression("#r = :read")
            .expression_attribute_values(":read", to_attribute_value(&0u64)?);
        query
            .send()
            .await
            .map_err(|e| {
                warn!(error = ?e, unread_only=?request.unread_only, "querying entries from index");
                e
            })?
            .items
    } else {
        // for all entries request we just scan the entries table to retrieve all of them
        let query = client
            .scan()
            .table_name("albibek-rss-entries")
            .index_name("entry-read-status");
        query
            .send()
            .await
            .map_err(|e| {
                warn!(error = ?e, unread_only=?request.unread_only, "scanning entries in index");
                e
            })?
            .items
    };

    let mut indexed_entries = if let Some(items) = result {
        debug!("got {} items", items.len());
        items
    } else {
        return Ok(Response::AllEntries(Vec::new()));
    };

    for entry in &mut indexed_entries {
        // each entry is already HashMap<String, AttributeValue> and is exactly what we want
        // to pass to batchGet as entry keys, but with additional attributes
        entry.retain(|k, _| k.as_str() == "entryId" || k.as_str() == "published");
    }

    let handlers = tokio::task::LocalSet::new();
    let all_entries = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    for chunk in indexed_entries.as_slice().chunks(100) {
        let keys_attrs = KeysAndAttributes::builder()
            .set_keys(Some(Vec::from(chunk)))
            .build();
        let client = Client::new(&shared_config);
        let query = client
            .batch_get_item()
            .set_request_items(Some(HashMap::from([(
                "albibek-rss-entries".to_string(),
                keys_attrs,
            )])));
        let in_all_entries = all_entries.clone();
        debug!("spawning");
        let handler = spawn(async move {
            debug!("start query");
            let result = query
                .send()
                .await
                .map_err(|e| warn!(error=?e, "querying unread entries from main table"));
            debug!(?result);
            let result = if let Ok(result) = result {
                if let Some(mut result) = result.responses {
                    if let Some(result) = result.remove("albibek-rss-entries") {
                        result
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            } else {
                return;
            };

            let mut all_entries = in_all_entries.lock().await;

            debug!("lock taken");
            all_entries.extend(result);
        });
        handlers.spawn_local(handler);
    }
    handlers.await;

    debug!("handlers ready");
    let all_entries = Arc::try_unwrap(all_entries).unwrap().into_inner();
    let entries: Vec<StoredEntry> = from_items(all_entries).map_err(|e| {
        warn!(error = ?e, "error converting entries");
        e
    })?;

    debug!(?entries);
    Ok(Response::AllEntries(entries))
}

pub async fn mark_read_unread(entries: Vec<(EntryID, u64, bool)>) -> Result<Response, Error> {
    let shared_config = aws_config::load_from_env().await;
    let client = Client::new(&shared_config);
    let now = chrono::Utc::now();
    let now_ts = now.timestamp_millis();
    let now = now.timestamp_millis().to_string();
    let mut response = Vec::new();
    for (entry_id, published, is_read) in entries {
        let read_ts = if is_read {
            AttributeValue::N(now.clone())
        } else {
            AttributeValue::N(0.to_string())
        };
        // TODO: batching and parallel requests
        client
            .update_item()
            .table_name("albibek-rss-entries")
            .key("entryId", AttributeValue::N(format!("{}", entry_id.0)))
            .key("published", AttributeValue::N(format!("{}", published)))
            .attribute_updates(
                "readTs",
                AttributeValueUpdate::builder()
                    .value(read_ts)
                    .action(AttributeAction::Put)
                    .build(),
            )
            .send()
            .await
            .map_err(|e| {
                warn!(error = ?e, id = ?entry_id.0, "updating entry");
            })
            .map(|_| ())
            .unwrap_or(());
        response.push((entry_id, published, if is_read { now_ts } else { 0 }));
    }

    Ok(Response::MarkedRead(response))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::init_tracing;

    #[tokio::test]
    async fn test_all_entries() {
        init_tracing(true).unwrap();
        let event = AllEntries {
            unread_only: true,
            feed_id: None,
        };
        let response = all_entries(event).await.unwrap();
        debug!(?response, "all_feeds");
    }
}
