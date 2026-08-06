use anyhow::Result;
use reqwest::Client;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn, error};
use chrono::{Utc, TimeDelta};

use crate::models::{database::EndpointRow, reddit_api::RedditListing};
use crate::rate_limiter::RateLimiter;
use crate::services::DatabaseService;

/// How long to wait before retrying after a database error or while no
/// subscriptions are configured.
const RETRY_DELAY: Duration = Duration::from_secs(30);

/// Upper bound on how long to honor a Retry-After header from Reddit.
const MAX_BACKOFF: Duration = Duration::from_secs(300);

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
/// The set of subreddits to poll is re-derived from the database every cycle,
/// so subscriptions added or removed via the TUI take effect without a restart.
///
/// # Arguments
/// * `db` - Database service
/// * `client` - HTTP client for making Reddit API calls
/// * `rate_limiter` - Rate limiter to respect Reddit's API limits
///
/// # Polling Behavior
/// The poller runs continuously, making API calls as fast as the rate limiter allows.
/// Configure the rate limiter (via REDDIT_RATE_LIMIT_PER_MINUTE) to control polling frequency.
/// Default: 20 requests/minute. Reddit's limit is approximately 60 requests/minute.
pub async fn poll_combined_subreddits_loop<D: DatabaseService>(
    db: Arc<D>,
    client: Client,
    rate_limiter: RateLimiter,
) -> Result<()> {
    // Reddit allows up to 100 subreddits in a multi-subreddit URL
    const MAX_SUBREDDITS_PER_BATCH: usize = 100;

    let reddit_base = "https://www.reddit.com";

    loop {
        // Fetch the subreddit-to-endpoints mapping once per poll cycle
        // This is more efficient than querying for each post
        let mappings = match db.all_subreddit_endpoint_mappings().await {
            Ok(m) => m,
            Err(e) => {
                error!(
                    "Failed to fetch subreddit-endpoint mappings: {} - retrying in {}s",
                    e,
                    RETRY_DELAY.as_secs()
                );
                tokio::time::sleep(RETRY_DELAY).await;
                continue;
            }
        };

        if mappings.is_empty() {
            info!(
                "No subreddits with active endpoints - checking again in {}s",
                RETRY_DELAY.as_secs()
            );
            tokio::time::sleep(RETRY_DELAY).await;
            continue;
        }

        // Rebuild batches from this cycle's mappings so both halves of the
        // configuration (subreddits and endpoints) refresh together
        let mut subreddits: Vec<&str> = mappings.keys().map(String::as_str).collect();
        subreddits.sort_unstable();
        let batches: Vec<Vec<&str>> = subreddits
            .chunks(MAX_SUBREDDITS_PER_BATCH)
            .map(|chunk| chunk.to_vec())
            .collect();

        // Poll each batch
        for batch in &batches {
            // Wait for rate limiter before making the API call
            rate_limiter.acquire().await;

            // Build the combined subreddit URL (e.g., /r/sub1+sub2+sub3/new.json)
            let combined_subreddit = batch.join("+");
            let json_url = format!("{}/r/{}/new.json?limit=100", reddit_base, combined_subreddit);

            match client.get(&json_url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if !status.is_success() {
                        // Back off when Reddit is throttling or failing, honoring
                        // Retry-After when present - hammering a 429 at full rate
                        // is how throttling escalates into a ban
                        if status == reqwest::StatusCode::TOO_MANY_REQUESTS
                            || status.is_server_error()
                        {
                            let retry_after = resp
                                .headers()
                                .get(reqwest::header::RETRY_AFTER)
                                .and_then(|v| v.to_str().ok())
                                .and_then(|s| s.parse::<u64>().ok())
                                .map(Duration::from_secs)
                                .unwrap_or(RETRY_DELAY)
                                .min(MAX_BACKOFF);
                            warn!(
                                "Reddit GET {} -> {} - backing off for {}s",
                                json_url,
                                status,
                                retry_after.as_secs()
                            );
                            tokio::time::sleep(retry_after).await;
                        } else {
                            let body = resp.text().await.unwrap_or_default();
                            let snippet: String = body.chars().take(200).collect();
                            warn!("Reddit GET {} -> {}: {}", json_url, status, snippet);
                        }
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

                    let notify = |ep: EndpointRow, subreddit: String, title: String, url: String| {
                        let client = client.clone();
                        async move {
                            let notifier = crate::notifiers::build_notifier(&ep, client)?;
                            notifier.send(&subreddit, &title, &url).await
                        }
                    };
                    process_listing(db.as_ref(), &mappings, listing, reddit_base, &notify).await;
                }
                Err(e) => {
                    warn!("HTTP error fetching combined URL {}: {}", json_url, e);
                }
            }
        }
        // Loop continues immediately - rate limiter controls polling frequency
    }
}

/// Process one fetched listing: filter posts, resolve endpoints, deliver, record.
///
/// Delivery is attempted *before* the post is recorded as notified, and the post
/// is only recorded when at least one endpoint succeeded - so a transient outage
/// results in a retry on the next cycle instead of a permanently lost
/// notification (at-least-once beats at-most-once for a notifier).
///
/// `notify` is injected so tests can exercise this logic without HTTP; production
/// passes a closure that builds the real notifier and sends.
async fn process_listing<D, F, Fut>(
    db: &D,
    mappings: &HashMap<String, Vec<EndpointRow>>,
    listing: RedditListing,
    reddit_base: &str,
    notify: &F,
) where
    D: DatabaseService,
    F: Fn(EndpointRow, String, String, String) -> Fut,
    Fut: Future<Output = Result<()>>,
{
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

        // Get endpoints for this specific subreddit from our mapping.
        // The map is keyed by lowercase name because Reddit returns the
        // canonical display name (e.g. "AskReddit") regardless of the
        // casing used in the request URL.
        let endpoints = match mappings.get(&subreddit.to_lowercase()) {
            Some(eps) => eps,
            None => {
                // No endpoints subscribed to this subreddit
                // This can happen if mappings changed between poll cycles
                info!("No endpoints for r/{}, skipping post {}", subreddit, post.id);
                continue;
            }
        };

        // Check if we've already notified about this post
        match db.is_post_notified(subreddit, &post.id).await {
            Ok(true) => continue, // Already seen this post
            Ok(false) => {}
            Err(e) => {
                error!(
                    "Failed to check post {} for r/{}: {} - skipping this post",
                    post.id, subreddit, e
                );
                continue;
            }
        }

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
        let endpoint_count = unique_endpoints.len();
        let mut successes = 0usize;
        for ep in unique_endpoints {
            match notify(ep.clone(), subreddit.clone(), post.title.clone(), url.clone()).await {
                Ok(()) => successes += 1,
                Err(e) => {
                    error!("Notify error ({} id={}): {}", ep.kind.as_str(), ep.id, e);
                }
            }
        }

        // Record the post only after at least one delivery succeeded
        if successes > 0 {
            if successes < endpoint_count {
                warn!(
                    "Post {} delivered to {}/{} endpoint(s) - failed endpoints will not be retried",
                    post.id, successes, endpoint_count
                );
            }
            if let Err(e) = db.record_if_new(subreddit, &post.id).await {
                error!(
                    "Failed to record post {} for r/{}: {} - post may be re-notified",
                    post.id, subreddit, e
                );
            }
        } else {
            warn!(
                "All {} deliveries failed for post {} in r/{} - will retry next cycle",
                endpoint_count, post.id, subreddit
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::database::EndpointKind;
    use crate::models::reddit_api::{RedditChild, RedditListingData, RedditPost};
    use crate::services::mock_database::MockDatabaseService;
    use std::sync::Mutex;

    const REDDIT_BASE: &str = "https://www.reddit.com";

    fn endpoint(id: i64) -> EndpointRow {
        EndpointRow {
            id,
            kind: EndpointKind::Discord,
            config_json: r#"{"webhook_url":"https://discord.com/api/webhooks/1/x"}"#.to_string(),
            active: true,
            note: None,
        }
    }

    fn listing(posts: Vec<(&str, &str, chrono::DateTime<Utc>)>) -> RedditListing {
        RedditListing {
            data: RedditListingData {
                children: posts
                    .into_iter()
                    .map(|(id, subreddit, created_utc)| RedditChild {
                        data: RedditPost {
                            id: id.to_string(),
                            title: format!("title-{}", id),
                            subreddit: subreddit.to_string(),
                            permalink: Some(format!("/r/{}/comments/{}", subreddit, id)),
                            url: None,
                            created_utc,
                        },
                    })
                    .collect(),
            },
        }
    }

    fn mappings_for(subreddit: &str, endpoints: Vec<EndpointRow>) -> HashMap<String, Vec<EndpointRow>> {
        HashMap::from([(subreddit.to_string(), endpoints)])
    }

    /// notify stub that records which endpoints were called and fails for
    /// endpoint ids listed in `fail_ids`
    struct NotifySpy {
        calls: Mutex<Vec<i64>>,
        fail_ids: Vec<i64>,
    }

    impl NotifySpy {
        fn new(fail_ids: Vec<i64>) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                fail_ids,
            }
        }

        async fn notify(&self, ep: EndpointRow) -> Result<()> {
            self.calls.lock().unwrap().push(ep.id);
            if self.fail_ids.contains(&ep.id) {
                anyhow::bail!("simulated delivery failure");
            }
            Ok(())
        }

        fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
    }

    #[tokio::test]
    async fn delivers_and_records_with_case_insensitive_lookup() {
        let db = MockDatabaseService::new();
        // Subscription stored lowercase; Reddit returns canonical display name
        let mappings = mappings_for("askreddit", vec![endpoint(1)]);
        let spy = NotifySpy::new(vec![]);

        process_listing(
            &db,
            &mappings,
            listing(vec![("p1", "AskReddit", Utc::now())]),
            REDDIT_BASE,
            &|ep, _, _, _| spy.notify(ep),
        )
        .await;

        assert_eq!(spy.call_count(), 1);
        assert!(db.is_post_notified("AskReddit", "p1").await.unwrap());
    }

    #[tokio::test]
    async fn does_not_record_when_all_deliveries_fail() {
        let db = MockDatabaseService::new();
        let mappings = mappings_for("rust", vec![endpoint(1)]);
        let spy = NotifySpy::new(vec![1]);

        process_listing(
            &db,
            &mappings,
            listing(vec![("p1", "rust", Utc::now())]),
            REDDIT_BASE,
            &|ep, _, _, _| spy.notify(ep),
        )
        .await;

        assert_eq!(spy.call_count(), 1);
        // Not recorded -> the next cycle retries instead of losing the notification
        assert!(!db.is_post_notified("rust", "p1").await.unwrap());

        // Next cycle: delivery works again -> post is notified and recorded
        let spy = NotifySpy::new(vec![]);
        process_listing(
            &db,
            &mappings,
            listing(vec![("p1", "rust", Utc::now())]),
            REDDIT_BASE,
            &|ep, _, _, _| spy.notify(ep),
        )
        .await;

        assert_eq!(spy.call_count(), 1);
        assert!(db.is_post_notified("rust", "p1").await.unwrap());
    }

    #[tokio::test]
    async fn records_when_at_least_one_endpoint_succeeds() {
        let db = MockDatabaseService::new();
        let mappings = mappings_for("rust", vec![endpoint(1), endpoint(2)]);
        let spy = NotifySpy::new(vec![2]);

        process_listing(
            &db,
            &mappings,
            listing(vec![("p1", "rust", Utc::now())]),
            REDDIT_BASE,
            &|ep, _, _, _| spy.notify(ep),
        )
        .await;

        assert_eq!(spy.call_count(), 2);
        assert!(db.is_post_notified("rust", "p1").await.unwrap());
    }

    #[tokio::test]
    async fn skips_already_notified_posts() {
        let db = MockDatabaseService::new();
        db.record_if_new("rust", "p1").await.unwrap();
        let mappings = mappings_for("rust", vec![endpoint(1)]);
        let spy = NotifySpy::new(vec![]);

        process_listing(
            &db,
            &mappings,
            listing(vec![("p1", "rust", Utc::now())]),
            REDDIT_BASE,
            &|ep, _, _, _| spy.notify(ep),
        )
        .await;

        assert_eq!(spy.call_count(), 0);
    }

    #[tokio::test]
    async fn skips_posts_outside_24h_window_without_recording() {
        let db = MockDatabaseService::new();
        let mappings = mappings_for("rust", vec![endpoint(1)]);
        let spy = NotifySpy::new(vec![]);
        let old = Utc::now() - TimeDelta::hours(25);

        process_listing(
            &db,
            &mappings,
            listing(vec![("p1", "rust", old)]),
            REDDIT_BASE,
            &|ep, _, _, _| spy.notify(ep),
        )
        .await;

        assert_eq!(spy.call_count(), 0);
        assert!(!db.is_post_notified("rust", "p1").await.unwrap());
    }

    #[tokio::test]
    async fn skips_unmatched_subreddits_without_recording() {
        let db = MockDatabaseService::new();
        let mappings = mappings_for("rust", vec![endpoint(1)]);
        let spy = NotifySpy::new(vec![]);

        process_listing(
            &db,
            &mappings,
            listing(vec![("p1", "python", Utc::now())]),
            REDDIT_BASE,
            &|ep, _, _, _| spy.notify(ep),
        )
        .await;

        assert_eq!(spy.call_count(), 0);
        // Not recorded -> a subscription added within the 24h window still fires
        assert!(!db.is_post_notified("python", "p1").await.unwrap());
    }

    #[tokio::test]
    async fn deduplicates_endpoints_before_delivery() {
        let db = MockDatabaseService::new();
        // Same endpoint linked via two subscriptions
        let mappings = mappings_for("rust", vec![endpoint(1), endpoint(1)]);
        let spy = NotifySpy::new(vec![]);

        process_listing(
            &db,
            &mappings,
            listing(vec![("p1", "rust", Utc::now())]),
            REDDIT_BASE,
            &|ep, _, _, _| spy.notify(ep),
        )
        .await;

        assert_eq!(spy.call_count(), 1);
    }
}
