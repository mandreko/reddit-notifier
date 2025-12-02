use anyhow::Result;
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

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

pub async fn endpoints_for_subreddit(pool: &SqlitePool, subreddit: &str) -> Result<Vec<EndpointRow>> {
    let rows = sqlx::query(
        r#"
        SELECT e.id as id, e.kind as kind, e.config_json as config_json, e.active as active, e.note as note
        FROM endpoints e
        JOIN subscription_endpoints se ON se.endpoint_id = e.id
        JOIN subscriptions s ON s.id = se.subscription_id
        WHERE s.subreddit = ?1 AND e.active = 1
        "#,
    )
    .bind(subreddit)
    .fetch_all(pool)
    .await?;

    // Parse each row and skip any with invalid endpoint kinds
    let mut endpoints = Vec::new();
    for row in rows {
        let id = row.get::<i64, _>("id");
        let kind_str = row.get::<String, _>("kind");

        // Try to parse the kind - if it fails, log a warning and skip this endpoint
        let kind = match EndpointKind::from_str(&kind_str) {
            Some(k) => k,
            None => {
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
        let kind = match EndpointKind::from_str(&kind_str) {
            Some(k) => k,
            None => {
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
        let kind = match EndpointKind::from_str(&kind_str) {
            Some(k) => k,
            None => {
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
    let kind = match EndpointKind::from_str(&kind_str) {
        Some(k) => k,
        None => {
            return Err(anyhow::anyhow!(
                "Invalid endpoint kind '{}' for endpoint id {}",
                kind_str,
                endpoint_id
            ));
        }
    };

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
