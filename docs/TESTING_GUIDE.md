# Testing Guide

This guide provides comprehensive information on testing strategies, patterns, and best practices for the reddit-notifier TUI.

## Table of Contents

- [Overview](#overview)
- [Test Organization](#test-organization)
- [Unit Testing](#unit-testing)
- [Integration Testing](#integration-testing)
- [Testing Patterns](#testing-patterns)
- [Mock Database](#mock-database)
- [Testing Widgets](#testing-widgets)
- [Testing Screens](#testing-screens)
- [Testing Validation](#testing-validation)
- [Test Coverage](#test-coverage)
- [Running Tests](#running-tests)
- [Best Practices](#best-practices)

## Overview

The test suite uses Rust's built-in testing framework along with `tokio::test` for async tests. Tests are organized by module and component type, with clear separation between unit and integration tests.

### Test Categories

1. **Unit Tests**: Test individual functions and methods
2. **Integration Tests**: Test screen flows and component interactions
3. **Widget Tests**: Test UI components in isolation
4. **Validation Tests**: Test sync and async validators
5. **Database Tests**: Test database operations with mocks

## Test Organization

```
src/
├── tui/
│   ├── widgets/
│   │   ├── text_input.rs         // Widget code
│   │   │   └── #[cfg(test)] mod tests { ... }
│   │   ├── form_field.rs
│   │   │   └── #[cfg(test)] mod tests { ... }
│   │   └── ...
│   ├── validation/
│   │   ├── async_validator.rs
│   │   │   └── #[cfg(test)] mod tests { ... }
│   │   └── ...
│   └── tests.rs                   // Integration tests
└── ...
```

### Naming Conventions

- Test functions: `test_<functionality>_<scenario>`
- Test modules: `#[cfg(test)] mod tests`
- Async tests: `#[tokio::test]`
- Regular tests: `#[test]`

## Unit Testing

Unit tests focus on testing individual functions and methods in isolation.

### Basic Unit Test

```rust
#[test]
fn test_text_input_new() {
    let input = TextInput::new();
    assert_eq!(input.value(), "");
    assert_eq!(input.cursor_pos, 0);
    assert!(!input.is_focused);
}
```

### Testing with State Changes

```rust
#[test]
fn test_text_input_char_input() {
    let mut input = TextInput::new();

    input.handle_key(KeyEvent::from(KeyCode::Char('a')));
    assert_eq!(input.value(), "a");

    input.handle_key(KeyEvent::from(KeyCode::Char('b')));
    assert_eq!(input.value(), "ab");
}
```

### Testing Validation

```rust
#[test]
fn test_validator() {
    let mut input = TextInput::new()
        .with_validator(|s| s.len() >= 3);

    // Invalid - too short
    input.set_value("ab");
    assert!(!input.is_valid());

    // Valid
    input.set_value("abc");
    assert!(input.is_valid());
}
```

## Integration Testing

Integration tests verify that multiple components work together correctly.

### Screen Transition Tests

```rust
use reddit_notifier::tui::app::{App, Screen};
use reddit_notifier::services::MockDatabaseService;

#[tokio::test]
async fn test_main_menu_to_subscriptions_navigation() {
    let db = MockDatabaseService::new();
    let mut app = App::new(db);

    // Start on main menu
    assert!(matches!(app.context.current_screen, Screen::MainMenu));

    // Select subscriptions option
    app.states.main_menu_state.selected = 0;
    let key = KeyEvent::from(KeyCode::Enter);

    let result = app.handle_input(key).await;
    assert!(result.is_ok());

    // Should be on subscriptions screen
    assert!(matches!(app.context.current_screen, Screen::Subscriptions));
}
```

### Full Workflow Tests

```rust
#[tokio::test]
async fn test_create_subscription_workflow() {
    let db = MockDatabaseService::new();
    let mut app = App::new(db);

    // Navigate to subscriptions
    app.navigate_to(Screen::Subscriptions).await.unwrap();

    // Enter create mode
    app.handle_input(KeyEvent::from(KeyCode::Char('n'))).await.unwrap();
    assert!(matches!(
        app.states.subscriptions_state.mode,
        SubscriptionsMode::Creating(_)
    ));

    // Type subreddit name
    for ch in "rust".chars() {
        app.handle_input(KeyEvent::from(KeyCode::Char(ch))).await.unwrap();
    }

    // Save
    app.handle_input(KeyEvent::from(KeyCode::Enter)).await.unwrap();

    // Should be back in list mode
    assert!(matches!(
        app.states.subscriptions_state.mode,
        SubscriptionsMode::List
    ));

    // Verify subscription was created
    let subs = app.context.db.list_subscriptions().await.unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].subreddit, "rust");
}
```

## Testing Patterns

### Pattern 1: Arrange-Act-Assert

```rust
#[test]
fn test_selection_style_selected() {
    // Arrange
    let is_selected = true;

    // Act
    let (prefix, style) = selection_style(is_selected);

    // Assert
    assert_eq!(prefix, "> ");
    assert_eq!(style.fg, Some(Color::Yellow));
    assert!(style.add_modifier.contains(Modifier::BOLD));
}
```

### Pattern 2: Given-When-Then

```rust
#[tokio::test]
async fn test_delete_subscription() {
    // Given: A subscription exists
    let db = MockDatabaseService::new();
    let sub_id = db.create_subscription("rust").await.unwrap();

    // When: Delete the subscription
    db.delete_subscription(sub_id).await.unwrap();

    // Then: Subscription should not exist
    let subs = db.list_subscriptions().await.unwrap();
    assert!(subs.is_empty());
}
```

### Pattern 3: Table-Driven Tests

```rust
#[test]
fn test_url_validation() {
    let test_cases = vec![
        ("https://example.com", true),
        ("http://example.com", true),
        ("ftp://example.com", false),
        ("not a url", false),
        ("", false),
    ];

    let validator = UrlFormatValidator;

    for (input, expected_valid) in test_cases {
        let result = validator.validate(input).await;
        assert_eq!(result.is_ok(), expected_valid, "Failed for input: {}", input);
    }
}
```

## Mock Database

The `MockDatabaseService` provides in-memory storage for testing without a real database.

### Basic Usage

```rust
use reddit_notifier::services::MockDatabaseService;

#[tokio::test]
async fn test_with_mock_db() {
    let db = MockDatabaseService::new();

    // Create data
    let id = db.create_subscription("rust").await.unwrap();

    // Read data
    let subs = db.list_subscriptions().await.unwrap();
    assert_eq!(subs.len(), 1);

    // Update data
    // ... (mock doesn't support updates yet)

    // Delete data
    db.delete_subscription(id).await.unwrap();
    assert!(db.list_subscriptions().await.unwrap().is_empty());
}
```

### Pre-populating Data

```rust
#[tokio::test]
async fn test_with_existing_data() {
    let db = MockDatabaseService::new();

    // Pre-populate
    db.create_subscription("rust").await.unwrap();
    db.create_subscription("python").await.unwrap();
    db.create_subscription("go").await.unwrap();

    // Test operations on existing data
    let subs = db.list_subscriptions().await.unwrap();
    assert_eq!(subs.len(), 3);
}
```

### Testing Error Conditions

```rust
#[tokio::test]
async fn test_delete_nonexistent() {
    let db = MockDatabaseService::new();

    // Try to delete non-existent subscription
    let result = db.delete_subscription(999).await;

    // Mock returns Ok even for non-existent IDs
    // In real implementation, this might error
    assert!(result.is_ok());
}
```

## Testing Widgets

Widgets should be tested in isolation from screens.

### Testing Widget State

```rust
#[test]
fn test_checkbox_list_toggle() {
    let mut list = CheckboxList::new(vec!["A", "B", "C"]);

    // Initially none checked
    assert_eq!(list.get_checked_items().len(), 0);

    // Toggle first item
    list.selected = 0;
    list.toggle_current();
    assert_eq!(list.get_checked_items().len(), 1);

    // Toggle all
    list.toggle_all();
    assert_eq!(list.get_checked_items().len(), 3);
}
```

### Testing Widget Navigation

```rust
#[test]
fn test_selectable_table_navigation() {
    let items = vec![1, 2, 3, 4, 5];
    let columns = vec![ColumnDef::new("Num", Constraint::Length(5))];
    let mut table = SelectableTable::new(items, columns);

    // Start at first item
    assert_eq!(table.selected, 0);

    // Navigate down
    table.next();
    assert_eq!(table.selected, 1);

    // Navigate up
    table.previous();
    assert_eq!(table.selected, 0);

    // Wrap around at start
    table.previous();
    assert_eq!(table.selected, 4);
}
```

### Testing Widget Keyboard Handling

```rust
#[test]
fn test_paginated_view_keyboard() {
    let items: Vec<i32> = (1..=100).collect();
    let mut pager = PaginatedView::new(items, 10);

    // Test PageDown
    let handled = pager.handle_key(KeyEvent::from(KeyCode::PageDown));
    assert!(handled);
    assert_eq!(pager.current_page_number(), 2);

    // Test PageUp
    let handled = pager.handle_key(KeyEvent::from(KeyCode::PageUp));
    assert!(handled);
    assert_eq!(pager.current_page_number(), 1);

    // Test unhandled key
    let handled = pager.handle_key(KeyEvent::from(KeyCode::Enter));
    assert!(!handled);
}
```

## Testing Screens

Screen tests verify rendering and event handling logic.

### Testing Screen Modes

```rust
#[tokio::test]
async fn test_endpoints_create_mode() {
    let db = MockDatabaseService::new();
    let mut app = App::new(db);
    app.navigate_to(Screen::Endpoints).await.unwrap();

    // Enter create mode
    app.handle_input(KeyEvent::from(KeyCode::Char('n'))).await.unwrap();

    // Verify mode changed
    assert!(matches!(
        app.states.endpoints_state.mode,
        EndpointsMode::Creating(_)
    ));

    // Cancel
    app.handle_input(KeyEvent::from(KeyCode::Esc)).await.unwrap();

    // Back to list mode
    assert!(matches!(
        app.states.endpoints_state.mode,
        EndpointsMode::List
    ));
}
```

### Testing Screen Data Loading

```rust
#[tokio::test]
async fn test_subscriptions_loads_data_on_enter() {
    let db = MockDatabaseService::new();

    // Pre-populate database
    db.create_subscription("rust").await.unwrap();
    db.create_subscription("python").await.unwrap();

    let mut app = App::new(db);

    // Navigate to subscriptions (triggers on_enter)
    app.navigate_to(Screen::Subscriptions).await.unwrap();

    // Verify data was loaded
    assert_eq!(app.states.subscriptions_state.subscriptions.len(), 2);
}
```

## Testing Validation

### Sync Validation Tests

```rust
#[tokio::test]
async fn test_non_empty_validator() {
    let validator = NonEmptyValidator;

    // Valid
    let result = validator.validate("text").await;
    assert!(result.is_ok());

    // Invalid - empty
    let result = validator.validate("").await;
    assert!(result.is_err());

    // Invalid - whitespace
    let result = validator.validate("   ").await;
    assert!(result.is_err());
}
```

### Async Validation Tests

```rust
#[tokio::test]
async fn test_webhook_validator_discord() {
    let validator = WebhookValidator::new(EndpointKind::Discord);

    // Invalid format
    let result = validator.validate("https://example.com").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid Discord webhook"));

    // Valid format but unreachable (will fail with network error)
    let result = validator.validate("https://discord.com/api/webhooks/123/abc").await;
    assert!(result.is_err());
}
```

### Form Field Validation Tests

```rust
#[tokio::test]
async fn test_form_field_async_validation() {
    let validator = Arc::new(NonEmptyValidator);
    let mut field = FormField::new("Test")
        .with_async_validator(validator);

    // Set empty value
    field.input.set_value("");

    // Validate
    let result = field.validate_async().await;
    assert!(result.is_err());
    assert!(matches!(field.validation_state, ValidationState::Invalid(_)));

    // Set valid value
    field.input.set_value("test");

    // Validate again
    let result = field.validate_async().await;
    assert!(result.is_ok());
    assert!(matches!(field.validation_state, ValidationState::Valid(_)));
}
```

## Test Coverage

### Current Coverage

As of Phase 6:
- **Total Tests**: 134
- **Widget Tests**: 65 (48%)
- **Validation Tests**: 15 (11%)
- **Integration Tests**: 36 (27%)
- **Other Tests**: 18 (14%)

### Coverage Goals

Target coverage by module:
- Widgets: 80%+ (line coverage)
- Validation: 80%+ (line coverage)
- Screens: 60%+ (line coverage)
- State Management: 80%+ (line coverage)

### Measuring Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Run coverage
cargo tarpaulin --out Html --output-dir coverage

# View report
open coverage/index.html
```

### Improving Coverage

Focus areas for additional tests:
1. Edge cases in widget navigation
2. Error handling paths
3. Async validation timeout scenarios
4. Complex screen transitions
5. Database error conditions

## Running Tests

### Run All Tests

```bash
cargo test
```

### Run Specific Test

```bash
cargo test test_text_input_validation
```

### Run Tests for a Module

```bash
cargo test tui::widgets::text_input
```

### Run Integration Tests Only

```bash
cargo test --test '*'
```

### Run with Output

```bash
cargo test -- --nocapture
```

### Run in Parallel

```bash
cargo test -- --test-threads=4
```

### Run with Specific Features

```bash
cargo test --features "feature_name"
```

## Best Practices

### 1. Test One Thing

Each test should verify one specific behavior:

```rust
// Good: Single assertion
#[test]
fn test_text_input_max_length() {
    let mut input = TextInput::new().with_max_length(5);
    for c in "abcdef".chars() {
        input.handle_key(KeyEvent::from(KeyCode::Char(c)));
    }
    assert_eq!(input.value(), "abcde"); // Truncated at 5
}

// Avoid: Multiple unrelated assertions
#[test]
fn test_text_input_everything() {
    // Tests max length, validation, cursor, etc. all at once
}
```

### 2. Use Descriptive Names

Test names should describe what they test:

```rust
// Good
#[test]
fn test_checkbox_list_toggle_all_checks_all_items() { ... }

// Avoid
#[test]
fn test_checkbox() { ... }
```

### 3. Test Edge Cases

Don't just test the happy path:

```rust
#[test]
fn test_pagination_with_empty_list() {
    let pager = PaginatedView::new(vec![], 10);
    assert_eq!(pager.total_pages(), 1);
    assert!(pager.current_page_items().is_empty());
}

#[test]
fn test_pagination_with_exact_page_size() {
    let items: Vec<i32> = (1..=30).collect();
    let pager = PaginatedView::new(items, 10);
    assert_eq!(pager.total_pages(), 3);
}
```

### 4. Use Test Helpers

Create helper functions for common setup:

```rust
fn create_test_app() -> App<MockDatabaseService> {
    let db = MockDatabaseService::new();
    App::new(db)
}

fn create_populated_db() -> MockDatabaseService {
    let db = MockDatabaseService::new();
    db.create_subscription("rust").await.unwrap();
    db.create_subscription("python").await.unwrap();
    db
}
```

### 5. Clean Up After Tests

Use RAII or explicit cleanup:

```rust
#[tokio::test]
async fn test_with_cleanup() {
    let db = MockDatabaseService::new();
    let id = db.create_subscription("test").await.unwrap();

    // Test operations

    // Cleanup (though MockDB handles this automatically)
    db.delete_subscription(id).await.unwrap();
}
```

### 6. Test Async Operations

Always await async operations in tests:

```rust
#[tokio::test]
async fn test_async_operation() {
    let db = MockDatabaseService::new();

    // Correct: Await the future
    let result = db.create_subscription("test").await;
    assert!(result.is_ok());

    // Wrong: Don't await (won't compile)
    // let result = db.create_subscription("test");
}
```

### 7. Avoid Test Interdependence

Tests should be independent and run in any order:

```rust
// Bad: Tests depend on execution order
static mut COUNTER: i32 = 0;

#[test]
fn test_first() {
    unsafe { COUNTER = 1; }
}

#[test]
fn test_second() {
    unsafe { assert_eq!(COUNTER, 1); } // Fragile!
}

// Good: Each test is self-contained
#[test]
fn test_counter_increment() {
    let mut counter = 0;
    counter += 1;
    assert_eq!(counter, 1);
}
```

### 8. Document Complex Tests

Add comments for non-obvious test logic:

```rust
#[tokio::test]
async fn test_webhook_validation_retry_logic() {
    // This test verifies that webhook validation retries
    // up to 3 times with exponential backoff before failing.
    // The mock server is configured to fail twice, then succeed.

    let validator = WebhookValidator::new(EndpointKind::Discord);
    // ... test implementation
}
```

### 9. Use Assertions Effectively

Choose the right assertion for the job:

```rust
// For equality
assert_eq!(actual, expected);

// For boolean conditions
assert!(result.is_ok());
assert!(!value.is_empty());

// For patterns
assert!(matches!(state, ValidationState::Valid(_)));

// With custom messages
assert_eq!(count, 5, "Expected 5 items but got {}", count);
```

### 10. Test Error Paths

Don't just test success cases:

```rust
#[tokio::test]
async fn test_create_subscription_validation_error() {
    let db = MockDatabaseService::new();
    let mut app = App::new(db);

    // Navigate to subscriptions
    app.navigate_to(Screen::Subscriptions).await.unwrap();

    // Try to create with empty name (should fail)
    app.handle_input(KeyEvent::from(KeyCode::Char('n'))).await.unwrap();
    app.handle_input(KeyEvent::from(KeyCode::Enter)).await.unwrap();

    // Should show error message
    assert!(app.context.messages.has_error());
}
```

## Troubleshooting

### Tests Fail Intermittently

- Check for race conditions in async code
- Ensure tests are independent
- Look for shared mutable state

### Tests Pass Locally But Fail in CI

- Check for timing issues
- Verify test isolation
- Check for filesystem dependencies

### Slow Tests

- Reduce timeout values for quick failures
- Mock expensive operations
- Run tests in parallel

### Cannot Mock External Services

- Use traits and dependency injection
- Create test doubles
- Use the MockDatabaseService pattern

## Conclusion

Comprehensive testing ensures code quality and prevents regressions. By following these patterns and practices, you can build a robust, maintainable test suite.

For more information:
- [TUI Architecture](TUI_ARCHITECTURE.md) - System design
- [Widget Library](WIDGET_LIBRARY.md) - Widget documentation
