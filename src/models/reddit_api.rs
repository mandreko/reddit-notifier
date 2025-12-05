use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_with::{serde_as, TimestampSecondsWithFrac};

/// Reddit API models
#[derive(Debug, Deserialize)]
pub struct RedditListing {
    pub data: RedditListingData,
}

#[derive(Debug, Deserialize)]
pub struct RedditListingData {
    pub children: Vec<RedditChild>,
}

#[derive(Debug, Deserialize)]
pub struct RedditChild {
    pub data: RedditPost,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct RedditPost {
    pub id: String,
    pub title: String,
    pub subreddit: String,
    pub permalink: Option<String>,
    pub url: Option<String>,
    #[serde_as(as = "TimestampSecondsWithFrac<f64>")]
    pub created_utc: DateTime<Utc>,
}
