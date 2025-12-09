use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};

use crate::models::database::EndpointRow;
use crate::services::DatabaseService;
use crate::tui::app::{App, Screen};
use crate::tui::screen_trait::{Screen as ScreenTrait, ScreenId, ScreenTransition};
use crate::tui::state::Navigable;
use crate::tui::widgets::common;
use crate::tui::widgets::{ConfigAction, ConfigBuilder};

#[derive(Debug, Clone)]
pub enum EndpointsMode {
    List,
    Creating(ConfigBuilder),
    Editing {
        endpoint_id: i64,
        builder: ConfigBuilder,
    },
    Viewing {
        endpoint: EndpointRow,
    },
    ConfirmDelete {
        endpoint_id: i64,
        endpoint_desc: String,
    },
}

pub struct EndpointsState {
    pub endpoints: Vec<EndpointRow>,
    pub selected: usize,
    pub mode: EndpointsMode,
}

impl Default for EndpointsState {
    fn default() -> Self {
        Self::new()
    }
}

impl EndpointsState {
    pub fn new() -> Self {
        Self {
            endpoints: Vec::new(),
            selected: 0,
            mode: EndpointsMode::List,
        }
    }
}

impl Navigable for EndpointsState {
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

pub async fn load_endpoints<D: DatabaseService>(state: &mut EndpointsState, context: &mut crate::tui::app::AppContext<D>) -> Result<()> {
    let endpoints = context.db.list_endpoints().await?;
    state.endpoints = endpoints;
    if state.selected >= state.endpoints.len()
        && !state.endpoints.is_empty()
    {
        state.selected = state.endpoints.len() - 1;
    }
    Ok(())
}

pub fn render<D: DatabaseService>(frame: &mut Frame, app: &App<D>) {
    let area = frame.area();

    match &app.states.endpoints_state.mode {
        EndpointsMode::List => render_list(frame, app, area),
        EndpointsMode::Creating(builder) => {
            render_list(frame, app, area);
            builder.render(frame, area);
        }
        EndpointsMode::Editing { builder, .. } => {
            render_list(frame, app, area);
            builder.render(frame, area);
        }
        EndpointsMode::Viewing { endpoint } => render_viewing(frame, app, area, endpoint),
        EndpointsMode::ConfirmDelete { endpoint_desc, .. } => {
            render_list(frame, app, area);
            let prompt = format!("Delete {}?", endpoint_desc);
            common::render_confirm_dialog(frame, area, &prompt, "Confirm Delete");
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
    let title = Paragraph::new("Manage Endpoints")
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        );
    frame.render_widget(title, chunks[0]);

    // Table
    if app.states.endpoints_state.endpoints.is_empty() {
        let empty = Paragraph::new("No endpoints yet. Press 'n' to create one.")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(empty, chunks[1]);
    } else {
        let rows: Vec<Row> = app
            .states
            .endpoints_state
            .endpoints
            .iter()
            .enumerate()
            .map(|(i, endpoint)| {
                let is_selected = i == app.states.endpoints_state.selected;
                let (prefix, style) = common::selection_style(is_selected);

                let active = if endpoint.active { "[x]" } else { "[ ]" };
                let kind_str = endpoint.kind.as_str();

                let note_display = endpoint.note.as_deref().unwrap_or("");

                Row::new(vec![
                    prefix.to_string(),
                    endpoint.id.to_string(),
                    kind_str.to_string(),
                    active.to_string(),
                    note_display.to_string(),
                    endpoint.config_json.clone(),
                ])
                .style(style)
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(2),      // Selection marker
                Constraint::Length(5),      // ID
                Constraint::Length(10),     // Type
                Constraint::Length(8),      // Active
                Constraint::Percentage(20), // Note
                Constraint::Percentage(55), // Config (takes remaining space)
            ],
        )
        .header(
            Row::new(vec!["", "ID", "Type", "Active", "Note", "Config"])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(Block::default().borders(Borders::ALL));

        frame.render_widget(table, chunks[1]);
    }

    // Help text
    let help = Paragraph::new(Line::from(vec![
        "[↑/↓] Navigate  ".into(),
        "[n] New  ".into(),
        "[e] Edit  ".into(),
        "[d] Delete  ".into(),
        "[Space] Toggle  ".into(),
        "[Enter] View  ".into(),
        "[Esc] Back".into(),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[2]);
}

fn render_viewing<D: DatabaseService>(frame: &mut Frame, _app: &App<D>, area: Rect, endpoint: &EndpointRow) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    let kind_str = endpoint.kind.as_str();
    let active_str = if endpoint.active { "Active" } else { "Inactive" };
    let title = Paragraph::new(format!(
        "{} Endpoint (ID: {}) - {}",
        kind_str, endpoint.id, active_str
    ))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(title, chunks[0]);

    // Pretty print JSON
    let pretty_json = if let Ok(value) = serde_json::from_str::<serde_json::Value>(&endpoint.config_json) {
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| endpoint.config_json.clone())
    } else {
        endpoint.config_json.clone()
    };

    let config = Paragraph::new(pretty_json)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Configuration JSON"),
        )
        .style(Style::default().fg(Color::Green));
    frame.render_widget(config, chunks[1]);

    let help = Paragraph::new("[Esc] Back")
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[2]);
}

async fn handle_list_mode<D: DatabaseService>(
    state: &mut EndpointsState,
    context: &mut crate::tui::app::AppContext<D>,
    key: KeyEvent,
) -> Result<()> {
    match key.code {
        KeyCode::Up => state.previous(),
        KeyCode::Down => state.next(),
        KeyCode::Char('n') => {
            state.mode = EndpointsMode::Creating(ConfigBuilder::new());
        }
        KeyCode::Char('e') => {
            if !state.endpoints.is_empty() {
                let endpoint = state.endpoints[state.selected].clone();
                match ConfigBuilder::from_existing(endpoint.kind.clone(), &endpoint.config_json, endpoint.note.clone()) {
                    Ok(builder) => {
                        state.mode = EndpointsMode::Editing {
                            endpoint_id: endpoint.id,
                            builder,
                        };
                    }
                    Err(e) => {
                        context.messages.set_error(format!("Failed to load config: {}", e));
                    }
                }
            }
        }
        KeyCode::Char('d') => {
            if !state.endpoints.is_empty() {
                let endpoint = state.endpoints[state.selected].clone();
                let kind_str = endpoint.kind.as_str();
                state.mode = EndpointsMode::ConfirmDelete {
                    endpoint_id: endpoint.id,
                    endpoint_desc: format!("{} (ID: {})", kind_str, endpoint.id),
                };
            }
        }
        KeyCode::Char(' ') => {
            if !state.endpoints.is_empty() {
                let endpoint_id = state.endpoints[state.selected].id;
                match context.db.toggle_endpoint_active(endpoint_id).await {
                    Ok(_new_status) => {
                        load_endpoints(state, context).await?;
                        // Silently update the list - no success message needed
                    }
                    Err(e) => {
                        context.messages.set_error(format!("Failed to toggle: {}", e));
                    }
                }
            }
        }
        KeyCode::Enter => {
            if !state.endpoints.is_empty() {
                let endpoint = state.endpoints[state.selected].clone();
                state.mode = EndpointsMode::Viewing { endpoint };
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
    state: &mut EndpointsState,
    context: &mut crate::tui::app::AppContext<D>,
    key: KeyEvent,
    builder: &ConfigBuilder,
) -> Result<()> {
    let mut new_builder = builder.clone();

    match new_builder.handle_input(key)? {
        Some(ConfigAction::Save) => {
            match new_builder.build_json() {
                Ok(json) => {
                    let kind_str = new_builder.endpoint_type.as_str();
                    let note = new_builder.get_note();
                    match context.db.create_endpoint(kind_str, &json, note).await {
                        Ok(_) => {
                            load_endpoints(state, context).await?;
                            state.mode = EndpointsMode::List;
                        }
                        Err(e) => {
                            context.messages.set_error(format!("Failed to create endpoint: {}", e));
                            state.mode = EndpointsMode::List;
                        }
                    }
                }
                Err(e) => {
                    context.messages.set_error(format!("Validation error: {}", e));
                    state.mode = EndpointsMode::List;
                }
            }
        }
        Some(ConfigAction::Cancel) => {
            state.mode = EndpointsMode::List;
        }
        None => {
            state.mode = EndpointsMode::Creating(new_builder);
        }
    }

    Ok(())
}

async fn handle_editing_mode<D: DatabaseService>(
    state: &mut EndpointsState,
    context: &mut crate::tui::app::AppContext<D>,
    key: KeyEvent,
    endpoint_id: i64,
    builder: &ConfigBuilder,
) -> Result<()> {
    let mut new_builder = builder.clone();

    match new_builder.handle_input(key)? {
        Some(ConfigAction::Save) => {
            match new_builder.build_json() {
                Ok(json) => {
                    let note = new_builder.get_note();
                    match context.db.update_endpoint(endpoint_id, &json, note).await {
                        Ok(_) => {
                            load_endpoints(state, context).await?;
                            state.mode = EndpointsMode::List;
                        }
                        Err(e) => {
                            context.messages.set_error(format!("Failed to update endpoint: {}", e));
                            state.mode = EndpointsMode::List;
                        }
                    }
                }
                Err(e) => {
                    context.messages.set_error(format!("Validation error: {}", e));
                    state.mode = EndpointsMode::List;
                }
            }
        }
        Some(ConfigAction::Cancel) => {
            state.mode = EndpointsMode::List;
        }
        None => {
            state.mode = EndpointsMode::Editing {
                endpoint_id,
                builder: new_builder,
            };
        }
    }

    Ok(())
}

async fn handle_viewing_mode<D: DatabaseService>(
    state: &mut EndpointsState,
    _context: &mut crate::tui::app::AppContext<D>,
    _key: KeyEvent,
) -> Result<()> {
    state.mode = EndpointsMode::List;
    Ok(())
}

async fn handle_confirm_delete_mode<D: DatabaseService>(
    state: &mut EndpointsState,
    context: &mut crate::tui::app::AppContext<D>,
    key: KeyEvent,
    endpoint_id: i64,
    _endpoint_desc: &str,
) -> Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            match context.db.delete_endpoint(endpoint_id).await {
                Ok(_) => {
                    load_endpoints(state, context).await?;
                    state.mode = EndpointsMode::List;
                }
                Err(e) => {
                    context.messages.set_error(format!("Failed to delete: {}", e));
                    state.mode = EndpointsMode::List;
                }
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            state.mode = EndpointsMode::List;
        }
        _ => {}
    }
    Ok(())
}

#[async_trait]
impl<D: DatabaseService> ScreenTrait<D> for EndpointsState {
    fn render(&self, frame: &mut Frame, app: &App<D>) {
        super::endpoints::render(frame, app)
    }

    async fn handle_key(&mut self, context: &mut crate::tui::app::AppContext<D>, key: KeyEvent) -> Result<ScreenTransition> {
        // Clear messages on any key if shown
        if context.messages.has_message() {
            context.messages.clear();
            return Ok(ScreenTransition::Stay);
        }

        let prev_screen = context.current_screen.clone();

        match &self.mode.clone() {
            EndpointsMode::List => handle_list_mode(self, context, key).await?,
            EndpointsMode::Creating(builder) => handle_creating_mode(self, context, key, builder).await?,
            EndpointsMode::Editing {
                endpoint_id,
                builder,
            } => handle_editing_mode(self, context, key, *endpoint_id, builder).await?,
            EndpointsMode::Viewing { .. } => handle_viewing_mode(self, context, key).await?,
            EndpointsMode::ConfirmDelete {
                endpoint_id,
                endpoint_desc,
            } => handle_confirm_delete_mode(self, context, key, *endpoint_id, endpoint_desc).await?,
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
        super::endpoints::load_endpoints(self, context).await
    }

    fn id(&self) -> ScreenId {
        ScreenId::Endpoints
    }
}
