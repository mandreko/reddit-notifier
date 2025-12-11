# Widget Library Guide

This guide provides comprehensive documentation for the reddit-notifier TUI widget library, including usage examples, customization options, and best practices.

## Table of Contents

- [Overview](#overview)
- [Form Widgets](#form-widgets)
  - [TextInput](#textinput)
  - [FormField](#formfield)
  - [ModalDialog](#modaldialog)
- [Data Display Widgets](#data-display-widgets)
  - [SelectableTable](#selectabletable)
  - [CheckboxList](#checkboxlist)
  - [Dropdown](#dropdown)
  - [PaginatedView](#paginatedview)
- [Utility Functions](#utility-functions)
- [Validation System](#validation-system)
- [Best Practices](#best-practices)

## Overview

The widget library provides reusable, composable UI components that follow consistent patterns and styling. All widgets are located in `src/tui/widgets/` and are designed to work seamlessly with the ratatui framework.

### Design Principles

1. **Composable**: Widgets can be nested and combined
2. **Configurable**: Builder pattern for flexible initialization
3. **Consistent**: Unified styling and behavior
4. **Testable**: All widgets have comprehensive unit tests
5. **Documented**: Rich documentation with examples

## Form Widgets

### TextInput

Single-line text input with validation, cursor control, and length limits.

#### Features

- Character-by-character input
- Cursor positioning (Home, End, Left, Right arrows)
- Maximum length enforcement
- Custom validation functions
- Placeholder text
- Focus state management

#### Basic Usage

```rust
use reddit_notifier::tui::widgets::TextInput;

// Create a simple text input
let mut input = TextInput::new();

// With configuration
let mut input = TextInput::new()
    .with_placeholder("Enter subreddit name...")
    .with_max_length(50)
    .with_value("rust"); // Pre-fill with value

// Handle keyboard input
use crossterm::event::{KeyCode, KeyEvent};
let key = KeyEvent::from(KeyCode::Char('a'));
input.handle_key(key);

// Get the current value
let text = input.value();
```

#### Custom Validation

```rust
// Alphanumeric only
let input = TextInput::new()
    .with_validator(|s| {
        s.chars().all(|c| c.is_alphanumeric())
    });

// URL validation
let input = TextInput::new()
    .with_validator(|s| {
        s.starts_with("https://")
    });

// Minimum length
let input = TextInput::new()
    .with_validator(|s| s.len() >= 3);
```

#### Rendering

```rust
// Render in a frame
input.render(frame, input_area);
```

#### Methods

- `new()` - Create empty input
- `with_value(s)` - Set initial value
- `with_placeholder(s)` - Set placeholder text
- `with_max_length(n)` - Set maximum length
- `with_validator(f)` - Set validation function
- `set_focused(bool)` - Set focus state
- `handle_key(KeyEvent)` - Process keyboard input
- `value()` - Get current text
- `is_empty()` - Check if empty
- `clear()` - Clear all text
- `render(Frame, Rect)` - Draw the widget

### FormField

Combines label, TextInput, and validation state display into a cohesive form field.

#### Features

- Label display with required indicator
- Integrated TextInput
- Validation state tracking (Idle, Validating, Valid, Invalid)
- Help text support
- Color-coded validation feedback
- Async validation support

#### Basic Usage

```rust
use reddit_notifier::tui::widgets::FormField;

// Simple field
let mut field = FormField::new("Username");

// Required field with help text
let mut field = FormField::new("Email")
    .required()
    .with_help("We'll never share your email");

// With custom input
let input = TextInput::new()
    .with_placeholder("user@example.com")
    .with_max_length(100);

let mut field = FormField::new("Email")
    .with_input(input);

// Handle input
field.handle_key(key_event);

// Validate
field.validate_sync()?;
```

#### Async Validation

```rust
use std::sync::Arc;
use reddit_notifier::tui::validation::{AsyncValidator, WebhookValidator};

// Create async validator
let validator = Arc::new(WebhookValidator::new(EndpointKind::Discord));

let mut field = FormField::new("Webhook URL")
    .required()
    .with_async_validator(validator);

// Trigger async validation
field.validate_async().await?;

// Check validation state
match field.validation_state {
    ValidationState::Valid(_) => println!("Valid!"),
    ValidationState::Invalid(msg) => println!("Error: {}", msg),
    ValidationState::Validating => println!("Checking..."),
    ValidationState::Idle => println!("Not validated yet"),
}
```

#### Validation States

The `ValidationState` enum provides visual feedback:

```rust
pub enum ValidationState {
    Idle,                    // White, no icon
    Validating,              // Yellow, ⋯
    Valid(Option<String>),   // Green, ✓
    Invalid(String),         // Red, ✗
}
```

#### Methods

- `new(label)` - Create new field
- `required()` - Mark as required
- `with_help(text)` - Add help text
- `with_input(TextInput)` - Set input widget
- `with_async_validator(Arc<dyn AsyncValidator>)` - Set async validator
- `set_focused(bool)` - Set focus state
- `handle_key(KeyEvent)` - Process input
- `validate_sync()` - Synchronous validation
- `validate_async()` - Asynchronous validation
- `value()` - Get current value
- `is_valid()` - Check if valid
- `is_empty()` - Check if empty
- `render(Frame, Rect)` - Draw the widget

### ModalDialog

Unified dialog system for errors, successes, warnings, information, and confirmations.

#### Features

- Multiple dialog types (Error, Success, Warning, Info, Confirm)
- Type-specific colors and titles
- Multi-line content support
- Centered modal rendering
- Configurable size
- Dismissal prompt for non-confirm dialogs

#### Dialog Types

```rust
use reddit_notifier::tui::widgets::ModalDialog;

// Error dialog
let dialog = ModalDialog::error("Failed to connect to database");

// Success dialog
let dialog = ModalDialog::success("Subscription created successfully!");

// Warning dialog
let dialog = ModalDialog::warning("This action cannot be undone");

// Info dialog
let dialog = ModalDialog::info("Loading data, please wait...");

// Confirmation dialog
let dialog = ModalDialog::confirm("Delete this endpoint?");
```

#### Custom Content

```rust
// Multi-line content
let mut dialog = ModalDialog::error("An error occurred");
dialog.add_line("Error code: 404");
dialog.add_line("Resource not found");

// Multiple messages at once
let dialog = ModalDialog::new(
    DialogType::Warning,
    vec![
        "Warning: Low disk space",
        "Only 100MB remaining",
        "Please free up space",
    ],
);
```

#### Custom Sizing

```rust
let dialog = ModalDialog::error("Error message")
    .with_width_percent(80)   // 80% of screen width
    .with_height_percent(30); // 30% of screen height
```

#### Rendering

```rust
// Render over existing content
dialog.render(frame, full_screen_area);
```

#### Methods

- `error(msg)` - Create error dialog (red)
- `success(msg)` - Create success dialog (green)
- `warning(msg)` - Create warning dialog (yellow)
- `info(msg)` - Create info dialog (cyan)
- `confirm(prompt)` - Create confirmation dialog (red)
- `new(DialogType, Vec<String>)` - Create with custom type
- `add_line(text)` - Add content line
- `with_content(Vec<String>)` - Set all content
- `with_width_percent(u16)` - Set width (0-100)
- `with_height_percent(u16)` - Set height (0-100)
- `render(Frame, Rect)` - Draw the dialog

## Data Display Widgets

### SelectableTable

Table widget with keyboard navigation, sorting, and custom row formatting.

#### Features

- Up/down arrow navigation
- Page Up/Page Down support
- Custom column definitions
- Optional sorting by column
- Empty state messaging
- Custom row formatting via closures
- Selection tracking
- Optional block titles

#### Basic Usage

```rust
use reddit_notifier::tui::widgets::{SelectableTable, ColumnDef};
use ratatui::layout::Constraint;

// Define columns
let columns = vec![
    ColumnDef::new("", Constraint::Length(2)),           // Selection marker
    ColumnDef::new("ID", Constraint::Length(5)),
    ColumnDef::new("Name", Constraint::Percentage(40)),
    ColumnDef::new("Status", Constraint::Percentage(30)),
];

// Create table with data
let mut table = SelectableTable::new(items, columns)
    .with_empty_message("No items found");

// Sync selection with app state
table.selected = app_state.selected;

// Render with custom formatter
table.render(frame, area, |item, index, is_selected| {
    let (prefix, style) = selection_style(is_selected);

    Row::new(vec![
        prefix.to_string(),
        item.id.to_string(),
        item.name.clone(),
        item.status.clone(),
    ])
    .style(style)
});
```

#### With Block Title

```rust
let table = SelectableTable::new(items, columns)
    .with_block_title("Page 2 of 5");
```

#### Navigation

```rust
// Handle keyboard input
table.handle_key(KeyEvent::from(KeyCode::Down)); // Next item
table.handle_key(KeyEvent::from(KeyCode::Up));   // Previous item
table.handle_key(KeyEvent::from(KeyCode::PageDown)); // Next page
table.handle_key(KeyEvent::from(KeyCode::PageUp));   // Previous page

// Or use Navigable trait methods
table.next();      // Select next
table.previous();  // Select previous
table.first();     // Select first
table.last();      // Select last
```

#### Methods

- `new(items, columns)` - Create table
- `with_empty_message(msg)` - Set empty state message
- `with_block_title(title)` - Set custom block title
- `handle_key(KeyEvent)` - Process navigation keys
- `selected_item()` - Get currently selected item
- `render(Frame, Rect, formatter)` - Draw table with custom formatter

### CheckboxList

Multi-selection list with checkboxes and keyboard controls.

#### Features

- Up/down navigation
- Space to toggle individual items
- 'a' key to toggle all items
- Get checked/unchecked items
- Custom item formatting
- Pre-checked items support

#### Basic Usage

```rust
use reddit_notifier::tui::widgets::CheckboxList;

// Create list
let mut list = CheckboxList::new(vec![
    "Option 1".to_string(),
    "Option 2".to_string(),
    "Option 3".to_string(),
]);

// With pre-checked items
let list = CheckboxList::with_checked(
    items,
    vec![0, 2], // Indices of checked items
);

// Handle input
list.handle_key(KeyEvent::from(KeyCode::Char(' '))); // Toggle current
list.handle_key(KeyEvent::from(KeyCode::Char('a'))); // Toggle all

// Get checked items
let checked = list.get_checked_items();
let checked_indices = list.get_checked_indices();
```

#### Rendering

```rust
// Render with custom formatter
list.render(frame, area, |item| {
    format!("{} - Description", item.name)
});
```

#### Methods

- `new(items)` - Create list
- `with_checked(items, indices)` - Create with pre-checked items
- `toggle_current()` - Toggle currently selected item
- `toggle_all()` - Toggle all items
- `select_all()` - Check all items
- `deselect_all()` - Uncheck all items
- `get_checked_items()` - Get references to checked items (sorted by index)
- `get_checked_indices()` - Get indices of checked items
- `is_checked(index)` - Check if item is checked
- `handle_key(KeyEvent)` - Process input
- `render(Frame, Rect, formatter)` - Draw list

### Dropdown

Filterable dropdown with search and keyboard navigation.

#### Features

- Type-ahead filtering
- Keyboard navigation
- Optional "None" selection
- Custom option formatting
- Opens/closes on demand

#### Basic Usage

```rust
use reddit_notifier::tui::widgets::Dropdown;

// Create dropdown
let mut dropdown = Dropdown::new(vec![
    "Rust".to_string(),
    "Python".to_string(),
    "JavaScript".to_string(),
    "Go".to_string(),
]);

// With "None" option
let dropdown = Dropdown::new(options)
    .with_none_option();

// Handle input
dropdown.handle_key(KeyEvent::from(KeyCode::Char('r'))); // Filter to "Rust"
dropdown.handle_key(KeyEvent::from(KeyCode::Down));      // Navigate
dropdown.handle_key(KeyEvent::from(KeyCode::Enter));     // Select

// Get selected option
if let Some(selected) = dropdown.selected_option() {
    println!("Selected: {}", selected);
}
```

#### Filtering

The dropdown automatically filters options as the user types:

```rust
// User types "py"
// Dropdown shows only "Python"

// User types "ja"
// Dropdown shows only "JavaScript"
```

#### Methods

- `new(options)` - Create dropdown
- `with_none_option()` - Add "None" selection
- `open()` - Open dropdown
- `close()` - Close dropdown
- `toggle()` - Toggle open/close
- `is_open()` - Check if open
- `selected_option()` - Get selected value
- `selected_index()` - Get selected index
- `filtered_options()` - Get filtered options
- `handle_key(KeyEvent)` - Process input
- `render(Frame, Rect)` - Draw dropdown

### PaginatedView

Pagination wrapper for managing large lists of items.

#### Features

- Page-based navigation
- Configurable page size
- Current page tracking
- Page info display
- Item range display
- First/last page jumping

#### Basic Usage

```rust
use reddit_notifier::tui::widgets::PaginatedView;

// Create paginated view
let mut pager = PaginatedView::new(items, 20); // 20 items per page

// Get current page items
let page_items = pager.current_page_items();

// Navigation
pager.next_page();     // Go to next page
pager.prev_page();     // Go to previous page
pager.first_page();    // Go to first page
pager.last_page();     // Go to last page

// Display info
println!("{}", pager.page_info());        // "Page 2/5"
println!("{}", pager.item_range_info());  // "Showing 21-40 of 97"
```

#### Keyboard Handling

```rust
// Handle navigation keys
pager.handle_key(KeyEvent::from(KeyCode::PageDown)); // Next page
pager.handle_key(KeyEvent::from(KeyCode::PageUp));   // Previous page
pager.handle_key(KeyEvent::from(KeyCode::Char(']'))); // Next page
pager.handle_key(KeyEvent::from(KeyCode::Char('['))); // Previous page
pager.handle_key(KeyEvent::from(KeyCode::Home));      // First page
pager.handle_key(KeyEvent::from(KeyCode::End));       // Last page
```

#### Usage with SelectableTable

```rust
// Combine pagination with table
let pager = PaginatedView::new(all_items, 50);
let page_items = pager.current_page_items();

let table = SelectableTable::new(page_items.to_vec(), columns)
    .with_block_title(pager.page_info());

table.render(frame, area, |item, _, is_selected| {
    // Format rows
});

// Show pagination info
let info = Paragraph::new(pager.item_range_info());
frame.render_widget(info, info_area);
```

#### Methods

- `new(items, page_size)` - Create paginated view
- `current_page_items()` - Get items for current page
- `total_pages()` - Get total number of pages
- `next_page()` - Move to next page
- `prev_page()` - Move to previous page
- `first_page()` - Move to first page
- `last_page()` - Move to last page
- `current_page_number()` - Get current page (1-indexed)
- `page_info()` - Get "Page X/Y" string
- `item_range_info()` - Get "Showing X-Y of Z" string
- `is_first_page()` - Check if on first page
- `is_last_page()` - Check if on last page
- `total_items()` - Get total item count
- `is_empty()` - Check if no items
- `handle_key(KeyEvent)` - Process navigation keys

## Utility Functions

### Common Helpers

Located in `src/tui/widgets/common.rs`:

#### render_screen_layout

Creates standard 3-section layout (title, content, help):

```rust
let chunks = render_screen_layout(area);
// chunks[0] = title area (height: 3)
// chunks[1] = content area (flexible)
// chunks[2] = help area (height: 3)
```

#### render_title

Renders styled title bar:

```rust
render_title(frame, title_area, "My Screen");
```

#### render_help

Renders help text with keyboard shortcuts:

```rust
render_help(frame, help_area, &[
    ("↑/↓", "Navigate"),
    ("Enter", "Select"),
    ("Esc", "Back"),
]);
```

#### centered_rect

Calculates centered rectangle for popups:

```rust
let popup_area = centered_rect(60, 40, full_screen);
// 60% width, 40% height, centered
```

#### selection_style

Gets selection marker and style for list items:

```rust
let (prefix, style) = selection_style(is_selected);
// prefix = "> " if selected, "  " otherwise
// style = yellow bold if selected, default otherwise
```

## Validation System

### Sync Validators

Built-in validators for common patterns:

#### NonEmptyValidator

```rust
use reddit_notifier::tui::validation::NonEmptyValidator;

let validator = NonEmptyValidator;
let result = validator.validate("some text").await;
// Ok(None) if valid, Err(msg) if empty
```

#### UrlFormatValidator

```rust
use reddit_notifier::tui::validation::UrlFormatValidator;

let validator = UrlFormatValidator;
let result = validator.validate("https://example.com").await;
// Ok(None) if valid URL, Err(msg) if invalid
```

### Async Validators

#### WebhookValidator

Tests Discord and Pushover webhooks:

```rust
use reddit_notifier::tui::validation::WebhookValidator;
use reddit_notifier::models::database::EndpointKind;

// Discord webhook
let validator = WebhookValidator::new(EndpointKind::Discord);
let result = validator.validate("https://discord.com/api/webhooks/123/abc").await;
// Sends test message, returns Ok if successful

// Pushover credentials
let validator = WebhookValidator::new(EndpointKind::Pushover);
let json = r#"{"token": "abc123", "user": "def456"}"#;
let result = validator.validate(json).await;
// Tests API credentials, returns Ok if valid
```

### Custom Validators

Implement `AsyncValidator` trait:

```rust
use async_trait::async_trait;
use reddit_notifier::tui::validation::{AsyncValidator, ValidationResult};

struct MyValidator;

#[async_trait]
impl AsyncValidator for MyValidator {
    async fn validate(&self, value: &str) -> ValidationResult {
        // Perform async validation
        if value.len() < 3 {
            Err("Too short".to_string())
        } else {
            Ok(Some("Looks good!".to_string()))
        }
    }
}
```

## Best Practices

### 1. Widget Composition

Prefer composing existing widgets over creating new ones:

```rust
// Good: Compose FormField with TextInput
let input = TextInput::new().with_validator(my_validator);
let field = FormField::new("Username").with_input(input);

// Avoid: Creating custom widget for simple combination
```

### 2. Builder Pattern

Use builders for cleaner initialization:

```rust
// Good: Fluent builder
let dialog = ModalDialog::error("Error occurred")
    .with_width_percent(70)
    .with_height_percent(30);

// Avoid: Multiple mutation statements
let mut dialog = ModalDialog::error("Error occurred");
dialog.width_percent = 70;
dialog.height_percent = 30;
```

### 3. State Management

Keep widget state in parent components:

```rust
// Good: Parent tracks selection
let mut table = SelectableTable::new(items, columns);
table.selected = screen_state.selected_index;

// Avoid: Trying to extract state from widget
let index = table.get_internal_state(); // Don't do this
```

### 4. Custom Formatting

Use closures for flexible formatting:

```rust
table.render(frame, area, |item, index, is_selected| {
    // Custom logic based on item properties
    let color = if item.is_active {
        Color::Green
    } else {
        Color::Gray
    };

    Row::new(vec![...]).style(Style::default().fg(color))
});
```

### 5. Error Handling

Always handle validation errors gracefully:

```rust
match field.validate_async().await {
    Ok(Some(msg)) => {
        // Show success message
        context.messages.set_success(msg);
    }
    Ok(None) => {
        // Valid but no message
    }
    Err(msg) => {
        // Show error
        context.messages.set_error(msg);
    }
}
```

### 6. Async Operations

Keep async operations in event handlers, not render functions:

```rust
// Good: Async in handle_key
async fn handle_key(&mut self, context: &mut AppContext<D>, key: KeyEvent) -> Result<ScreenTransition> {
    if key.code == KeyCode::Char('t') {
        // Async validation
        self.builder.validate_webhook().await.ok();
    }
    Ok(ScreenTransition::Stay)
}

// Avoid: Async in render (won't compile anyway)
fn render(&self, frame: &mut Frame, app: &App<D>) {
    // Can't await here
}
```

### 7. Testing

Test widgets in isolation:

```rust
#[test]
fn test_text_input_validation() {
    let mut input = TextInput::new()
        .with_validator(|s| s.len() >= 3);

    input.set_value("ab");
    assert!(!input.is_valid());

    input.set_value("abc");
    assert!(input.is_valid());
}
```

### 8. Documentation

Document custom formatters and validators:

```rust
/// Custom formatter for user rows
///
/// Displays username in yellow if online, gray if offline
fn format_user_row(user: &User, _: usize, is_selected: bool) -> Row<'static> {
    let color = if user.online {
        Color::Yellow
    } else {
        Color::Gray
    };
    // ...
}
```

## Performance Tips

1. **Avoid Cloning Large Data**: Use references when possible
```rust
// Good
table.render(frame, area, |item, _, is_selected| {
    // item is a reference, no cloning
});

// Avoid cloning in tight loops
```

2. **Reuse Widget Instances**: Don't create widgets every frame
```rust
// Good: Create once, reuse
struct MyState {
    table: SelectableTable<Item>,
}

// Avoid: Creating every render
fn render() {
    let table = SelectableTable::new(...); // Don't do this every frame
}
```

3. **Batch Updates**: Update state before rendering
```rust
// Good: Update then render
state.update_all_fields();
state.render(frame, area);

// Avoid: Multiple renders
state.render_field1(frame);  // Don't split up
state.render_field2(frame);
```

## Troubleshooting

### Widget Not Displaying

- Check area size is non-zero
- Verify widget is rendered after parent
- Ensure no overlapping widgets

### Input Not Working

- Verify widget has focus
- Check event handler is calling widget's `handle_key()`
- Ensure event matches expected key codes

### Validation Not Running

- Confirm validator is set
- Check async function is awaited
- Verify validation state is rendered

### Table Selection Issues

- Sync selection with app state before rendering
- Handle out-of-bounds indices
- Check Navigable trait implementation

## Examples

See working examples in:
- `src/tui/screens/` - Real-world screen implementations
- `src/tui/widgets/*/tests.rs` - Unit test examples
- `src/tui/tests.rs` - Integration test examples

## Conclusion

The widget library provides a comprehensive set of tools for building interactive terminal UIs. By following these guidelines and examples, you can create consistent, maintainable, and user-friendly interfaces.

For more information:
- [TUI Architecture](TUI_ARCHITECTURE.md) - Overall system design
- [Testing Guide](TESTING_GUIDE.md) - Testing strategies
