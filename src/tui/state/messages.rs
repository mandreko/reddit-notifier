//! Centralized message handling for the TUI
//!
//! This module provides a unified way to display error and success messages
//! across all screens, eliminating duplicate message handling code.

use ratatui::{layout::Rect, Frame};

use crate::tui::widgets::ModalDialog;

/// Manages error and success messages for the TUI
///
/// Provides centralized state management for displaying temporary messages
/// to the user. Only one message (error or success) can be displayed at a time.
///
/// # Example
/// ```
/// use reddit_notifier::tui::state::MessageDisplay;
///
/// let mut messages = MessageDisplay::default();
///
/// // Display an error
/// messages.set_error("Failed to save".to_string());
/// assert!(messages.has_message());
///
/// // Clear the message
/// messages.clear();
/// assert!(!messages.has_message());
/// ```
#[derive(Debug, Default, Clone)]
pub struct MessageDisplay {
    error: Option<String>,
    success: Option<String>,
}

impl MessageDisplay {
    /// Create a new empty MessageDisplay
    pub fn new() -> Self {
        Self::default()
    }

    /// Set an error message, clearing any existing success message
    pub fn set_error(&mut self, msg: String) {
        self.error = Some(msg);
        self.success = None;
    }

    /// Set a success message, clearing any existing error message
    pub fn set_success(&mut self, msg: String) {
        self.success = Some(msg);
        self.error = None;
    }

    /// Clear all messages
    pub fn clear(&mut self) {
        self.error = None;
        self.success = None;
    }

    /// Check if any message is currently set
    pub fn has_message(&self) -> bool {
        self.error.is_some() || self.success.is_some()
    }

    /// Render the current message as a popup
    ///
    /// If an error message is set, it will be displayed in red with the title "Error".
    /// If a success message is set, it will be displayed in green with the title "Success".
    /// If no message is set, nothing is rendered.
    ///
    /// # Arguments
    /// * `frame` - The frame to render to
    /// * `area` - The full screen area (popup will be centered)
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if let Some(msg) = &self.error {
            let dialog = ModalDialog::error(msg.clone());
            dialog.render(frame, area);
        } else if let Some(msg) = &self.success {
            let dialog = ModalDialog::success(msg.clone());
            dialog.render(frame, area);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_is_empty() {
        let msg = MessageDisplay::new();
        assert!(!msg.has_message());
        assert_eq!(msg.error, None);
        assert_eq!(msg.success, None);
    }

    #[test]
    fn test_set_error() {
        let mut msg = MessageDisplay::default();
        msg.set_error("Test error".to_string());
        assert!(msg.has_message());
        assert_eq!(msg.error, Some("Test error".to_string()));
        assert_eq!(msg.success, None);
    }

    #[test]
    fn test_set_success() {
        let mut msg = MessageDisplay::default();
        msg.set_success("Test success".to_string());
        assert!(msg.has_message());
        assert_eq!(msg.success, Some("Test success".to_string()));
        assert_eq!(msg.error, None);
    }

    #[test]
    fn test_set_success_clears_error() {
        let mut msg = MessageDisplay::default();
        msg.set_error("Error".to_string());
        msg.set_success("Success".to_string());
        assert_eq!(msg.error, None);
        assert_eq!(msg.success, Some("Success".to_string()));
    }

    #[test]
    fn test_set_error_clears_success() {
        let mut msg = MessageDisplay::default();
        msg.set_success("Success".to_string());
        msg.set_error("Error".to_string());
        assert_eq!(msg.success, None);
        assert_eq!(msg.error, Some("Error".to_string()));
    }

    #[test]
    fn test_clear() {
        let mut msg = MessageDisplay::default();
        msg.set_error("Error".to_string());
        assert!(msg.has_message());
        msg.clear();
        assert!(!msg.has_message());
        assert_eq!(msg.error, None);
        assert_eq!(msg.success, None);
    }
}
