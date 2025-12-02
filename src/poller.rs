use anyhow::Result;
use reqwest::Client;
use sqlx::SqlitePool;
use std::{collections::HashSet, time::Duration};
use tracing::{info, warn, error};
use chrono::{Utc, TimeDelta};

use crate::database::{endpoints_for_subreddit, record_if_new};
use crate::models::RedditListing;

pub async fn poll_subreddit_loop(
    pool: SqlitePool,
    client: Client,
    subreddit: String,
    poll_interval_secs: u64,
) -> Result<()> {
    info!(target: "reddit_notifier", "Spawned poller for r/{}", subreddit);
    let reddit_base = "https://www.reddit.com";
    let json_url = format!("{}/r/{}/new.json?limit=10", reddit_base, subreddit);

    loop {
        match client.get(&json_url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    warn!("Reddit GET {} -> {}", json_url, resp.status());
                } else {
                    let listing: RedditListing = match resp.json().await {
                        Ok(v) => v,
                        Err(e) => {
                            warn!("Failed to parse Reddit JSON for r/{}: {}", subreddit, e);
                            tokio::time::sleep(Duration::from_secs(poll_interval_secs)).await;
                            continue;
                        }
                    };

                    let endpoints = match endpoints_for_subreddit(&pool, &subreddit).await {
                        Ok(eps) => eps,
                        Err(e) => {
                            error!("Failed to fetch endpoints for r/{}: {} - will retry next iteration", subreddit, e);
                            tokio::time::sleep(Duration::from_secs(poll_interval_secs)).await;
                            continue;
                        }
                    };
                    let mut unique_endpoint_ids = HashSet::new();
                    let endpoints: Vec<_> = endpoints.into_iter().filter(|e| unique_endpoint_ids.insert(e.id)).collect();

                    for child in listing.data.children {
                        let post = child.data;

                        // Check if post is within ±24 hours
                        // This was added because Reddit's API would randomly return old posts that we alerted on and users hated
                        let now = Utc::now();
                        let time_diff = now.signed_duration_since(post.created_utc);
                        let is_within_24h = time_diff.abs() <= TimeDelta::hours(24);
                        if !is_within_24h {
                            info!("Skipping post {} - outside 24h window (posted: {})", post.id, post.created_utc);
                            continue;
                        }

                        let is_new = match record_if_new(&pool, &subreddit, &post.id).await {
                            Ok(new) => new,
                            Err(e) => {
                                error!("Failed to record post {} for r/{}: {} - skipping this post", post.id, subreddit, e);
                                continue;
                            }
                        };
                        if !is_new {
                            info!("Skipping post {} - already seen", post.id);
                            continue;
                        }

                        let url = post
                            .permalink
                            .as_ref()
                            .map(|p| format!("{}{}", reddit_base, p))
                            .or(post.url.clone())
                            .unwrap_or_else(|| format!("{}/r/{}/comments/{}", reddit_base, subreddit, post.id));

                        info!("New post in r/{}: {} -> notifying {} endpoint(s)", subreddit, post.title, endpoints.len());
                        for ep in &endpoints {
                            let client_clone = client.clone();
                            match crate::notifiers::build_notifier(ep, client_clone) {
                                Ok(notifier) => {
                                    if let Err(e) = notifier.send(&subreddit, &post.title, &url).await {
                                        error!("Notify error ({} id={}): {}", notifier.kind(), ep.id, e);
                                    }
                                }
                                Err(e) => {
                                    error!("Build notifier failed for endpoint id {}: {}", ep.id, e);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!("HTTP error fetching r/{}: {}", subreddit, e);
            }
        }

        tokio::time::sleep(Duration::from_secs(poll_interval_secs)).await;
    }
}
