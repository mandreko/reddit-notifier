use anyhow::Result;
use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use ratatui::DefaultTerminal;
use std::sync::Arc;
use std::time::Duration;

use crate::services::DatabaseService;
use super::screen_trait::{Screen as ScreenTrait, ScreenId, ScreenTransition};
use super::screens;
use super::state::MessageDisplay;
use super::state_machine::ScreenStateMachine;

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    MainMenu,
    Subscriptions,
    Endpoints,
    TestNotification,
    Logs,
}

/// Context that screens need access to (everything except screen states)
pub struct AppContext<D: DatabaseService> {
    pub db: Arc<D>,
    pub current_screen: Screen,  // Kept for backward compatibility with existing screen code
    pub should_quit: bool,
    pub messages: MessageDisplay,
    pub state_machine: ScreenStateMachine,
}

/// Container for all screen states
pub struct ScreenStates {
    pub main_menu_state: screens::MainMenuState,
    pub subscriptions_state: screens::SubscriptionsState,
    pub endpoints_state: screens::EndpointsState,
    pub test_notification_state: screens::TestNotificationState,
    pub logs_state: screens::LogsState,
}

pub struct App<D: DatabaseService> {
    pub context: AppContext<D>,
    pub states: ScreenStates,
}

// Provide convenient access to context fields (backward compatibility)
impl<D: DatabaseService> App<D> {
    pub fn db(&self) -> &Arc<D> {
        &self.context.db
    }

    pub fn db_mut(&mut self) -> &mut Arc<D> {
        &mut self.context.db
    }
}

impl<D: DatabaseService> App<D> {
    pub fn new(db: Arc<D>) -> Result<Self> {
        Ok(Self {
            context: AppContext {
                db,
                current_screen: Screen::MainMenu,
                should_quit: false,
                messages: MessageDisplay::new(),
                state_machine: ScreenStateMachine::new(),
            },
            states: ScreenStates {
                main_menu_state: screens::MainMenuState::new(),
                subscriptions_state: screens::SubscriptionsState::new(),
                endpoints_state: screens::EndpointsState::new(),
                test_notification_state: screens::TestNotificationState::new(),
                logs_state: screens::LogsState::new(),
            },
        })
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let mut last_screen_id = self.context.state_machine.current();

        while !self.context.should_quit {
            let current_screen_id = self.context.state_machine.current();

            // Call on_enter when entering a new screen using the trait
            if current_screen_id != last_screen_id {
                let context = &mut self.context;
                let states = &mut self.states;

                match current_screen_id {
                    ScreenId::MainMenu => {
                        states.main_menu_state.on_enter(context).await?;
                    }
                    ScreenId::Subscriptions => {
                        states.subscriptions_state.on_enter(context).await?;
                    }
                    ScreenId::Endpoints => {
                        states.endpoints_state.on_enter(context).await?;
                    }
                    ScreenId::TestNotification => {
                        states.test_notification_state.on_enter(context).await?;
                    }
                    ScreenId::Logs => {
                        states.logs_state.on_enter(context).await?;
                    }
                }
                last_screen_id = current_screen_id;
            }

            // Render the current screen using the trait
            terminal.draw(|frame| {
                match self.context.state_machine.current() {
                    ScreenId::MainMenu => {
                        self.states.main_menu_state.render(frame, self);
                    }
                    ScreenId::Subscriptions => {
                        self.states.subscriptions_state.render(frame, self);
                    }
                    ScreenId::Endpoints => {
                        self.states.endpoints_state.render(frame, self);
                    }
                    ScreenId::TestNotification => {
                        self.states.test_notification_state.render(frame, self);
                    }
                    ScreenId::Logs => {
                        self.states.logs_state.render(frame, self);
                    }
                }
                self.context.messages.render(frame, frame.area());
            })?;

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

    /// Handle key input for the current screen
    ///
    /// Now that we've split App into context and states, we can call the trait methods directly!
    /// This is the proper way to use the Screen trait without borrow checker issues.
    async fn handle_key_for_current_screen(&mut self, key: KeyEvent) -> Result<ScreenTransition> {
        // Split borrows: context and states are separate, so we can borrow both
        let context = &mut self.context;
        let states = &mut self.states;

        // Call the trait method directly on each screen state
        let transition = match context.state_machine.current() {
            ScreenId::MainMenu => {
                states.main_menu_state.handle_key(context, key).await?
            }
            ScreenId::Subscriptions => {
                states.subscriptions_state.handle_key(context, key).await?
            }
            ScreenId::Endpoints => {
                states.endpoints_state.handle_key(context, key).await?
            }
            ScreenId::TestNotification => {
                states.test_notification_state.handle_key(context, key).await?
            }
            ScreenId::Logs => {
                states.logs_state.handle_key(context, key).await?
            }
        };

        Ok(transition)
    }

    /// Sync the old current_screen enum with the state machine
    /// (for backward compatibility with existing screen code)
    fn sync_current_screen(&mut self) {
        self.context.current_screen = match self.context.state_machine.current() {
            ScreenId::MainMenu => Screen::MainMenu,
            ScreenId::Subscriptions => Screen::Subscriptions,
            ScreenId::Endpoints => Screen::Endpoints,
            ScreenId::TestNotification => Screen::TestNotification,
            ScreenId::Logs => Screen::Logs,
        };
    }

    /// Public method for tests to simulate key presses
    ///
    /// This delegates to the internal handle_key_for_current_screen and processes
    /// the resulting transition, updating the app state accordingly.
    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        let transition = self.handle_key_for_current_screen(key).await?;

        match transition {
            ScreenTransition::Stay => {
                // Do nothing
            }
            ScreenTransition::GoTo(screen_id) => {
                self.context.state_machine.go_to(screen_id);
                self.sync_current_screen();
            }
            ScreenTransition::Back => {
                self.context.state_machine.go_back();
                self.sync_current_screen();
            }
            ScreenTransition::Quit => {
                self.context.should_quit = true;
            }
        }

        Ok(())
    }

    /// Test helper to navigate directly to a screen
    ///
    /// This updates both the state machine and the backward-compatible current_screen field.
    /// Use this in tests instead of directly setting current_screen.
    #[cfg(test)]
    pub fn goto_screen(&mut self, screen: Screen) {
        let screen_id = match screen {
            Screen::MainMenu => ScreenId::MainMenu,
            Screen::Subscriptions => ScreenId::Subscriptions,
            Screen::Endpoints => ScreenId::Endpoints,
            Screen::TestNotification => ScreenId::TestNotification,
            Screen::Logs => ScreenId::Logs,
        };

        // Only add to history if we're changing screens
        if self.context.state_machine.current() != screen_id {
            self.context.state_machine.go_to(screen_id);
        }

        self.context.current_screen = screen;
    }
}
