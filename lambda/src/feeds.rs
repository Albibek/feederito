use std::collections::HashMap;
use tokio::task::spawn;

use anyhow::{anyhow, Error};
use tracing::{debug, warn};

use aws_sdk_dynamodb::{
    model::AttributeAction, model::AttributeValue, model::AttributeValueUpdate, model::ReturnValue,
    types::SdkError, Client,
};
use aws_types::sdk_config::SdkConfig;

use serde_dynamo::{from_items, to_attribute_value, to_item};

use rss::Channel;

use hyper::{body::Buf, body::Bytes, client::Client as HttpClient, Body, Uri};
use hyper_rustls::HttpsConnectorBuilder;

use model::entry::StoredEntry;
use model::events::*;
use model::feed::*;
use model::util::*;

pub async fn all_feeds(_: ()) -> Result<Response, Error> {
    let feeds = get_all_feeds().await?;
    Ok(Response::AllFeeds(feeds))
}

pub async fn get_all_feeds() -> Result<Vec<StoredFeed>, Error> {
    let shared_config = aws_config::load_from_env().await;
    let client = Client::new(&shared_config);
    let result = client.scan().table_name("albibek-rss-feeds").send().await?;
    if let Some(items) = result.items {
        Ok(from_items(items)?)
    } else {
        Ok(Vec::new())
    }
}

pub async fn add_feed(feed_url: String) -> Result<Response, Error> {
    let config = aws_config::load_from_env().await;
    let result = fetch_feed(&feed_url).await?;
    let rss_reader = result.clone().reader();
    //let mut entries = HashMap::new();

    let now = chrono::Utc::now();
    let mut feed = StoredFeed {
        feed_id: FeedID(hash(feed_url.as_bytes())),
        last_update: now.timestamp_millis() as u64,
        url: feed_url.clone(),
        title: None,
        description: None,
        ext_last_update: None,
    };
    let ok;
    match rss::Channel::read_from(rss_reader) {
        Ok(channel) => {
            ok = true;
            feed.update_from_rss_channel(channel).map_err(|e| {
                warn!(error = ?e, "could not update feed from rss");
                e
            })?;
        }
        Err(e) => {
            warn!(error = ?e, "could not update feed from rss");
            ok = false;
            //return Err(e.into());
        }
    }

    if ok {
        let dynamo = Client::new(&config);
        let item: HashMap<String, AttributeValue> = to_item(&feed)?;

        let resp = dynamo
            .put_item()
            .table_name("albibek-rss-feeds")
            .set_item(Some(to_item(&feed)?))
            //.return_consumed_capacity(aws_sdk_dynamodb::model::ReturnConsumedCapacity::Total)
            //.return_values(aws_sdk_dynamodb::model::ReturnValue::AllOld)
            .send()
            .await
            .map_err(|e| {
                warn!(error = ?e, "error putting feed");
                e
            })?;

        debug!(?resp, ?item, "put feed response");
    }

    debug!(?ok, "fetch ok");
    Ok(Response::Ok(OkResponse::new(format!(
        "added  {} feed",
        feed_url
    ))))
}

pub async fn refresh_feeds(request: RefreshFeeds) -> Result<Response, Error> {
    let shared_config = aws_config::load_from_env().await;
    let feeds = get_all_feeds().await?;
    let len = feeds.len();
    let tasks = tokio::task::LocalSet::new();
    for feed in feeds {
        if !request.force {
            // TODO don't update feed
        }
        let url = feed.url.clone();
        let shared_config = shared_config.clone();
        let handle = tokio::task::spawn(async move {
            feed_worker(shared_config, feed)
                .await
                .map_err(|e| warn!("error fetching {:?}: {:?}", url, e))
                .unwrap_or(());
        });
        tasks.spawn_local(handle);
    }
    tasks.await;
    Ok(Response::Ok(OkResponse::new(format!(
        "updated {} feeds",
        len
    ))))
}

pub async fn feed_worker(config: SdkConfig, feed: StoredFeed) -> Result<(), Error> {
    // TODO: set feed's last_update
    let result = fetch_feed(&feed.url).await?;

    let rss_reader = result.clone().reader();
    let mut new_feed = feed.clone();
    //let mut entries = HashMap::new();

    if let Ok(mut channel) = rss::Channel::read_from(rss_reader) {
        debug!("{:?}", channel);

        let mut tasks = Vec::new();
        while let Some(item) = channel.items.pop() {
            let eitem = item.clone();
            match StoredEntry::from_rss_item(feed.feed_id, item) {
                Ok(entry) => {
                    let expression_values = HashMap::from([(
                        ":published".to_string(),
                        to_attribute_value(&entry.published)?,
                    )]);
                    let dynamo_item = to_item(entry)?;
                    let config = config.clone();
                    let handle = spawn(async move {
                        let dynamo = Client::new(&config);
                        debug!(item = ?dynamo_item, "putting item");
                        dynamo
                            .put_item()
                            .table_name("albibek-rss-entries")
                            .set_item(Some(dynamo_item))
                            .set_condition_expression(Some(
                                "attribute_not_exists(entryId) OR published <> :published".into(),
                            ))
                            .set_expression_attribute_values(Some(expression_values))
                            .set_return_values(Some(ReturnValue::None))
                            .send()
                            .await
                            .map(|response| {
                                debug!(response = ?response, "item written");
                            })
                            .unwrap_or_else(|e| {
                                if let SdkError::ServiceError { ref err, .. } = e {
                                    if err.is_conditional_check_failed_exception() {
                                        return ();
                                    }
                                }
                                warn!(error = ?e, "error writing item");
                            });
                    });
                    tasks.push(handle);
                }
                Err(e) => {
                    warn!(error = ?e, item = ?eitem, "error processing item");
                    continue;
                }
            }
        }
        while let Some(task) = tasks.pop() {
            task.await
                .unwrap_or_else(|e| warn!(error = ?e, "error updating feed"));
        }
        new_feed.update_from_rss_channel(channel)?;

        //debug!("{:?}", entries);
    } else {
        let atom_reader = result.reader();
        if let Ok(atom) = atom_syndication::Feed::read_from(atom_reader) {
            new_feed.update_from_atom_feed(atom)?
        } else {
            return Err(anyhow!("bad feed"));
        }
    };

    let now = chrono::Utc::now();
    //if feed.last_update == 0 ||
    let dynamo = Client::new(&config);
    new_feed.last_update = now.timestamp_millis() as u64;
    debug!(feed = ?new_feed, "put");
    let put_response = dynamo
        .put_item()
        .table_name("albibek-rss-feeds")
        .set_item(Some(to_item(&new_feed)?))
        //.set_condition_expression(Some("attribute_not_exists(feedId)".into()))
        //.expression_attribute_values(
        //"lastUpdate",
        //AttributeValue::N(format!("{}", feed.last_update)),
        //)
        //.attribute_updates(
        //"lastUpdate",
        //AttributeValueUpdate::builder()
        //.value(AttributeValue::N(format!("{}", now.timestamp_millis())))
        //.action(AttributeAction::Put)
        //.build(),
        //)
        .send()
        .await
        .map_err(|e| {
            warn!(error = ?e, "error updating feed");
            e
        })?;

    debug!(?put_response, "put feed response");
    Ok(())
}

pub async fn fetch_feed(url: &str) -> Result<Bytes, Error> {
    let url = url.parse::<Uri>()?;
    debug!("fetching {:?}", url);
    let https = HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_or_http()
        .enable_http1()
        .build();

    let client: HttpClient<_, hyper::Body> = HttpClient::builder().build(https);

    let res = client.get(url).await?;
    if res.status() != 200 {
        debug!("error fetching {:?}", res);
        return Err(anyhow!("non-200 response"));
    }
    Ok(hyper::body::to_bytes(res).await?)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::init_tracing;
    use model::events::*;

    #[tokio::test]
    async fn test_add_feed() {
        init_tracing(true).unwrap();
        let event = "https://www.opennet.ru/opennews/opennews_all_utf.rss".to_string();
        add_feed(event).await.unwrap();
        let response = all_feeds(()).await.unwrap();
        debug!(?response, "all_feeds");
    }

    #[tokio::test]
    async fn test_refresh_feeds() {
        init_tracing(true).unwrap();
        let event = RefreshFeeds { force: true };
        let response = refresh_feeds(event).await.unwrap();
        debug!(?response, "all_feeds");
    }
}
