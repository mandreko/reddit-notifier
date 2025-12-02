use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table},
    Frame,
};

use crate::database;
use crate::models::EndpointRow;
use crate::tui::app::{App, Screen};
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
    pub error_message: Option<String>,
    pub success_message: Option<String>,
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
            error_message: None,
            success_message: None,
        }
    }

    pub fn next(&mut self) {
        if !self.endpoints.is_empty() {
            self.selected = (self.selected + 1) % self.endpoints.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.endpoints.is_empty() {
            if self.selected > 0 {
                self.selected -= 1;
            } else {
                self.selected = self.endpoints.len() - 1;
            }
        }
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
            render_confirm_delete(frame, area, endpoint_desc);
        }
    }

    // Show error/success messages
    if let Some(msg) = &app.endpoints_state.error_message {
        render_message(frame, area, msg, Color::Red);
    } else if let Some(msg) = &app.endpoints_state.success_message {
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
                let prefix = if i == app.endpoints_state.selected {
                    ">"
                } else {
                    " "
                };
                let style = if i == app.endpoints_state.selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let active = if endpoint.active { "✓" } else { "✗" };
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
        "[t] Toggle  ".into(),
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

fn render_confirm_delete(frame: &mut Frame, area: Rect, endpoint_desc: &str) {
    let popup_area = centered_rect(50, 30, area);
    let text = format!("Delete endpoint '{}'?", endpoint_desc);
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
    if app.endpoints_state.error_message.is_some()
        || app.endpoints_state.success_message.is_some()
    {
        app.endpoints_state.error_message = None;
        app.endpoints_state.success_message = None;
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
                        app.endpoints_state.error_message =
                            Some(format!("Failed to load config: {}", e));
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
        KeyCode::Char('t') => {
            if !app.endpoints_state.endpoints.is_empty() {
                let endpoint_id = app.endpoints_state.endpoints[app.endpoints_state.selected].id;
                match database::toggle_endpoint_active(&app.pool, endpoint_id).await {
                    Ok(_new_status) => {
                        load_endpoints(app).await?;
                        // Silently update the list - no success message needed
                    }
                    Err(e) => {
                        app.endpoints_state.error_message =
                            Some(format!("Failed to toggle: {}", e));
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
                            app.endpoints_state.error_message =
                                Some(format!("Failed to create endpoint: {}", e));
                            app.endpoints_state.mode = EndpointsMode::List;
                        }
                    }
                }
                Err(e) => {
                    app.endpoints_state.error_message = Some(format!("Validation error: {}", e));
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
                            app.endpoints_state.error_message =
                                Some(format!("Failed to update endpoint: {}", e));
                            app.endpoints_state.mode = EndpointsMode::List;
                        }
                    }
                }
                Err(e) => {
                    app.endpoints_state.error_message = Some(format!("Validation error: {}", e));
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
                    app.endpoints_state.error_message = Some(format!("Failed to delete: {}", e));
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
