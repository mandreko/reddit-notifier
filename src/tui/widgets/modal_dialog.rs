use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// A modal dialog widget for displaying messages and confirmations
///
/// This widget provides:
/// - Centered modal display
/// - Different dialog types (Error, Success, Warning, Info, Confirmation)
/// - Customizable size
/// - Automatic message wrapping
#[derive(Debug, Clone)]
pub struct ModalDialog {
    /// Dialog title
    pub title: String,

    /// Dialog content lines
    pub content: Vec<Line<'static>>,

    /// Type of dialog (affects styling)
    pub dialog_type: DialogType,

    /// Width as percentage of screen (1-100)
    pub width_percent: u16,

    /// Height as percentage of screen (1-100)
    pub height_percent: u16,
}

/// Types of modal dialogs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogType {
    /// Error message (red border)
    Error,

    /// Success message (green border)
    Success,

    /// Warning message (yellow border)
    Warning,

    /// Information message (blue border)
    Info,

    /// Confirmation prompt (yellow border)
    Confirmation,
}

impl DialogType {
    /// Get the border color for this dialog type
    pub fn border_color(&self) -> Color {
        match self {
            Self::Error => Color::Red,
            Self::Success => Color::Green,
            Self::Warning => Color::Yellow,
            Self::Info => Color::Cyan,
            Self::Confirmation => Color::Yellow,
        }
    }

    /// Get the default title for this dialog type
    pub fn default_title(&self) -> &'static str {
        match self {
            Self::Error => "Error",
            Self::Success => "Success",
            Self::Warning => "Warning",
            Self::Info => "Information",
            Self::Confirmation => "Confirm",
        }
    }
}

impl ModalDialog {
    /// Create a new modal dialog
    pub fn new(dialog_type: DialogType, title: impl Into<String>, message: impl Into<String>) -> Self {
        let message_str = message.into();
        Self {
            title: title.into(),
            content: vec![
                Line::from(""),
                Line::from(message_str).alignment(Alignment::Center),
                Line::from(""),
            ],
            dialog_type,
            width_percent: 60,
            height_percent: 20,
        }
    }

    /// Create an error dialog
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(DialogType::Error, "Error", message)
    }

    /// Create a success dialog
    pub fn success(message: impl Into<String>) -> Self {
        let mut dialog = Self::new(DialogType::Success, "Success", message);
        dialog.content.push(Line::from("[Press any key]").alignment(Alignment::Center));
        dialog
    }

    /// Create a warning dialog
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(DialogType::Warning, "Warning", message)
    }

    /// Create an info dialog
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(DialogType::Info, "Information", message)
    }

    /// Create a confirmation dialog
    pub fn confirm(prompt: impl Into<String>) -> Self {
        let mut dialog = Self::new(DialogType::Confirmation, "Confirm", prompt);
        dialog.content.push(Line::from(""));
        dialog.content.push(Line::from("[Y] Yes    [N] No").alignment(Alignment::Center));
        dialog
    }

    /// Set the dialog width as percentage
    pub fn with_width_percent(mut self, width: u16) -> Self {
        self.width_percent = width.clamp(1, 100);
        self
    }

    /// Set the dialog height as percentage
    pub fn with_height_percent(mut self, height: u16) -> Self {
        self.height_percent = height.clamp(1, 100);
        self
    }

    /// Set custom content lines
    pub fn with_content(mut self, content: Vec<Line<'static>>) -> Self {
        self.content = content;
        self
    }

    /// Add a line to the content
    pub fn add_line(mut self, line: Line<'static>) -> Self {
        self.content.push(line);
        self
    }

    /// Calculate the centered rectangle for the modal
    fn centered_rect(&self, area: Rect) -> Rect {
        let popup_layout = Layout::vertical([
            Constraint::Percentage((100 - self.height_percent) / 2),
            Constraint::Percentage(self.height_percent),
            Constraint::Percentage((100 - self.height_percent) / 2),
        ])
        .split(area);

        Layout::horizontal([
            Constraint::Percentage((100 - self.width_percent) / 2),
            Constraint::Percentage(self.width_percent),
            Constraint::Percentage((100 - self.width_percent) / 2),
        ])
        .split(popup_layout[1])[1]
    }

    /// Render the modal dialog
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let modal_area = self.centered_rect(area);

        // Clear the background
        frame.render_widget(Clear, modal_area);

        // Create the dialog box
        let border_style = Style::default()
            .fg(self.dialog_type.border_color())
            .add_modifier(Modifier::BOLD);

        let block = Block::default()
            .title(self.title.clone())
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .style(border_style);

        let paragraph = Paragraph::new(self.content.clone())
            .block(block)
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, modal_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_modal() {
        let dialog = ModalDialog::new(DialogType::Info, "Test", "This is a test");
        assert_eq!(dialog.title, "Test");
        assert_eq!(dialog.dialog_type, DialogType::Info);
        assert_eq!(dialog.width_percent, 60);
        assert_eq!(dialog.height_percent, 20);
    }

    #[test]
    fn test_error_dialog() {
        let dialog = ModalDialog::error("Something went wrong");
        assert_eq!(dialog.title, "Error");
        assert_eq!(dialog.dialog_type, DialogType::Error);
    }

    #[test]
    fn test_success_dialog() {
        let dialog = ModalDialog::success("Operation completed");
        assert_eq!(dialog.title, "Success");
        assert_eq!(dialog.dialog_type, DialogType::Success);
        // Should have the "Press any key" message
        assert!(dialog.content.len() > 3);
    }

    #[test]
    fn test_warning_dialog() {
        let dialog = ModalDialog::warning("This is a warning");
        assert_eq!(dialog.title, "Warning");
        assert_eq!(dialog.dialog_type, DialogType::Warning);
    }

    #[test]
    fn test_info_dialog() {
        let dialog = ModalDialog::info("For your information");
        assert_eq!(dialog.title, "Information");
        assert_eq!(dialog.dialog_type, DialogType::Info);
    }

    #[test]
    fn test_confirm_dialog() {
        let dialog = ModalDialog::confirm("Are you sure?");
        assert_eq!(dialog.title, "Confirm");
        assert_eq!(dialog.dialog_type, DialogType::Confirmation);
        // Should have the Y/N prompt
        assert!(dialog.content.len() > 3);
    }

    #[test]
    fn test_with_width_percent() {
        let dialog = ModalDialog::info("Test").with_width_percent(80);
        assert_eq!(dialog.width_percent, 80);
    }

    #[test]
    fn test_with_height_percent() {
        let dialog = ModalDialog::info("Test").with_height_percent(30);
        assert_eq!(dialog.height_percent, 30);
    }

    #[test]
    fn test_width_percent_clamping() {
        let dialog1 = ModalDialog::info("Test").with_width_percent(0);
        assert_eq!(dialog1.width_percent, 1);

        let dialog2 = ModalDialog::info("Test").with_width_percent(150);
        assert_eq!(dialog2.width_percent, 100);
    }

    #[test]
    fn test_height_percent_clamping() {
        let dialog1 = ModalDialog::info("Test").with_height_percent(0);
        assert_eq!(dialog1.height_percent, 1);

        let dialog2 = ModalDialog::info("Test").with_height_percent(200);
        assert_eq!(dialog2.height_percent, 100);
    }

    #[test]
    fn test_dialog_type_colors() {
        assert_eq!(DialogType::Error.border_color(), Color::Red);
        assert_eq!(DialogType::Success.border_color(), Color::Green);
        assert_eq!(DialogType::Warning.border_color(), Color::Yellow);
        assert_eq!(DialogType::Info.border_color(), Color::Cyan);
        assert_eq!(DialogType::Confirmation.border_color(), Color::Yellow);
    }

    #[test]
    fn test_dialog_type_titles() {
        assert_eq!(DialogType::Error.default_title(), "Error");
        assert_eq!(DialogType::Success.default_title(), "Success");
        assert_eq!(DialogType::Warning.default_title(), "Warning");
        assert_eq!(DialogType::Info.default_title(), "Information");
        assert_eq!(DialogType::Confirmation.default_title(), "Confirm");
    }

    #[test]
    fn test_add_line() {
        let dialog = ModalDialog::info("Test")
            .add_line(Line::from("Extra line 1"))
            .add_line(Line::from("Extra line 2"));
        // Original 3 lines + 2 added = 5
        assert!(dialog.content.len() >= 5);
    }

    #[test]
    fn test_with_content() {
        let custom_content = vec![
            Line::from("Line 1"),
            Line::from("Line 2"),
        ];
        let dialog = ModalDialog::info("Test").with_content(custom_content.clone());
        assert_eq!(dialog.content.len(), 2);
    }
}
