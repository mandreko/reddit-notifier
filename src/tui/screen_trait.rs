use async_trait::async_trait;
use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::Frame;

use crate::services::DatabaseService;
use crate::tui::app::{App, AppContext};

/// Screen trait defines the interface for all TUI screens
///
/// This trait formalizes the screen lifecycle and provides a consistent
/// interface for rendering, input handling, and lifecycle hooks.
///
/// Note: Methods take `&mut self` for state and `context` for app-level resources.
/// This separation allows us to borrow screen state and app context independently,
/// avoiding self-referential borrow issues.
#[async_trait]
pub trait Screen<D: DatabaseService>: Send {
    /// Render this screen to the terminal frame
    fn render(&self, frame: &mut Frame, app: &App<D>);

    /// Handle keyboard input and return the next screen transition
    async fn handle_key(&mut self, context: &mut AppContext<D>, key: KeyEvent) -> Result<ScreenTransition>;

    /// Called when entering this screen (optional lifecycle hook)
    ///
    /// Use this to load data or initialize state when the screen becomes active
    async fn on_enter(&mut self, context: &mut AppContext<D>) -> Result<()> {
        let _ = context;
        Ok(())
    }

    /// Called when leaving this screen (optional lifecycle hook)
    ///
    /// Use this to clean up or save state when navigating away
    async fn on_exit(&mut self, context: &mut AppContext<D>) -> Result<()> {
        let _ = context;
        Ok(())
    }

    /// Get the screen identifier
    fn id(&self) -> ScreenId;
}

/// Screen transition represents the result of handling input
///
/// This enum defines all possible navigation actions that can occur
/// after processing a key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenTransition {
    /// Stay on the current screen (no navigation)
    Stay,

    /// Navigate to a specific screen
    GoTo(ScreenId),

    /// Go back to the previous screen in history
    Back,

    /// Quit the application
    Quit,
}

/// Screen identifier for each screen in the application
///
/// This enum provides a type-safe way to reference screens throughout
/// the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenId {
    MainMenu,
    Subscriptions,
    Endpoints,
    TestNotification,
    Logs,
}
