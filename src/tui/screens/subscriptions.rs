use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Row},
    Frame,
};

use crate::models::database::{EndpointRow, SubscriptionRow};
use crate::services::DatabaseService;
use crate::tui::app::{App, Screen};
use crate::tui::screen_trait::{Screen as ScreenTrait, ScreenId, ScreenTransition};
use crate::tui::state::Navigable;
use crate::tui::widgets::{common, text_input, CheckboxList, ColumnDef, ModalDialog, SelectableTable, TextInput};

#[derive(Debug, Clone, PartialEq)]
pub enum SubscriptionsMode {
    List,
    Creating(TextInput), // Input widget
    ManagingEndpoints {
        subscription_id: i64,
        checkbox_list: CheckboxList<EndpointRow>,
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

pub async fn load_subscriptions<D: DatabaseService>(
    state: &mut SubscriptionsState,
    context: &mut crate::tui::app::AppContext<D>,
) -> Result<()> {
    let subs = context.db.list_subscriptions().await?;
    state.subscriptions = subs;
    if state.selected >= state.subscriptions.len()
        && !state.subscriptions.is_empty()
    {
        state.selected = state.subscriptions.len() - 1;
    }
    Ok(())
}

pub fn render<D: DatabaseService>(frame: &mut Frame, app: &App<D>) {
    let area = frame.area();

    match &app.states.subscriptions_state.mode {
        SubscriptionsMode::List => render_list(frame, app, area),
        SubscriptionsMode::Creating(input) => render_creating(frame, app, area, input),
        SubscriptionsMode::ManagingEndpoints { checkbox_list, .. } => {
            render_managing_endpoints(frame, app, area, checkbox_list)
        }
        SubscriptionsMode::ConfirmDelete { subreddit_name, .. } => {
            render_list(frame, app, area);
            let prompt = format!("Delete subscription '{}'?", subreddit_name);
            let dialog = ModalDialog::confirm(prompt);
            dialog.render(frame, area);
        }
    }

    // Show error/success messages using centralized display
    app.context.messages.render(frame, area);
}

fn render_list<D: DatabaseService>(frame: &mut Frame, app: &App<D>, area: Rect) {
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

    // Table using SelectableTable
    let columns = vec![
        ColumnDef::new("", Constraint::Length(2)),           // Selection marker
        ColumnDef::new("ID", Constraint::Length(5)),
        ColumnDef::new("Subreddit", Constraint::Percentage(60)),
        ColumnDef::new("Created", Constraint::Percentage(40)),
    ];

    let mut table = SelectableTable::new(
        app.states.subscriptions_state.subscriptions.clone(),
        columns,
    )
    .with_empty_message("No subscriptions yet. Press 'n' to create one.");

    // Sync the selection with the app state
    table.selected = app.states.subscriptions_state.selected;

    table.render(frame, chunks[1], |sub, _i, is_selected| {
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
    });

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

fn render_creating<D: DatabaseService>(frame: &mut Frame, _app: &App<D>, area: Rect, input: &TextInput) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1), // Label
        Constraint::Length(3), // Input
        Constraint::Min(0),
        Constraint::Length(3), // Help
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

    // Label
    let label = Paragraph::new("Subreddit name (alphanumeric + underscores only):")
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(label, chunks[1]);

    // TextInput widget
    input.render(frame, chunks[2]);

    let help = Paragraph::new(Line::from(vec![
        "[Enter] Save  ".into(),
        "[Esc] Cancel".into(),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[4]);
}

fn render_managing_endpoints<D: DatabaseService>(
    frame: &mut Frame,
    app: &App<D>,
    area: Rect,
    checkbox_list: &CheckboxList<EndpointRow>,
) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    let selected_sub = &app.states.subscriptions_state.subscriptions[app.states.subscriptions_state.selected];
    let title = Paragraph::new(format!("Link Endpoints to '{}'", selected_sub.subreddit))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        );
    frame.render_widget(title, chunks[0]);

    if checkbox_list.is_empty() {
        let empty = Paragraph::new("No endpoints available. Create one first in Manage Endpoints.")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(empty, chunks[1]);
    } else {
        checkbox_list.render(frame, chunks[1], |endpoint| {
            let kind_str = endpoint.kind.as_str();
            if let Some(note) = &endpoint.note {
                if !note.is_empty() {
                    format!("{} - {} ({})", kind_str, endpoint.id, note)
                } else {
                    format!("{} - {}", kind_str, endpoint.id)
                }
            } else {
                format!("{} - {}", kind_str, endpoint.id)
            }
        });
    }

    let help = Paragraph::new(Line::from(vec![
        "[↑/↓] Navigate  ".into(),
        "[Space] Toggle  ".into(),
        "[a] Toggle All  ".into(),
        "[Enter] Save  ".into(),
        "[Esc] Cancel".into(),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[2]);
}

async fn handle_list_mode<D: DatabaseService>(
    state: &mut SubscriptionsState,
    context: &mut crate::tui::app::AppContext<D>,
    key: KeyEvent,
) -> Result<()> {
    match key.code {
        KeyCode::Up => state.previous(),
        KeyCode::Down => state.next(),
        KeyCode::Char('n') => {
            let mut input = TextInput::new()
                .with_placeholder("Enter subreddit name")
                .with_validator(text_input::subreddit_validator);
            input.set_focused(true);
            state.mode = SubscriptionsMode::Creating(input);
        }
        KeyCode::Char('d') => {
            if !state.subscriptions.is_empty() {
                let sub = &state.subscriptions[state.selected];
                state.mode = SubscriptionsMode::ConfirmDelete {
                    subscription_id: sub.id,
                    subreddit_name: sub.subreddit.clone(),
                };
            }
        }
        KeyCode::Enter => {
            if !state.subscriptions.is_empty() {
                let sub = &state.subscriptions[state.selected];
                let all_endpoints = context.db.list_endpoints().await?;
                let linked = context.db.get_subscription_endpoints(sub.id).await?;
                let linked_ids: Vec<i64> = linked.iter().map(|e| e.id).collect();

                // Find indices of linked endpoints
                let checked_indices: Vec<usize> = all_endpoints
                    .iter()
                    .enumerate()
                    .filter(|(_, endpoint)| linked_ids.contains(&endpoint.id))
                    .map(|(i, _)| i)
                    .collect();

                let checkbox_list = CheckboxList::with_checked(all_endpoints, checked_indices);

                state.mode = SubscriptionsMode::ManagingEndpoints {
                    subscription_id: sub.id,
                    checkbox_list,
                };
            }
        }
        KeyCode::Esc => {
            context.current_screen = Screen::MainMenu;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_creating_mode<D: DatabaseService>(
    state: &mut SubscriptionsState,
    context: &mut crate::tui::app::AppContext<D>,
    key: KeyEvent,
    input: &TextInput,
) -> Result<()> {
    let mut new_input = input.clone();

    match key.code {
        KeyCode::Enter => {
            if new_input.value().trim().is_empty() {
                context.messages.set_error("Subreddit name cannot be empty".to_string());
                state.mode = SubscriptionsMode::List;
            } else {
                match context.db.create_subscription(new_input.value()).await {
                    Ok(_) => {
                        load_subscriptions(state, context).await?;
                        state.mode = SubscriptionsMode::List;
                    }
                    Err(e) => {
                        context.messages.set_error(format!("Failed to create subscription: {}", e));
                        state.mode = SubscriptionsMode::List;
                    }
                }
            }
        }
        KeyCode::Esc => {
            state.mode = SubscriptionsMode::List;
        }
        _ => {
            // Let TextInput handle the key
            new_input.handle_key(key);
            state.mode = SubscriptionsMode::Creating(new_input);
        }
    }
    Ok(())
}

async fn handle_managing_endpoints_mode<D: DatabaseService>(
    state: &mut SubscriptionsState,
    context: &mut crate::tui::app::AppContext<D>,
    key: KeyEvent,
    subscription_id: i64,
    checkbox_list: &CheckboxList<EndpointRow>,
) -> Result<()> {
    let mut new_list = checkbox_list.clone();

    // Let CheckboxList handle its own keys (Up, Down, Space, 'a')
    if new_list.handle_key(key) {
        state.mode = SubscriptionsMode::ManagingEndpoints {
            subscription_id,
            checkbox_list: new_list,
        };
        return Ok(());
    }

    // Handle other keys
    match key.code {
        KeyCode::Enter => {
            // Save changes
            let original_linked = context.db.get_subscription_endpoints(subscription_id)
                .await?
                .iter()
                .map(|e| e.id)
                .collect::<Vec<_>>();

            // Get IDs of checked endpoints
            let new_linked: Vec<i64> = new_list
                .get_checked_items()
                .iter()
                .map(|endpoint| endpoint.id)
                .collect();

            // Unlink removed endpoints
            for id in original_linked.iter() {
                if !new_linked.contains(id) {
                    context.db.unlink_subscription_endpoint(subscription_id, *id)
                        .await?;
                }
            }

            // Link new endpoints
            for id in new_linked.iter() {
                if !original_linked.contains(id) {
                    context.db.link_subscription_endpoint(subscription_id, *id).await?;
                }
            }

            state.mode = SubscriptionsMode::List;
        }
        KeyCode::Esc => {
            state.mode = SubscriptionsMode::List;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_confirm_delete_mode<D: DatabaseService>(
    state: &mut SubscriptionsState,
    context: &mut crate::tui::app::AppContext<D>,
    key: KeyEvent,
    subscription_id: i64,
    _subreddit_name: &str,
) -> Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            match context.db.delete_subscription(subscription_id).await {
                Ok(_) => {
                    load_subscriptions(state, context).await?;
                    state.mode = SubscriptionsMode::List;
                }
                Err(e) => {
                    context.messages.set_error(format!("Failed to delete: {}", e));
                    state.mode = SubscriptionsMode::List;
                }
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            state.mode = SubscriptionsMode::List;
        }
        _ => {}
    }
    Ok(())
}

#[async_trait]
impl<D: DatabaseService> ScreenTrait<D> for SubscriptionsState {
    fn render(&self, frame: &mut Frame, app: &App<D>) {
        super::subscriptions::render(frame, app)
    }

    async fn handle_key(&mut self, context: &mut crate::tui::app::AppContext<D>, key: KeyEvent) -> Result<ScreenTransition> {
        // Clear messages on any key if shown
        if context.messages.has_message() {
            context.messages.clear();
            return Ok(ScreenTransition::Stay);
        }

        let prev_screen = context.current_screen.clone();

        match &self.mode.clone() {
            SubscriptionsMode::List => handle_list_mode(self, context, key).await?,
            SubscriptionsMode::Creating(input) => handle_creating_mode(self, context, key, input).await?,
            SubscriptionsMode::ManagingEndpoints {
                subscription_id,
                checkbox_list,
            } => {
                handle_managing_endpoints_mode(
                    self,
                    context,
                    key,
                    *subscription_id,
                    checkbox_list,
                )
                .await?
            }
            SubscriptionsMode::ConfirmDelete {
                subscription_id,
                subreddit_name,
            } => handle_confirm_delete_mode(self, context, key, *subscription_id, subreddit_name).await?,
        }

        // Check if screen changed
        if context.current_screen != prev_screen {
            let screen_id = match context.current_screen {
                Screen::MainMenu => ScreenId::MainMenu,
                Screen::Subscriptions => ScreenId::Subscriptions,
                Screen::Endpoints => ScreenId::Endpoints,
                Screen::TestNotification => ScreenId::TestNotification,
                Screen::Logs => ScreenId::Logs,
            };
            return Ok(ScreenTransition::GoTo(screen_id));
        }

        Ok(ScreenTransition::Stay)
    }

    async fn on_enter(&mut self, context: &mut crate::tui::app::AppContext<D>) -> Result<()> {
        super::subscriptions::load_subscriptions(self, context).await
    }

    fn id(&self) -> ScreenId {
        ScreenId::Subscriptions
    }
}
