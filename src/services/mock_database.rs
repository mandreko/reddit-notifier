use async_trait::async_trait;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::models::database::{EndpointKind, EndpointRow, NotifiedPostRow, SubscriptionRow};
use crate::services::database::DatabaseService;

/// Mock implementation of DatabaseService for testing
///
/// This implementation stores all data in memory and doesn't require a real database.
/// It's useful for testing TUI logic without database dependencies.
#[derive(Debug, Clone, Default)]
pub struct MockDatabaseService {
    subscriptions: Arc<Mutex<Vec<SubscriptionRow>>>,
    endpoints: Arc<Mutex<Vec<EndpointRow>>>,
    posts: Arc<Mutex<Vec<NotifiedPostRow>>>,
    links: Arc<Mutex<Vec<(i64, i64)>>>, // (subscription_id, endpoint_id)
    next_id: Arc<Mutex<i64>>,
}

impl MockDatabaseService {
    /// Create a new empty MockDatabaseService
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(Mutex::new(Vec::new())),
            endpoints: Arc::new(Mutex::new(Vec::new())),
            posts: Arc::new(Mutex::new(Vec::new())),
            links: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    /// Create a MockDatabaseService pre-populated with test data
    pub fn with_test_data() -> Self {
        let service = Self::new();

        // Add test subscriptions
        let mut subscriptions = service.subscriptions.lock().unwrap();
        subscriptions.push(SubscriptionRow {
            id: 1,
            subreddit: "rust".to_string(),
            created_at: "2024-01-01 00:00:00".to_string(),
        });
        subscriptions.push(SubscriptionRow {
            id: 2,
            subreddit: "programming".to_string(),
            created_at: "2024-01-02 00:00:00".to_string(),
        });
        drop(subscriptions);

        // Add test endpoints
        let mut endpoints = service.endpoints.lock().unwrap();
        endpoints.push(EndpointRow {
            id: 1,
            kind: EndpointKind::Discord,
            config_json: r#"{"webhook_url":"https://discord.com/api/webhooks/test"}"#.to_string(),
            active: true,
            note: Some("Test Discord endpoint".to_string()),
        });
        endpoints.push(EndpointRow {
            id: 2,
            kind: EndpointKind::Pushover,
            config_json: r#"{"token":"test_token","user":"test_user"}"#.to_string(),
            active: true,
            note: Some("Test Pushover endpoint".to_string()),
        });
        drop(endpoints);

        // Add test links
        let mut links = service.links.lock().unwrap();
        links.push((1, 1)); // rust -> Discord
        links.push((2, 1)); // programming -> Discord
        links.push((2, 2)); // programming -> Pushover
        drop(links);

        // Set next_id to 3
        *service.next_id.lock().unwrap() = 3;

        service
    }

    fn get_next_id(&self) -> i64 {
        let mut next_id = self.next_id.lock().unwrap();
        let id = *next_id;
        *next_id += 1;
        id
    }
}

#[async_trait]
impl DatabaseService for MockDatabaseService {
    // ========================================================================
    // Subscription Operations
    // ========================================================================

    async fn list_subscriptions(&self) -> Result<Vec<SubscriptionRow>> {
        let subscriptions = self.subscriptions.lock().unwrap();
        Ok(subscriptions.clone())
    }

    async fn create_subscription(&self, subreddit: &str) -> Result<i64> {
        let id = self.get_next_id();
        let mut subscriptions = self.subscriptions.lock().unwrap();
        subscriptions.push(SubscriptionRow {
            id,
            subreddit: subreddit.to_string(),
            created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        });
        Ok(id)
    }

    async fn delete_subscription(&self, id: i64) -> Result<()> {
        let mut subscriptions = self.subscriptions.lock().unwrap();
        subscriptions.retain(|s| s.id != id);

        // Also delete associated links
        let mut links = self.links.lock().unwrap();
        links.retain(|(sub_id, _)| *sub_id != id);

        Ok(())
    }

    async fn get_subscription_endpoints(&self, subscription_id: i64) -> Result<Vec<EndpointRow>> {
        let links = self.links.lock().unwrap();
        let endpoints = self.endpoints.lock().unwrap();

        let endpoint_ids: Vec<i64> = links
            .iter()
            .filter(|(sub_id, _)| *sub_id == subscription_id)
            .map(|(_, end_id)| *end_id)
            .collect();

        let result = endpoints
            .iter()
            .filter(|e| endpoint_ids.contains(&e.id))
            .cloned()
            .collect();

        Ok(result)
    }

    // ========================================================================
    // Endpoint Operations
    // ========================================================================

    async fn list_endpoints(&self) -> Result<Vec<EndpointRow>> {
        let endpoints = self.endpoints.lock().unwrap();
        Ok(endpoints.clone())
    }

    async fn get_endpoint(&self, id: i64) -> Result<EndpointRow> {
        let endpoints = self.endpoints.lock().unwrap();
        endpoints
            .iter()
            .find(|e| e.id == id)
            .cloned()
            .ok_or_else(|| anyhow!("Endpoint not found: {}", id))
    }

    async fn create_endpoint(
        &self,
        kind: &str,
        config_json: &str,
        note: Option<&str>,
    ) -> Result<i64> {
        let id = self.get_next_id();
        let parsed_kind = kind
            .parse::<EndpointKind>()
            .map_err(|e| anyhow!("Invalid endpoint kind: {}", e))?;
        let mut endpoints = self.endpoints.lock().unwrap();
        endpoints.push(EndpointRow {
            id,
            kind: parsed_kind,
            config_json: config_json.to_string(),
            active: true,
            note: note.map(|s| s.to_string()),
        });
        Ok(id)
    }

    async fn update_endpoint(
        &self,
        id: i64,
        config_json: &str,
        note: Option<&str>,
    ) -> Result<()> {
        let mut endpoints = self.endpoints.lock().unwrap();
        let endpoint = endpoints
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| anyhow!("Endpoint not found: {}", id))?;

        endpoint.config_json = config_json.to_string();
        endpoint.note = note.map(|s| s.to_string());
        Ok(())
    }

    async fn delete_endpoint(&self, id: i64) -> Result<()> {
        let mut endpoints = self.endpoints.lock().unwrap();
        endpoints.retain(|e| e.id != id);

        // Also delete associated links
        let mut links = self.links.lock().unwrap();
        links.retain(|(_, end_id)| *end_id != id);

        Ok(())
    }

    async fn toggle_endpoint_active(&self, id: i64) -> Result<bool> {
        let mut endpoints = self.endpoints.lock().unwrap();
        let endpoint = endpoints
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| anyhow!("Endpoint not found: {}", id))?;

        endpoint.active = !endpoint.active;
        Ok(endpoint.active)
    }

    // ========================================================================
    // Junction Table Operations
    // ========================================================================

    async fn link_subscription_endpoint(
        &self,
        subscription_id: i64,
        endpoint_id: i64,
    ) -> Result<()> {
        let mut links = self.links.lock().unwrap();
        if !links.contains(&(subscription_id, endpoint_id)) {
            links.push((subscription_id, endpoint_id));
        }
        Ok(())
    }

    async fn unlink_subscription_endpoint(
        &self,
        subscription_id: i64,
        endpoint_id: i64,
    ) -> Result<()> {
        let mut links = self.links.lock().unwrap();
        links.retain(|(sub_id, end_id)| {
            *sub_id != subscription_id || *end_id != endpoint_id
        });
        Ok(())
    }

    // ========================================================================
    // Notified Posts Operations
    // ========================================================================

    async fn list_notified_posts(&self, limit: i64, offset: i64) -> Result<Vec<NotifiedPostRow>> {
        let posts = self.posts.lock().unwrap();
        let start = offset as usize;
        let end = (start + limit as usize).min(posts.len());
        Ok(posts[start..end].to_vec())
    }

    async fn list_notified_posts_by_subreddit(
        &self,
        subreddit: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<NotifiedPostRow>> {
        let posts = self.posts.lock().unwrap();
        let filtered: Vec<NotifiedPostRow> = posts
            .iter()
            .filter(|p| p.subreddit == subreddit)
            .cloned()
            .collect();

        let start = offset as usize;
        let end = (start + limit as usize).min(filtered.len());
        Ok(filtered[start..end].to_vec())
    }

    async fn delete_notified_post(&self, id: i64) -> Result<()> {
        let mut posts = self.posts.lock().unwrap();
        posts.retain(|p| p.id != id);
        Ok(())
    }

    async fn cleanup_old_posts(&self, _days_to_keep: i64) -> Result<u64> {
        // In mock, we don't have real timestamps, so just return 0
        Ok(0)
    }

    // ========================================================================
    // Poller-Specific Operations
    // ========================================================================

    async fn unique_subreddits(&self) -> Result<Vec<String>> {
        let subscriptions = self.subscriptions.lock().unwrap();
        let links = self.links.lock().unwrap();
        let endpoints = self.endpoints.lock().unwrap();

        // Get subscription IDs that have active endpoints
        let sub_ids_with_active: Vec<i64> = links
            .iter()
            .filter(|(_, end_id)| {
                endpoints
                    .iter()
                    .any(|e| e.id == *end_id && e.active)
            })
            .map(|(sub_id, _)| *sub_id)
            .collect();

        // Get unique subreddit names
        let mut subreddits: Vec<String> = subscriptions
            .iter()
            .filter(|s| sub_ids_with_active.contains(&s.id))
            .map(|s| s.subreddit.clone())
            .collect();

        subreddits.sort();
        subreddits.dedup();
        Ok(subreddits)
    }

    async fn all_subreddit_endpoint_mappings(
        &self,
    ) -> Result<HashMap<String, Vec<EndpointRow>>> {
        let subscriptions = self.subscriptions.lock().unwrap();
        let links = self.links.lock().unwrap();
        let endpoints = self.endpoints.lock().unwrap();

        let mut mappings: HashMap<String, Vec<EndpointRow>> = HashMap::new();

        for (sub_id, end_id) in links.iter() {
            // Find the subscription
            if let Some(sub) = subscriptions.iter().find(|s| s.id == *sub_id) {
                // Find the endpoint
                if let Some(endpoint) = endpoints.iter().find(|e| e.id == *end_id && e.active) {
                    mappings
                        .entry(sub.subreddit.clone())
                        .or_default()
                        .push(endpoint.clone());
                }
            }
        }

        Ok(mappings)
    }

    async fn record_if_new(&self, subreddit: &str, post_id: &str) -> Result<bool> {
        let mut posts = self.posts.lock().unwrap();

        // Check if already exists
        if posts.iter().any(|p| p.subreddit == subreddit && p.post_id == post_id) {
            return Ok(false);
        }

        // Add new post
        let id = self.get_next_id();
        posts.push(NotifiedPostRow {
            id,
            subreddit: subreddit.to_string(),
            post_id: post_id.to_string(),
            first_seen_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        });

        Ok(true)
    }
}
