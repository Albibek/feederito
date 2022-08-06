mod entries;
mod feeds;
mod utils;

use anyhow::Error;
use lambda_runtime::{service_fn, LambdaEvent};

use tracing::{debug, warn};

use crate::entries::*;
use crate::feeds::*;
use crate::utils::*;
use model::events::*;

pub fn init_tracing(test: bool) -> Result<(), Error> {
    std::env::set_var("RUST_LOG", "off");
    let level = if test {
        tracing::Level::TRACE
    } else if !std::env::var("LAMBDA_DEBUG_LOG")
        .unwrap_or_default()
        .is_empty()
    {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    let builder = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(level)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("bootstrap=trace".parse()?),
        )
        .with_target(false);

    if test {
        builder.try_init().unwrap_or(());
    } else {
        builder
            .try_init()
            .unwrap_or_else(|e| warn!(error = ?e, "log already initialized???"));
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    eprintln!("main started");
    init_tracing(false)?;

    let func = service_fn(route_lambda);

    if std::env::var("DEBUG").unwrap_or_default().is_empty() {
        debug!("starting lambda runtime");
        lambda_runtime::run(func).await.map_err(|e| {
            warn!(error = ?e, "error running lambda runtime");
            e
        })?;
    } else {
        debug!("falling down to debug");

        let event = WrappedEvent::Just(Event::FixDatabase(()));
        let result = route(event).await?;
        debug!("{:?}", result);
    }
    Ok(())
}

async fn route_lambda(inevent: LambdaEvent<WrappedEvent>) -> Result<Response, Error> {
    route(inevent.payload).await
}

async fn route(inevent: WrappedEvent) -> Result<Response, Error> {
    debug!(event=?inevent, "event incoming");
    let decoded = inevent.into_event();
    debug!(event=?&decoded, "decoded to");
    let response = match decoded {
        Event::RefreshFeeds(ev) => refresh_feeds(ev).await?,
        Event::AllFeeds(ev) => all_feeds(ev).await?,
        Event::AllEntries(ev) => all_entries(ev).await?,
        Event::MarkReadUnread(ev) => mark_read_unread(ev).await?,
        Event::AddFeed(ev) => add_feed(ev).await?,
        Event::FixDatabase(ev) => fix_database(ev).await?,
        Event::Bad(input, err_string) => {
            let in_string = String::from_utf8_lossy(&input);
            warn!(event = ?&in_string, error = ?&err_string, "could not deserialize input event");
            Response::Ok(OkResponse::new(format!(
                "WTF: {} {}",
                in_string, err_string
            )))
        }
    };
    Ok(response)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn event_deserialize_wrapped() {
        init_tracing(true).unwrap();
        let s = "{\"body\":\"{\\\"allEntries\\\":{\\\"unreadOnly\\\":false}}\",\"headers\":{\"accept\":\"*/*\",\"accept-encoding\":\"gzip, deflate, br\",\"accept-language\":\"ru,en-US;q=0.9,en;q=0.8\",\"content-type\":\"application/json\",\"host\":\"redacted.lambda-url.eu-west-1.on.aws\",\"origin\":\"http://localhost:8000\",\"referer\":\"http://localhost:8000/\",\"sec-ch-ua\":\"\\\" Not A;Brand\\\";v=\\\"99\\\", \\\"Chromium\\\";v=\\\"100\\\"\",\"sec-ch-ua-mobile\":\"?0\",\"sec-ch-ua-platform\":\"\\\"Linux\\\"\",\"sec-fetch-dest\":\"empty\",\"sec-fetch-mode\":\"cors\",\"sec-fetch-site\":\"cross-site\",\"user-agent\":\"Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/100.0.4896.127 Safari/537.36\",\"x-amz-date\":\"20220417T091935Z\",\"x-amzn-trace-id\":\"Root=1-625bdba8-1cd5aaf11fbf728b3507126d\",\"x-forwarded-for\":\"52.201.234.161\",\"x-forwarded-port\":\"443\",\"x-forwarded-proto\":\"https\"},\"isBase64Encoded\":false,\"rawPath\":\"/\",\"rawQueryString\":\"\",\"requestContext\":{\"accountId\":\"123456789012\",\"apiId\":\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\",\"authorizer\":{\"iam\":{\"accessKey\":\"REDACTEDREDACTEDDDDD\",\"accountId\":\"012345678900\",\"callerId\":\"REDACTED2341234\",\"cognitoIdentity\":null,\"principalOrgId\":null,\"userArn\":\"arn:aws:iam::123456789012:user/xxx-external-user\",\"userId\":\"REDACTED0123401234034\"}},\"domainName\":\"redacted.lambda-url.eu-west-1.on.aws\",\"domainPrefix\":\"redacted\",\"http\":{\"method\":\"POST\",\"path\":\"/\",\"protocol\":\"HTTP/1.1\",\"sourceIp\":\"52.201.234.161\",\"userAgent\":\"Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/100.0.4896.127 Safari/537.36\"},\"requestId\":\"214f46c4-88b0-4105-8304-da27da3c2cbb\",\"routeKey\":\"$default\",\"stage\":\"$default\",\"time\":\"17/Apr/2022:09:19:36 +0000\",\"timeEpoch\":1650187176480},\"routeKey\":\"$default\",\"version\":\"2.0\"}";
        let e: WrappedEvent = serde_json::from_str(s).unwrap();
        debug!("{:?}", e);
        debug!("{:?}", e.into_event());
    }

    #[test]
    fn event_deserialize() {
        init_tracing(true).unwrap();
        let s = "{\"allEntries\":{\"unreadOnly\":false}}";
        let e: WrappedEvent = serde_json::from_str(s).unwrap();
        debug!("{:?}", e);
        debug!("{:?}", e.into_event());
    }
}
