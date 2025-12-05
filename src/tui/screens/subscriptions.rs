use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table},
    Frame,
};

use crate::database;
use crate::models::database::{EndpointRow, SubscriptionRow};
use crate::tui::app::{App, Screen};
use crate::tui::state::Navigable;
use crate::tui::widgets::common;

#[derive(Debug, Clone, PartialEq)]
pub enum SubscriptionsMode {
    List,
    Creating(String), // Input buffer
    ManagingEndpoints {
        subscription_id: i64,
        all_endpoints: Vec<EndpointRow>,
        linked_endpoint_ids: Vec<i64>,
        selected_idx: usize,
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
        }
    }
}

impl Navigable for SubscriptionsState {
    fn len(&self) -> usize {
        self.subscriptions.len()
    }

    fn selected(&self) -> usize {
        self.selected
    }

    fn set_selected(&mut self, index: usize) {
        self.selected = index;
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
        SubscriptionsMode::ManagingEndpoints {
            all_endpoints,
            linked_endpoint_ids,
            selected_idx,
            ..
        } => render_managing_endpoints(frame, app, area, all_endpoints, linked_endpoint_ids, *selected_idx),
        SubscriptionsMode::ConfirmDelete { subreddit_name, .. } => {
            render_list(frame, app, area);
            let prompt = format!("Delete subscription '{}'?", subreddit_name);
            common::render_confirm_dialog(frame, area, &prompt, "Confirm Delete");
        }
    }

    // Show error/success messages using centralized display
    app.messages.render(frame, area);
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
                let is_selected = i == app.subscriptions_state.selected;
                let (prefix, style) = common::selection_style(is_selected);
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
        "[Enter] Manage Endpoints  ".into(),
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

fn render_managing_endpoints(
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
                let is_selected = i == selected_idx;
                let (prefix, style) = common::selection_style(is_selected);
                let kind_str = endpoint.kind.as_str();
                let display = if let Some(note) = &endpoint.note {
                    if !note.is_empty() {
                        format!("{}{} {} - {} ({})", prefix, checkbox, kind_str, endpoint.id, note)
                    } else {
                        format!("{}{} {} - {}", prefix, checkbox, kind_str, endpoint.id)
                    }
                } else {
                    format!("{}{} {} - {}", prefix, checkbox, kind_str, endpoint.id)
                };
                ListItem::new(display).style(style)
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

pub async fn handle_key(app: &mut App, key: KeyEvent) -> Result<()> {
    // Clear messages on any key if shown
    if app.messages.has_message() {
        app.messages.clear();
        return Ok(());
    }

    match &app.subscriptions_state.mode.clone() {
        SubscriptionsMode::List => handle_list_mode(app, key).await?,
        SubscriptionsMode::Creating(input) => handle_creating_mode(app, key, input).await?,
        SubscriptionsMode::ManagingEndpoints {
            subscription_id,
            all_endpoints,
            linked_endpoint_ids,
            selected_idx,
        } => {
            handle_managing_endpoints_mode(
                app,
                key,
                *subscription_id,
                all_endpoints,
                linked_endpoint_ids,
                *selected_idx,
            )
            .await?
        }
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
        KeyCode::Enter => {
            if !app.subscriptions_state.subscriptions.is_empty() {
                let sub = &app.subscriptions_state.subscriptions[app.subscriptions_state.selected];
                let all_endpoints = database::list_endpoints(&app.pool).await?;
                let linked = database::get_subscription_endpoints(&app.pool, sub.id).await?;
                let linked_ids: Vec<i64> = linked.iter().map(|e| e.id).collect();

                app.subscriptions_state.mode = SubscriptionsMode::ManagingEndpoints {
                    subscription_id: sub.id,
                    all_endpoints,
                    linked_endpoint_ids: linked_ids,
                    selected_idx: 0,
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
                app.messages.set_error("Subreddit name cannot be empty".to_string());
                app.subscriptions_state.mode = SubscriptionsMode::List;
            } else {
                match database::create_subscription(&app.pool, &new_input).await {
                    Ok(_) => {
                        load_subscriptions(app).await?;
                        app.subscriptions_state.mode = SubscriptionsMode::List;
                    }
                    Err(e) => {
                        app.messages.set_error(format!("Failed to create subscription: {}", e));
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

async fn handle_managing_endpoints_mode(
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
            app.subscriptions_state.mode = SubscriptionsMode::ManagingEndpoints {
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
            app.subscriptions_state.mode = SubscriptionsMode::ManagingEndpoints {
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
                app.subscriptions_state.mode = SubscriptionsMode::ManagingEndpoints {
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
                    app.messages.set_error(format!("Failed to delete: {}", e));
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
