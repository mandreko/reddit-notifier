use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::services::DatabaseService;
use crate::tui::app::App;
use crate::tui::screen_trait::{Screen as ScreenTrait, ScreenId, ScreenTransition};
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
        .states
        .main_menu_state
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == app.states.main_menu_state.selected;
            let (prefix, style) = common::selection_style(is_selected);
            ListItem::new(format!("{}{}", prefix, item)).style(style)
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL));

    let mut list_state = ListState::default();
    list_state.select(Some(app.states.main_menu_state.selected));

    frame.render_stateful_widget(list, chunks[1], &mut list_state);

    // Render help text using common component
    common::render_help(
        frame,
        chunks[2],
        &[("↑/↓", "Navigate"), ("Enter", "Select"), ("q", "Quit")],
    );
}

#[async_trait]
impl<D: DatabaseService> ScreenTrait<D> for MainMenuState {
    fn render(&self, frame: &mut Frame, app: &App<D>) {
        super::main_menu::render(frame, app)
    }

    async fn handle_key(&mut self, _context: &mut crate::tui::app::AppContext<D>, key: KeyEvent) -> Result<ScreenTransition> {
        match key.code {
            KeyCode::Up => self.previous(),
            KeyCode::Down => self.next(),
            KeyCode::Enter => {
                match self.selected() {
                    0 => return Ok(ScreenTransition::GoTo(ScreenId::Subscriptions)),
                    1 => return Ok(ScreenTransition::GoTo(ScreenId::Endpoints)),
                    2 => return Ok(ScreenTransition::GoTo(ScreenId::TestNotification)),
                    3 => return Ok(ScreenTransition::GoTo(ScreenId::Logs)),
                    4 => return Ok(ScreenTransition::Quit),
                    _ => {}
                }
            }
            KeyCode::Char('q') => return Ok(ScreenTransition::Quit),
            _ => {}
        }

        Ok(ScreenTransition::Stay)
    }

    fn id(&self) -> ScreenId {
        ScreenId::MainMenu
    }
}
