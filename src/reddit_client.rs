use anyhow::{Context, Result};
use reqwest::{Client, Response};
use tracing::{info, warn};

/// Reddit client that handles both authenticated and unauthenticated requests
#[derive(Debug, Clone)]
pub struct RedditClient {
    client: Client,
    base_url: String,
    session_cookie: Option<String>,
}

impl RedditClient {
    /// Create a new Reddit client
    ///
    /// # Arguments
    /// * `user_agent` - User agent string for Reddit requests
    /// * `session_cookie` - Optional Reddit session cookie for authentication
    pub fn new(user_agent: String, session_cookie: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .user_agent(user_agent.clone())
            .build()
            .context("Failed to build HTTP client")?;

        if session_cookie.is_some() {
            info!("Reddit client created with session cookie authentication");
        } else {
            info!("Reddit client created without authentication");
        }

        Ok(Self {
            client,
            base_url: "https://www.reddit.com".to_string(),
            session_cookie,
        })
    }

    /// Get subreddit posts with optional authentication
    ///
    /// # Arguments
    /// * `subreddit` - Subreddit name (can be combined like "sub1+sub2")
    /// * `limit` - Number of posts to fetch (max 100)
    pub async fn get_subreddit_posts(
        &self,
        subreddit: &str,
        limit: u32,
    ) -> Result<Response> {
        // Use .json endpoint for both authenticated and unauthenticated requests
        // Reddit accepts cookies on the public JSON endpoints
        let url = format!("{}/r/{}/new.json?limit={}", self.base_url, subreddit, limit);

        let mut request_builder = self.client.get(&url);

        // Add session cookie if available
        if let Some(ref cookie_value) = self.session_cookie {
            request_builder = request_builder.header("Cookie", format!("reddit_session={}", cookie_value));
        }

        let response = request_builder
            .send()
            .await
            .context("Failed to send Reddit API request")?;

        if !response.status().is_success() {
            let status = response.status();

            // Check for authentication-related errors
            if status == 401 {
                warn!("Reddit API returned 401 Unauthorized - check your Reddit session cookie");
            } else if status == 403 {
                warn!("Reddit API returned 403 Forbidden - you may be rate limited, banned, or have an invalid session cookie");
            } else if status == 429 {
                warn!("Reddit API returned 429 Too Many Requests - rate limit exceeded");
            }

            warn!("Reddit API request failed: GET {} -> {}", url, status);
        }

        Ok(response)
    }

    /// Check if the client is configured for authenticated requests
    pub fn is_authenticated(&self) -> bool {
        self.session_cookie.is_some()
    }

    /// Get the underlying HTTP client for other operations
    pub fn http_client(&self) -> &Client {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reddit_client_creation_without_auth() {
        let client = RedditClient::new("test_agent".to_string(), None).unwrap();
        assert!(!client.is_authenticated());
    }

    #[test]
    fn test_reddit_client_creation_with_auth() {
        let client = RedditClient::new(
            "test_agent".to_string(),
            Some("test_session_cookie".to_string()),
        ).unwrap();
        assert!(client.is_authenticated());
    }
}