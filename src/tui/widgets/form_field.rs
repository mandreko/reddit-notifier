use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::text_input::TextInput;

/// A form field widget that combines a label, input, and validation state
///
/// This widget provides:
/// - Label display
/// - Integrated TextInput
/// - Validation state tracking
/// - Help text support
/// - Focus management
#[derive(Debug, Clone)]
pub struct FormField {
    /// Field label
    pub label: String,

    /// The text input widget
    pub input: TextInput,

    /// Whether this field is required
    pub required: bool,

    /// Current validation state
    pub validation_state: ValidationState,

    /// Whether this field is focused
    pub is_focused: bool,

    /// Optional help text displayed below the field
    pub help_text: Option<String>,
}

/// Validation state for a form field
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationState {
    /// No validation performed yet
    Idle,

    /// Validation in progress (async)
    Validating,

    /// Field is valid (with optional success message)
    Valid(Option<String>),

    /// Field is invalid (with error message)
    Invalid(String),
}

impl ValidationState {
    /// Get the color for this validation state
    pub fn color(&self) -> Color {
        match self {
            Self::Idle => Color::White,
            Self::Validating => Color::Yellow,
            Self::Valid(_) => Color::Green,
            Self::Invalid(_) => Color::Red,
        }
    }

    /// Get the icon for this validation state
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Idle => "",
            Self::Validating => "⋯",
            Self::Valid(_) => "✓",
            Self::Invalid(_) => "✗",
        }
    }

    /// Get the message for this validation state
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Valid(Some(msg)) => Some(msg),
            Self::Invalid(msg) => Some(msg),
            _ => None,
        }
    }
}

impl FormField {
    /// Create a new form field with a label
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            input: TextInput::new(),
            required: false,
            validation_state: ValidationState::Idle,
            is_focused: false,
            help_text: None,
        }
    }

    /// Mark this field as required
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Set help text
    pub fn with_help(mut self, text: impl Into<String>) -> Self {
        self.help_text = Some(text.into());
        self
    }

    /// Set the input widget
    pub fn with_input(mut self, input: TextInput) -> Self {
        self.input = input;
        self
    }

    /// Set focus state
    pub fn set_focused(&mut self, focused: bool) {
        self.is_focused = focused;
        self.input.set_focused(focused);
    }

    /// Perform synchronous validation
    pub fn validate_sync(&mut self) -> Result<(), String> {
        if self.required && self.input.value().trim().is_empty() {
            self.validation_state = ValidationState::Invalid("Field is required".to_string());
            return Err("Field is required".to_string());
        }
        self.validation_state = ValidationState::Valid(None);
        Ok(())
    }

    /// Handle keyboard input
    ///
    /// Returns true if the input was modified
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        // Clear validation state when user starts typing
        if let ValidationState::Invalid(_) = self.validation_state {
            self.validation_state = ValidationState::Idle;
        }

        self.input.handle_key(key)
    }

    /// Render the form field
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Calculate layout based on whether we have help text or validation messages
        let has_message = self.help_text.is_some() || self.validation_state.message().is_some();

        let chunks = if has_message {
            Layout::vertical([
                Constraint::Length(1), // Label
                Constraint::Length(3), // Input
                Constraint::Length(1), // Help/validation message
            ])
            .split(area)
        } else {
            Layout::vertical([
                Constraint::Length(1), // Label
                Constraint::Length(3), // Input
            ])
            .split(area)
        };

        // Render label
        let label_style = if self.is_focused {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let label_text = if self.required {
            format!("{} *", self.label)
        } else {
            self.label.clone()
        };

        let validation_icon = self.validation_state.icon();
        let label_line = if !validation_icon.is_empty() {
            Line::from(vec![
                Span::styled(label_text, label_style),
                Span::raw(" "),
                Span::styled(validation_icon, Style::default().fg(self.validation_state.color())),
            ])
        } else {
            Line::from(Span::styled(label_text, label_style))
        };

        let label_para = Paragraph::new(label_line);
        frame.render_widget(label_para, chunks[0]);

        // Render input
        self.input.render(frame, chunks[1]);

        // Render help text or validation message
        if has_message && chunks.len() > 2 {
            let message = if let Some(val_msg) = self.validation_state.message() {
                (val_msg, self.validation_state.color())
            } else if let Some(help) = &self.help_text {
                (help.as_str(), Color::DarkGray)
            } else {
                ("", Color::White)
            };

            let message_para = Paragraph::new(Line::from(
                Span::styled(message.0, Style::default().fg(message.1))
            ));
            frame.render_widget(message_para, chunks[2]);
        }
    }

    /// Get the current value
    pub fn value(&self) -> &str {
        self.input.value()
    }

    /// Check if the field is valid
    pub fn is_valid(&self) -> bool {
        matches!(self.validation_state, ValidationState::Valid(_))
    }

    /// Check if the field is empty
    pub fn is_empty(&self) -> bool {
        self.input.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode;

    #[test]
    fn test_new_form_field() {
        let field = FormField::new("Username");
        assert_eq!(field.label, "Username");
        assert!(!field.required);
        assert!(matches!(field.validation_state, ValidationState::Idle));
        assert!(!field.is_focused);
    }

    #[test]
    fn test_required_field() {
        let field = FormField::new("Email").required();
        assert!(field.required);
    }

    #[test]
    fn test_with_help() {
        let field = FormField::new("Password").with_help("Must be at least 8 characters");
        assert_eq!(field.help_text, Some("Must be at least 8 characters".to_string()));
    }

    #[test]
    fn test_validate_sync_empty_required() {
        let mut field = FormField::new("Name").required();
        let result = field.validate_sync();
        assert!(result.is_err());
        assert!(matches!(field.validation_state, ValidationState::Invalid(_)));
    }

    #[test]
    fn test_validate_sync_empty_optional() {
        let mut field = FormField::new("Nickname");
        let result = field.validate_sync();
        assert!(result.is_ok());
        assert!(matches!(field.validation_state, ValidationState::Valid(_)));
    }

    #[test]
    fn test_validate_sync_with_value() {
        let mut field = FormField::new("Name")
            .required()
            .with_input(TextInput::new().with_value("John"));
        let result = field.validate_sync();
        assert!(result.is_ok());
        assert!(matches!(field.validation_state, ValidationState::Valid(_)));
    }

    #[test]
    fn test_validation_clears_on_input() {
        let mut field = FormField::new("Name").required();
        field.validate_sync().ok();
        assert!(matches!(field.validation_state, ValidationState::Invalid(_)));

        // Type a character
        field.handle_key(KeyEvent::from(KeyCode::Char('a')));
        assert!(matches!(field.validation_state, ValidationState::Idle));
    }

    #[test]
    fn test_focus_state() {
        let mut field = FormField::new("Test");
        assert!(!field.is_focused);
        assert!(!field.input.is_focused);

        field.set_focused(true);
        assert!(field.is_focused);
        assert!(field.input.is_focused);
    }

    #[test]
    fn test_validation_state_color() {
        assert_eq!(ValidationState::Idle.color(), Color::White);
        assert_eq!(ValidationState::Validating.color(), Color::Yellow);
        assert_eq!(ValidationState::Valid(None).color(), Color::Green);
        assert_eq!(ValidationState::Invalid("error".to_string()).color(), Color::Red);
    }

    #[test]
    fn test_validation_state_icon() {
        assert_eq!(ValidationState::Idle.icon(), "");
        assert_eq!(ValidationState::Validating.icon(), "⋯");
        assert_eq!(ValidationState::Valid(None).icon(), "✓");
        assert_eq!(ValidationState::Invalid("error".to_string()).icon(), "✗");
    }

    #[test]
    fn test_validation_state_message() {
        assert_eq!(ValidationState::Idle.message(), None);
        assert_eq!(ValidationState::Validating.message(), None);
        assert_eq!(ValidationState::Valid(Some("Good!".to_string())).message(), Some("Good!"));
        assert_eq!(ValidationState::Valid(None).message(), None);
        assert_eq!(ValidationState::Invalid("Bad!".to_string()).message(), Some("Bad!"));
    }
}
