use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table},
    Frame,
};

use crate::tui::state::Navigable;

/// A selectable table widget with navigation and optional sorting
///
/// This widget provides:
/// - Item selection with up/down navigation
/// - Customizable column definitions
/// - Optional sorting by column
/// - Empty state message
/// - Custom row formatting via closure
#[derive(Debug, Clone)]
pub struct SelectableTable<T: Clone> {
    /// The items to display in the table
    pub items: Vec<T>,

    /// Currently selected index
    pub selected: usize,

    /// Column definitions
    pub columns: Vec<ColumnDef>,

    /// Optional sort column index
    pub sort_column: Option<usize>,

    /// Sort direction (true = ascending, false = descending)
    pub sort_ascending: bool,

    /// Message to display when the table is empty
    pub empty_message: String,

    /// Optional block title
    pub block_title: Option<String>,
}

/// Column definition for a table
#[derive(Debug, Clone)]
pub struct ColumnDef {
    /// Column header title
    pub title: &'static str,

    /// Column width constraint
    pub width: Constraint,

    /// Whether this column is sortable
    pub sortable: bool,
}

impl ColumnDef {
    /// Create a new column definition
    pub fn new(title: &'static str, width: Constraint) -> Self {
        Self {
            title,
            width,
            sortable: false,
        }
    }

    /// Mark this column as sortable
    pub fn sortable(mut self) -> Self {
        self.sortable = true;
        self
    }
}

impl<T: Clone> SelectableTable<T> {
    /// Create a new SelectableTable
    pub fn new(items: Vec<T>, columns: Vec<ColumnDef>) -> Self {
        Self {
            items,
            selected: 0,
            columns,
            sort_column: None,
            sort_ascending: true,
            empty_message: "No items".to_string(),
            block_title: None,
        }
    }

    /// Set the empty message
    pub fn with_empty_message(mut self, message: impl Into<String>) -> Self {
        self.empty_message = message.into();
        self
    }

    /// Set the block title
    pub fn with_block_title(mut self, title: impl Into<String>) -> Self {
        self.block_title = Some(title.into());
        self
    }

    /// Get the currently selected item
    pub fn selected_item(&self) -> Option<&T> {
        self.items.get(self.selected)
    }

    /// Get a mutable reference to the currently selected item
    pub fn selected_item_mut(&mut self) -> Option<&mut T> {
        self.items.get_mut(self.selected)
    }

    /// Check if the table is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the number of items
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Handle keyboard navigation
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
            _ => false,
        }
    }

    /// Render the table with a custom row formatter
    ///
    /// The formatter takes (item, index, is_selected) and returns a Row
    pub fn render<F>(&self, frame: &mut Frame, area: Rect, row_formatter: F)
    where
        F: Fn(&T, usize, bool) -> Row<'static>,
    {
        if self.items.is_empty() {
            // Render empty state
            let empty = ratatui::widgets::Paragraph::new(self.empty_message.clone())
                .alignment(ratatui::layout::Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            frame.render_widget(empty, area);
            return;
        }

        // Build header
        let header_cells: Vec<_> = self
            .columns
            .iter()
            .enumerate()
            .map(|(i, col)| {
                let mut title = col.title.to_string();
                if let Some(sort_col) = self.sort_column {
                    if sort_col == i {
                        title.push_str(if self.sort_ascending { " ▲" } else { " ▼" });
                    }
                }
                title
            })
            .collect();

        let header = Row::new(header_cells)
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .bottom_margin(1);

        // Build rows
        let rows: Vec<Row> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| row_formatter(item, i, i == self.selected))
            .collect();

        // Build table
        let widths: Vec<Constraint> = self.columns.iter().map(|c| c.width).collect();

        let block = if let Some(ref title) = self.block_title {
            Block::default().borders(Borders::ALL).title(title.as_str())
        } else {
            Block::default().borders(Borders::ALL)
        };

        let table = Table::new(rows, widths)
            .header(header)
            .block(block);

        frame.render_widget(table, area);
    }
}

impl<T: Clone> Navigable for SelectableTable<T> {
    fn len(&self) -> usize {
        self.items.len()
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

    #[derive(Debug, Clone, PartialEq)]
    struct TestItem {
        id: i64,
        name: String,
    }

    fn create_test_table() -> SelectableTable<TestItem> {
        let items = vec![
            TestItem { id: 1, name: "Alice".to_string() },
            TestItem { id: 2, name: "Bob".to_string() },
            TestItem { id: 3, name: "Charlie".to_string() },
        ];

        let columns = vec![
            ColumnDef::new("ID", Constraint::Length(5)),
            ColumnDef::new("Name", Constraint::Percentage(50)),
        ];

        SelectableTable::new(items, columns)
    }

    #[test]
    fn test_new_table() {
        let table = create_test_table();
        assert_eq!(table.len(), 3);
        assert_eq!(table.selected, 0);
        assert!(!table.is_empty());
    }

    #[test]
    fn test_empty_table() {
        let columns = vec![ColumnDef::new("ID", Constraint::Length(5))];
        let table: SelectableTable<TestItem> = SelectableTable::new(vec![], columns);
        assert_eq!(table.len(), 0);
        assert!(table.is_empty());
    }

    #[test]
    fn test_selected_item() {
        let table = create_test_table();
        let item = table.selected_item().unwrap();
        assert_eq!(item.id, 1);
        assert_eq!(item.name, "Alice");
    }

    #[test]
    fn test_navigation() {
        let mut table = create_test_table();

        // Start at 0
        assert_eq!(table.selected(), 0);

        // Move down
        table.next();
        assert_eq!(table.selected(), 1);

        table.next();
        assert_eq!(table.selected(), 2);

        // Wrap around
        table.next();
        assert_eq!(table.selected(), 0);

        // Move up
        table.previous();
        assert_eq!(table.selected(), 2);
    }

    #[test]
    fn test_handle_key() {
        let mut table = create_test_table();

        assert!(table.handle_key(KeyEvent::from(KeyCode::Down)));
        assert_eq!(table.selected(), 1);

        assert!(table.handle_key(KeyEvent::from(KeyCode::Up)));
        assert_eq!(table.selected(), 0);

        assert!(!table.handle_key(KeyEvent::from(KeyCode::Enter)));
    }

    #[test]
    fn test_with_empty_message() {
        let columns = vec![ColumnDef::new("ID", Constraint::Length(5))];
        let table: SelectableTable<TestItem> = SelectableTable::new(vec![], columns)
            .with_empty_message("No data available");

        assert_eq!(table.empty_message, "No data available");
    }

    #[test]
    fn test_column_def() {
        let col = ColumnDef::new("Test", Constraint::Percentage(50));
        assert_eq!(col.title, "Test");
        assert!(!col.sortable);

        let col = col.sortable();
        assert!(col.sortable);
    }

    #[test]
    fn test_navigable_trait() {
        let mut table = create_test_table();
        assert_eq!(Navigable::len(&table), 3);
        assert_eq!(Navigable::selected(&table), 0);

        Navigable::set_selected(&mut table, 2);
        assert_eq!(Navigable::selected(&table), 2);
    }
}
