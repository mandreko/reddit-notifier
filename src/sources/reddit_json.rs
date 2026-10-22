//! [`PostSource`] backed by Reddit's unauthenticated `/r/<subs>/new.json` listing.
//!
//! This is the daemon's original data path. Reddit now returns 403 for it from many
//! IP ranges and has announced the public Data API will close in March 2027, so it is
//! kept only for deployments where it still works. Up to 100 subreddits can be joined
//! with `+` in one request.

use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use std::time::SystemTime;
use tracing::debug;

use super::{retry_after_from_headers, truncate_body, FetchError, PostSource};
use crate::models::{RedditListing, RedditPost};

pub const MAX_SUBREDDITS_PER_REQUEST: usize = 100;

pub struct RedditJsonSource {
    client: Client,
    base_url: String,
}

impl RedditJsonSource {
    pub fn new(client: Client, base_url: impl Into<String>) -> Self {
        Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    pub(crate) fn listing_url(&self, subreddits: &[String]) -> String {
        format!("{}/r/{}/new.json?limit=100", self.base_url, subreddits.join("+"))
    }
}

#[async_trait]
impl PostSource for RedditJsonSource {
    fn name(&self) -> &'static str {
        "reddit_json"
    }

    fn batch_size(&self) -> usize {
        MAX_SUBREDDITS_PER_REQUEST
    }

    async fn fetch_new(&self, subreddits: &[String]) -> Result<Vec<RedditPost>, FetchError> {
        if subreddits.is_empty() {
            return Err(FetchError::Api("reddit_json needs at least one subreddit".into()));
        }

        let url = self.listing_url(subreddits);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();

        if status == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = retry_after_from_headers(resp.headers(), SystemTime::now());
            return Err(FetchError::RateLimited { retry_after });
        }

        let body = resp.text().await?;
        if !status.is_success() {
            return Err(FetchError::Status { status, body: truncate_body(&body) });
        }

        let listing: RedditListing = serde_json::from_str(&body)
            .map_err(|e| FetchError::Parse(format!("{} (body: {})", e, truncate_body(&body))))?;
        let posts = listing.into_posts();
        debug!("reddit_json: {} post(s) for {} subreddit(s)", posts.len(), subreddits.len());
        Ok(posts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const UA: &str = "reddit_notifier_test/0.0";

    fn client() -> Client {
        Client::builder().user_agent(UA).timeout(Duration::from_millis(500)).build().unwrap()
    }

    fn subs(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn listing_url_joins_batch_with_plus() {
        let s = RedditJsonSource::new(client(), "https://www.reddit.com/");
        assert_eq!(
            s.listing_url(&subs(&["rust", "golang"])),
            "https://www.reddit.com/r/rust+golang/new.json?limit=100"
        );
        assert_eq!(s.name(), "reddit_json");
        assert_eq!(s.batch_size(), 100);
    }

    #[tokio::test]
    async fn fetch_success_flattens_children() {
        let server = MockServer::start().await;
        let body = r#"{"kind":"Listing","data":{"children":[
            {"kind":"t3","data":{"id":"a","title":"A","subreddit":"rust","permalink":"/r/rust/comments/a/x/","url":"https://www.reddit.com/r/rust/comments/a/x/","created_utc":1792587240.0}},
            {"kind":"t3","data":{"id":"b","title":"B","subreddit":"golang","permalink":"/r/golang/comments/b/y/","url":"https://example.com","created_utc":1792587300.0}}
        ]}}"#;
        Mock::given(method("GET"))
            .and(path("/r/rust+golang/new.json"))
            .and(query_param("limit", "100"))
            .and(header("user-agent", UA))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .expect(1)
            .mount(&server)
            .await;

        let source = RedditJsonSource::new(client(), server.uri());
        let posts = source.fetch_new(&subs(&["rust", "golang"])).await.unwrap();
        assert_eq!(posts.len(), 2);
        assert_eq!(posts[1].subreddit, "golang");
        assert_eq!(posts[0].permalink.as_deref(), Some("/r/rust/comments/a/x/"));
    }

    #[tokio::test]
    async fn fetch_403_is_status_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(403).set_body_string("<html>blocked</html>"))
            .mount(&server)
            .await;
        let source = RedditJsonSource::new(client(), server.uri());
        match source.fetch_new(&subs(&["rust"])).await {
            Err(FetchError::Status { status, body }) => {
                assert_eq!(status, StatusCode::FORBIDDEN);
                assert_eq!(body, "<html>blocked</html>");
            }
            other => panic!("expected Status, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn fetch_429_is_rate_limited_using_retry_after() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "30"))
            .mount(&server)
            .await;
        let source = RedditJsonSource::new(client(), server.uri());
        match source.fetch_new(&subs(&["rust"])).await {
            Err(FetchError::RateLimited { retry_after }) => assert_eq!(retry_after, Duration::from_secs(30)),
            other => panic!("expected RateLimited, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn fetch_malformed_json_is_parse_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{\"data\":{\"children\":[{}]}}"))
            .mount(&server)
            .await;
        let source = RedditJsonSource::new(client(), server.uri());
        assert!(matches!(source.fetch_new(&subs(&["rust"])).await, Err(FetchError::Parse(_))));
    }

    #[tokio::test]
    async fn fetch_empty_batch_is_rejected_without_request() {
        let server = MockServer::start().await;
        let source = RedditJsonSource::new(client(), server.uri());
        assert!(matches!(source.fetch_new(&[]).await, Err(FetchError::Api(_))));
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}
