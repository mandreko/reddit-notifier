//! Common reusable UI components for the TUI
//!
//! This module provides standard UI patterns used across multiple screens,
//! reducing code duplication and ensuring consistent styling.

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Standard 3-section screen layout (title, content, help)
///
/// Returns a 3-element array with fixed-height title and help sections,
/// and a flexible-height content section.
///
/// # Example
/// ```no_run
/// # use ratatui::layout::Rect;
/// # use reddit_notifier::tui::widgets::common::render_screen_layout;
/// # let area = Rect::default();
/// let chunks = render_screen_layout(area);
/// // chunks[0] = title area (height: 3)
/// // chunks[1] = content area (flexible)
/// // chunks[2] = help area (height: 3)
/// ```
pub fn render_screen_layout(area: Rect) -> [Rect; 3] {
    let chunks = Layout::vertical([
        Constraint::Length(3),    // Title
        Constraint::Min(0),       // Content
        Constraint::Length(3),    // Help
    ])
    .split(area);

    [chunks[0], chunks[1], chunks[2]]
}

/// Render a styled title bar
///
/// Creates a centered title with cyan borders.
pub fn render_title(frame: &mut Frame, area: Rect, title: &str) {
    let widget = Paragraph::new(title)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        );
    frame.render_widget(widget, area);
}

/// Render help text with keyboard shortcuts
///
/// Takes an array of (key, description) tuples and renders them
/// as a centered help bar at the bottom of the screen.
///
/// # Example
/// ```no_run
/// # use ratatui::Frame;
/// # use ratatui::layout::Rect;
/// # use reddit_notifier::tui::widgets::common::render_help;
/// # let mut frame: Frame = panic!();
/// # let help_area = Rect::default();
/// render_help(&mut frame, help_area, &[
///     ("↑/↓", "Navigate"),
///     ("Enter", "Select"),
///     ("q", "Quit"),
/// ]);
/// ```
pub fn render_help(frame: &mut Frame, area: Rect, items: &[(&str, &str)]) {
    let spans: Vec<Span> = items
        .iter()
        .flat_map(|(key, desc)| {
            vec![Span::raw(format!("[{}] {}  ", key, desc))]
        })
        .collect();

    let widget = Paragraph::new(Line::from(spans))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(widget, area);
}

/// Render an empty state message
///
/// Used when a list or table has no items to display.
pub fn render_empty_state(frame: &mut Frame, area: Rect, message: &str) {
    let widget = Paragraph::new(message)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(widget, area);
}

/// Calculate a centered rectangle for popups
///
/// Returns a rectangle centered within the given area, with the specified
/// percentage of width and height.
///
/// # Arguments
/// * `percent_x` - Width as percentage of parent (0-100)
/// * `percent_y` - Height as percentage of parent (0-100)
/// * `r` - Parent rectangle to center within
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

/// Render a message popup (error or success)
///
/// Displays a centered popup with a message and a prompt to dismiss.
///
/// # Arguments
/// * `frame` - The frame to render to
/// * `area` - The full screen area (popup will be centered)
/// * `message` - The message to display
/// * `color` - Color for the popup border (e.g., Red for errors, Green for success)
/// * `title` - Title for the popup (e.g., "Error" or "Success")
pub fn render_message_popup(
    frame: &mut Frame,
    area: Rect,
    message: &str,
    color: Color,
    title: &str,
) {
    let popup_area = centered_rect(60, 20, area);
    let widget = Paragraph::new(vec![
        Line::from(""),
        Line::from(message).alignment(Alignment::Center),
        Line::from(""),
        Line::from("[Press any key]").alignment(Alignment::Center),
    ])
    .block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .style(Style::default().fg(color)),
    );

    frame.render_widget(Clear, popup_area);
    frame.render_widget(widget, popup_area);
}

/// Render a confirmation dialog
///
/// Displays a centered yes/no confirmation dialog.
///
/// # Arguments
/// * `frame` - The frame to render to
/// * `area` - The full screen area (dialog will be centered)
/// * `prompt` - The question to ask the user
/// * `title` - Title for the dialog
pub fn render_confirm_dialog(
    frame: &mut Frame,
    area: Rect,
    prompt: &str,
    title: &str,
) {
    let popup_area = centered_rect(50, 30, area);
    let widget = Paragraph::new(vec![
        Line::from(""),
        Line::from(prompt).alignment(Alignment::Center),
        Line::from(""),
        Line::from(vec![
            Span::raw("["),
            Span::styled("y", Style::default().fg(Color::Yellow)),
            Span::raw("] Yes    ["),
            Span::styled("n", Style::default().fg(Color::Yellow)),
            Span::raw("] No"),
        ])
        .alignment(Alignment::Center),
    ])
    .block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Red)),
    );

    frame.render_widget(Clear, popup_area);
    frame.render_widget(widget, popup_area);
}

/// Get selection marker and style for list items
///
/// Returns a tuple of (prefix, style) to be applied to list items.
/// Selected items get a ">" prefix and yellow bold styling.
///
/// # Example
/// ```no_run
/// # use ratatui::widgets::ListItem;
/// # use reddit_notifier::tui::widgets::common::selection_style;
/// # let is_selected = true;
/// # let item_text = "Menu Item";
/// let (prefix, style) = selection_style(is_selected);
/// let text = format!("{}{}", prefix, item_text);
/// let list_item = ListItem::new(text).style(style);
/// ```
pub fn selection_style(is_selected: bool) -> (String, Style) {
    let prefix = if is_selected {
        "> ".to_string()
    } else {
        "  ".to_string()
    };
    let style = if is_selected {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    (prefix, style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_style_selected() {
        let (prefix, style) = selection_style(true);
        assert_eq!(prefix, "> ");
        assert_eq!(style.fg, Some(Color::Yellow));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_selection_style_not_selected() {
        let (prefix, style) = selection_style(false);
        assert_eq!(prefix, "  ");
        assert_eq!(style.fg, None);
    }

    #[test]
    fn test_centered_rect() {
        // Test centering with different percentages
        let area = Rect::new(0, 0, 100, 100);

        // 50x50 should be centered at (25, 25)
        let centered = centered_rect(50, 50, area);
        assert_eq!(centered.width, 50);
        assert_eq!(centered.height, 50);
        assert_eq!(centered.x, 25);
        assert_eq!(centered.y, 25);

        // 60x20 should be centered at (20, 40)
        let centered = centered_rect(60, 20, area);
        assert_eq!(centered.width, 60);
        assert_eq!(centered.height, 20);
        assert_eq!(centered.x, 20);
        assert_eq!(centered.y, 40);
    }

    #[test]
    fn test_screen_layout() {
        let area = Rect::new(0, 0, 100, 50);
        let chunks = render_screen_layout(area);

        assert_eq!(chunks.len(), 3);

        // Title at top: height 3, starts at y=0
        assert_eq!(chunks[0].height, 3);
        assert_eq!(chunks[0].y, 0);

        // Content in middle: takes remaining space, starts after title
        assert_eq!(chunks[1].height, 44); // 50 - 3 - 3 = 44
        assert_eq!(chunks[1].y, 3);

        // Help at bottom: height 3, at end
        assert_eq!(chunks[2].height, 3);
        assert_eq!(chunks[2].y, 47);
    }
}
