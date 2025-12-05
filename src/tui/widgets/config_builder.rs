use anyhow::{anyhow, Result};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use serde_json::json;

use crate::models::{
    database::EndpointKind,
    notifiers::{DiscordConfig, PushoverConfig},
};

#[derive(Debug, Clone)]
pub struct FormField {
    pub label: String,
    pub value: String,
    pub required: bool,
    pub placeholder: String,
}

impl FormField {
    pub fn new(label: &str, required: bool, placeholder: &str) -> Self {
        Self {
            label: label.to_string(),
            value: String::new(),
            required,
            placeholder: placeholder.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigBuilder {
    pub endpoint_type: EndpointKind,
    pub fields: Vec<FormField>,
    pub note: String,
    pub current_field: usize,
    pub type_selection_mode: bool,
    pub editing_note: bool,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigBuilder {
    pub fn new() -> Self {
        let mut builder = Self {
            endpoint_type: EndpointKind::Discord,
            fields: Vec::new(),
            note: String::new(),
            current_field: 0,
            type_selection_mode: true,
            editing_note: false,
        };
        builder.set_type(EndpointKind::Discord);
        builder
    }

    pub fn from_existing(kind: EndpointKind, config_json: &str, note: Option<String>) -> Result<Self> {
        let mut builder = Self {
            endpoint_type: kind.clone(),
            fields: Vec::new(),
            note: note.unwrap_or_default(),
            current_field: 0,
            type_selection_mode: false,
            editing_note: false,
        };

        builder.set_type(kind);

        // Parse existing JSON and populate fields
        match builder.endpoint_type {
            EndpointKind::Discord => {
                let config: DiscordConfig = serde_json::from_str(config_json)?;
                builder.fields[0].value = config.webhook_url;
                if let Some(username) = config.username {
                    builder.fields[1].value = username;
                }
            }
            EndpointKind::Pushover => {
                let config: PushoverConfig = serde_json::from_str(config_json)?;
                builder.fields[0].value = config.token;
                builder.fields[1].value = config.user;
                if let Some(device) = config.device {
                    builder.fields[2].value = device;
                }
            }
        }

        Ok(builder)
    }

    pub fn set_type(&mut self, kind: EndpointKind) {
        self.endpoint_type = kind;
        self.fields.clear();
        self.current_field = 0;

        match self.endpoint_type {
            EndpointKind::Discord => {
                self.fields
                    .push(FormField::new("Webhook URL", true, "https://discord.com/api/webhooks/..."));
                self.fields
                    .push(FormField::new("Username (optional)", false, "Reddit Notifier"));
            }
            EndpointKind::Pushover => {
                self.fields.push(FormField::new("Token", true, "your-app-token"));
                self.fields.push(FormField::new("User Key", true, "your-user-key"));
                self.fields
                    .push(FormField::new("Device (optional)", false, ""));
            }
        }
    }

    pub fn handle_input(&mut self, key: KeyEvent) -> Result<Option<ConfigAction>> {
        if self.type_selection_mode {
            return self.handle_type_selection(key);
        }

        match key.code {
            KeyCode::Tab => {
                if self.editing_note {
                    self.editing_note = false;
                    self.current_field = 0;
                } else if self.current_field == self.fields.len() - 1 {
                    self.editing_note = true;
                } else {
                    self.current_field += 1;
                }
                Ok(None)
            }
            KeyCode::BackTab => {
                if self.editing_note {
                    self.editing_note = false;
                    self.current_field = self.fields.len() - 1;
                } else if self.current_field == 0 {
                    self.editing_note = true;
                } else {
                    self.current_field -= 1;
                }
                Ok(None)
            }
            KeyCode::Char(c) => {
                if self.editing_note {
                    self.note.push(c);
                } else {
                    self.fields[self.current_field].value.push(c);
                }
                Ok(None)
            }
            KeyCode::Backspace => {
                if self.editing_note {
                    self.note.pop();
                } else {
                    self.fields[self.current_field].value.pop();
                }
                Ok(None)
            }
            KeyCode::Enter => {
                // Validate and build JSON
                self.validate_and_build()?;
                Ok(Some(ConfigAction::Save))
            }
            KeyCode::Esc => Ok(Some(ConfigAction::Cancel)),
            _ => Ok(None),
        }
    }

    fn handle_type_selection(&mut self, key: KeyEvent) -> Result<Option<ConfigAction>> {
        match key.code {
            KeyCode::Up | KeyCode::Down => {
                // Toggle between Discord and Pushover
                let new_type = match self.endpoint_type {
                    EndpointKind::Discord => EndpointKind::Pushover,
                    EndpointKind::Pushover => EndpointKind::Discord,
                };
                self.set_type(new_type);
                Ok(None)
            }
            KeyCode::Enter => {
                self.type_selection_mode = false;
                Ok(None)
            }
            KeyCode::Esc => Ok(Some(ConfigAction::Cancel)),
            _ => Ok(None),
        }
    }

    fn validate_and_build(&self) -> Result<()> {
        // Check required fields
        for field in &self.fields {
            if field.required && field.value.trim().is_empty() {
                return Err(anyhow!("Field '{}' is required", field.label));
            }
        }

        // Additional validation for Discord webhook URL
        if self.endpoint_type == EndpointKind::Discord {
            let webhook_url = &self.fields[0].value;
            if !webhook_url.starts_with("https://") {
                return Err(anyhow!("Webhook URL must start with https://"));
            }
        }

        Ok(())
    }

    pub fn build_json(&self) -> Result<String> {
        self.validate_and_build()?;

        let json_value = match self.endpoint_type {
            EndpointKind::Discord => {
                let username = if self.fields[1].value.trim().is_empty() {
                    None
                } else {
                    Some(self.fields[1].value.trim())
                };

                if let Some(user) = username {
                    json!({
                        "webhook_url": self.fields[0].value.trim(),
                        "username": user
                    })
                } else {
                    json!({
                        "webhook_url": self.fields[0].value.trim()
                    })
                }
            }
            EndpointKind::Pushover => {
                let device = if self.fields[2].value.trim().is_empty() {
                    None
                } else {
                    Some(self.fields[2].value.trim())
                };

                if let Some(dev) = device {
                    json!({
                        "token": self.fields[0].value.trim(),
                        "user": self.fields[1].value.trim(),
                        "device": dev
                    })
                } else {
                    json!({
                        "token": self.fields[0].value.trim(),
                        "user": self.fields[1].value.trim()
                    })
                }
            }
        };

        Ok(serde_json::to_string(&json_value)?)
    }

    pub fn preview_json(&self) -> String {
        match self.build_json() {
            Ok(json) => {
                // Pretty print
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) {
                    serde_json::to_string_pretty(&value).unwrap_or(json)
                } else {
                    json
                }
            }
            Err(e) => format!("Validation error: {}", e),
        }
    }

    pub fn get_note(&self) -> Option<&str> {
        if self.note.is_empty() {
            None
        } else {
            Some(&self.note)
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(80, 80, area);

        if self.type_selection_mode {
            self.render_type_selection(frame, popup_area);
        } else {
            self.render_form(frame, popup_area);
        }
    }

    fn render_type_selection(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

        let title = Paragraph::new("Select Endpoint Type")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Cyan)),
            );

        let items = vec![
            ListItem::new(if self.endpoint_type == EndpointKind::Discord {
                "> Discord"
            } else {
                "  Discord"
            })
            .style(if self.endpoint_type == EndpointKind::Discord {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            }),
            ListItem::new(if self.endpoint_type == EndpointKind::Pushover {
                "> Pushover"
            } else {
                "  Pushover"
            })
            .style(if self.endpoint_type == EndpointKind::Pushover {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            }),
        ];

        let list = List::new(items).block(Block::default().borders(Borders::ALL));

        let help = Paragraph::new(Line::from(vec![
            "[↑/↓] Select  ".into(),
            "[Enter] Confirm  ".into(),
            "[Esc] Cancel".into(),
        ]))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

        frame.render_widget(Clear, area);
        frame.render_widget(title, chunks[0]);
        frame.render_widget(list, chunks[1]);
        frame.render_widget(help, chunks[2]);
    }

    fn render_form(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(4), // Note field
            Constraint::Length((self.fields.len() * 3 + 1) as u16),
            Constraint::Length(6),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

        // Title
        let type_name = match self.endpoint_type {
            EndpointKind::Discord => "Discord",
            EndpointKind::Pushover => "Pushover",
        };
        let title = Paragraph::new(format!("Configure {} Endpoint", type_name))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Cyan)),
            );

        // Note field
        let note_label_style = if self.editing_note {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let note_cursor = if self.editing_note { "_" } else { "" };
        let note_widget = Paragraph::new(vec![
            Line::from(Span::styled("Note (optional):", note_label_style)),
            Line::from(vec![
                Span::raw("["),
                Span::raw(&self.note),
                Span::raw(note_cursor),
                Span::raw("]"),
            ]),
        ])
        .block(Block::default().borders(Borders::ALL));

        // Form fields
        let field_lines: Vec<Line> = self
            .fields
            .iter()
            .enumerate()
            .flat_map(|(i, field)| {
                let is_current = !self.editing_note && i == self.current_field;
                let label_style = if is_current {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let value_display = if field.value.is_empty() {
                    Span::styled(&field.placeholder, Style::default().fg(Color::DarkGray))
                } else {
                    Span::raw(&field.value)
                };

                let cursor = if is_current { "_" } else { "" };

                vec![
                    Line::from(Span::styled(
                        format!("{}:", field.label),
                        label_style,
                    )),
                    Line::from(vec![
                        Span::raw("["),
                        value_display,
                        Span::raw(cursor),
                        Span::raw("]"),
                    ]),
                    Line::from(""),
                ]
            })
            .collect();

        let form = Paragraph::new(field_lines)
            .block(Block::default().borders(Borders::ALL).title("Endpoint Configuration"));

        // JSON Preview
        let preview = Paragraph::new(self.preview_json())
            .block(Block::default().borders(Borders::ALL).title("JSON Preview"))
            .style(Style::default().fg(Color::Green));

        // Help text
        let help = Paragraph::new(Line::from(vec![
            "[Tab] Next Field  ".into(),
            "[Shift+Tab] Previous  ".into(),
            "[Enter] Save  ".into(),
            "[Esc] Cancel".into(),
        ]))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

        frame.render_widget(Clear, area);
        frame.render_widget(title, chunks[0]);
        frame.render_widget(note_widget, chunks[1]);
        frame.render_widget(form, chunks[2]);
        frame.render_widget(preview, chunks[3]);
        frame.render_widget(help, chunks[5]);
    }
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

pub enum ConfigAction {
    Save,
    Cancel,
}
