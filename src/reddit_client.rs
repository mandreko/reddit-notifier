use anyhow::{Context, Result};
use base64::prelude::*;
use reqwest::{Client, Response};
use tracing::{info, warn};

/// Reddit client that handles both authenticated and unauthenticated requests
#[derive(Debug, Clone)]
pub struct RedditClient {
    client: Client,
    base_url: String,
    auth_headers: Option<String>,
}

impl RedditClient {
    /// Create a new Reddit client
    ///
    /// # Arguments
    /// * `user_agent` - User agent string for Reddit requests
    /// * `username` - Optional Reddit username for authentication
    /// * `password` - Optional Reddit password for authentication
    pub fn new(
        user_agent: String,
        username: Option<String>,
        password: Option<String>,
    ) -> Result<Self> {
        let client = Client::builder()
            .user_agent(user_agent.clone())
            .build()
            .context("Failed to build HTTP client")?;

        // Create basic auth header if credentials are provided
        let auth_headers = if let (Some(username), Some(password)) = (username, password) {
            let credentials = format!("{}:{}", username, password);
            let encoded = BASE64_STANDARD.encode(credentials);
            Some(format!("Basic {}", encoded))
        } else {
            None
        };

        if auth_headers.is_some() {
            info!("Reddit client created with authentication");
        } else {
            info!("Reddit client created without authentication");
        }

        Ok(Self {
            client,
            base_url: "https://www.reddit.com".to_string(),
            auth_headers,
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
        let url = if self.auth_headers.is_some() {
            // Use OAuth API endpoint for authenticated requests
            format!("{}/r/{}/new/.json?limit={}", self.base_url, subreddit, limit)
        } else {
            // Use public JSON API for unauthenticated requests
            format!("{}/r/{}/new.json?limit={}", self.base_url, subreddit, limit)
        };

        let mut request_builder = self.client.get(&url);

        // Add authentication header if available
        if let Some(ref auth_header) = self.auth_headers {
            request_builder = request_builder.header("Authorization", auth_header);
        }

        let response = request_builder
            .send()
            .await
            .context("Failed to send Reddit API request")?;

        if !response.status().is_success() {
            let status = response.status();

            // Check for authentication-related errors
            if status == 401 {
                warn!("Reddit API returned 401 Unauthorized - check your Reddit credentials");
            } else if status == 403 {
                warn!("Reddit API returned 403 Forbidden - you may be rate limited or banned");
            } else if status == 429 {
                warn!("Reddit API returned 429 Too Many Requests - rate limit exceeded");
            }

            warn!("Reddit API request failed: GET {} -> {}", url, status);
        }

        Ok(response)
    }

    /// Check if the client is configured for authenticated requests
    pub fn is_authenticated(&self) -> bool {
        self.auth_headers.is_some()
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
        let client = RedditClient::new(
            "test_agent".to_string(),
            None,
            None,
        ).unwrap();

        assert!(!client.is_authenticated());
    }

    #[test]
    fn test_reddit_client_creation_with_auth() {
        let client = RedditClient::new(
            "test_agent".to_string(),
            Some("testuser".to_string()),
            Some("testpass".to_string()),
        ).unwrap();

        assert!(client.is_authenticated());
    }

    #[test]
    fn test_reddit_client_partial_auth() {
        let client = RedditClient::new(
            "test_agent".to_string(),
            Some("testuser".to_string()),
            None,
        ).unwrap();

        assert!(!client.is_authenticated());
    }
}