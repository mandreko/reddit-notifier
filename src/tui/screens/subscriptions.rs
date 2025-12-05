use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Row, Table},
    Frame,
};

use crate::database;
use crate::models::database::{EndpointRow, SubscriptionRow};
use crate::tui::app::{App, Screen};

#[derive(Debug, Clone, PartialEq)]
pub enum SubscriptionsMode {
    List,
    Creating(String), // Input buffer
    Linking {
        subscription_id: i64,
        all_endpoints: Vec<EndpointRow>,
        linked_endpoint_ids: Vec<i64>,
        selected_idx: usize,
    },
    Viewing {
        subscription_id: i64,
        linked_endpoints: Vec<EndpointRow>,
    },
    ConfirmDelete {
        subscription_id: i64,
        subreddit_name: String,
    },
}

pub struct SubscriptionsState {
    pub subscriptions: Vec<SubscriptionRow>,
    pub selected: usize,
    pub mode: SubscriptionsMode,
    pub error_message: Option<String>,
    pub success_message: Option<String>,
}

impl Default for SubscriptionsState {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriptionsState {
    pub fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
            selected: 0,
            mode: SubscriptionsMode::List,
            error_message: None,
            success_message: None,
        }
    }

    pub fn next(&mut self) {
        if !self.subscriptions.is_empty() {
            self.selected = (self.selected + 1) % self.subscriptions.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.subscriptions.is_empty() {
            if self.selected > 0 {
                self.selected -= 1;
            } else {
                self.selected = self.subscriptions.len() - 1;
            }
        }
    }
}

pub async fn load_subscriptions(app: &mut App) -> Result<()> {
    let subs = database::list_subscriptions(&app.pool).await?;
    app.subscriptions_state.subscriptions = subs;
    if app.subscriptions_state.selected >= app.subscriptions_state.subscriptions.len()
        && !app.subscriptions_state.subscriptions.is_empty()
    {
        app.subscriptions_state.selected = app.subscriptions_state.subscriptions.len() - 1;
    }
    Ok(())
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    match &app.subscriptions_state.mode {
        SubscriptionsMode::List => render_list(frame, app, area),
        SubscriptionsMode::Creating(input) => render_creating(frame, app, area, input),
        SubscriptionsMode::Linking {
            all_endpoints,
            linked_endpoint_ids,
            selected_idx,
            ..
        } => render_linking(frame, app, area, all_endpoints, linked_endpoint_ids, *selected_idx),
        SubscriptionsMode::Viewing {
            linked_endpoints, ..
        } => render_viewing(frame, app, area, linked_endpoints),
        SubscriptionsMode::ConfirmDelete { subreddit_name, .. } => {
            render_list(frame, app, area);
            render_confirm_delete(frame, area, subreddit_name);
        }
    }

    // Show error/success messages
    if let Some(msg) = &app.subscriptions_state.error_message {
        render_message(frame, area, msg, Color::Red);
    } else if let Some(msg) = &app.subscriptions_state.success_message {
        render_message(frame, area, msg, Color::Green);
    }
}

fn render_list(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    // Title
    let title = Paragraph::new("Manage Subscriptions")
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        );
    frame.render_widget(title, chunks[0]);

    // Table
    if app.subscriptions_state.subscriptions.is_empty() {
        let empty = Paragraph::new("No subscriptions yet. Press 'n' to create one.")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(empty, chunks[1]);
    } else {
        let rows: Vec<Row> = app
            .subscriptions_state
            .subscriptions
            .iter()
            .enumerate()
            .map(|(i, sub)| {
                let prefix = if i == app.subscriptions_state.selected {
                    ">"
                } else {
                    " "
                };
                let style = if i == app.subscriptions_state.selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let created_short = sub
                    .created_at
                    .split(' ')
                    .next()
                    .unwrap_or(&sub.created_at);
                Row::new(vec![
                    prefix.to_string(),
                    sub.id.to_string(),
                    sub.subreddit.clone(),
                    created_short.to_string(),
                ])
                .style(style)
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(2),      // Selection marker
                Constraint::Length(5),      // ID
                Constraint::Percentage(60), // Subreddit (takes most space)
                Constraint::Percentage(40), // Created timestamp
            ],
        )
        .header(
            Row::new(vec!["", "ID", "Subreddit", "Created"])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(Block::default().borders(Borders::ALL));

        frame.render_widget(table, chunks[1]);
    }

    // Help text
    let help = Paragraph::new(Line::from(vec![
        "[↑/↓] Navigate  ".into(),
        "[n] New  ".into(),
        "[d] Delete  ".into(),
        "[l] Link  ".into(),
        "[Enter] View  ".into(),
        "[Esc] Back".into(),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[2]);
}

fn render_creating(frame: &mut Frame, _app: &App, area: Rect, input: &str) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    let title = Paragraph::new("Create New Subscription")
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        );
    frame.render_widget(title, chunks[0]);

    let input_widget = Paragraph::new(format!("Subreddit name: {}_", input)).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Enter subreddit name (alphanumeric + underscores only)"),
    );
    frame.render_widget(input_widget, chunks[1]);

    let help = Paragraph::new(Line::from(vec![
        "[Enter] Save  ".into(),
        "[Esc] Cancel".into(),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[2]);
}

fn render_linking(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    endpoints: &[EndpointRow],
    linked_ids: &[i64],
    selected_idx: usize,
) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    let selected_sub = &app.subscriptions_state.subscriptions[app.subscriptions_state.selected];
    let title = Paragraph::new(format!("Link Endpoints to '{}'", selected_sub.subreddit))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        );
    frame.render_widget(title, chunks[0]);

    if endpoints.is_empty() {
        let empty = Paragraph::new("No endpoints available. Create one first in Manage Endpoints.")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(empty, chunks[1]);
    } else {
        let items: Vec<ListItem> = endpoints
            .iter()
            .enumerate()
            .map(|(i, endpoint)| {
                let checkbox = if linked_ids.contains(&endpoint.id) {
                    "[x]"
                } else {
                    "[ ]"
                };
                let prefix = if i == selected_idx { "> " } else { "  " };
                let style = if i == selected_idx {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let kind_str = endpoint.kind.as_str();
                ListItem::new(format!("{}{} {} - {}", prefix, checkbox, kind_str, endpoint.id))
                    .style(style)
            })
            .collect();

        let list = List::new(items).block(Block::default().borders(Borders::ALL));
        frame.render_widget(list, chunks[1]);
    }

    let help = Paragraph::new(Line::from(vec![
        "[↑/↓] Navigate  ".into(),
        "[Space] Toggle  ".into(),
        "[Enter] Save  ".into(),
        "[Esc] Cancel".into(),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[2]);
}

fn render_viewing(frame: &mut Frame, app: &App, area: Rect, endpoints: &[EndpointRow]) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    let selected_sub = &app.subscriptions_state.subscriptions[app.subscriptions_state.selected];
    let title = Paragraph::new(format!(
        "Endpoints Linked to '{}'",
        selected_sub.subreddit
    ))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(title, chunks[0]);

    if endpoints.is_empty() {
        let empty = Paragraph::new("No endpoints linked. Press 'l' to link endpoints.")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(empty, chunks[1]);
    } else {
        let items: Vec<ListItem> = endpoints
            .iter()
            .map(|endpoint| {
                let active = if endpoint.active { "✓" } else { "✗" };
                let kind_str = endpoint.kind.as_str();
                ListItem::new(format!(
                    "{} {} (ID: {}) - Active: {}",
                    kind_str, endpoint.id, endpoint.id, active
                ))
            })
            .collect();

        let list = List::new(items).block(Block::default().borders(Borders::ALL));
        frame.render_widget(list, chunks[1]);
    }

    let help = Paragraph::new("[Esc] Back")
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[2]);
}

fn render_confirm_delete(frame: &mut Frame, area: Rect, subreddit_name: &str) {
    let popup_area = centered_rect(50, 30, area);
    let text = format!("Delete subscription '{}'?", subreddit_name);
    let popup = Paragraph::new(vec![
        Line::from(""),
        Line::from(text).alignment(Alignment::Center),
        Line::from("").alignment(Alignment::Center),
        Line::from(vec![
            Span::raw("["),
            Span::styled("y", Style::default().fg(Color::Yellow)),
            Span::raw("] Yes    ["),
            Span::styled("n", Style::default().fg(Color::Yellow)),
            Span::raw("] No"),
        ])
        .alignment(Alignment::Center),
    ])
    .block(
        Block::default()
            .title("Confirm Delete")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Red)),
    );

    frame.render_widget(Clear, popup_area);
    frame.render_widget(popup, popup_area);
}

fn render_message(frame: &mut Frame, area: Rect, message: &str, color: Color) {
    let popup_area = centered_rect(60, 20, area);
    let popup = Paragraph::new(vec![
        Line::from(""),
        Line::from(message).alignment(Alignment::Center),
        Line::from(""),
        Line::from("[Press any key]").alignment(Alignment::Center),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(color)),
    );

    frame.render_widget(Clear, popup_area);
    frame.render_widget(popup, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

pub async fn handle_key(app: &mut App, key: KeyEvent) -> Result<()> {
    // Clear messages on any key if shown
    if app.subscriptions_state.error_message.is_some()
        || app.subscriptions_state.success_message.is_some()
    {
        app.subscriptions_state.error_message = None;
        app.subscriptions_state.success_message = None;
        return Ok(());
    }

    match &app.subscriptions_state.mode.clone() {
        SubscriptionsMode::List => handle_list_mode(app, key).await?,
        SubscriptionsMode::Creating(input) => handle_creating_mode(app, key, input).await?,
        SubscriptionsMode::Linking {
            subscription_id,
            all_endpoints,
            linked_endpoint_ids,
            selected_idx,
        } => {
            handle_linking_mode(
                app,
                key,
                *subscription_id,
                all_endpoints,
                linked_endpoint_ids,
                *selected_idx,
            )
            .await?
        }
        SubscriptionsMode::Viewing { .. } => handle_viewing_mode(app, key).await?,
        SubscriptionsMode::ConfirmDelete {
            subscription_id,
            subreddit_name,
        } => handle_confirm_delete_mode(app, key, *subscription_id, subreddit_name).await?,
    }

    Ok(())
}

async fn handle_list_mode(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Up => app.subscriptions_state.previous(),
        KeyCode::Down => app.subscriptions_state.next(),
        KeyCode::Char('n') => {
            app.subscriptions_state.mode = SubscriptionsMode::Creating(String::new());
        }
        KeyCode::Char('d') => {
            if !app.subscriptions_state.subscriptions.is_empty() {
                let sub = &app.subscriptions_state.subscriptions[app.subscriptions_state.selected];
                app.subscriptions_state.mode = SubscriptionsMode::ConfirmDelete {
                    subscription_id: sub.id,
                    subreddit_name: sub.subreddit.clone(),
                };
            }
        }
        KeyCode::Char('l') => {
            if !app.subscriptions_state.subscriptions.is_empty() {
                let sub = &app.subscriptions_state.subscriptions[app.subscriptions_state.selected];
                let all_endpoints = database::list_endpoints(&app.pool).await?;
                let linked = database::get_subscription_endpoints(&app.pool, sub.id).await?;
                let linked_ids: Vec<i64> = linked.iter().map(|e| e.id).collect();

                app.subscriptions_state.mode = SubscriptionsMode::Linking {
                    subscription_id: sub.id,
                    all_endpoints,
                    linked_endpoint_ids: linked_ids,
                    selected_idx: 0,
                };
            }
        }
        KeyCode::Enter => {
            if !app.subscriptions_state.subscriptions.is_empty() {
                let sub = &app.subscriptions_state.subscriptions[app.subscriptions_state.selected];
                let linked = database::get_subscription_endpoints(&app.pool, sub.id).await?;
                app.subscriptions_state.mode = SubscriptionsMode::Viewing {
                    subscription_id: sub.id,
                    linked_endpoints: linked,
                };
            }
        }
        KeyCode::Esc => {
            app.current_screen = Screen::MainMenu;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_creating_mode(app: &mut App, key: KeyEvent, input: &str) -> Result<()> {
    let mut new_input = input.to_string();

    match key.code {
        KeyCode::Char(c) if c.is_alphanumeric() || c == '_' => {
            new_input.push(c);
            app.subscriptions_state.mode = SubscriptionsMode::Creating(new_input);
        }
        KeyCode::Backspace => {
            new_input.pop();
            app.subscriptions_state.mode = SubscriptionsMode::Creating(new_input);
        }
        KeyCode::Enter => {
            if new_input.is_empty() {
                app.subscriptions_state.error_message =
                    Some("Subreddit name cannot be empty".to_string());
                app.subscriptions_state.mode = SubscriptionsMode::List;
            } else {
                match database::create_subscription(&app.pool, &new_input).await {
                    Ok(_) => {
                        load_subscriptions(app).await?;
                        app.subscriptions_state.mode = SubscriptionsMode::List;
                    }
                    Err(e) => {
                        app.subscriptions_state.error_message =
                            Some(format!("Failed to create subscription: {}", e));
                        app.subscriptions_state.mode = SubscriptionsMode::List;
                    }
                }
            }
        }
        KeyCode::Esc => {
            app.subscriptions_state.mode = SubscriptionsMode::List;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_linking_mode(
    app: &mut App,
    key: KeyEvent,
    subscription_id: i64,
    all_endpoints: &[EndpointRow],
    linked_ids: &[i64],
    selected_idx: usize,
) -> Result<()> {
    let mut new_selected = selected_idx;
    let mut new_linked = linked_ids.to_vec();

    match key.code {
        KeyCode::Up => {
            if new_selected > 0 {
                new_selected -= 1;
            } else {
                new_selected = all_endpoints.len().saturating_sub(1);
            }
            app.subscriptions_state.mode = SubscriptionsMode::Linking {
                subscription_id,
                all_endpoints: all_endpoints.to_vec(),
                linked_endpoint_ids: new_linked,
                selected_idx: new_selected,
            };
        }
        KeyCode::Down => {
            if !all_endpoints.is_empty() {
                new_selected = (new_selected + 1) % all_endpoints.len();
            }
            app.subscriptions_state.mode = SubscriptionsMode::Linking {
                subscription_id,
                all_endpoints: all_endpoints.to_vec(),
                linked_endpoint_ids: new_linked,
                selected_idx: new_selected,
            };
        }
        KeyCode::Char(' ') => {
            if !all_endpoints.is_empty() {
                let endpoint_id = all_endpoints[selected_idx].id;
                if let Some(pos) = new_linked.iter().position(|&id| id == endpoint_id) {
                    new_linked.remove(pos);
                } else {
                    new_linked.push(endpoint_id);
                }
                app.subscriptions_state.mode = SubscriptionsMode::Linking {
                    subscription_id,
                    all_endpoints: all_endpoints.to_vec(),
                    linked_endpoint_ids: new_linked,
                    selected_idx: new_selected,
                };
            }
        }
        KeyCode::Enter => {
            // Save changes
            let original_linked = database::get_subscription_endpoints(&app.pool, subscription_id)
                .await?
                .iter()
                .map(|e| e.id)
                .collect::<Vec<_>>();

            // Unlink removed endpoints
            for id in original_linked.iter() {
                if !new_linked.contains(id) {
                    database::unlink_subscription_endpoint(&app.pool, subscription_id, *id)
                        .await?;
                }
            }

            // Link new endpoints
            for id in new_linked.iter() {
                if !original_linked.contains(id) {
                    database::link_subscription_endpoint(&app.pool, subscription_id, *id).await?;
                }
            }

            app.subscriptions_state.mode = SubscriptionsMode::List;
        }
        KeyCode::Esc => {
            app.subscriptions_state.mode = SubscriptionsMode::List;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_viewing_mode(app: &mut App, _key: KeyEvent) -> Result<()> {
    app.subscriptions_state.mode = SubscriptionsMode::List;
    Ok(())
}

async fn handle_confirm_delete_mode(
    app: &mut App,
    key: KeyEvent,
    subscription_id: i64,
    _subreddit_name: &str,
) -> Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            match database::delete_subscription(&app.pool, subscription_id).await {
                Ok(_) => {
                    load_subscriptions(app).await?;
                    app.subscriptions_state.mode = SubscriptionsMode::List;
                }
                Err(e) => {
                    app.subscriptions_state.error_message =
                        Some(format!("Failed to delete: {}", e));
                    app.subscriptions_state.mode = SubscriptionsMode::List;
                }
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.subscriptions_state.mode = SubscriptionsMode::List;
        }
        _ => {}
    }
    Ok(())
}
