use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::tui::state::Navigable;

/// A dropdown widget with filtering and selection
///
/// This widget provides:
/// - Filterable list of options
/// - Keyboard navigation
/// - Optional "None" selection
/// - Popup rendering
#[derive(Debug, Clone)]
pub struct Dropdown {
    /// Available options
    pub options: Vec<String>,

    /// Currently selected index (in filtered list)
    pub selected: usize,

    /// Filter text
    pub filter: String,

    /// Dropdown title
    pub title: String,

    /// Whether to allow selecting "None"
    pub allow_none: bool,
}

impl Dropdown {
    /// Create a new dropdown
    pub fn new(options: Vec<String>, title: impl Into<String>) -> Self {
        Self {
            options,
            selected: 0,
            filter: String::new(),
            title: title.into(),
            allow_none: false,
        }
    }

    /// Allow selecting "None" as an option
    pub fn with_none_option(mut self) -> Self {
        self.allow_none = true;
        self
    }

    /// Get the filtered options with their original indices
    pub fn filtered_options(&self) -> Vec<(usize, &String)> {
        self.options
            .iter()
            .enumerate()
            .filter(|(_, opt)| {
                self.filter.is_empty()
                    || opt.to_lowercase().contains(&self.filter.to_lowercase())
            })
            .collect()
    }

    /// Get the currently selected option (if any)
    pub fn selected_option(&self) -> Option<&String> {
        let filtered = self.filtered_options();
        filtered.get(self.selected).map(|(_, opt)| *opt)
    }

    /// Get the original index of the selected option
    pub fn selected_index(&self) -> Option<usize> {
        let filtered = self.filtered_options();
        filtered.get(self.selected).map(|(idx, _)| *idx)
    }

    /// Handle keyboard input
    ///
    /// Returns Some(index) if an option was selected, None otherwise
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<usize> {
        match key.code {
            KeyCode::Up => {
                self.previous();
                None
            }
            KeyCode::Down => {
                self.next();
                None
            }
            KeyCode::Enter => self.selected_index(),
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.selected = 0;
                None
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.selected = 0;
                None
            }
            _ => None,
        }
    }

    /// Render the dropdown as a popup
    pub fn render_as_popup(&self, frame: &mut Frame, area: Rect) {
        let popup_area = self.centered_rect(60, 50, area);

        // Clear the background
        frame.render_widget(Clear, popup_area);

        // Split into filter and list areas
        let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(popup_area);

        // Render filter input
        let filter_text = if self.filter.is_empty() {
            "[Type to filter...]".to_string()
        } else {
            self.filter.clone()
        };

        let filter_widget = Paragraph::new(filter_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.title.clone())
                    .title_alignment(Alignment::Center),
            )
            .style(Style::default().fg(Color::Yellow));

        frame.render_widget(filter_widget, chunks[0]);

        // Render filtered options
        let filtered = self.filtered_options();

        let items: Vec<ListItem> = filtered
            .iter()
            .enumerate()
            .map(|(i, (_, opt))| {
                let prefix = if i == self.selected { "> " } else { "  " };
                let style = if i == self.selected {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(format!("{}{}", prefix, opt))).style(style)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("{} options", filtered.len())),
        );

        frame.render_widget(list, chunks[1]);
    }

    /// Calculate centered rectangle for popup
    fn centered_rect(&self, percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
}

impl Navigable for Dropdown {
    fn len(&self) -> usize {
        self.filtered_options().len()
    }

    fn selected(&self) -> usize {
        self.selected
    }

    fn set_selected(&mut self, index: usize) {
        self.selected = index;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_dropdown() -> Dropdown {
        let options = vec![
            "Apple".to_string(),
            "Banana".to_string(),
            "Cherry".to_string(),
            "Date".to_string(),
        ];

        Dropdown::new(options, "Select Fruit")
    }

    #[test]
    fn test_new_dropdown() {
        let dropdown = create_test_dropdown();
        assert_eq!(dropdown.options.len(), 4);
        assert_eq!(dropdown.selected, 0);
        assert!(dropdown.filter.is_empty());
        assert_eq!(dropdown.title, "Select Fruit");
        assert!(!dropdown.allow_none);
    }

    #[test]
    fn test_with_none_option() {
        let dropdown = create_test_dropdown().with_none_option();
        assert!(dropdown.allow_none);
    }

    #[test]
    fn test_filtered_options() {
        let mut dropdown = create_test_dropdown();

        // No filter - all options
        let filtered = dropdown.filtered_options();
        assert_eq!(filtered.len(), 4);

        // Filter for "an"
        dropdown.filter = "an".to_string();
        let filtered = dropdown.filtered_options();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].1, "Banana");

        // Filter for "a" (case insensitive)
        dropdown.filter = "a".to_string();
        let filtered = dropdown.filtered_options();
        assert_eq!(filtered.len(), 3); // Apple, Banana, Date
    }

    #[test]
    fn test_selected_option() {
        let dropdown = create_test_dropdown();
        let selected = dropdown.selected_option();
        assert_eq!(selected, Some(&"Apple".to_string()));
    }

    #[test]
    fn test_selected_index() {
        let dropdown = create_test_dropdown();
        let index = dropdown.selected_index();
        assert_eq!(index, Some(0));
    }

    #[test]
    fn test_handle_key_navigation() {
        let mut dropdown = create_test_dropdown();

        // Down
        assert_eq!(dropdown.handle_key(KeyEvent::from(KeyCode::Down)), None);
        assert_eq!(dropdown.selected, 1);

        // Up
        assert_eq!(dropdown.handle_key(KeyEvent::from(KeyCode::Up)), None);
        assert_eq!(dropdown.selected, 0);

        // Enter returns index
        assert_eq!(dropdown.handle_key(KeyEvent::from(KeyCode::Enter)), Some(0));
    }

    #[test]
    fn test_handle_key_filter() {
        let mut dropdown = create_test_dropdown();

        // Type 'a'
        dropdown.handle_key(KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(dropdown.filter, "a");
        assert_eq!(dropdown.selected, 0); // Reset to 0

        // Type 'n'
        dropdown.handle_key(KeyEvent::from(KeyCode::Char('n')));
        assert_eq!(dropdown.filter, "an");

        // Backspace
        dropdown.handle_key(KeyEvent::from(KeyCode::Backspace));
        assert_eq!(dropdown.filter, "a");
    }

    #[test]
    fn test_navigation() {
        let mut dropdown = create_test_dropdown();

        // Start at 0
        assert_eq!(dropdown.selected(), 0);

        // Move down
        dropdown.next();
        assert_eq!(dropdown.selected(), 1);

        dropdown.next();
        assert_eq!(dropdown.selected(), 2);

        // Move up
        dropdown.previous();
        assert_eq!(dropdown.selected(), 1);

        // Set selected
        dropdown.set_selected(3);
        assert_eq!(dropdown.selected(), 3);
    }

    #[test]
    fn test_navigable_len() {
        let mut dropdown = create_test_dropdown();
        assert_eq!(Navigable::len(&dropdown), 4);

        // With filter
        dropdown.filter = "a".to_string();
        assert_eq!(Navigable::len(&dropdown), 3);
    }
}
