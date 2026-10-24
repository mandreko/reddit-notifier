//! Pluggable backends for discovering new Reddit posts.
//!
//! Reddit is retiring every free data surface (RSS on 2026-11-13, the public Data API
//! in March 2027, and unauthenticated `.json` listings already 403 from many IPs).
//! The poller therefore talks to a [`PostSource`] trait object so backends can be
//! swapped by configuration. See [`arctic_shift`] (default) and [`reddit_json`].

use async_trait::async_trait;
use reqwest::header::HeaderMap;
use reqwest::{Client, StatusCode};
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::models::{AppConfig, PostSourceKind, RedditPost};

pub mod arctic_shift;
pub mod reddit_json;

/// Base used to build user-facing links to posts, regardless of source.
pub const REDDIT_WEB_BASE: &str = "https://www.reddit.com";

/// Fallback wait when a 429 carries no usable timing header.
const DEFAULT_RETRY_AFTER: Duration = Duration::from_secs(60);
const MIN_RETRY_AFTER: Duration = Duration::from_secs(1);
const MAX_RETRY_AFTER: Duration = Duration::from_secs(300);
/// How much of a response body to keep in error messages.
const MAX_BODY_IN_ERROR: usize = 200;

/// Errors a [`PostSource`] can produce for a single batch.
#[derive(Debug)]
pub enum FetchError {
    /// The upstream asked us to slow down (HTTP 429).
    RateLimited { retry_after: Duration },
    /// A non-success HTTP status without a structured API error.
    Status { status: StatusCode, body: String },
    /// The API returned a structured error message.
    Api(String),
    /// Connection, timeout, or body-read failure.
    Transport(reqwest::Error),
    /// The body could not be deserialized.
    Parse(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::RateLimited { retry_after } => {
                write!(f, "rate limited; retry after {}s", retry_after.as_secs())
            }
            FetchError::Status { status, body } => write!(f, "HTTP {}: {}", status, body),
            FetchError::Api(msg) => write!(f, "API error: {}", msg),
            FetchError::Transport(e) => write!(f, "transport error: {}", e),
            FetchError::Parse(msg) => write!(f, "parse error: {}", msg),
        }
    }
}

impl std::error::Error for FetchError {}

impl From<reqwest::Error> for FetchError {
    fn from(e: reqwest::Error) -> Self {
        FetchError::Transport(e)
    }
}

/// A backend that can list recent posts for a batch of subreddits.
#[async_trait]
pub trait PostSource: Send + Sync {
    /// Short identifier used in logs and config (`arctic_shift`, `reddit_json`).
    fn name(&self) -> &'static str;

    /// Maximum number of subreddits a single `fetch_new` call may receive.
    fn batch_size(&self) -> usize;

    /// Fetch recent posts for `subreddits` (at most `batch_size()` entries).
    ///
    /// Implementations return every recent post they know about; the poller owns
    /// deduplication and time-window filtering.
    async fn fetch_new(&self, subreddits: &[String]) -> Result<Vec<RedditPost>, FetchError>;
}

/// Construct the configured source.
pub fn build_post_source(cfg: &AppConfig, client: Client) -> Box<dyn PostSource> {
    match cfg.post_source {
        PostSourceKind::ArcticShift => Box::new(arctic_shift::ArcticShiftSource::new(
            client,
            cfg.arctic_shift_base_url.clone(),
        )),
        PostSourceKind::RedditJson => {
            Box::new(reddit_json::RedditJsonSource::new(client, REDDIT_WEB_BASE))
        }
    }
}

/// Build the link included in notifications.
///
/// Prefers Reddit's `permalink` (always present in Reddit's own JSON). Falls back to a
/// constructed comments URL: Arctic Shift cannot return `permalink` when selecting
/// fields, and a post's `url` points off-site for link posts, so the constructed
/// thread URL is the right second choice.
pub fn post_url(post: &RedditPost) -> String {
    match post.permalink.as_deref().filter(|p| !p.is_empty()) {
        Some(p) if p.starts_with('/') => format!("{}{}", REDDIT_WEB_BASE, p),
        Some(p) => format!("{}/{}", REDDIT_WEB_BASE, p),
        None => format!("{}/r/{}/comments/{}/", REDDIT_WEB_BASE, post.subreddit, post.id),
    }
}

/// Derive a wait duration from rate-limit headers.
///
/// Precedence: `x-ratelimit-reset` (seconds) → `x-ratelimit-reset-at` (epoch ms) →
/// `retry-after` (seconds) → 60s. Clamped to 1s..=300s so a bogus header can neither
/// spin nor stall the poller.
pub(crate) fn retry_after_from_headers(headers: &HeaderMap, now: SystemTime) -> Duration {
    let header_u64 = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<f64>().ok())
            .filter(|n| n.is_finite() && *n >= 0.0)
    };

    let secs = header_u64("x-ratelimit-reset")
        .or_else(|| {
            header_u64("x-ratelimit-reset-at").and_then(|reset_at_ms| {
                let now_ms = now.duration_since(UNIX_EPOCH).ok()?.as_millis() as f64;
                Some(((reset_at_ms - now_ms) / 1000.0).max(0.0))
            })
        })
        .or_else(|| header_u64("retry-after"));

    secs.map(Duration::from_secs_f64)
        .unwrap_or(DEFAULT_RETRY_AFTER)
        .clamp(MIN_RETRY_AFTER, MAX_RETRY_AFTER)
}

/// Shorten a body for inclusion in an error message.
pub(crate) fn truncate_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.chars().count() <= MAX_BODY_IN_ERROR {
        trimmed.to_string()
    } else {
        let head: String = trimmed.chars().take(MAX_BODY_IN_ERROR).collect();
        format!("{}…", head)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use reqwest::header::HeaderValue;

    fn post(permalink: Option<&str>) -> RedditPost {
        RedditPost {
            id: "abc123".into(),
            title: "Title".into(),
            subreddit: "rust".into(),
            permalink: permalink.map(str::to_string),
            url: Some("https://example.com/external".into()),
            created_utc: Utc.timestamp_opt(1_792_587_240, 0).unwrap(),
        }
    }

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(*k, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn post_url_prefers_permalink() {
        let p = post(Some("/r/rust/comments/abc123/title/"));
        assert_eq!(post_url(&p), "https://www.reddit.com/r/rust/comments/abc123/title/");
    }

    #[test]
    fn post_url_handles_permalink_without_leading_slash() {
        let p = post(Some("r/rust/comments/abc123/title/"));
        assert_eq!(post_url(&p), "https://www.reddit.com/r/rust/comments/abc123/title/");
    }

    #[test]
    fn post_url_falls_back_to_comments_url_not_external_url() {
        for permalink in [None, Some("")] {
            let p = post(permalink);
            assert_eq!(post_url(&p), "https://www.reddit.com/r/rust/comments/abc123/");
        }
    }

    #[test]
    fn retry_after_prefers_reset_seconds() {
        let h = headers(&[("x-ratelimit-reset", "42"), ("x-ratelimit-reset-at", "0"), ("retry-after", "7")]);
        assert_eq!(retry_after_from_headers(&h, SystemTime::now()), Duration::from_secs(42));
    }

    #[test]
    fn retry_after_uses_reset_at_milliseconds() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000);
        let h = headers(&[("x-ratelimit-reset-at", "1012000")]);
        assert_eq!(retry_after_from_headers(&h, now), Duration::from_secs(12));
    }

    #[test]
    fn retry_after_reset_at_in_the_past_clamps_to_minimum() {
        let now = UNIX_EPOCH + Duration::from_secs(1_000);
        let h = headers(&[("x-ratelimit-reset-at", "1")]);
        assert_eq!(retry_after_from_headers(&h, now), MIN_RETRY_AFTER);
    }

    #[test]
    fn retry_after_falls_back_to_retry_after_header() {
        let h = headers(&[("retry-after", "9")]);
        assert_eq!(retry_after_from_headers(&h, SystemTime::now()), Duration::from_secs(9));
    }

    #[test]
    fn retry_after_defaults_and_clamps() {
        assert_eq!(retry_after_from_headers(&HeaderMap::new(), SystemTime::now()), DEFAULT_RETRY_AFTER);
        let h = headers(&[("x-ratelimit-reset", "99999")]);
        assert_eq!(retry_after_from_headers(&h, SystemTime::now()), MAX_RETRY_AFTER);
        let h = headers(&[("x-ratelimit-reset", "0")]);
        assert_eq!(retry_after_from_headers(&h, SystemTime::now()), MIN_RETRY_AFTER);
        let h = headers(&[("x-ratelimit-reset", "soon")]);
        assert_eq!(retry_after_from_headers(&h, SystemTime::now()), DEFAULT_RETRY_AFTER);
    }

    #[test]
    fn truncate_body_keeps_short_and_shortens_long() {
        assert_eq!(truncate_body("  ok  "), "ok");
        let long = "x".repeat(500);
        let t = truncate_body(&long);
        assert_eq!(t.chars().count(), MAX_BODY_IN_ERROR + 1);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn fetch_error_display() {
        let e = FetchError::RateLimited { retry_after: Duration::from_secs(5) };
        assert_eq!(e.to_string(), "rate limited; retry after 5s");
        let e = FetchError::Status { status: StatusCode::FORBIDDEN, body: "nope".into() };
        assert_eq!(e.to_string(), "HTTP 403 Forbidden: nope");
        assert_eq!(FetchError::Api("bad".into()).to_string(), "API error: bad");
        assert_eq!(FetchError::Parse("eof".into()).to_string(), "parse error: eof");
    }

    #[test]
    fn build_post_source_selects_by_config() {
        let base = AppConfig::from_lookup(|k| match k {
            "DATABASE_URL" => Some("sqlite://x.db".into()),
            _ => None,
        })
        .unwrap();
        let client = Client::new();
        assert_eq!(build_post_source(&base, client.clone()).name(), "arctic_shift");

        let cfg = AppConfig { post_source: PostSourceKind::RedditJson, ..base };
        assert_eq!(build_post_source(&cfg, client).name(), "reddit_json");
    }
}
