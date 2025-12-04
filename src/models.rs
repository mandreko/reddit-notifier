use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_with::{serde_as, TimestampSecondsWithFrac};
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub rate_limit_per_minute: u32,
    pub reddit_user_agent: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let database_url =
            std::env::var("DATABASE_URL").context("DATABASE_URL is required (e.g., sqlite://data.db)")?;

        // Rate limit for Reddit API calls (requests per minute)
        // Default: 20 requests/minute (conservative to avoid Reddit's ~60/min limit)
        // Maximum: 50 requests/minute (safety cap to avoid Reddit bans)
        // Reddit's actual limit is ~60/min for unauthenticated requests
        const MAX_RATE_LIMIT: u32 = 50;
        const DEFAULT_RATE_LIMIT: u32 = 20;

        let requested_rate = std::env::var("REDDIT_RATE_LIMIT_PER_MINUTE")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(DEFAULT_RATE_LIMIT);

        let rate_limit_per_minute = if requested_rate > MAX_RATE_LIMIT {
            tracing::warn!(
                "REDDIT_RATE_LIMIT_PER_MINUTE is set to {}, which exceeds the safe maximum of {}. Capping at {} req/min to avoid Reddit API bans.",
                requested_rate,
                MAX_RATE_LIMIT,
                MAX_RATE_LIMIT
            );
            MAX_RATE_LIMIT
        } else {
            requested_rate
        };

        let reddit_user_agent = std::env::var("REDDIT_USER_AGENT")
            .unwrap_or_else(|_| "reddit_notifier (https://github.com/example)".to_string());

        Ok(Self {
            database_url,
            rate_limit_per_minute,
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
}

impl FromStr for EndpointKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "discord" => Ok(Self::Discord),
            "pushover" => Ok(Self::Pushover),
            _ => Err(format!("Unknown endpoint kind: {}", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EndpointRow {
    pub id: i64,
    pub kind: EndpointKind,
    pub config_json: String,
    pub active: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SubscriptionRow {
    pub id: i64,
    pub subreddit: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct NotifiedPostRow {
    pub id: i64,
    pub subreddit: String,
    pub post_id: String,
    pub first_seen_at: String,
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
#[serde_as]
#[derive(Debug, Deserialize)]
pub struct RedditPost {
    pub id: String,
    pub title: String,
    pub subreddit: String,
    pub permalink: Option<String>,
    pub url: Option<String>,
    #[serde_as(as = "TimestampSecondsWithFrac<f64>")]
    pub created_utc: DateTime<Utc>
}
