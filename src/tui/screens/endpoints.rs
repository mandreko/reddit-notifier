use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};

use crate::database;
use crate::models::database::EndpointRow;
use crate::tui::app::{App, Screen};
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

pub async fn load_endpoints(app: &mut App) -> Result<()> {
    let endpoints = database::list_endpoints(&app.pool).await?;
    app.endpoints_state.endpoints = endpoints;
    if app.endpoints_state.selected >= app.endpoints_state.endpoints.len()
        && !app.endpoints_state.endpoints.is_empty()
    {
        app.endpoints_state.selected = app.endpoints_state.endpoints.len() - 1;
    }
    Ok(())
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    match &app.endpoints_state.mode {
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
    let title = Paragraph::new("Manage Endpoints")
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        );
    frame.render_widget(title, chunks[0]);

    // Table
    if app.endpoints_state.endpoints.is_empty() {
        let empty = Paragraph::new("No endpoints yet. Press 'n' to create one.")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(empty, chunks[1]);
    } else {
        let rows: Vec<Row> = app
            .endpoints_state
            .endpoints
            .iter()
            .enumerate()
            .map(|(i, endpoint)| {
                let is_selected = i == app.endpoints_state.selected;
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

fn render_viewing(frame: &mut Frame, _app: &App, area: Rect, endpoint: &EndpointRow) {
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

pub async fn handle_key(app: &mut App, key: KeyEvent) -> Result<()> {
    // Clear messages on any key if shown
    if app.messages.has_message() {
        app.messages.clear();
        return Ok(());
    }

    match &app.endpoints_state.mode.clone() {
        EndpointsMode::List => handle_list_mode(app, key).await?,
        EndpointsMode::Creating(builder) => handle_creating_mode(app, key, builder).await?,
        EndpointsMode::Editing {
            endpoint_id,
            builder,
        } => handle_editing_mode(app, key, *endpoint_id, builder).await?,
        EndpointsMode::Viewing { .. } => handle_viewing_mode(app, key).await?,
        EndpointsMode::ConfirmDelete {
            endpoint_id,
            endpoint_desc,
        } => handle_confirm_delete_mode(app, key, *endpoint_id, endpoint_desc).await?,
    }

    Ok(())
}

async fn handle_list_mode(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Up => app.endpoints_state.previous(),
        KeyCode::Down => app.endpoints_state.next(),
        KeyCode::Char('n') => {
            app.endpoints_state.mode = EndpointsMode::Creating(ConfigBuilder::new());
        }
        KeyCode::Char('e') => {
            if !app.endpoints_state.endpoints.is_empty() {
                let endpoint = app.endpoints_state.endpoints[app.endpoints_state.selected].clone();
                match ConfigBuilder::from_existing(endpoint.kind.clone(), &endpoint.config_json, endpoint.note.clone()) {
                    Ok(builder) => {
                        app.endpoints_state.mode = EndpointsMode::Editing {
                            endpoint_id: endpoint.id,
                            builder,
                        };
                    }
                    Err(e) => {
                        app.messages.set_error(format!("Failed to load config: {}", e));
                    }
                }
            }
        }
        KeyCode::Char('d') => {
            if !app.endpoints_state.endpoints.is_empty() {
                let endpoint = app.endpoints_state.endpoints[app.endpoints_state.selected].clone();
                let kind_str = endpoint.kind.as_str();
                app.endpoints_state.mode = EndpointsMode::ConfirmDelete {
                    endpoint_id: endpoint.id,
                    endpoint_desc: format!("{} (ID: {})", kind_str, endpoint.id),
                };
            }
        }
        KeyCode::Char(' ') => {
            if !app.endpoints_state.endpoints.is_empty() {
                let endpoint_id = app.endpoints_state.endpoints[app.endpoints_state.selected].id;
                match database::toggle_endpoint_active(&app.pool, endpoint_id).await {
                    Ok(_new_status) => {
                        load_endpoints(app).await?;
                        // Silently update the list - no success message needed
                    }
                    Err(e) => {
                        app.messages.set_error(format!("Failed to toggle: {}", e));
                    }
                }
            }
        }
        KeyCode::Enter => {
            if !app.endpoints_state.endpoints.is_empty() {
                let endpoint = app.endpoints_state.endpoints[app.endpoints_state.selected].clone();
                app.endpoints_state.mode = EndpointsMode::Viewing { endpoint };
            }
        }
        KeyCode::Esc => {
            app.current_screen = Screen::MainMenu;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_creating_mode(app: &mut App, key: KeyEvent, builder: &ConfigBuilder) -> Result<()> {
    let mut new_builder = builder.clone();

    match new_builder.handle_input(key)? {
        Some(ConfigAction::Save) => {
            match new_builder.build_json() {
                Ok(json) => {
                    let kind_str = new_builder.endpoint_type.as_str();
                    let note = new_builder.get_note();
                    match database::create_endpoint(&app.pool, kind_str, &json, note).await {
                        Ok(_) => {
                            load_endpoints(app).await?;
                            app.endpoints_state.mode = EndpointsMode::List;
                        }
                        Err(e) => {
                            app.messages.set_error(format!("Failed to create endpoint: {}", e));
                            app.endpoints_state.mode = EndpointsMode::List;
                        }
                    }
                }
                Err(e) => {
                    app.messages.set_error(format!("Validation error: {}", e));
                    app.endpoints_state.mode = EndpointsMode::List;
                }
            }
        }
        Some(ConfigAction::Cancel) => {
            app.endpoints_state.mode = EndpointsMode::List;
        }
        None => {
            app.endpoints_state.mode = EndpointsMode::Creating(new_builder);
        }
    }

    Ok(())
}

async fn handle_editing_mode(
    app: &mut App,
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
                    match database::update_endpoint(&app.pool, endpoint_id, &json, note).await {
                        Ok(_) => {
                            load_endpoints(app).await?;
                            app.endpoints_state.mode = EndpointsMode::List;
                        }
                        Err(e) => {
                            app.messages.set_error(format!("Failed to update endpoint: {}", e));
                            app.endpoints_state.mode = EndpointsMode::List;
                        }
                    }
                }
                Err(e) => {
                    app.messages.set_error(format!("Validation error: {}", e));
                    app.endpoints_state.mode = EndpointsMode::List;
                }
            }
        }
        Some(ConfigAction::Cancel) => {
            app.endpoints_state.mode = EndpointsMode::List;
        }
        None => {
            app.endpoints_state.mode = EndpointsMode::Editing {
                endpoint_id,
                builder: new_builder,
            };
        }
    }

    Ok(())
}

async fn handle_viewing_mode(app: &mut App, _key: KeyEvent) -> Result<()> {
    app.endpoints_state.mode = EndpointsMode::List;
    Ok(())
}

async fn handle_confirm_delete_mode(
    app: &mut App,
    key: KeyEvent,
    endpoint_id: i64,
    _endpoint_desc: &str,
) -> Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            match database::delete_endpoint(&app.pool, endpoint_id).await {
                Ok(_) => {
                    load_endpoints(app).await?;
                    app.endpoints_state.mode = EndpointsMode::List;
                }
                Err(e) => {
                    app.messages.set_error(format!("Failed to delete: {}", e));
                    app.endpoints_state.mode = EndpointsMode::List;
                }
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.endpoints_state.mode = EndpointsMode::List;
        }
        _ => {}
    }
    Ok(())
}
