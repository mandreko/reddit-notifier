use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::DefaultTerminal;
use sqlx::SqlitePool;
use std::time::Duration;

use super::screens;
use super::ui;

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    MainMenu,
    Subscriptions,
    Endpoints,
    TestNotification,
    Logs,
}

pub struct App {
    pub pool: SqlitePool,
    pub current_screen: Screen,
    pub should_quit: bool,
    pub main_menu_state: screens::MainMenuState,
    pub subscriptions_state: screens::SubscriptionsState,
    pub endpoints_state: screens::EndpointsState,
    pub test_notification_state: screens::TestNotificationState,
    pub logs_state: screens::LogsState,
}

impl App {
    pub fn new(pool: SqlitePool) -> Result<Self> {
        Ok(Self {
            pool,
            current_screen: Screen::MainMenu,
            should_quit: false,
            main_menu_state: screens::MainMenuState::new(),
            subscriptions_state: screens::SubscriptionsState::new(),
            endpoints_state: screens::EndpointsState::new(),
            test_notification_state: screens::TestNotificationState::new(),
            logs_state: screens::LogsState::new(),
        })
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let mut last_screen = Screen::MainMenu;

        while !self.should_quit {
            // Load data when entering a new screen
            if self.current_screen != last_screen {
                match self.current_screen {
                    Screen::Subscriptions => {
                        screens::subscriptions::load_subscriptions(self).await?;
                    }
                    Screen::Endpoints => {
                        screens::endpoints::load_endpoints(self).await?;
                    }
                    Screen::TestNotification => {
                        screens::test_notification::load_endpoints(self).await?;
                    }
                    Screen::Logs => {
                        screens::logs::load_logs(self).await?;
                    }
                    _ => {}
                }
                last_screen = self.current_screen.clone();
            }

            // Render the current screen
            terminal.draw(|frame| ui::render(frame, self))?;

            // Handle input with timeout
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key).await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Global quit key
        if key.code == KeyCode::Char('q') && self.current_screen == Screen::MainMenu {
            self.should_quit = true;
            return Ok(());
        }

        // Delegate to current screen
        match self.current_screen {
            Screen::MainMenu => screens::main_menu::handle_key(self, key).await?,
            Screen::Subscriptions => screens::subscriptions::handle_key(self, key).await?,
            Screen::Endpoints => screens::endpoints::handle_key(self, key).await?,
            Screen::TestNotification => screens::test_notification::handle_key(self, key).await?,
            Screen::Logs => screens::logs::handle_key(self, key).await?,
        }

        Ok(())
    }
}
