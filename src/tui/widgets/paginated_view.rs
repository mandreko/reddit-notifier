use crossterm::event::{KeyCode, KeyEvent};

/// A pagination wrapper for managing large lists of items
///
/// This widget provides:
/// - Page-based navigation
/// - Configurable page size
/// - Current page tracking
/// - Page info rendering
#[derive(Debug, Clone)]
pub struct PaginatedView<T: Clone> {
    /// All items to paginate
    pub items: Vec<T>,

    /// Current page number (0-indexed)
    pub current_page: usize,

    /// Number of items per page
    pub page_size: usize,
}

impl<T: Clone> PaginatedView<T> {
    /// Create a new PaginatedView
    pub fn new(items: Vec<T>, page_size: usize) -> Self {
        Self {
            items,
            current_page: 0,
            page_size: page_size.max(1), // Ensure at least 1 item per page
        }
    }

    /// Get the items for the current page
    pub fn current_page_items(&self) -> &[T] {
        let start = self.current_page * self.page_size;
        let end = (start + self.page_size).min(self.items.len());
        &self.items[start..end]
    }

    /// Get the total number of pages
    pub fn total_pages(&self) -> usize {
        if self.items.is_empty() {
            1
        } else {
            self.items.len().div_ceil(self.page_size)
        }
    }

    /// Move to the next page
    pub fn next_page(&mut self) {
        if self.current_page < self.total_pages() - 1 {
            self.current_page += 1;
        }
    }

    /// Move to the previous page
    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
        }
    }

    /// Move to the first page
    pub fn first_page(&mut self) {
        self.current_page = 0;
    }

    /// Move to the last page
    pub fn last_page(&mut self) {
        if self.total_pages() > 0 {
            self.current_page = self.total_pages() - 1;
        }
    }

    /// Get the current page number (1-indexed for display)
    pub fn current_page_number(&self) -> usize {
        self.current_page + 1
    }

    /// Generate a page info string (e.g., "Page 2/5")
    pub fn page_info(&self) -> String {
        format!("Page {}/{}", self.current_page_number(), self.total_pages())
    }

    /// Generate an item count string (e.g., "Showing 11-20 of 47")
    pub fn item_range_info(&self) -> String {
        if self.items.is_empty() {
            "No items".to_string()
        } else {
            let start = self.current_page * self.page_size + 1;
            let end = ((self.current_page + 1) * self.page_size).min(self.items.len());
            let total = self.items.len();
            format!("Showing {}-{} of {}", start, end, total)
        }
    }

    /// Handle keyboard input for pagination
    ///
    /// Returns true if the key was handled
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::PageDown | KeyCode::Char(']') => {
                self.next_page();
                true
            }
            KeyCode::PageUp | KeyCode::Char('[') => {
                self.prev_page();
                true
            }
            KeyCode::Home => {
                self.first_page();
                true
            }
            KeyCode::End => {
                self.last_page();
                true
            }
            _ => false,
        }
    }

    /// Check if we're on the first page
    pub fn is_first_page(&self) -> bool {
        self.current_page == 0
    }

    /// Check if we're on the last page
    pub fn is_last_page(&self) -> bool {
        self.current_page >= self.total_pages().saturating_sub(1)
    }

    /// Get the total number of items
    pub fn total_items(&self) -> usize {
        self.items.len()
    }

    /// Check if the view is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestItem {
        id: i64,
    }

    fn create_test_view(count: usize, page_size: usize) -> PaginatedView<TestItem> {
        let items: Vec<TestItem> = (1..=count).map(|id| TestItem { id: id as i64 }).collect();
        PaginatedView::new(items, page_size)
    }

    #[test]
    fn test_new_view() {
        let view = create_test_view(25, 10);
        assert_eq!(view.total_items(), 25);
        assert_eq!(view.page_size, 10);
        assert_eq!(view.current_page, 0);
    }

    #[test]
    fn test_total_pages() {
        let view = create_test_view(25, 10);
        assert_eq!(view.total_pages(), 3); // 10 + 10 + 5

        let view = create_test_view(30, 10);
        assert_eq!(view.total_pages(), 3); // Exactly 3 pages

        let view = create_test_view(5, 10);
        assert_eq!(view.total_pages(), 1);

        let view: PaginatedView<TestItem> = PaginatedView::new(vec![], 10);
        assert_eq!(view.total_pages(), 1); // Empty view has 1 page
    }

    #[test]
    fn test_current_page_items() {
        let view = create_test_view(25, 10);

        // First page
        let items = view.current_page_items();
        assert_eq!(items.len(), 10);
        assert_eq!(items[0].id, 1);
        assert_eq!(items[9].id, 10);
    }

    #[test]
    fn test_pagination() {
        let mut view = create_test_view(25, 10);

        // Start on page 1
        assert_eq!(view.current_page_number(), 1);
        assert!(view.is_first_page());
        assert!(!view.is_last_page());

        // Next page
        view.next_page();
        assert_eq!(view.current_page_number(), 2);
        assert!(!view.is_first_page());
        assert!(!view.is_last_page());

        let items = view.current_page_items();
        assert_eq!(items.len(), 10);
        assert_eq!(items[0].id, 11);

        // Next page (last page)
        view.next_page();
        assert_eq!(view.current_page_number(), 3);
        assert!(!view.is_first_page());
        assert!(view.is_last_page());

        let items = view.current_page_items();
        assert_eq!(items.len(), 5);
        assert_eq!(items[0].id, 21);

        // Can't go beyond last page
        view.next_page();
        assert_eq!(view.current_page_number(), 3);

        // Previous page
        view.prev_page();
        assert_eq!(view.current_page_number(), 2);
    }

    #[test]
    fn test_first_last_page() {
        let mut view = create_test_view(25, 10);

        view.last_page();
        assert_eq!(view.current_page_number(), 3);
        assert!(view.is_last_page());

        view.first_page();
        assert_eq!(view.current_page_number(), 1);
        assert!(view.is_first_page());
    }

    #[test]
    fn test_page_info() {
        let view = create_test_view(25, 10);
        assert_eq!(view.page_info(), "Page 1/3");
    }

    #[test]
    fn test_item_range_info() {
        let mut view = create_test_view(25, 10);

        assert_eq!(view.item_range_info(), "Showing 1-10 of 25");

        view.next_page();
        assert_eq!(view.item_range_info(), "Showing 11-20 of 25");

        view.next_page();
        assert_eq!(view.item_range_info(), "Showing 21-25 of 25");

        let empty: PaginatedView<TestItem> = PaginatedView::new(vec![], 10);
        assert_eq!(empty.item_range_info(), "No items");
    }

    #[test]
    fn test_handle_key() {
        let mut view = create_test_view(25, 10);

        // PageDown / ]
        assert!(view.handle_key(KeyEvent::from(KeyCode::PageDown)));
        assert_eq!(view.current_page_number(), 2);

        assert!(view.handle_key(KeyEvent::from(KeyCode::Char(']'))));
        assert_eq!(view.current_page_number(), 3);

        // PageUp / [
        assert!(view.handle_key(KeyEvent::from(KeyCode::PageUp)));
        assert_eq!(view.current_page_number(), 2);

        assert!(view.handle_key(KeyEvent::from(KeyCode::Char('['))));
        assert_eq!(view.current_page_number(), 1);

        // Home
        view.current_page = 2;
        assert!(view.handle_key(KeyEvent::from(KeyCode::Home)));
        assert_eq!(view.current_page_number(), 1);

        // End
        assert!(view.handle_key(KeyEvent::from(KeyCode::End)));
        assert_eq!(view.current_page_number(), 3);

        // Unhandled key
        assert!(!view.handle_key(KeyEvent::from(KeyCode::Enter)));
    }

    #[test]
    fn test_page_size_minimum() {
        let view = PaginatedView::new(vec![TestItem { id: 1 }], 0);
        assert_eq!(view.page_size, 1); // Should be at least 1
    }

    #[test]
    fn test_empty_view() {
        let view: PaginatedView<TestItem> = PaginatedView::new(vec![], 10);
        assert!(view.is_empty());
        assert_eq!(view.total_pages(), 1);
        assert_eq!(view.current_page_items().len(), 0);
    }
}
