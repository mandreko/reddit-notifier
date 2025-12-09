use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// A reusable text input widget with validation and cursor support
///
/// This widget provides:
/// - Character-by-character input handling
/// - Optional validation of input characters
/// - Maximum length enforcement
/// - Cursor position tracking
/// - Visual feedback for focus state
#[derive(Debug, Clone)]
pub struct TextInput {
    /// Current input value
    pub value: String,

    /// Placeholder text shown when empty
    pub placeholder: String,

    /// Maximum allowed length (None = unlimited)
    pub max_length: Option<usize>,

    /// Character validator function
    pub validator: Option<fn(char) -> bool>,

    /// Current cursor position (0-indexed)
    pub cursor_pos: usize,

    /// Whether this input is currently focused
    pub is_focused: bool,
}

// Manual PartialEq implementation that compares all fields except validator
// (function pointers don't have meaningful equality)
impl PartialEq for TextInput {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
            && self.placeholder == other.placeholder
            && self.max_length == other.max_length
            && self.cursor_pos == other.cursor_pos
            && self.is_focused == other.is_focused
            // Note: We don't compare validator as function pointer equality is unreliable
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInput {
    /// Create a new empty TextInput
    pub fn new() -> Self {
        Self {
            value: String::new(),
            placeholder: String::new(),
            max_length: None,
            validator: None,
            cursor_pos: 0,
            is_focused: false,
        }
    }

    /// Set the placeholder text
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Set the maximum length
    pub fn with_max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }

    /// Set a character validator function
    pub fn with_validator(mut self, validator: fn(char) -> bool) -> Self {
        self.validator = Some(validator);
        self
    }

    /// Set the initial value
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        let val = value.into();
        self.cursor_pos = val.len();
        self.value = val;
        self
    }

    /// Set focus state
    pub fn set_focused(&mut self, focused: bool) {
        self.is_focused = focused;
    }

    /// Check if a character is valid according to the validator
    fn is_valid_char(&self, c: char) -> bool {
        if let Some(validator) = self.validator {
            validator(c)
        } else {
            true
        }
    }

    /// Handle keyboard input
    ///
    /// Returns true if the input was modified
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) if self.is_valid_char(c) => {
                if self.max_length.is_none_or(|max| self.value.len() < max) {
                    self.value.insert(self.cursor_pos, c);
                    self.cursor_pos += 1;
                    true
                } else {
                    false
                }
            }
            KeyCode::Backspace if self.cursor_pos > 0 => {
                self.value.remove(self.cursor_pos - 1);
                self.cursor_pos -= 1;
                true
            }
            KeyCode::Delete if self.cursor_pos < self.value.len() => {
                self.value.remove(self.cursor_pos);
                true
            }
            KeyCode::Left if self.cursor_pos > 0 => {
                self.cursor_pos -= 1;
                true
            }
            KeyCode::Right if self.cursor_pos < self.value.len() => {
                self.cursor_pos += 1;
                true
            }
            KeyCode::Home => {
                if self.cursor_pos > 0 {
                    self.cursor_pos = 0;
                    true
                } else {
                    false
                }
            }
            KeyCode::End => {
                if self.cursor_pos < self.value.len() {
                    self.cursor_pos = self.value.len();
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Render the text input widget
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let border_style = if self.is_focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .style(border_style);

        let display_text = if self.value.is_empty() {
            // Show placeholder in gray
            Line::from(self.placeholder.clone()).style(Style::default().fg(Color::DarkGray))
        } else {
            // Show actual value with cursor
            if self.is_focused {
                // Insert cursor character
                let mut display = self.value.clone();
                if self.cursor_pos < display.len() {
                    display.insert(self.cursor_pos, '█');
                } else {
                    display.push('█');
                }
                Line::from(display)
            } else {
                Line::from(self.value.clone())
            }
        };

        let paragraph = Paragraph::new(display_text).block(block);
        frame.render_widget(paragraph, area);
    }

    /// Clear the input value
    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor_pos = 0;
    }

    /// Get the current value
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Check if the input is empty
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}

// Predefined validators

/// Accepts alphanumeric characters and underscores
pub fn alphanumeric_validator(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Accepts only digits
pub fn digit_validator(c: char) -> bool {
    c.is_ascii_digit()
}

/// Accepts URL-safe characters
pub fn url_validator(c: char) -> bool {
    c.is_alphanumeric() || ":/.-_?&=".contains(c)
}

/// Accepts subreddit name characters (alphanumeric and underscore)
pub fn subreddit_validator(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_text_input() {
        let input = TextInput::new();
        assert_eq!(input.value, "");
        assert_eq!(input.cursor_pos, 0);
        assert!(!input.is_focused);
    }

    #[test]
    fn test_with_value() {
        let input = TextInput::new().with_value("hello");
        assert_eq!(input.value, "hello");
        assert_eq!(input.cursor_pos, 5);
    }

    #[test]
    fn test_char_input() {
        let mut input = TextInput::new();
        let key = KeyEvent::from(KeyCode::Char('a'));
        assert!(input.handle_key(key));
        assert_eq!(input.value, "a");
        assert_eq!(input.cursor_pos, 1);
    }

    #[test]
    fn test_backspace() {
        let mut input = TextInput::new().with_value("abc");
        let key = KeyEvent::from(KeyCode::Backspace);
        assert!(input.handle_key(key));
        assert_eq!(input.value, "ab");
        assert_eq!(input.cursor_pos, 2);
    }

    #[test]
    fn test_max_length() {
        let mut input = TextInput::new().with_max_length(3);
        input.handle_key(KeyEvent::from(KeyCode::Char('a')));
        input.handle_key(KeyEvent::from(KeyCode::Char('b')));
        input.handle_key(KeyEvent::from(KeyCode::Char('c')));
        assert!(!input.handle_key(KeyEvent::from(KeyCode::Char('d'))));
        assert_eq!(input.value, "abc");
    }

    #[test]
    fn test_validator() {
        let mut input = TextInput::new().with_validator(digit_validator);
        assert!(input.handle_key(KeyEvent::from(KeyCode::Char('5'))));
        assert!(!input.handle_key(KeyEvent::from(KeyCode::Char('a'))));
        assert_eq!(input.value, "5");
    }

    #[test]
    fn test_cursor_movement() {
        let mut input = TextInput::new().with_value("abc");
        input.cursor_pos = 3;

        assert!(input.handle_key(KeyEvent::from(KeyCode::Left)));
        assert_eq!(input.cursor_pos, 2);

        assert!(input.handle_key(KeyEvent::from(KeyCode::Right)));
        assert_eq!(input.cursor_pos, 3);

        assert!(input.handle_key(KeyEvent::from(KeyCode::Home)));
        assert_eq!(input.cursor_pos, 0);

        assert!(input.handle_key(KeyEvent::from(KeyCode::End)));
        assert_eq!(input.cursor_pos, 3);
    }

    #[test]
    fn test_clear() {
        let mut input = TextInput::new().with_value("test");
        input.clear();
        assert_eq!(input.value, "");
        assert_eq!(input.cursor_pos, 0);
    }

    #[test]
    fn test_alphanumeric_validator() {
        assert!(alphanumeric_validator('a'));
        assert!(alphanumeric_validator('Z'));
        assert!(alphanumeric_validator('5'));
        assert!(alphanumeric_validator('_'));
        assert!(!alphanumeric_validator('-'));
        assert!(!alphanumeric_validator(' '));
    }

    #[test]
    fn test_digit_validator() {
        assert!(digit_validator('0'));
        assert!(digit_validator('9'));
        assert!(!digit_validator('a'));
        assert!(!digit_validator('-'));
    }

    #[test]
    fn test_url_validator() {
        assert!(url_validator('h'));
        assert!(url_validator(':'));
        assert!(url_validator('/'));
        assert!(url_validator('.'));
        assert!(url_validator('?'));
        assert!(!url_validator(' '));
        assert!(!url_validator('#'));
    }
}
