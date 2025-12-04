use anyhow::Result;
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};
use std::collections::HashMap;

use crate::models::{EndpointKind, EndpointRow, NotifiedPostRow, SubscriptionRow};

pub async fn unique_subreddits(pool: &SqlitePool) -> Result<Vec<String>> {
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT s.subreddit
        FROM subscriptions s
        JOIN subscription_endpoints se ON se.subscription_id = s.id
        JOIN endpoints e ON e.id = se.endpoint_id
        WHERE e.active = 1
        "#,
    )
    .fetch_all(pool)
    .await?;

    let subs = rows
        .into_iter()
        .filter_map(|r| r.try_get::<String, _>("subreddit").ok())
        .collect();
    Ok(subs)
}

/// Fetch all subreddit-to-endpoints mappings in a single query
///
/// Returns a HashMap where keys are subreddit names and values are vectors of active
/// endpoints subscribed to that subreddit.
///
/// This function is used by the combined poller to determine which endpoints should
/// receive notifications for posts from each subreddit.
pub async fn all_subreddit_endpoint_mappings(
    pool: &SqlitePool,
) -> Result<HashMap<String, Vec<EndpointRow>>> {
    let rows = sqlx::query(
        r#"
        SELECT
            s.subreddit,
            e.id as id,
            e.kind as kind,
            e.config_json as config_json,
            e.active as active,
            e.note as note
        FROM endpoints e
        JOIN subscription_endpoints se ON se.endpoint_id = e.id
        JOIN subscriptions s ON s.id = se.subscription_id
        WHERE e.active = 1
        ORDER BY s.subreddit
        "#,
    )
    .fetch_all(pool)
    .await?;

    // Group endpoints by subreddit
    let mut mappings: HashMap<String, Vec<EndpointRow>> = HashMap::new();

    for row in rows {
        let subreddit = row.get::<String, _>("subreddit");
        let id = row.get::<i64, _>("id");
        let kind_str = row.get::<String, _>("kind");

        // Try to parse the kind - if it fails, log a warning and skip this endpoint
        let kind = match kind_str.parse::<EndpointKind>() {
            Ok(k) => k,
            Err(_) => {
                tracing::warn!(
                    "Invalid endpoint kind '{}' for endpoint id {} - skipping",
                    kind_str,
                    id
                );
                continue;
            }
        };

        let endpoint = EndpointRow {
            id,
            kind,
            config_json: row.get::<String, _>("config_json"),
            active: row.get::<i64, _>("active") != 0,
            note: row.get::<Option<String>, _>("note"),
        };

        mappings
            .entry(subreddit)
            .or_default()
            .push(endpoint);
    }

    Ok(mappings)
}

/// Returns true if the (subreddit, post_id) was newly inserted.
pub async fn record_if_new(pool: &SqlitePool, subreddit: &str, post_id: &str) -> Result<bool> {
    let res = sqlx::query(
        r#"
        INSERT OR IGNORE INTO notified_posts (subreddit, post_id)
        VALUES (?1, ?2)
        "#,
    )
    .bind(subreddit)
    .bind(post_id)
    .execute(pool)
    .await?;

    Ok(res.rows_affected() == 1)
}

// =============================================================================
// TUI Database Functions
// =============================================================================

// --- Subscriptions CRUD ---

/// List all subscriptions with metadata
pub async fn list_subscriptions(pool: &SqlitePool) -> Result<Vec<SubscriptionRow>> {
    let rows = sqlx::query(
        r#"
        SELECT
            s.id,
            s.subreddit,
            s.created_at,
            COUNT(se.endpoint_id) as endpoint_count
        FROM subscriptions s
        LEFT JOIN subscription_endpoints se ON se.subscription_id = s.id
        GROUP BY s.id, s.subreddit, s.created_at
        ORDER BY s.created_at DESC
        "#,
    )
    .map(|row: SqliteRow| SubscriptionRow {
        id: row.get::<i64, _>("id"),
        subreddit: row.get::<String, _>("subreddit"),
        created_at: row.get::<String, _>("created_at"),
    })
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Create a new subscription
pub async fn create_subscription(pool: &SqlitePool, subreddit: &str) -> Result<i64> {
    let res = sqlx::query(
        r#"
        INSERT INTO subscriptions (subreddit)
        VALUES (?1)
        "#,
    )
    .bind(subreddit)
    .execute(pool)
    .await?;

    Ok(res.last_insert_rowid())
}

/// Delete a subscription (cascade deletes links)
pub async fn delete_subscription(pool: &SqlitePool, id: i64) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM subscriptions WHERE id = ?1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get all endpoints linked to a subscription
pub async fn get_subscription_endpoints(pool: &SqlitePool, subscription_id: i64) -> Result<Vec<EndpointRow>> {
    let rows = sqlx::query(
        r#"
        SELECT e.id, e.kind, e.config_json, e.active, e.note
        FROM endpoints e
        JOIN subscription_endpoints se ON se.endpoint_id = e.id
        WHERE se.subscription_id = ?1
        ORDER BY e.id
        "#,
    )
    .bind(subscription_id)
    .fetch_all(pool)
    .await?;

    // Parse each row and skip any with invalid endpoint kinds
    let mut endpoints = Vec::new();
    for row in rows {
        let id = row.get::<i64, _>("id");
        let kind_str = row.get::<String, _>("kind");

        // Try to parse the kind - if it fails, log a warning and skip this endpoint
        let kind = match kind_str.parse::<EndpointKind>() {
            Ok(k) => k,
            Err(_) => {
                tracing::warn!("Invalid endpoint kind '{}' for endpoint id {} - skipping", kind_str, id);
                continue; // Skip this endpoint
            }
        };

        endpoints.push(EndpointRow {
            id,
            kind,
            config_json: row.get::<String, _>("config_json"),
            active: row.get::<i64, _>("active") != 0,
            note: row.get::<Option<String>, _>("note"),
        });
    }

    Ok(endpoints)
}

// --- Endpoints CRUD ---

/// List all endpoints with type and active status
pub async fn list_endpoints(pool: &SqlitePool) -> Result<Vec<EndpointRow>> {
    let rows = sqlx::query(
        r#"
        SELECT id, kind, config_json, active, note
        FROM endpoints
        ORDER BY id
        "#,
    )
    .fetch_all(pool)
    .await?;

    // Parse each row and skip any with invalid endpoint kinds
    let mut endpoints = Vec::new();
    for row in rows {
        let id = row.get::<i64, _>("id");
        let kind_str = row.get::<String, _>("kind");

        // Try to parse the kind - if it fails, log a warning and skip this endpoint
        let kind = match kind_str.parse::<EndpointKind>() {
            Ok(k) => k,
            Err(_) => {
                tracing::warn!("Invalid endpoint kind '{}' for endpoint id {} - skipping", kind_str, id);
                continue; // Skip this endpoint
            }
        };

        endpoints.push(EndpointRow {
            id,
            kind,
            config_json: row.get::<String, _>("config_json"),
            active: row.get::<i64, _>("active") != 0,
            note: row.get::<Option<String>, _>("note"),
        });
    }

    Ok(endpoints)
}

/// Get a single endpoint by ID
pub async fn get_endpoint(pool: &SqlitePool, id: i64) -> Result<EndpointRow> {
    let row = sqlx::query(
        r#"
        SELECT id, kind, config_json, active, note
        FROM endpoints
        WHERE id = ?1
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    // Extract the fields from the database row
    let endpoint_id = row.get::<i64, _>("id");
    let kind_str = row.get::<String, _>("kind");

    // Try to parse the kind - return an error if it's invalid (for single item, we can't skip)
    let kind = kind_str
        .parse::<EndpointKind>()
        .map_err(|_| anyhow::anyhow!("Invalid endpoint kind '{}' for endpoint id {}", kind_str, endpoint_id))?;

    Ok(EndpointRow {
        id: endpoint_id,
        kind,
        config_json: row.get::<String, _>("config_json"),
        active: row.get::<i64, _>("active") != 0,
        note: row.get::<Option<String>, _>("note"),
    })
}

/// Create a new endpoint
pub async fn create_endpoint(pool: &SqlitePool, kind: &str, config_json: &str, note: Option<&str>) -> Result<i64> {
    let res = sqlx::query(
        r#"
        INSERT INTO endpoints (kind, config_json, note)
        VALUES (?1, ?2, ?3)
        "#,
    )
    .bind(kind)
    .bind(config_json)
    .bind(note)
    .execute(pool)
    .await?;

    Ok(res.last_insert_rowid())
}

/// Update an endpoint's configuration and note
pub async fn update_endpoint(pool: &SqlitePool, id: i64, config_json: &str, note: Option<&str>) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE endpoints
        SET config_json = ?1, note = ?2
        WHERE id = ?3
        "#,
    )
    .bind(config_json)
    .bind(note)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Delete an endpoint (cascade deletes links)
pub async fn delete_endpoint(pool: &SqlitePool, id: i64) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM endpoints WHERE id = ?1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Toggle an endpoint's active status, returns new status
pub async fn toggle_endpoint_active(pool: &SqlitePool, id: i64) -> Result<bool> {
    // Atomically toggle using SQL (1 - active flips 0->1 and 1->0)
    let row = sqlx::query(
        r#"
        UPDATE endpoints
        SET active = 1 - active
        WHERE id = ?1
        RETURNING active
        "#,
    )
    .bind(id)
    .map(|row: SqliteRow| row.get::<i64, _>("active") != 0)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

// --- Junction Table Management ---

/// Link a subscription to an endpoint
pub async fn link_subscription_endpoint(pool: &SqlitePool, subscription_id: i64, endpoint_id: i64) -> Result<()> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO subscription_endpoints (subscription_id, endpoint_id)
        VALUES (?1, ?2)
        "#,
    )
    .bind(subscription_id)
    .bind(endpoint_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Unlink a subscription from an endpoint
pub async fn unlink_subscription_endpoint(pool: &SqlitePool, subscription_id: i64, endpoint_id: i64) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM subscription_endpoints
        WHERE subscription_id = ?1 AND endpoint_id = ?2
        "#,
    )
    .bind(subscription_id)
    .bind(endpoint_id)
    .execute(pool)
    .await?;

    Ok(())
}

// --- Logs ---

/// List notified posts with pagination
pub async fn list_notified_posts(pool: &SqlitePool, limit: i64, offset: i64) -> Result<Vec<NotifiedPostRow>> {
    let rows = sqlx::query(
        r#"
        SELECT id, subreddit, post_id, first_seen_at
        FROM notified_posts
        ORDER BY first_seen_at DESC
        LIMIT ?1 OFFSET ?2
        "#,
    )
    .bind(limit)
    .bind(offset)
    .map(|row: SqliteRow| NotifiedPostRow {
        id: row.get::<i64, _>("id"),
        subreddit: row.get::<String, _>("subreddit"),
        post_id: row.get::<String, _>("post_id"),
        first_seen_at: row.get::<String, _>("first_seen_at"),
    })
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// List notified posts filtered by subreddit with pagination
pub async fn list_notified_posts_by_subreddit(pool: &SqlitePool, subreddit: &str, limit: i64, offset: i64) -> Result<Vec<NotifiedPostRow>> {
    let rows = sqlx::query(
        r#"
        SELECT id, subreddit, post_id, first_seen_at
        FROM notified_posts
        WHERE subreddit = ?1
        ORDER BY first_seen_at DESC
        LIMIT ?2 OFFSET ?3
        "#,
    )
    .bind(subreddit)
    .bind(limit)
    .bind(offset)
    .map(|row: SqliteRow| NotifiedPostRow {
        id: row.get::<i64, _>("id"),
        subreddit: row.get::<String, _>("subreddit"),
        post_id: row.get::<String, _>("post_id"),
        first_seen_at: row.get::<String, _>("first_seen_at"),
    })
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Delete a notified post by ID
pub async fn delete_notified_post(pool: &SqlitePool, id: i64) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM notified_posts WHERE id = ?1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Clean up old notified posts, deleting records older than the specified number of days
///
/// This prevents unbounded growth of the notified_posts table. Since the application
/// only notifies on posts within 24 hours of the current time (see poller.rs), we can
/// safely delete older records without risk of duplicate notifications.
///
/// # Arguments
/// * `pool` - Database connection pool
/// * `days_to_keep` - Number of days of history to keep (default: 7)
///
/// # Returns
/// Number of records deleted
pub async fn cleanup_old_posts(pool: &SqlitePool, days_to_keep: i64) -> Result<u64> {
    let result = sqlx::query(
        r#"
        DELETE FROM notified_posts
        WHERE first_seen_at < datetime('now', '-' || ?1 || ' days')
        "#,
    )
    .bind(days_to_keep)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Get statistics about notified posts per subreddit
///
/// Useful for monitoring database growth and cleanup effectiveness
pub async fn get_post_statistics(pool: &SqlitePool) -> Result<Vec<(String, i64)>> {
    let rows = sqlx::query(
        r#"
        SELECT subreddit, COUNT(*) as count
        FROM notified_posts
        GROUP BY subreddit
        ORDER BY count DESC
        "#,
    )
    .map(|row: sqlx::sqlite::SqliteRow| {
        (
            row.get::<String, _>("subreddit"),
            row.get::<i64, _>("count"),
        )
    })
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cleanup_old_posts() {
        // Create an in-memory test database
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        // Insert test data with different ages
        // Recent posts (within 7 days) - should NOT be deleted
        for i in 1..=3 {
            sqlx::query(
                "INSERT INTO notified_posts (subreddit, post_id, first_seen_at) VALUES (?1, ?2, datetime('now', ?3))",
            )
            .bind("testA")
            .bind(format!("recent_{}", i))
            .bind(format!("-{} days", i))
            .execute(&pool)
            .await
            .unwrap();
        }

        // Old posts (older than 7 days) - should be deleted
        for i in 1..=4 {
            sqlx::query(
                "INSERT INTO notified_posts (subreddit, post_id, first_seen_at) VALUES (?1, ?2, datetime('now', ?3))",
            )
            .bind("testB")
            .bind(format!("old_{}", i))
            .bind(format!("-{} days", 7 + i))
            .execute(&pool)
            .await
            .unwrap();
        }

        // Clean up posts older than 7 days
        let deleted = cleanup_old_posts(&pool, 7).await.unwrap();

        // Should delete 4 old posts
        assert_eq!(deleted, 4);

        // Verify 3 recent posts remain
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notified_posts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(remaining, 3);
    }
}
