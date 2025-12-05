use anyhow::Result;
use reqwest::Client;
use sqlx::SqlitePool;
use std::collections::HashSet;
use tracing::{info, warn, error};
use chrono::{Utc, TimeDelta};

use crate::database::{all_subreddit_endpoint_mappings, record_if_new};
use crate::models::{database::EndpointRow, reddit_api::RedditListing};
use crate::rate_limiter::RateLimiter;

/// Combined subreddit poller - polls multiple subreddits in a single API call
///
/// This is more efficient than spawning one poller per subreddit. Reddit allows
/// combining up to 100 subreddits in a single URL using the format:
/// `/r/sub1+sub2+sub3/new.json`
///
/// Benefits:
/// - Fewer API calls to Reddit (better for rate limiting)
/// - More efficient resource usage
/// - Easier to implement global rate limiting
///
/// # Arguments
/// * `pool` - Database connection pool
/// * `client` - HTTP client for making Reddit API calls
/// * `subreddits` - List of subreddit names to poll (will be automatically batched)
/// * `rate_limiter` - Rate limiter to respect Reddit's API limits
///
/// # Polling Behavior
/// The poller runs continuously, making API calls as fast as the rate limiter allows.
/// Configure the rate limiter (via REDDIT_RATE_LIMIT_PER_MINUTE) to control polling frequency.
/// Default: 20 requests/minute. Reddit's limit is approximately 60 requests/minute.
pub async fn poll_combined_subreddits_loop(
    pool: SqlitePool,
    client: Client,
    subreddits: Vec<String>,
    rate_limiter: RateLimiter,
) -> Result<()> {
    if subreddits.is_empty() {
        info!("No subreddits to poll");
        return Ok(());
    }

    // Reddit allows up to 100 subreddits in a multi-subreddit URL
    const MAX_SUBREDDITS_PER_BATCH: usize = 100;

    // Split subreddits into batches if there are more than 100
    let batches: Vec<Vec<String>> = subreddits
        .chunks(MAX_SUBREDDITS_PER_BATCH)
        .map(|chunk| chunk.to_vec())
        .collect();

    info!(
        target: "reddit_notifier",
        "Spawned combined poller for {} subreddit(s) across {} batch(es)",
        subreddits.len(),
        batches.len()
    );

    let reddit_base = "https://www.reddit.com";

    loop {
        // Fetch the subreddit-to-endpoints mapping once per poll cycle
        // This is more efficient than querying for each post
        let mappings = match all_subreddit_endpoint_mappings(&pool).await {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to fetch subreddit-endpoint mappings: {} - will retry", e);
                continue;
            }
        };

        // Poll each batch
        for batch in &batches {
            // Wait for rate limiter before making the API call
            rate_limiter.acquire().await;

            // Build the combined subreddit URL (e.g., /r/sub1+sub2+sub3/new.json)
            let combined_subreddit = batch.join("+");
            let json_url = format!("{}/r/{}/new.json?limit=100", reddit_base, combined_subreddit);

            match client.get(&json_url).send().await {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        warn!("Reddit GET {} -> {}", json_url, resp.status());
                        continue;
                    }

                    let listing: RedditListing = match resp.json().await {
                        Ok(v) => v,
                        Err(e) => {
                            warn!("Failed to parse Reddit JSON for combined URL: {}", e);
                            continue;
                        }
                    };

                    info!(
                        "Fetched {} posts from {} subreddit(s)",
                        listing.data.children.len(),
                        batch.len()
                    );

                    // Process each post
                    for child in listing.data.children {
                        let post = child.data;

                        // The post.subreddit field tells us which subreddit this post came from
                        // This is crucial for the combined poller approach
                        let subreddit = &post.subreddit;

                        // Check if post is within ±24 hours
                        // This was added because Reddit's API would randomly return old posts
                        let now = Utc::now();
                        let time_diff = now.signed_duration_since(post.created_utc);
                        let is_within_24h = time_diff.abs() <= TimeDelta::hours(24);
                        if !is_within_24h {
                            info!(
                                "Skipping post {} from r/{} - outside 24h window (posted: {})",
                                post.id, subreddit, post.created_utc
                            );
                            continue;
                        }

                        // Check if we've already notified about this post
                        let is_new = match record_if_new(&pool, subreddit, &post.id).await {
                            Ok(new) => new,
                            Err(e) => {
                                error!(
                                    "Failed to record post {} for r/{}: {} - skipping this post",
                                    post.id, subreddit, e
                                );
                                continue;
                            }
                        };
                        if !is_new {
                            continue; // Already seen this post
                        }

                        // Get endpoints for this specific subreddit from our mapping
                        let endpoints = match mappings.get(subreddit) {
                            Some(eps) => eps,
                            None => {
                                // No endpoints subscribed to this subreddit
                                // This can happen if mappings changed between poll cycles
                                info!("No endpoints for r/{}, skipping post {}", subreddit, post.id);
                                continue;
                            }
                        };

                        // Deduplicate endpoints (same endpoint might be subscribed multiple times)
                        let mut unique_endpoint_ids = HashSet::new();
                        let unique_endpoints: Vec<&EndpointRow> = endpoints
                            .iter()
                            .filter(|e| unique_endpoint_ids.insert(e.id))
                            .collect();

                        // Build the post URL
                        let url = post
                            .permalink
                            .as_ref()
                            .map(|p| format!("{}{}", reddit_base, p))
                            .or(post.url.clone())
                            .unwrap_or_else(|| {
                                format!("{}/r/{}/comments/{}", reddit_base, subreddit, post.id)
                            });

                        info!(
                            "New post in r/{}: {} -> notifying {} endpoint(s)",
                            subreddit,
                            post.title,
                            unique_endpoints.len()
                        );

                        // Send notifications to all endpoints
                        for ep in unique_endpoints {
                            let client_clone = client.clone();
                            match crate::notifiers::build_notifier(ep, client_clone) {
                                Ok(notifier) => {
                                    if let Err(e) =
                                        notifier.send(subreddit, &post.title, &url).await
                                    {
                                        error!(
                                            "Notify error ({} id={}): {}",
                                            notifier.kind(),
                                            ep.id,
                                            e
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!("Build notifier failed for endpoint id {}: {}", ep.id, e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("HTTP error fetching combined URL {}: {}", json_url, e);
                }
            }
        }
        // Loop continues immediately - rate limiter controls polling frequency
    }
}
