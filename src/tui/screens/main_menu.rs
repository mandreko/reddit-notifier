use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::tui::app::{App, Screen};

pub struct MainMenuState {
    selected: usize,
    items: Vec<&'static str>,
}

impl Default for MainMenuState {
    fn default() -> Self {
        Self::new()
    }
}

impl MainMenuState {
    pub fn new() -> Self {
        Self {
            selected: 0,
            items: vec![
                "Manage Subscriptions",
                "Manage Endpoints",
                "Test Notification",
                "View Logs",
                "Quit",
            ],
        }
    }

    pub fn next(&mut self) {
        self.selected = (self.selected + 1) % self.items.len();
    }

    pub fn previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        } else {
            self.selected = self.items.len() - 1;
        }
    }

    pub fn selected(&self) -> usize {
        self.selected
    }
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Create layout
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    // Title
    let title = Paragraph::new("Reddit Notifier TUI")
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        );
    frame.render_widget(title, chunks[0]);

    // Menu items
    let items: Vec<ListItem> = app
        .main_menu_state
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let prefix = if i == app.main_menu_state.selected {
                "> "
            } else {
                "  "
            };
            let style = if i == app.main_menu_state.selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("{}{}", prefix, item)).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default()),
    );

    let mut list_state = ListState::default();
    list_state.select(Some(app.main_menu_state.selected));

    frame.render_stateful_widget(list, chunks[1], &mut list_state);

    // Help text
    let help = Paragraph::new(Line::from(vec![
        "[↑/↓] Navigate  ".into(),
        "[Enter] Select  ".into(),
        "[q] Quit".into(),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[2]);
}

pub async fn handle_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Up => app.main_menu_state.previous(),
        KeyCode::Down => app.main_menu_state.next(),
        KeyCode::Enter => {
            match app.main_menu_state.selected() {
                0 => app.current_screen = Screen::Subscriptions,
                1 => app.current_screen = Screen::Endpoints,
                2 => app.current_screen = Screen::TestNotification,
                3 => app.current_screen = Screen::Logs,
                4 => app.should_quit = true,
                _ => {}
            }
        }
        KeyCode::Char('q') => app.should_quit = true,
        _ => {}
    }
    Ok(())
}
