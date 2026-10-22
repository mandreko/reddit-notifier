use anyhow::Result;
use chrono::{DateTime, TimeDelta, Utc};
use reqwest::Client;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

use crate::models::{database::EndpointRow, RedditPost};
use crate::rate_limiter::RateLimiter;
use crate::services::DatabaseService;
use crate::sources::{post_url, FetchError, PostSource};

/// Posts older (or, for clock-skew safety, newer) than this are ignored. Sources have
/// been observed to return stale items, and the Arctic Shift query window matches it.
pub const FRESHNESS_WINDOW: TimeDelta = TimeDelta::hours(24);
/// Extra wait after a cycle in which every batch failed (upstream outage).
const OUTAGE_BACKOFF: Duration = Duration::from_secs(30);
/// Wait before retrying when the subreddit→endpoint mapping cannot be loaded.
const MAPPING_RETRY_DELAY: Duration = Duration::from_secs(5);

/// Counters for one pass over every batch.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CycleStats {
    pub batches_ok: usize,
    pub batches_failed: usize,
    pub posts_seen: usize,
    pub posts_notified: usize,
    /// The endpoint mapping could not be loaded; no batches were attempted.
    pub mapping_failed: bool,
}

impl CycleStats {
    fn all_batches_failed(&self) -> bool {
        self.batches_ok == 0 && (self.batches_failed > 0 || self.mapping_failed)
    }
}

/// Run the poller forever.
///
/// Subreddits are chunked by `source.batch_size()`; each chunk costs one token from
/// `rate_limiter`. A cycle visits every chunk once, logs a summary, then waits until
/// `min_cycle_interval` has elapsed since the cycle began (plus a flat backoff if the
/// whole cycle failed). Set `min_cycle_interval` to zero to let the rate limiter alone
/// pace requests.
pub async fn poll_loop<D: DatabaseService>(
    db: Arc<D>,
    source: Box<dyn PostSource>,
    client: Client,
    subreddits: Vec<String>,
    rate_limiter: RateLimiter,
    min_cycle_interval: Duration,
) -> Result<()> {
    if subreddits.is_empty() {
        info!("No subreddits to poll");
        return Ok(());
    }

    let batches = chunk_subreddits(&subreddits, source.batch_size());

    info!(
        target: "reddit_notifier",
        "Polling {} subreddit(s) in {} batch(es) via {}",
        subreddits.len(),
        batches.len(),
        source.name()
    );

    loop {
        let started = Instant::now();
        let stats = poll_once(db.as_ref(), source.as_ref(), &client, &batches, &rate_limiter).await;

        info!(
            "Cycle done via {}: {} batch(es) ok, {} failed, {} post(s) seen, {} notified, {:.1}s",
            source.name(),
            stats.batches_ok,
            stats.batches_failed,
            stats.posts_seen,
            stats.posts_notified,
            started.elapsed().as_secs_f64()
        );

        let mut wait = min_cycle_interval.saturating_sub(started.elapsed());
        if stats.mapping_failed {
            wait = wait.max(MAPPING_RETRY_DELAY);
        }
        if stats.all_batches_failed() {
            warn!(
                "Every batch failed this cycle; backing off an extra {}s",
                OUTAGE_BACKOFF.as_secs()
            );
            wait += OUTAGE_BACKOFF;
        }
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }
}

/// Split subreddits into batches no larger than the source accepts.
pub fn chunk_subreddits(subreddits: &[String], batch_size: usize) -> Vec<Vec<String>> {
    subreddits
        .chunks(batch_size.max(1))
        .map(|chunk| chunk.to_vec())
        .collect()
}

/// One pass over every batch. Never aborts early: a failing batch is logged and the
/// next one is attempted. A 429 is honoured by sleeping for the advertised duration.
pub async fn poll_once<D: DatabaseService>(
    db: &D,
    source: &dyn PostSource,
    client: &Client,
    batches: &[Vec<String>],
    rate_limiter: &RateLimiter,
) -> CycleStats {
    let mut stats = CycleStats::default();

    // Fetch the subreddit-to-endpoints mapping once per cycle so endpoint changes made
    // in the TUI are picked up without a restart.
    let mappings = match db.all_subreddit_endpoint_mappings().await {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to fetch subreddit-endpoint mappings: {} - will retry", e);
            stats.mapping_failed = true;
            return stats;
        }
    };

    for batch in batches {
        rate_limiter.acquire().await;

        match source.fetch_new(batch).await {
            Ok(posts) => {
                stats.batches_ok += 1;
                stats.posts_seen += posts.len();
                debug!("{}: fetched {} post(s) for {:?}", source.name(), posts.len(), batch);

                let now = Utc::now();
                for post in &posts {
                    if process_post(db, &mappings, client, post, now).await {
                        stats.posts_notified += 1;
                    }
                }
            }
            Err(FetchError::RateLimited { retry_after }) => {
                stats.batches_failed += 1;
                warn!(
                    "{}: rate limited fetching {:?}; sleeping {}s",
                    source.name(),
                    batch,
                    retry_after.as_secs()
                );
                tokio::time::sleep(retry_after).await;
            }
            Err(e) => {
                stats.batches_failed += 1;
                warn!("{}: fetch failed for {:?}: {}", source.name(), batch, e);
            }
        }
    }

    stats
}

/// Filter, deduplicate, and dispatch one post. Returns `true` when the post was new
/// and notifications were attempted for it.
async fn process_post<D: DatabaseService>(
    db: &D,
    mappings: &HashMap<String, Vec<EndpointRow>>,
    client: &Client,
    post: &RedditPost,
    now: DateTime<Utc>,
) -> bool {
    let subreddit = &post.subreddit;

    if !is_within_window(post.created_utc, now, FRESHNESS_WINDOW) {
        debug!(
            "Skipping post {} from r/{} - outside {}h window (posted: {})",
            post.id,
            subreddit,
            FRESHNESS_WINDOW.num_hours(),
            post.created_utc
        );
        return false;
    }

    // Record before notifying so a crash mid-dispatch cannot cause duplicate alerts.
    let is_new = match db.record_if_new(subreddit, &post.id).await {
        Ok(new) => new,
        Err(e) => {
            error!(
                "Failed to record post {} for r/{}: {} - skipping this post",
                post.id, subreddit, e
            );
            return false;
        }
    };
    if !is_new {
        return false;
    }

    let endpoints = match mappings.get(subreddit) {
        Some(eps) => eps,
        None => {
            // Mappings can change between cycles, or the source may return a
            // differently-cased subreddit name than the one subscribed.
            info!("No endpoints for r/{}, skipping post {}", subreddit, post.id);
            return false;
        }
    };

    // The same endpoint can be linked through several subscriptions.
    let mut seen_endpoint_ids = HashSet::new();
    let unique_endpoints: Vec<&EndpointRow> = endpoints
        .iter()
        .filter(|e| seen_endpoint_ids.insert(e.id))
        .collect();

    let url = post_url(post);

    info!(
        "New post in r/{}: {} -> notifying {} endpoint(s)",
        subreddit,
        post.title,
        unique_endpoints.len()
    );

    for ep in unique_endpoints {
        match crate::notifiers::build_notifier(ep, client.clone()) {
            Ok(notifier) => {
                if let Err(e) = notifier.send(subreddit, &post.title, &url).await {
                    error!("Notify error ({} id={}): {}", notifier.kind(), ep.id, e);
                }
            }
            Err(e) => {
                error!("Build notifier failed for endpoint id {}: {}", ep.id, e);
            }
        }
    }

    true
}

/// `true` when `created` is within `window` of `now` in either direction.
pub(crate) fn is_within_window(created: DateTime<Utc>, now: DateTime<Utc>, window: TimeDelta) -> bool {
    now.signed_duration_since(created).abs() <= window
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::mock_database::MockDatabaseService;
    use async_trait::async_trait;
    use chrono::TimeZone;
    use std::sync::Mutex;
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// A source that replays scripted results, one per `fetch_new` call.
    struct FakeSource {
        batch_size: usize,
        script: Mutex<Vec<Result<Vec<RedditPost>, FetchError>>>,
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl FakeSource {
        fn new(batch_size: usize, script: Vec<Result<Vec<RedditPost>, FetchError>>) -> Self {
            // Scripts are consumed from the front.
            let mut script = script;
            script.reverse();
            Self { batch_size, script: Mutex::new(script), calls: Mutex::new(Vec::new()) }
        }
    }

    #[async_trait]
    impl PostSource for FakeSource {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn batch_size(&self) -> usize {
            self.batch_size
        }
        async fn fetch_new(&self, subreddits: &[String]) -> Result<Vec<RedditPost>, FetchError> {
            self.calls.lock().unwrap().push(subreddits.to_vec());
            self.script.lock().unwrap().pop().unwrap_or_else(|| Ok(Vec::new()))
        }
    }

    fn post(id: &str, subreddit: &str, created: DateTime<Utc>) -> RedditPost {
        RedditPost {
            id: id.to_string(),
            title: format!("Title {id}"),
            subreddit: subreddit.to_string(),
            permalink: None,
            url: None,
            created_utc: created,
        }
    }

    fn limiter() -> RateLimiter {
        RateLimiter::new(1000, Duration::from_millis(1))
    }

    fn batches(names: &[&str]) -> Vec<Vec<String>> {
        names.iter().map(|n| vec![n.to_string()]).collect()
    }

    /// A mock DB with one subscription to `rust` routed to a Discord webhook on `server`.
    async fn db_with_discord(server: &MockServer) -> MockDatabaseService {
        let db = MockDatabaseService::new();
        let sub = db.create_subscription("rust").await.unwrap();
        let cfg = format!(r#"{{"webhook_url":"{}/webhook"}}"#, server.uri());
        let ep = db.create_endpoint("discord", &cfg, None).await.unwrap();
        db.link_subscription_endpoint(sub, ep).await.unwrap();
        db
    }

    async fn mount_webhook(server: &MockServer, expected_hits: u64) {
        Mock::given(method("POST"))
            .and(path("/webhook"))
            .respond_with(ResponseTemplate::new(204))
            .expect(expected_hits)
            .mount(server)
            .await;
    }

    #[test]
    fn window_edges() {
        let now = Utc.timestamp_opt(1_792_600_000, 0).unwrap();
        let w = TimeDelta::hours(24);
        assert!(is_within_window(now, now, w));
        assert!(is_within_window(now - TimeDelta::hours(24), now, w));
        assert!(is_within_window(now + TimeDelta::hours(24), now, w));
        assert!(!is_within_window(now - TimeDelta::hours(24) - TimeDelta::seconds(1), now, w));
        assert!(!is_within_window(now + TimeDelta::hours(25), now, w));
    }

    #[test]
    fn chunking_respects_batch_size_and_guards_zero() {
        let subs: Vec<String> = (0..5).map(|i| format!("s{i}")).collect();
        assert_eq!(chunk_subreddits(&subs, 2).len(), 3);
        assert_eq!(chunk_subreddits(&subs, 100).len(), 1);
        assert_eq!(chunk_subreddits(&subs, 0).len(), 5);
        assert!(chunk_subreddits(&[], 1).is_empty());
    }

    #[test]
    fn all_batches_failed_detection() {
        assert!(CycleStats { batches_failed: 2, ..Default::default() }.all_batches_failed());
        assert!(CycleStats { mapping_failed: true, ..Default::default() }.all_batches_failed());
        assert!(!CycleStats { batches_ok: 1, batches_failed: 2, ..Default::default() }.all_batches_failed());
        assert!(!CycleStats::default().all_batches_failed());
    }

    #[tokio::test]
    async fn poll_once_notifies_new_post_with_constructed_url() {
        let server = MockServer::start().await;
        mount_webhook(&server, 1).await;
        Mock::given(method("POST"))
            .and(path("/webhook"))
            .and(body_partial_json(serde_json::json!({
                "embeds": [{"url": "https://www.reddit.com/r/rust/comments/p1/"}]
            })))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let db = db_with_discord(&server).await;
        let source = FakeSource::new(1, vec![Ok(vec![post("p1", "rust", Utc::now())])]);

        let stats = poll_once(&db, &source, &Client::new(), &batches(&["rust"]), &limiter()).await;

        assert_eq!(stats, CycleStats { batches_ok: 1, posts_seen: 1, posts_notified: 1, ..Default::default() });
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
        assert_eq!(*source.calls.lock().unwrap(), vec![vec!["rust".to_string()]]);
    }

    #[tokio::test]
    async fn poll_once_skips_duplicate_post() {
        let server = MockServer::start().await;
        mount_webhook(&server, 1).await;
        let db = db_with_discord(&server).await;
        let p = post("dup", "rust", Utc::now());
        let source = FakeSource::new(1, vec![Ok(vec![p.clone()]), Ok(vec![p])]);
        let b = batches(&["rust"]);

        let first = poll_once(&db, &source, &Client::new(), &b, &limiter()).await;
        let second = poll_once(&db, &source, &Client::new(), &b, &limiter()).await;

        assert_eq!(first.posts_notified, 1);
        assert_eq!(second.posts_notified, 0);
        assert_eq!(second.posts_seen, 1);
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn poll_once_skips_post_outside_window_and_does_not_record_it() {
        let server = MockServer::start().await;
        mount_webhook(&server, 0).await;
        let db = db_with_discord(&server).await;
        let stale = post("old", "rust", Utc::now() - TimeDelta::hours(48));
        let source = FakeSource::new(1, vec![Ok(vec![stale])]);

        let stats = poll_once(&db, &source, &Client::new(), &batches(&["rust"]), &limiter()).await;

        assert_eq!(stats.posts_seen, 1);
        assert_eq!(stats.posts_notified, 0);
        assert!(db.list_notified_posts(10, 0).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn poll_once_skips_post_for_unsubscribed_subreddit() {
        let server = MockServer::start().await;
        mount_webhook(&server, 0).await;
        let db = db_with_discord(&server).await;
        let source = FakeSource::new(1, vec![Ok(vec![post("x", "golang", Utc::now())])]);

        let stats = poll_once(&db, &source, &Client::new(), &batches(&["golang"]), &limiter()).await;

        assert_eq!(stats.posts_notified, 0);
    }

    #[tokio::test]
    async fn poll_once_continues_after_failed_batch() {
        let server = MockServer::start().await;
        mount_webhook(&server, 1).await;
        let db = db_with_discord(&server).await;
        let source = FakeSource::new(
            1,
            vec![
                Err(FetchError::Api("boom".into())),
                Ok(vec![post("p2", "rust", Utc::now())]),
            ],
        );

        let stats = poll_once(&db, &source, &Client::new(), &batches(&["golang", "rust"]), &limiter()).await;

        assert_eq!(stats.batches_failed, 1);
        assert_eq!(stats.batches_ok, 1);
        assert_eq!(stats.posts_notified, 1);
        assert_eq!(source.calls.lock().unwrap().len(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn poll_once_sleeps_on_rate_limit_then_continues() {
        let db = MockDatabaseService::new();
        db.create_subscription("rust").await.unwrap();
        let source = FakeSource::new(
            1,
            vec![
                Err(FetchError::RateLimited { retry_after: Duration::from_secs(45) }),
                Ok(vec![]),
            ],
        );
        let started = tokio::time::Instant::now();

        let stats = poll_once(&db, &source, &Client::new(), &batches(&["a", "b"]), &limiter()).await;

        assert_eq!(stats.batches_failed, 1);
        assert_eq!(stats.batches_ok, 1);
        assert!(started.elapsed() >= Duration::from_secs(45));
    }

    #[tokio::test]
    async fn poll_once_notifier_failure_is_logged_not_fatal() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&server)
            .await;
        let db = db_with_discord(&server).await;
        let source = FakeSource::new(1, vec![Ok(vec![post("p1", "rust", Utc::now())])]);

        let stats = poll_once(&db, &source, &Client::new(), &batches(&["rust"]), &limiter()).await;

        // The post counts as handled (and is recorded) even though delivery failed.
        assert_eq!(stats.posts_notified, 1);
        assert_eq!(db.list_notified_posts(10, 0).await.unwrap().len(), 1);
    }
}
