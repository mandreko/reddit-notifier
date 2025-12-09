use async_trait::async_trait;
use anyhow::Result;
use std::collections::HashMap;

use crate::models::database::{EndpointRow, NotifiedPostRow, SubscriptionRow};

/// DatabaseService trait defines all database operations needed by the TUI and poller.
///
/// This trait abstracts database access to enable:
/// - Easy testing with mock implementations
/// - Flexibility to swap database backends
/// - Clear separation of concerns between UI and data layers
#[async_trait]
pub trait DatabaseService: Send + Sync {
    // ========================================================================
    // Subscription Operations
    // ========================================================================

    /// List all subscriptions with metadata
    async fn list_subscriptions(&self) -> Result<Vec<SubscriptionRow>>;

    /// Create a new subscription for a subreddit
    ///
    /// # Returns
    /// The ID of the newly created subscription
    async fn create_subscription(&self, subreddit: &str) -> Result<i64>;

    /// Delete a subscription by ID (cascade deletes junction table links)
    async fn delete_subscription(&self, id: i64) -> Result<()>;

    /// Get all endpoints linked to a specific subscription
    async fn get_subscription_endpoints(&self, subscription_id: i64) -> Result<Vec<EndpointRow>>;

    // ========================================================================
    // Endpoint Operations
    // ========================================================================

    /// List all endpoints with type and active status
    async fn list_endpoints(&self) -> Result<Vec<EndpointRow>>;

    /// Get a single endpoint by ID
    async fn get_endpoint(&self, id: i64) -> Result<EndpointRow>;

    /// Create a new endpoint
    ///
    /// # Arguments
    /// * `kind` - The endpoint type (e.g., "Discord", "Pushover")
    /// * `config_json` - JSON configuration string for the endpoint
    /// * `note` - Optional user note/description
    ///
    /// # Returns
    /// The ID of the newly created endpoint
    async fn create_endpoint(
        &self,
        kind: &str,
        config_json: &str,
        note: Option<&str>,
    ) -> Result<i64>;

    /// Update an endpoint's configuration and note
    async fn update_endpoint(&self, id: i64, config_json: &str, note: Option<&str>)
        -> Result<()>;

    /// Delete an endpoint by ID (cascade deletes junction table links)
    async fn delete_endpoint(&self, id: i64) -> Result<()>;

    /// Toggle an endpoint's active status
    ///
    /// # Returns
    /// The new active status (true = active, false = inactive)
    async fn toggle_endpoint_active(&self, id: i64) -> Result<bool>;

    // ========================================================================
    // Junction Table Operations
    // ========================================================================

    /// Link a subscription to an endpoint
    async fn link_subscription_endpoint(
        &self,
        subscription_id: i64,
        endpoint_id: i64,
    ) -> Result<()>;

    /// Unlink a subscription from an endpoint
    async fn unlink_subscription_endpoint(
        &self,
        subscription_id: i64,
        endpoint_id: i64,
    ) -> Result<()>;

    // ========================================================================
    // Notified Posts Operations
    // ========================================================================

    /// List notified posts with pagination
    async fn list_notified_posts(&self, limit: i64, offset: i64) -> Result<Vec<NotifiedPostRow>>;

    /// List notified posts filtered by subreddit with pagination
    async fn list_notified_posts_by_subreddit(
        &self,
        subreddit: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<NotifiedPostRow>>;

    /// Delete a notified post by ID
    async fn delete_notified_post(&self, id: i64) -> Result<()>;

    /// Clean up old notified posts, deleting records older than the specified number of days
    ///
    /// # Returns
    /// Number of records deleted
    async fn cleanup_old_posts(&self, days_to_keep: i64) -> Result<u64>;

    // ========================================================================
    // Poller-Specific Operations
    // ========================================================================

    /// Get list of unique subreddits that have active endpoints
    async fn unique_subreddits(&self) -> Result<Vec<String>>;

    /// Fetch all subreddit-to-endpoints mappings in a single query
    ///
    /// Returns a HashMap where keys are subreddit names and values are vectors
    /// of active endpoints subscribed to that subreddit.
    async fn all_subreddit_endpoint_mappings(&self)
        -> Result<HashMap<String, Vec<EndpointRow>>>;

    /// Record a post as notified if it's new
    ///
    /// # Returns
    /// `true` if the post was newly inserted, `false` if it already existed
    async fn record_if_new(&self, subreddit: &str, post_id: &str) -> Result<bool>;
}
