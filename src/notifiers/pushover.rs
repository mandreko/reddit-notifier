use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use html_escape::decode_html_entities;

use crate::models::notifiers::PushoverConfig;
use super::Notifier;

pub struct PushoverNotifier {
    pub client: Client,
    pub cfg: PushoverConfig,
}

#[async_trait]
impl Notifier for PushoverNotifier {
    fn kind(&self) -> &'static str {
        "pushover"
    }

    async fn send(&self, subreddit: &str, title: &str, url: &str) -> Result<()> {
        let mut form = vec![
            ("token", self.cfg.token.clone()),
            ("user", self.cfg.user.clone()),
            ("title", format!("New Reddit Post Alert ({})", subreddit).to_string()),
            ("message", decode_html_entities(title).to_string()),
            ("url", url.to_string()),
        ];
        if let Some(device) = &self.cfg.device {
            form.push(("device", device.clone()));
        }
        let res = self.client
            .post("https://api.pushover.net/1/messages.json")
            .form(&form)
            .send()
            .await?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            anyhow::bail!("pushover non-success: {} body: {}", status, body);
        }
        Ok(())
    }
}
