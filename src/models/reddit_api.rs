use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_with::{serde_as, TimestampSecondsWithFrac};

/// Reddit `Listing` envelope returned by `/r/<subs>/new.json`.
#[derive(Debug, Deserialize)]
pub struct RedditListing {
    pub data: RedditListingData,
}

impl RedditListing {
    /// Flatten the listing into its posts.
    pub fn into_posts(self) -> Vec<RedditPost> {
        self.data.children.into_iter().map(|c| c.data).collect()
    }
}

#[derive(Debug, Deserialize)]
pub struct RedditListingData {
    pub children: Vec<RedditChild>,
}

#[derive(Debug, Deserialize)]
pub struct RedditChild {
    pub data: RedditPost,
}

/// A Reddit post as serialized by both Reddit's own JSON API and the Arctic Shift
/// archive (which stores Reddit's objects verbatim). Only the fields the notifier
/// uses are deserialized; everything else is ignored.
#[serde_as]
#[derive(Debug, Clone, Deserialize)]
pub struct RedditPost {
    pub id: String,
    pub title: String,
    pub subreddit: String,
    #[serde(default)]
    pub permalink: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    /// Reddit emits this as a float (`1.7e9`), Arctic Shift as an integer; both parse.
    #[serde_as(as = "TimestampSecondsWithFrac<f64>")]
    pub created_utc: DateTime<Utc>,
}

/// Envelope returned by Arctic Shift's `/api/posts/search`.
///
/// Success: `{"data":[...]}`. Errors (400/422): `{"data":null,"error":"..."}`.
#[derive(Debug, Deserialize)]
pub struct ArcticShiftResponse {
    #[serde(default)]
    pub data: Option<Vec<RedditPost>>,
    #[serde(default)]
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reddit_post_parses_float_created_utc() {
        let json = r#"{"id":"abc","title":"T","subreddit":"rust","permalink":"/r/rust/comments/abc/t/","url":"https://x","created_utc":1792587240.0}"#;
        let post: RedditPost = serde_json::from_str(json).unwrap();
        assert_eq!(post.id, "abc");
        assert_eq!(post.created_utc.timestamp(), 1792587240);
        assert_eq!(post.permalink.as_deref(), Some("/r/rust/comments/abc/t/"));
    }

    #[test]
    fn reddit_post_parses_integer_created_utc_and_missing_optionals() {
        let json = r#"{"id":"abc","title":"T","subreddit":"rust","created_utc":1792587240}"#;
        let post: RedditPost = serde_json::from_str(json).unwrap();
        assert_eq!(post.created_utc.timestamp(), 1792587240);
        assert!(post.permalink.is_none());
        assert!(post.url.is_none());
    }

    #[test]
    fn reddit_post_ignores_unknown_fields() {
        let json = r#"{"id":"abc","title":"T","subreddit":"rust","created_utc":1,"score":5,"author":"me"}"#;
        assert!(serde_json::from_str::<RedditPost>(json).is_ok());
    }

    #[test]
    fn reddit_listing_flattens_children() {
        let json = r#"{"data":{"children":[
            {"kind":"t3","data":{"id":"a","title":"A","subreddit":"rust","created_utc":1.0}},
            {"kind":"t3","data":{"id":"b","title":"B","subreddit":"golang","created_utc":2.0}}
        ]}}"#;
        let listing: RedditListing = serde_json::from_str(json).unwrap();
        let posts = listing.into_posts();
        assert_eq!(posts.len(), 2);
        assert_eq!(posts[1].subreddit, "golang");
    }

    #[test]
    fn arctic_shift_response_success_shape() {
        let json = r#"{"data":[{"id":"a","title":"A","subreddit":"rust","created_utc":1792587240}]}"#;
        let resp: ArcticShiftResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.unwrap().len(), 1);
        assert!(resp.error.is_none());
    }

    #[test]
    fn arctic_shift_response_error_shape() {
        let json = r#"{"data":null,"error":"'limit' must be between 1 and 100"}"#;
        let resp: ArcticShiftResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_none());
        assert_eq!(resp.error.as_deref(), Some("'limit' must be between 1 and 100"));
    }

    #[test]
    fn arctic_shift_response_empty_data() {
        let resp: ArcticShiftResponse = serde_json::from_str(r#"{"data":[]}"#).unwrap();
        assert_eq!(resp.data.unwrap().len(), 0);
    }
}
