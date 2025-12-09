use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::services::DatabaseService;
use crate::tui::app::{App, Screen};
use crate::tui::state::Navigable;
use crate::tui::widgets::common;

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
}

impl Navigable for MainMenuState {
    fn len(&self) -> usize {
        self.items.len()
    }

    fn selected(&self) -> usize {
        self.selected
    }

    fn set_selected(&mut self, index: usize) {
        self.selected = index;
    }
}

pub fn render<D: DatabaseService>(frame: &mut Frame, app: &App<D>) {
    let area = frame.area();

    // Create standard 3-section layout using common component
    let chunks = common::render_screen_layout(area);

    // Render title using common component
    common::render_title(frame, chunks[0], "Reddit Notifier TUI");

    // Menu items using common selection style
    let items: Vec<ListItem> = app
        .main_menu_state
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == app.main_menu_state.selected;
            let (prefix, style) = common::selection_style(is_selected);
            ListItem::new(format!("{}{}", prefix, item)).style(style)
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL));

    let mut list_state = ListState::default();
    list_state.select(Some(app.main_menu_state.selected));

    frame.render_stateful_widget(list, chunks[1], &mut list_state);

    // Render help text using common component
    common::render_help(
        frame,
        chunks[2],
        &[("↑/↓", "Navigate"), ("Enter", "Select"), ("q", "Quit")],
    );
}

pub async fn handle_key<D: DatabaseService>(app: &mut App<D>, key: KeyEvent) -> Result<()> {
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
