use anyhow::Result;
use sqlx::{Row, Postgres};

use crate::models::{EndpointKind, EndpointRow};

pub async fn unique_subreddits(pool: &sqlx::Pool<Postgres>) -> Result<Vec<String>> {
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT s.subreddit
        FROM subscriptions s
        JOIN subscription_endpoints se ON se.subscription_id = s.id
        JOIN endpoints e ON e.id = se.endpoint_id
        WHERE e.active = true
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

pub async fn endpoints_for_subreddit(pool: &sqlx::Pool<Postgres>, subreddit: &str) -> Result<Vec<EndpointRow>> {
    let rows = sqlx::query(
        r#"
        SELECT e.id as id, e.kind as kind, e.config_json as config_json, e.active as active
        FROM endpoints e
        JOIN subscription_endpoints se ON se.endpoint_id = e.id
        JOIN subscriptions s ON s.id = se.subscription_id
        WHERE s.subreddit = $1 AND e.active = true
        "#,
    )
    .bind(subreddit)
        .map(|row: sqlx::postgres::PgRow| EndpointRow {
            id: row.get::<i32, _>("id"),
        kind: EndpointKind::from_str(row.get::<String, _>("kind").as_str()).unwrap(),
        config_json: row.get::<String, _>("config_json"),
        active: row.get::<bool, _>("active") != false,
    })
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Returns true if the (subreddit, post_id) was newly inserted.
pub async fn record_if_new(pool: &sqlx::Pool<Postgres>, subreddit: &str, post_id: &str) -> Result<bool> {
    let res = sqlx::query(
        r#"
        INSERT INTO notified_posts (subreddit, post_id)
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING;
        "#,
    )
    .bind(subreddit)
    .bind(post_id)
    .execute(pool)
    .await?;

    Ok(res.rows_affected() == 1)
}
