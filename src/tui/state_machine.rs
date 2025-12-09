use crate::tui::screen_trait::ScreenId;

/// ScreenStateMachine manages screen navigation state
///
/// This state machine tracks:
/// - The current active screen
/// - Navigation history for back button support
///
/// Unlike storing Box<dyn Screen>, this approach keeps screens
/// owned by the App struct and only tracks navigation state here.
pub struct ScreenStateMachine {
    /// Currently active screen
    current: ScreenId,

    /// Navigation history (for back button functionality)
    history: Vec<ScreenId>,
}

impl Default for ScreenStateMachine {
    fn default() -> Self {
        Self {
            current: ScreenId::MainMenu,
            history: vec![],
        }
    }
}

impl ScreenStateMachine {
    /// Create a new ScreenStateMachine starting at the main menu
    pub fn new() -> Self {
        Self::default()
    }

    /// Navigate to a specific screen
    pub fn go_to(&mut self, screen_id: ScreenId) {
        // Add current screen to history before navigating
        self.history.push(self.current);
        self.current = screen_id;
    }

    /// Go back to the previous screen
    ///
    /// Returns true if we went back, false if there's no history
    pub fn go_back(&mut self) -> bool {
        if let Some(previous) = self.history.pop() {
            self.current = previous;
            true
        } else {
            false
        }
    }

    /// Get the current screen ID
    pub fn current(&self) -> ScreenId {
        self.current
    }

    /// Get the navigation history
    pub fn history(&self) -> &[ScreenId] {
        &self.history
    }

    /// Clear the navigation history
    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}
