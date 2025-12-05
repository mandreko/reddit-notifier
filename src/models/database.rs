use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointKind {
    Discord,
    Pushover,
}

impl EndpointKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Discord => "discord",
            Self::Pushover => "pushover",
        }
    }
}

impl FromStr for EndpointKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "discord" => Ok(Self::Discord),
            "pushover" => Ok(Self::Pushover),
            _ => Err(format!("Unknown endpoint kind: {}", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EndpointRow {
    pub id: i64,
    pub kind: EndpointKind,
    pub config_json: String,
    pub active: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SubscriptionRow {
    pub id: i64,
    pub subreddit: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct NotifiedPostRow {
    pub id: i64,
    pub subreddit: String,
    pub post_id: String,
    pub first_seen_at: String,
}
