use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub poll_interval_secs: u64,
    pub reddit_user_agent: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let database_url =
            std::env::var("DATABASE_URL").context("DATABASE_URL is required (e.g., sqlite://data.db)")?;
        let poll_interval_secs = std::env::var("POLL_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(60);
        let reddit_user_agent = std::env::var("REDDIT_USER_AGENT")
            .unwrap_or_else(|_| "reddit_notifier (https://github.com/example)".to_string());

        Ok(Self {
            database_url,
            poll_interval_secs,
            reddit_user_agent,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointKind {
    Discord,
    Pushover,
}

impl EndpointKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Discord => "discord",
            Self::Pushover => "pushover",
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "discord" => Some(Self::Discord),
            "pushover" => Some(Self::Pushover),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EndpointRow {
    pub id: i64,
    pub kind: EndpointKind,
    pub config_json: String,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordConfig {
    pub webhook_url: String,
    #[serde(default)]
    pub username: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PushoverConfig {
    pub token: String,
    pub user: String,
    #[serde(default)]
    pub device: Option<String>,
}

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

#[derive(Debug, Deserialize)]
pub struct RedditPost {
    pub id: String,
    pub title: String,
    pub permalink: Option<String>,
    pub url: Option<String>,
}
