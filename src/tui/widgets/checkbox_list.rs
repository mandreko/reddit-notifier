use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem},
    Frame,
};
use std::collections::HashSet;

use crate::tui::state::Navigable;

/// A list widget with checkboxes for multi-selection
///
/// This widget provides:
/// - Navigation through items with up/down keys
/// - Toggle selection with Space
/// - Select/deselect all functionality
/// - Custom item formatting
#[derive(Debug, Clone, PartialEq)]
pub struct CheckboxList<T: Clone + PartialEq> {
    /// The items in the list
    pub items: Vec<T>,

    /// Currently selected (focused) index
    pub selected_idx: usize,

    /// Indices of checked items
    pub checked_indices: HashSet<usize>,
}

impl<T: Clone + PartialEq> CheckboxList<T> {
    /// Create a new CheckboxList
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            selected_idx: 0,
            checked_indices: HashSet::new(),
        }
    }

    /// Create a CheckboxList with pre-checked items
    pub fn with_checked(items: Vec<T>, checked: impl IntoIterator<Item = usize>) -> Self {
        Self {
            items,
            selected_idx: 0,
            checked_indices: checked.into_iter().collect(),
        }
    }

    /// Toggle the checkbox for the currently selected item
    pub fn toggle_current(&mut self) {
        if self.checked_indices.contains(&self.selected_idx) {
            self.checked_indices.remove(&self.selected_idx);
        } else {
            self.checked_indices.insert(self.selected_idx);
        }
    }

    /// Check if an item at the given index is checked
    pub fn is_checked(&self, index: usize) -> bool {
        self.checked_indices.contains(&index)
    }

    /// Select all items
    pub fn select_all(&mut self) {
        self.checked_indices = (0..self.items.len()).collect();
    }

    /// Deselect all items
    pub fn deselect_all(&mut self) {
        self.checked_indices.clear();
    }

    /// Toggle all items (if all checked, uncheck all; otherwise check all)
    pub fn toggle_all(&mut self) {
        if self.checked_indices.len() == self.items.len() {
            self.deselect_all();
        } else {
            self.select_all();
        }
    }

    /// Get references to all checked items (in index order)
    pub fn get_checked_items(&self) -> Vec<&T> {
        let mut indices: Vec<_> = self.checked_indices.iter().copied().collect();
        indices.sort_unstable();
        indices
            .iter()
            .filter_map(|&idx| self.items.get(idx))
            .collect()
    }

    /// Get indices of all checked items
    pub fn get_checked_indices(&self) -> Vec<usize> {
        let mut indices: Vec<_> = self.checked_indices.iter().copied().collect();
        indices.sort_unstable();
        indices
    }

    /// Check if the list is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the number of items
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Handle keyboard input
    ///
    /// Returns true if the key was handled
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Up => {
                self.previous();
                true
            }
            KeyCode::Down => {
                self.next();
                true
            }
            KeyCode::Char(' ') => {
                self.toggle_current();
                true
            }
            KeyCode::Char('a') => {
                self.toggle_all();
                true
            }
            _ => false,
        }
    }

    /// Render the checkbox list with a custom formatter
    ///
    /// The formatter takes a reference to the item and returns a String
    pub fn render<F>(&self, frame: &mut Frame, area: Rect, formatter: F)
    where
        F: Fn(&T) -> String,
    {
        let items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let checkbox = if self.is_checked(i) { "[x]" } else { "[ ]" };
                let text = formatter(item);
                let prefix = if i == self.selected_idx { "> " } else { "  " };

                let style = if i == self.selected_idx {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(format!("{}{} {}", prefix, checkbox, text))).style(style)
            })
            .collect();

        let list = List::new(items).block(Block::default().borders(Borders::ALL));

        frame.render_widget(list, area);
    }
}

impl<T: Clone + PartialEq> Navigable for CheckboxList<T> {
    fn len(&self) -> usize {
        self.items.len()
    }

    fn selected(&self) -> usize {
        self.selected_idx
    }

    fn set_selected(&mut self, index: usize) {
        self.selected_idx = index;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestItem {
        id: i64,
        name: String,
    }

    fn create_test_list() -> CheckboxList<TestItem> {
        let items = vec![
            TestItem { id: 1, name: "Alice".to_string() },
            TestItem { id: 2, name: "Bob".to_string() },
            TestItem { id: 3, name: "Charlie".to_string() },
        ];

        CheckboxList::new(items)
    }

    #[test]
    fn test_new_list() {
        let list = create_test_list();
        assert_eq!(list.len(), 3);
        assert_eq!(list.selected_idx, 0);
        assert!(list.checked_indices.is_empty());
        assert!(!list.is_empty());
    }

    #[test]
    fn test_with_checked() {
        let items = vec![
            TestItem { id: 1, name: "Alice".to_string() },
            TestItem { id: 2, name: "Bob".to_string() },
        ];

        let list = CheckboxList::with_checked(items, vec![0, 1]);
        assert_eq!(list.checked_indices.len(), 2);
        assert!(list.is_checked(0));
        assert!(list.is_checked(1));
    }

    #[test]
    fn test_toggle_current() {
        let mut list = create_test_list();

        assert!(!list.is_checked(0));
        list.toggle_current();
        assert!(list.is_checked(0));
        list.toggle_current();
        assert!(!list.is_checked(0));
    }

    #[test]
    fn test_select_all() {
        let mut list = create_test_list();
        list.select_all();

        assert_eq!(list.checked_indices.len(), 3);
        assert!(list.is_checked(0));
        assert!(list.is_checked(1));
        assert!(list.is_checked(2));
    }

    #[test]
    fn test_deselect_all() {
        let mut list = create_test_list();
        list.select_all();
        assert_eq!(list.checked_indices.len(), 3);

        list.deselect_all();
        assert!(list.checked_indices.is_empty());
    }

    #[test]
    fn test_toggle_all() {
        let mut list = create_test_list();

        // Toggle when empty -> select all
        list.toggle_all();
        assert_eq!(list.checked_indices.len(), 3);

        // Toggle when all selected -> deselect all
        list.toggle_all();
        assert!(list.checked_indices.is_empty());
    }

    #[test]
    fn test_get_checked_items() {
        let mut list = create_test_list();
        list.checked_indices.insert(0);
        list.checked_indices.insert(2);

        let checked = list.get_checked_items();
        assert_eq!(checked.len(), 2);
        assert_eq!(checked[0].name, "Alice");
        assert_eq!(checked[1].name, "Charlie");
    }

    #[test]
    fn test_get_checked_indices() {
        let mut list = create_test_list();
        list.checked_indices.insert(2);
        list.checked_indices.insert(0);

        let indices = list.get_checked_indices();
        assert_eq!(indices, vec![0, 2]); // Should be sorted
    }

    #[test]
    fn test_handle_key() {
        let mut list = create_test_list();

        // Navigate down
        assert!(list.handle_key(KeyEvent::from(KeyCode::Down)));
        assert_eq!(list.selected_idx, 1);

        // Navigate up
        assert!(list.handle_key(KeyEvent::from(KeyCode::Up)));
        assert_eq!(list.selected_idx, 0);

        // Toggle with space
        assert!(list.handle_key(KeyEvent::from(KeyCode::Char(' '))));
        assert!(list.is_checked(0));

        // Toggle all with 'a'
        assert!(list.handle_key(KeyEvent::from(KeyCode::Char('a'))));
        assert_eq!(list.checked_indices.len(), 3);

        // Unhandled key
        assert!(!list.handle_key(KeyEvent::from(KeyCode::Enter)));
    }

    #[test]
    fn test_navigation() {
        let mut list = create_test_list();

        // Start at 0
        assert_eq!(list.selected(), 0);

        // Move down
        list.next();
        assert_eq!(list.selected(), 1);

        list.next();
        assert_eq!(list.selected(), 2);

        // Wrap around
        list.next();
        assert_eq!(list.selected(), 0);

        // Move up
        list.previous();
        assert_eq!(list.selected(), 2);
    }

    #[test]
    fn test_navigable_trait() {
        let mut list = create_test_list();
        assert_eq!(Navigable::len(&list), 3);
        assert_eq!(Navigable::selected(&list), 0);

        Navigable::set_selected(&mut list, 2);
        assert_eq!(Navigable::selected(&list), 2);
    }
}
