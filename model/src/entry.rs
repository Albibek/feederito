use anyhow::{anyhow, Context as _, Error};
use serde::{Deserialize, Serialize};

use crate::feed::FeedID;

#[cfg(feature = "backend")]
use atom_syndication::Entry as AtomEntry;

#[cfg(feature = "backend")]
use rss::{Guid, Item};

#[cfg(feature = "backend")]
use crate::util::hash;

#[derive(Debug, PartialEq, Eq, Clone, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntryID(pub u64);

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StoredEntry {
    // internal properties of database
    pub entry_id: EntryID,
    pub feed_id: FeedID,

    pub published: u64,
    // id provided with feed
    pub link: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub content: Option<String>,
    pub read_ts: i64,
}

#[cfg(feature = "backend")]
impl StoredEntry {
    pub fn from_rss_item(feed_id: FeedID, item: Item) -> Result<Self, Error> {
        let Item {
            title,
            description,
            content,
            guid,
            link,
            pub_date,
            ..
        } = item;

        let entry_id = if let Some(guid) = guid {
            hash(guid.value.as_bytes())
        } else {
            if let Some(link) = &link {
                hash(link.as_bytes())
            } else {
                return Err(anyhow!("link is a required field"));
            }
        };
        let published = if let Some(pub_date) = pub_date {
            let date = chrono::DateTime::parse_from_rfc2822(&pub_date)?;
            date.timestamp() as u64
        } else {
            return Err(anyhow!("unknown publish date is not supported"));
        };
        Ok(Self {
            entry_id: EntryID(entry_id),
            feed_id,
            title,
            description,
            content,
            link,
            published,
            // entries are unread by default
            read_ts: 0,
        })
    }

    pub fn from_atom_feed_entry(entry: AtomEntry) -> Result<(), Error> {
        Err(anyhow!("atom feed not implemented yet"))
    }
}
