use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::database;
use crate::models::database::EndpointRow;
use crate::notifiers;
use crate::tui::app::{App, Screen};
use crate::tui::state::Navigable;
use crate::tui::widgets::common;

#[derive(Debug, Clone, PartialEq)]
pub enum TestStatus {
    Ready,
    Sending,
    Success(String),
    Error(String),
}

pub struct TestNotificationState {
    pub endpoints: Vec<EndpointRow>,
    pub selected: usize,
    pub status: TestStatus,
}

impl Default for TestNotificationState {
    fn default() -> Self {
        Self::new()
    }
}

impl TestNotificationState {
    pub fn new() -> Self {
        Self {
            endpoints: Vec::new(),
            selected: 0,
            status: TestStatus::Ready,
        }
    }
}

impl Navigable for TestNotificationState {
    fn len(&self) -> usize {
        self.endpoints.len()
    }

    fn selected(&self) -> usize {
        self.selected
    }

    fn set_selected(&mut self, index: usize) {
        self.selected = index;
    }
}

pub async fn load_endpoints(app: &mut App) -> Result<()> {
    let all_endpoints = database::list_endpoints(&app.pool).await?;
    // Filter to only active endpoints
    let active_endpoints: Vec<EndpointRow> = all_endpoints
        .into_iter()
        .filter(|e| e.active)
        .collect();
    app.test_notification_state.endpoints = active_endpoints;
    if app.test_notification_state.selected >= app.test_notification_state.endpoints.len()
        && !app.test_notification_state.endpoints.is_empty()
    {
        app.test_notification_state.selected = app.test_notification_state.endpoints.len() - 1;
    }
    Ok(())
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    // Title
    let title = Paragraph::new("Test Notification")
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        );
    frame.render_widget(title, chunks[0]);

    // Endpoint list
    if app.test_notification_state.endpoints.is_empty() {
        let empty = Paragraph::new("No active endpoints available. Create and activate an endpoint first.")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Select Endpoint"));
        frame.render_widget(empty, chunks[1]);
    } else {
        let items: Vec<ListItem> = app
            .test_notification_state
            .endpoints
            .iter()
            .enumerate()
            .map(|(i, endpoint)| {
                let is_selected = i == app.test_notification_state.selected;
                let (prefix, style) = common::selection_style(is_selected);
                let kind_str = endpoint.kind.as_str();

                // Format: "prefix number. kind (ID: id) - note"
                let display = if let Some(note) = &endpoint.note {
                    if !note.is_empty() {
                        format!(
                            "{}{}. {} (ID: {}) - {}",
                            prefix,
                            i + 1,
                            kind_str,
                            endpoint.id,
                            note
                        )
                    } else {
                        format!(
                            "{}{}. {} (ID: {})",
                            prefix,
                            i + 1,
                            kind_str,
                            endpoint.id
                        )
                    }
                } else {
                    format!(
                        "{}{}. {} (ID: {})",
                        prefix,
                        i + 1,
                        kind_str,
                        endpoint.id
                    )
                };

                ListItem::new(display).style(style)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Select Endpoint"),
        );
        frame.render_widget(list, chunks[1]);
    }

    // Test message details
    let test_message = Paragraph::new(vec![
        Line::from("Test Message Details:"),
        Line::from(""),
        Line::from("  Subreddit: test"),
        Line::from("  Title: Test notification from reddit-notifier TUI"),
        Line::from("  URL: https://reddit.com"),
    ])
    .block(Block::default().borders(Borders::ALL).title("Message"));
    frame.render_widget(test_message, chunks[2]);

    // Status
    let (status_text, status_color) = match &app.test_notification_state.status {
        TestStatus::Ready => ("Status: Ready to send test notification".to_string(), Color::White),
        TestStatus::Sending => ("Status: Sending...".to_string(), Color::Yellow),
        TestStatus::Success(msg) => (format!("Status: ✓ {}", msg), Color::Green),
        TestStatus::Error(msg) => (format!("Status: ✗ {}", msg), Color::Red),
    };

    let status = Paragraph::new(status_text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(status_color));
    frame.render_widget(status, chunks[3]);

    // Help text
    let help = Paragraph::new(Line::from(vec![
        "[↑/↓] Navigate  ".into(),
        "[Enter] Send Test  ".into(),
        "[Esc] Back".into(),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[4]);
}

pub async fn handle_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Up => app.test_notification_state.previous(),
        KeyCode::Down => app.test_notification_state.next(),
        KeyCode::Enter => {
            if !app.test_notification_state.endpoints.is_empty() {
                send_test_notification(app).await?;
            }
        }
        KeyCode::Esc => {
            app.current_screen = Screen::MainMenu;
        }
        _ => {}
    }
    Ok(())
}

async fn send_test_notification(app: &mut App) -> Result<()> {
    app.test_notification_state.status = TestStatus::Sending;

    let endpoint = app.test_notification_state.endpoints[app.test_notification_state.selected].clone();

    // Create HTTP client
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    // Build notifier
    let notifier = match notifiers::build_notifier(&endpoint, client) {
        Ok(n) => n,
        Err(e) => {
            app.test_notification_state.status =
                TestStatus::Error(format!("Failed to build notifier: {}", e));
            return Ok(());
        }
    };

    // Send test notification
    match notifier
        .send(
            "test",
            "Test notification from reddit-notifier TUI",
            "https://reddit.com",
        )
        .await
    {
        Ok(_) => {
            let kind_str = notifier.kind();
            app.test_notification_state.status =
                TestStatus::Success(format!("Successfully sent test to {} endpoint!", kind_str));
        }
        Err(e) => {
            app.test_notification_state.status = TestStatus::Error(format!("Send failed: {}", e));
        }
    }

    Ok(())
}
