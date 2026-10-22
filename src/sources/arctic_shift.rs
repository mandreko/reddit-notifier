//! [`PostSource`] backed by the Arctic Shift archive API.
//!
//! Arctic Shift (<https://arctic-shift.photon-reddit.com>) is a free, keyless,
//! third-party archive of Reddit that ingests posts within ~30 seconds of creation
//! and serves them as Reddit-shaped JSON. Characteristics that shape this source:
//!
//! - one `subreddit` per request (so `batch_size() == 1`);
//! - `limit` 1..=100, `sort` by `created_utc`, `after` accepts epoch seconds;
//! - `fields` trims the payload, but `permalink` is not selectable, so links are
//!   constructed from `subreddit` + `id` (see [`super::post_url`]);
//! - rate limits are dynamic: 429 plus `x-ratelimit-reset` headers;
//! - errors come back as `{"data":null,"error":"..."}` with 400/422;
//! - there is no uptime guarantee, and posts later removed by moderators still appear.

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use reqwest::{Client, StatusCode};
use std::time::SystemTime;
use tracing::debug;

use super::{retry_after_from_headers, truncate_body, FetchError, PostSource};
use crate::models::{ArcticShiftResponse, RedditPost};

/// Only the fields the notifier consumes; keeps a 100-post response around 20 KB.
pub const FIELDS: &str = "id,title,subreddit,url,created_utc";
const LIMIT: &str = "100";
/// Stateless window matching the poller's own freshness filter. Being a window rather
/// than a cursor means the ~30s archival lag can never cause a post to be skipped.
const LOOKBACK: TimeDelta = TimeDelta::hours(24);

pub struct ArcticShiftSource {
    client: Client,
    base_url: String,
}

impl ArcticShiftSource {
    pub fn new(client: Client, base_url: impl Into<String>) -> Self {
        Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    fn endpoint(&self) -> String {
        format!("{}/api/posts/search", self.base_url)
    }

    /// Query parameters for one subreddit at a given instant.
    pub(crate) fn query_params(subreddit: &str, now: DateTime<Utc>) -> Vec<(&'static str, String)> {
        let after = (now - LOOKBACK).timestamp();
        vec![
            ("subreddit", subreddit.to_string()),
            ("sort", "desc".to_string()),
            ("limit", LIMIT.to_string()),
            ("after", after.to_string()),
            ("fields", FIELDS.to_string()),
        ]
    }
}

#[async_trait]
impl PostSource for ArcticShiftSource {
    fn name(&self) -> &'static str {
        "arctic_shift"
    }

    fn batch_size(&self) -> usize {
        1
    }

    async fn fetch_new(&self, subreddits: &[String]) -> Result<Vec<RedditPost>, FetchError> {
        let subreddit = match subreddits {
            [single] => single,
            _ => {
                return Err(FetchError::Api(format!(
                    "arctic_shift fetches exactly one subreddit per request, got {}",
                    subreddits.len()
                )))
            }
        };

        let params = Self::query_params(subreddit, Utc::now());
        let resp = self.client.get(self.endpoint()).query(&params).send().await?;
        let status = resp.status();

        if status == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = retry_after_from_headers(resp.headers(), SystemTime::now());
            return Err(FetchError::RateLimited { retry_after });
        }

        let body = resp.text().await?;
        let parsed: Result<ArcticShiftResponse, _> = serde_json::from_str(&body);

        match parsed {
            Ok(ArcticShiftResponse { error: Some(msg), .. }) => {
                Err(FetchError::Api(format!("{} for r/{}: {}", status, subreddit, msg)))
            }
            Ok(ArcticShiftResponse { data: Some(posts), .. }) if status.is_success() => {
                debug!("arctic_shift: {} post(s) for r/{}", posts.len(), subreddit);
                Ok(posts)
            }
            Ok(_) if status.is_success() => Err(FetchError::Api(format!(
                "{} for r/{}: response contained neither data nor error",
                status, subreddit
            ))),
            Ok(_) => Err(FetchError::Status { status, body: truncate_body(&body) }),
            Err(e) if status.is_success() => Err(FetchError::Parse(format!(
                "{} (body: {})",
                e,
                truncate_body(&body)
            ))),
            Err(_) => Err(FetchError::Status { status, body: truncate_body(&body) }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::time::Duration;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const UA: &str = "reddit_notifier_test/0.0";

    fn client() -> Client {
        Client::builder()
            .user_agent(UA)
            .timeout(Duration::from_millis(500))
            .build()
            .unwrap()
    }

    async fn server_with(status: u16, body: &str, extra: &[(&str, &str)]) -> MockServer {
        let server = MockServer::start().await;
        let mut tpl = ResponseTemplate::new(status).set_body_string(body);
        for (k, v) in extra {
            tpl = tpl.insert_header(*k, *v);
        }
        Mock::given(method("GET"))
            .and(path("/api/posts/search"))
            .and(query_param("subreddit", "rust"))
            .and(query_param("sort", "desc"))
            .and(query_param("limit", "100"))
            .and(query_param("fields", FIELDS))
            .and(header("user-agent", UA))
            .respond_with(tpl)
            .expect(1)
            .mount(&server)
            .await;
        server
    }

    fn subs() -> Vec<String> {
        vec!["rust".to_string()]
    }

    #[test]
    fn query_params_use_24h_epoch_window() {
        let now = Utc.timestamp_opt(1_792_600_000, 0).unwrap();
        let params = ArcticShiftSource::query_params("rust", now);
        let get = |k: &str| params.iter().find(|(key, _)| *key == k).map(|(_, v)| v.clone()).unwrap();
        assert_eq!(get("subreddit"), "rust");
        assert_eq!(get("after"), (1_792_600_000 - 86_400).to_string());
        assert_eq!(get("limit"), "100");
        assert_eq!(get("sort"), "desc");
        assert_eq!(get("fields"), FIELDS);
        assert!(!FIELDS.contains("permalink"), "permalink is not selectable upstream");
    }

    #[test]
    fn metadata() {
        let s = ArcticShiftSource::new(client(), "http://localhost/");
        assert_eq!(s.name(), "arctic_shift");
        assert_eq!(s.batch_size(), 1);
        assert_eq!(s.endpoint(), "http://localhost/api/posts/search");
    }

    #[tokio::test]
    async fn fetch_success_returns_posts() {
        let body = r#"{"data":[
            {"id":"a1","title":"First","subreddit":"rust","url":"https://www.reddit.com/r/rust/comments/a1/first/","created_utc":1792587240},
            {"id":"b2","title":"Second","subreddit":"rust","url":"https://example.com","created_utc":1792587300}
        ]}"#;
        let server = server_with(200, body, &[]).await;
        let source = ArcticShiftSource::new(client(), server.uri());

        let posts = source.fetch_new(&subs()).await.unwrap();
        assert_eq!(posts.len(), 2);
        assert_eq!(posts[0].id, "a1");
        assert_eq!(posts[1].created_utc.timestamp(), 1792587300);
        assert!(posts[0].permalink.is_none());
    }

    #[tokio::test]
    async fn fetch_empty_data_is_ok() {
        let server = server_with(200, r#"{"data":[]}"#, &[]).await;
        let source = ArcticShiftSource::new(client(), server.uri());
        assert!(source.fetch_new(&subs()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn fetch_429_maps_to_rate_limited_with_reset_header() {
        let server = server_with(429, "slow down", &[("x-ratelimit-reset", "17")]).await;
        let source = ArcticShiftSource::new(client(), server.uri());
        match source.fetch_new(&subs()).await {
            Err(FetchError::RateLimited { retry_after }) => assert_eq!(retry_after, Duration::from_secs(17)),
            other => panic!("expected RateLimited, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn fetch_400_data_null_surfaces_error_text() {
        let server = server_with(400, r#"{"data":null,"error":"'limit' must be between 1 and 100"}"#, &[]).await;
        let source = ArcticShiftSource::new(client(), server.uri());
        match source.fetch_new(&subs()).await {
            Err(FetchError::Api(msg)) => {
                assert!(msg.contains("400"), "{msg}");
                assert!(msg.contains("'limit' must be between 1 and 100"), "{msg}");
                assert!(msg.contains("r/rust"), "{msg}");
            }
            other => panic!("expected Api, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn fetch_200_with_soft_error_is_api_error() {
        let server = server_with(200, r#"{"data":null,"error":"Timeout. Maybe slow down a bit"}"#, &[]).await;
        let source = ArcticShiftSource::new(client(), server.uri());
        assert!(matches!(source.fetch_new(&subs()).await, Err(FetchError::Api(_))));
    }

    #[tokio::test]
    async fn fetch_200_without_data_or_error_is_api_error() {
        let server = server_with(200, r#"{"data":null}"#, &[]).await;
        let source = ArcticShiftSource::new(client(), server.uri());
        assert!(matches!(source.fetch_new(&subs()).await, Err(FetchError::Api(_))));
    }

    #[tokio::test]
    async fn fetch_malformed_json_is_parse_error() {
        let server = server_with(200, "<html>not json</html>", &[]).await;
        let source = ArcticShiftSource::new(client(), server.uri());
        match source.fetch_new(&subs()).await {
            Err(FetchError::Parse(msg)) => assert!(msg.contains("not json"), "{msg}"),
            other => panic!("expected Parse, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn fetch_500_is_status_error() {
        let server = server_with(500, "boom", &[]).await;
        let source = ArcticShiftSource::new(client(), server.uri());
        match source.fetch_new(&subs()).await {
            Err(FetchError::Status { status, body }) => {
                assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
                assert_eq!(body, "boom");
            }
            other => panic!("expected Status, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn fetch_timeout_is_transport_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#).set_delay(Duration::from_secs(5)))
            .mount(&server)
            .await;
        let source = ArcticShiftSource::new(client(), server.uri());
        assert!(matches!(source.fetch_new(&subs()).await, Err(FetchError::Transport(_))));
    }

    #[tokio::test]
    async fn fetch_rejects_multi_subreddit_batches_without_a_request() {
        let server = MockServer::start().await; // no mocks: any request would 404
        let source = ArcticShiftSource::new(client(), server.uri());
        let batch = vec!["rust".to_string(), "golang".to_string()];
        assert!(matches!(source.fetch_new(&batch).await, Err(FetchError::Api(_))));
        assert!(matches!(source.fetch_new(&[]).await, Err(FetchError::Api(_))));
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}
