use async_trait::async_trait;
use anyhow::Result;
use sqlx::SqlitePool;
use std::collections::HashMap;

use crate::models::database::{EndpointRow, NotifiedPostRow, SubscriptionRow};
use crate::services::database::DatabaseService;

/// Production implementation of DatabaseService that uses SQLite
///
/// This implementation wraps the existing database:: functions and provides
/// them through the DatabaseService trait interface.
pub struct SqliteDatabaseService {
    pool: SqlitePool,
}

impl SqliteDatabaseService {
    /// Create a new SqliteDatabaseService with the given connection pool
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DatabaseService for SqliteDatabaseService {
    // ========================================================================
    // Subscription Operations
    // ========================================================================

    async fn list_subscriptions(&self) -> Result<Vec<SubscriptionRow>> {
        crate::database::list_subscriptions(&self.pool).await
    }

    async fn create_subscription(&self, subreddit: &str) -> Result<i64> {
        crate::database::create_subscription(&self.pool, subreddit).await
    }

    async fn delete_subscription(&self, id: i64) -> Result<()> {
        crate::database::delete_subscription(&self.pool, id).await
    }

    async fn get_subscription_endpoints(&self, subscription_id: i64) -> Result<Vec<EndpointRow>> {
        crate::database::get_subscription_endpoints(&self.pool, subscription_id).await
    }

    // ========================================================================
    // Endpoint Operations
    // ========================================================================

    async fn list_endpoints(&self) -> Result<Vec<EndpointRow>> {
        crate::database::list_endpoints(&self.pool).await
    }

    async fn get_endpoint(&self, id: i64) -> Result<EndpointRow> {
        crate::database::get_endpoint(&self.pool, id).await
    }

    async fn create_endpoint(
        &self,
        kind: &str,
        config_json: &str,
        note: Option<&str>,
    ) -> Result<i64> {
        crate::database::create_endpoint(&self.pool, kind, config_json, note).await
    }

    async fn update_endpoint(
        &self,
        id: i64,
        config_json: &str,
        note: Option<&str>,
    ) -> Result<()> {
        crate::database::update_endpoint(&self.pool, id, config_json, note).await
    }

    async fn delete_endpoint(&self, id: i64) -> Result<()> {
        crate::database::delete_endpoint(&self.pool, id).await
    }

    async fn toggle_endpoint_active(&self, id: i64) -> Result<bool> {
        crate::database::toggle_endpoint_active(&self.pool, id).await
    }

    // ========================================================================
    // Junction Table Operations
    // ========================================================================

    async fn link_subscription_endpoint(
        &self,
        subscription_id: i64,
        endpoint_id: i64,
    ) -> Result<()> {
        crate::database::link_subscription_endpoint(&self.pool, subscription_id, endpoint_id).await
    }

    async fn unlink_subscription_endpoint(
        &self,
        subscription_id: i64,
        endpoint_id: i64,
    ) -> Result<()> {
        crate::database::unlink_subscription_endpoint(&self.pool, subscription_id, endpoint_id)
            .await
    }

    // ========================================================================
    // Notified Posts Operations
    // ========================================================================

    async fn list_notified_posts(&self, limit: i64, offset: i64) -> Result<Vec<NotifiedPostRow>> {
        crate::database::list_notified_posts(&self.pool, limit, offset).await
    }

    async fn list_notified_posts_by_subreddit(
        &self,
        subreddit: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<NotifiedPostRow>> {
        crate::database::list_notified_posts_by_subreddit(&self.pool, subreddit, limit, offset)
            .await
    }

    async fn delete_notified_post(&self, id: i64) -> Result<()> {
        crate::database::delete_notified_post(&self.pool, id).await
    }

    async fn cleanup_old_posts(&self, days_to_keep: i64) -> Result<u64> {
        crate::database::cleanup_old_posts(&self.pool, days_to_keep).await
    }

    // ========================================================================
    // Poller-Specific Operations
    // ========================================================================

    async fn unique_subreddits(&self) -> Result<Vec<String>> {
        crate::database::unique_subreddits(&self.pool).await
    }

    async fn all_subreddit_endpoint_mappings(
        &self,
    ) -> Result<HashMap<String, Vec<EndpointRow>>> {
        crate::database::all_subreddit_endpoint_mappings(&self.pool).await
    }

    async fn record_if_new(&self, subreddit: &str, post_id: &str) -> Result<bool> {
        crate::database::record_if_new(&self.pool, subreddit, post_id).await
    }
}
