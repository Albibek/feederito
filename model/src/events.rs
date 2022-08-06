use anyhow::Error;
use serde::{Deserialize, Serialize};
use serde_json::from_slice;

use crate::entry::*;
use crate::feed::*;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Event {
    RefreshFeeds(RefreshFeeds),
    AllEntries(AllEntries),
    AllFeeds(()),
    AddFeed(String),
    MarkReadUnread(Vec<(EntryID, u64, bool)>),
    FixDatabase(()),
    #[serde(skip)]
    // this one is never created over deserialization, only created by hands
    Bad(Vec<u8>, String),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshFeeds {
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllEntries {
    #[serde(default)]
    pub unread_only: bool,

    #[serde(default)]
    pub feed_id: Option<FeedID>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    AllEntries(Vec<StoredEntry>),
    AllFeeds(Vec<StoredFeed>),
    MarkedRead(Vec<(EntryID, u64, i64)>),
    Ok(OkResponse),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OkResponse {
    message: String,
}

impl OkResponse {
    pub fn new<S: Into<String>>(s: S) -> Self {
        Self { message: s.into() }
    }
}

impl Response {
    pub fn new_ok<S: Into<String>>(s: S) -> Self {
        Response::Ok(OkResponse::new(s))
    }
}

impl<'a> TryFrom<&'a [u8]> for Event {
    type Error = Error;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        Ok(from_slice(value)?)
    }
}
