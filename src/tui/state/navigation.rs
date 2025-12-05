//! Navigation trait for TUI list selection
//!
//! Provides a common interface for navigating through selectable lists
//! in the TUI, eliminating duplicate next/previous logic across screens.

/// Trait for types that support list navigation
///
/// Provides default implementations for next/previous navigation with wrapping.
/// Types implementing this trait only need to provide the core data accessors.
///
/// # Example
/// ```
/// use reddit_notifier::tui::state::Navigable;
///
/// struct MyState {
///     items: Vec<String>,
///     selected: usize,
/// }
///
/// impl Navigable for MyState {
///     fn len(&self) -> usize {
///         self.items.len()
///     }
///
///     fn selected(&self) -> usize {
///         self.selected
///     }
///
///     fn set_selected(&mut self, index: usize) {
///         self.selected = index;
///     }
/// }
///
/// // Now MyState has next() and previous() methods automatically
/// # let mut state = MyState { items: vec!["a".to_string(), "b".to_string()], selected: 0 };
/// state.next();
/// assert_eq!(state.selected(), 1);
/// state.previous();
/// assert_eq!(state.selected(), 0);
/// ```
pub trait Navigable {
    /// Returns the number of items in the list
    fn len(&self) -> usize;

    /// Returns the currently selected index
    fn selected(&self) -> usize;

    /// Sets the selected index
    fn set_selected(&mut self, index: usize);

    /// Returns true if the list is empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Move selection to the next item, wrapping to start if at end
    fn next(&mut self) {
        if !self.is_empty() {
            let current = self.selected();
            self.set_selected((current + 1) % self.len());
        }
    }

    /// Move selection to the previous item, wrapping to end if at start
    fn previous(&mut self) {
        if !self.is_empty() {
            let current = self.selected();
            if current > 0 {
                self.set_selected(current - 1);
            } else {
                self.set_selected(self.len() - 1);
            }
        }
    }

    /// Move selection to the first item
    fn first(&mut self) {
        if !self.is_empty() {
            self.set_selected(0);
        }
    }

    /// Move selection to the last item
    fn last(&mut self) {
        let len = self.len();
        if len > 0 {
            self.set_selected(len - 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestNav {
        items: Vec<String>,
        selected: usize,
    }

    impl Navigable for TestNav {
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

    #[test]
    fn test_next_wraps_around() {
        let mut nav = TestNav {
            items: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            selected: 2,
        };
        nav.next();
        assert_eq!(nav.selected(), 0);
    }

    #[test]
    fn test_next_increments() {
        let mut nav = TestNav {
            items: vec!["a".to_string(), "b".to_string()],
            selected: 0,
        };
        nav.next();
        assert_eq!(nav.selected(), 1);
    }

    #[test]
    fn test_previous_wraps_around() {
        let mut nav = TestNav {
            items: vec!["a".to_string(), "b".to_string()],
            selected: 0,
        };
        nav.previous();
        assert_eq!(nav.selected(), 1);
    }

    #[test]
    fn test_previous_decrements() {
        let mut nav = TestNav {
            items: vec!["a".to_string(), "b".to_string()],
            selected: 1,
        };
        nav.previous();
        assert_eq!(nav.selected(), 0);
    }

    #[test]
    fn test_first_and_last() {
        let mut nav = TestNav {
            items: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            selected: 1,
        };
        nav.first();
        assert_eq!(nav.selected(), 0);
        nav.last();
        assert_eq!(nav.selected(), 2);
    }

    #[test]
    fn test_empty_list() {
        let mut nav = TestNav {
            items: vec![],
            selected: 0,
        };
        assert!(nav.is_empty());
        nav.next(); // Should not panic
        nav.previous(); // Should not panic
        nav.first(); // Should not panic
        nav.last(); // Should not panic
        assert_eq!(nav.selected(), 0);
    }

    #[test]
    fn test_single_item() {
        let mut nav = TestNav {
            items: vec!["only".to_string()],
            selected: 0,
        };
        nav.next();
        assert_eq!(nav.selected(), 0); // Wraps to same item
        nav.previous();
        assert_eq!(nav.selected(), 0); // Wraps to same item
    }
}
