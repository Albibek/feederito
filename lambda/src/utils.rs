use aws_sdk_dynamodb::model::AttributeValue;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use model::events::Event;

use anyhow::Error;

use model::events::*;
use tracing::warn;

/// events may come wrapped, for example when called using lambda function URL
/// this is a parser, that unifies them down to our common event
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum WrappedEvent {
    UrlWrapped(LambdaUrlRequest),
    Just(Event),
    Unknown(Value),
}

impl WrappedEvent {
    pub fn into_event(self) -> Event {
        match self {
            WrappedEvent::UrlWrapped(req) => match req.into_event() {
                Ok(event) => event,
                Err(e) => Event::Bad(req.body.into(), format!("{:?}", e).into()),
            },
            WrappedEvent::Just(event) => event,
            WrappedEvent::Unknown(value) => match serde_json::to_vec(&value) {
                Ok(data) => Event::Bad(data, "unknown event".into()),
                Err(e) => Event::Bad(Vec::new(), format!("{:?}", e).into()),
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LambdaUrlRequest {
    body: String,
    is_base64_encoded: bool,
}

impl LambdaUrlRequest {
    pub fn into_event(&self) -> Result<Event, Error> {
        self.body.as_bytes().try_into()
    }
}

pub async fn fix_database(_: ()) -> Result<Response, Error> {
    Ok(Response::new_ok("no fixes requred at the moment"))
    //use aws_sdk_dynamodb::{model::AttributeAction, model::AttributeValueUpdate, Client};
    //let shared_config = aws_config::load_from_env().await;
    //let client = Client::new(&shared_config);
    //let result = client
    //.scan()
    //.table_name("albibek-rss-entries")
    //////.filter_expression("attribute_not_exists(feedId)")
    ////.filter_expression("attribute_exists(backendOnly)")
    //.projection_expression("entryId, published, #r")
    //.expression_attribute_names("#r", "read")
    ////.select(aws_sdk_dynamodb::model::Select::AllProjectedAttributes)
    ////.limit(3)
    //.send()
    //.await
    //.map_err(|e| {
    //warn!(error = ?e, "error querying entries in fixDatabase");
    //e
    //})?;

    //let now = chrono::Utc::now();

    //let now = now.timestamp_millis().to_string();
    //warn!(result = ?&result, "fix result");
    //if let Some(mut entries) = result.items {
    //while let Some(entry) = entries.pop() {
    //warn!("entry: {:?}", entry);
    //let result = client
    //.update_item()
    //.table_name("albibek-rss-entries")
    //.key("entryId", entry.get("entryId").map(|v| v.clone()).unwrap())
    //.key(
    //"published",
    //entry.get("published").map(|v| v.clone()).unwrap(),
    //)
    //.attribute_updates(
    //"read",
    //AttributeValueUpdate::builder()
    //////.value(AttributeValue::N("1".to_string()))
    //.action(AttributeAction::Delete)
    //.build(),
    //)
    ////.expression_attribute_names("#r", "read")
    //.send()
    //.await;

    //warn!("update result for {:?}: {:?}", entry, result);

    ////let new_read = if let AttributeValue::Bool(v) =
    ////entry.get("read").unwrap_or(&AttributeValue::Bool(false))
    ////{
    ////if *v {
    ////now.clone()
    ////} else {
    ////"0".to_string()
    ////}
    ////} else {
    ////"0".to_string()
    //////panic!("WTF");
    ////};
    ////let result = client
    ////.update_item()
    ////.table_name("albibek-rss-entries")
    ////.key("entryId", entry.get("entryId").map(|v| v.clone()).unwrap())
    ////.key(
    ////"published",
    ////entry.get("published").map(|v| v.clone()).unwrap(),
    ////)
    ////.attribute_updates(
    ////"readTs",
    ////AttributeValueUpdate::builder()
    ////.value(AttributeValue::N(new_read))
    ////.action(AttributeAction::Put)
    ////.build(),
    ////)
    ////.send()
    ////.await;

    ////warn!("update result for {:?}: {:?}", entry, result);
    //}
    //}

    //Ok(Response::new_ok("table fixed"))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::init_tracing;
    use tracing::debug;

    #[tokio::test]
    async fn test_fix_database() {
        init_tracing(true).unwrap();
        let response = fix_database(()).await.unwrap();
        debug!(?response, "fix_database");
    }
}
