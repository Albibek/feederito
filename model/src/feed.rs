use anyhow::{anyhow, Error};
use serde::{Deserialize, Serialize};

#[cfg(feature = "backend")]
use atom_syndication::Feed as AtomFeed;

#[cfg(feature = "backend")]
use rss::Channel;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone, Copy)]
#[serde(transparent)]
pub struct FeedID(pub u64);

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StoredFeed {
    // internal properties of database
    pub feed_id: FeedID,
    // when we tried to update the feed last time
    pub last_update: u64,
    pub url: String,
    // id provided with feed
    //ext_id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    // last update time by version of feed authors
    pub ext_last_update: Option<u64>,
}

#[cfg(feature = "backend")]
impl StoredFeed {
    pub fn update_from_rss_channel(&mut self, channel: Channel) -> Result<(), Error> {
        let Channel {
            title, description, ..
        } = channel;
        self.title = Some(title);
        self.description = Some(description);
        // TODO: join last_build_date + pub_date + ttl + syndication.*
        // TODO: deal with skip_hours, skip_days
        //self.ext_last_update = I/
        Ok(())
    }

    pub fn update_from_atom_feed(&mut self, feed: AtomFeed) -> Result<(), Error> {
        Err(anyhow!("atom feed not implemented yet"))
    }
}

#[cfg(feature = "backend")]
impl FeedID {
    pub fn from_url(url: String) -> u64 {
        use crate::util::hash;
        hash(url.as_bytes())
    }
}
