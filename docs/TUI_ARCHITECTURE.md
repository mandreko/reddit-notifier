# TUI Architecture

This document provides an architectural overview of the reddit-notifier Terminal User Interface (TUI), detailing its design patterns, component organization, and implementation guidelines.

## Table of Contents

- [Overview](#overview)
- [Architecture Layers](#architecture-layers)
- [Core Components](#core-components)
- [Design Patterns](#design-patterns)
- [Data Flow](#data-flow)
- [Screen Lifecycle](#screen-lifecycle)
- [Widget System](#widget-system)
- [State Management](#state-management)
- [Validation System](#validation-system)

## Overview

The TUI is built using the [ratatui](https://github.com/ratatui-org/ratatui) framework and follows a layered, component-based architecture. The design emphasizes:

- **Separation of Concerns**: Clear boundaries between UI, business logic, and data access
- **Reusability**: Widget library provides consistent UI components
- **Testability**: Database abstraction and modular design enable comprehensive testing
- **Maintainability**: Trait-based patterns and state machines simplify navigation logic

## Architecture Layers

```
┌─────────────────────────────────────────────────────────────┐
│                         TUI Layer                           │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌─────────┐ │
│  │  Screens  │  │  Widgets  │  │   State   │  │  Events │ │
│  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘  └────┬────┘ │
└────────┼──────────────┼──────────────┼──────────────┼──────┘
         │              │              │              │
         └──────────────┴──────────────┴──────────────┘
                        ▼
         ┌──────────────────────────────────────────┐
         │       Business Logic Layer               │
         │  ┌────────────┐      ┌────────────────┐ │
         │  │ Validation │      │  State Machine │ │
         │  └────────────┘      └────────────────┘ │
         └───────────────────┬──────────────────────┘
                            │
                            ▼
         ┌──────────────────────────────────────────┐
         │         Data Access Layer                │
         │  ┌──────────────────────────────────┐   │
         │  │   DatabaseService Trait           │   │
         │  │  ┌─────────────┐  ┌───────────┐  │   │
         │  │  │   SQLite    │  │    Mock   │  │   │
         │  │  └─────────────┘  └───────────┘  │   │
         │  └──────────────────────────────────┘   │
         └──────────────────────────────────────────┘
```

### Layer Responsibilities

**TUI Layer**:
- Screen rendering and layout
- User input handling
- Widget composition
- Local state management

**Business Logic Layer**:
- Form validation (sync and async)
- Screen navigation and transitions
- Application workflow orchestration

**Data Access Layer**:
- Database operations (CRUD)
- Data persistence
- Query execution

## Core Components

### 1. App Structure

The `App<D: DatabaseService>` struct is the root container for the TUI application:

```rust
pub struct App<D: DatabaseService> {
    pub context: AppContext<D>,
    pub states: ScreenStates,
    pub state_machine: ScreenStateMachine,
}
```

**Key Features**:
- Generic over `DatabaseService` for dependency injection
- Owns all screen states
- Delegates navigation to state machine
- Provides context (database, messages) to screens

### 2. Screen Trait

All screens implement the `Screen` trait which defines the screen lifecycle:

```rust
#[async_trait]
pub trait Screen<D: DatabaseService>: Send {
    /// Render the screen
    fn render(&self, frame: &mut Frame, app: &App<D>);

    /// Handle keyboard input
    async fn handle_key(&mut self, context: &mut AppContext<D>, key: KeyEvent)
        -> Result<ScreenTransition>;

    /// Called when entering the screen
    async fn on_enter(&mut self, context: &mut AppContext<D>) -> Result<()>;

    /// Get the screen's unique identifier
    fn id(&self) -> ScreenId;
}
```

**Benefits**:
- Uniform interface for all screens
- Async support for database operations
- Clean separation of rendering and logic
- Lifecycle hooks for initialization

### 3. State Machine

The `ScreenStateMachine` manages screen transitions and navigation history:

```rust
pub struct ScreenStateMachine {
    current: ScreenId,
    history: Vec<ScreenId>,
}
```

**Capabilities**:
- Push new screens onto navigation stack
- Pop back to previous screens
- Clear history when needed
- Track current screen

### 4. Widget Library

Reusable UI components organized by functionality:

#### Form Widgets
- **TextInput**: Character input with validation and cursor control
- **FormField**: Label + input + validation state display
- **ModalDialog**: Unified dialog system (error, success, confirm, etc.)

#### Data Display Widgets
- **SelectableTable**: Table with navigation and custom formatting
- **CheckboxList**: Multi-selection list with keyboard controls
- **Dropdown**: Filterable dropdown with search
- **PaginatedView**: Pagination wrapper for large datasets

#### Utility Widgets
- **common**: Shared layout helpers and styling functions

## Design Patterns

### 1. Dependency Injection

The `DatabaseService` trait enables dependency injection:

```rust
#[async_trait]
pub trait DatabaseService: Send + Sync {
    async fn list_subscriptions(&self) -> Result<Vec<SubscriptionRow>>;
    async fn create_subscription(&self, subreddit: &str) -> Result<i64>;
    // ... more methods
}
```

**Benefits**:
- Production uses `SqliteDatabaseService`
- Tests use `MockDatabaseService`
- No database coupling in UI code
- Easy to swap implementations

### 2. Builder Pattern

Widgets use builders for fluent construction:

```rust
let input = TextInput::new()
    .with_placeholder("Enter subreddit...")
    .with_max_length(50)
    .with_validator(|s| s.chars().all(|c| c.is_alphanumeric() || c == '_'));
```

**Advantages**:
- Optional configuration
- Method chaining
- Self-documenting code
- Flexible initialization

### 3. State Pattern

Screens use enums to represent different modes:

```rust
pub enum SubscriptionsMode {
    List,
    Creating(TextInput),
    ManagingEndpoints { subscription_id: i64, checkbox_list: CheckboxList<EndpointRow> },
    ConfirmDelete { subscription_id: i64, subreddit_name: String },
}
```

**Benefits**:
- Type-safe state transitions
- Compiler-enforced exhaustive matching
- Clear separation of modes
- Easy to add new modes

### 4. Async/Await

All database operations are async:

```rust
async fn handle_list_mode<D: DatabaseService>(
    state: &mut SubscriptionsState,
    context: &mut AppContext<D>,
    key: KeyEvent,
) -> Result<()> {
    match key.code {
        KeyCode::Char('n') => {
            let subs = context.db.list_subscriptions().await?;
            // ... handle result
        }
        // ... other keys
    }
    Ok(())
}
```

**Advantages**:
- Non-blocking database I/O
- Natural error handling with `?`
- Composable async operations
- Clean syntax

## Data Flow

### Input Event Flow

```
User Input
    │
    ▼
┌──────────────────┐
│  Event Handler   │
│  (crossterm)     │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│   App::run()     │
│                  │
└────────┬─────────┘
         │
         ▼
┌──────────────────────────┐
│  Screen::handle_key()    │
│  • Process input         │
│  • Update state          │
│  • Call database         │
│  • Return transition     │
└────────┬─────────────────┘
         │
         ▼
┌──────────────────────────┐
│  StateMachine            │
│  • Handle transition     │
│  • Update current screen │
│  • Call on_enter()       │
└──────────────────────────┘
```

### Render Flow

```
Frame Available
    │
    ▼
┌──────────────────┐
│  App::ui()       │
│                  │
└────────┬─────────┘
         │
         ▼
┌──────────────────────────┐
│  Screen::render()        │
│  • Layout calculation    │
│  • Widget composition    │
│  • Frame rendering       │
└──────────────────────────┘
```

## Screen Lifecycle

### 1. Initialization

When the app starts or switches to a screen:

1. State machine sets `current` screen
2. `on_enter()` is called
3. Screen loads initial data from database
4. Screen state is initialized

### 2. Main Loop

While the screen is active:

1. Wait for input event
2. Call `handle_key()` with event
3. Screen updates its state
4. Screen performs database operations if needed
5. Screen returns `ScreenTransition`
6. If staying on screen, re-render

### 3. Transition

When leaving a screen:

1. Screen returns `ScreenTransition::GoTo(target)`
2. State machine pushes new screen
3. New screen's `on_enter()` is called
4. New screen renders

### 4. Cleanup

Screens don't have explicit cleanup (Drop trait is sufficient):
- State is preserved while in navigation stack
- Can return to previous screens with their state intact
- State is cleared when screen is removed from stack

## Widget System

### Widget Composition

Widgets are composed to build complex UIs:

```rust
// Endpoints creation screen
fn render(&self, frame: &mut Frame, app: &App<D>) {
    match &app.states.endpoints_state.mode {
        EndpointsMode::Creating(builder) => {
            // Base list
            render_list(frame, app, area);

            // Overlay with config builder
            builder.render(frame, area);
        }
    }
}
```

### Widget Communication

Widgets communicate through:

1. **Return values**: Widgets return action enums
```rust
pub enum ConfigAction {
    Save,
    Cancel,
    TestWebhook,
}
```

2. **Shared state**: Parent passes state to children
```rust
table.selected = app.states.endpoints_state.selected;
table.render(frame, area, |endpoint, _, is_selected| {
    // Custom formatter
});
```

3. **Event delegation**: Widgets handle their own events
```rust
if checkbox_list.handle_key(key) {
    // Key was handled, update parent state
}
```

## State Management

### Screen State

Each screen has its own state struct:

```rust
pub struct EndpointsState {
    pub endpoints: Vec<EndpointRow>,
    pub selected: usize,
    pub mode: EndpointsMode,
}
```

### Application Context

Shared context passed to all screens:

```rust
pub struct AppContext<D: DatabaseService> {
    pub db: D,
    pub messages: MessageDisplay,
    pub current_screen: Screen,
}
```

### State Persistence

- Screen states persist in `ScreenStates`
- Navigating away preserves state
- Returning to screen restores state
- `on_enter()` can refresh data if needed

## Validation System

### Sync Validation

For simple, immediate validation:

```rust
let input = TextInput::new()
    .with_validator(|s| s.len() >= 3);
```

### Async Validation

For validation requiring I/O (webhooks, API calls):

```rust
let validator = WebhookValidator::new(EndpointKind::Discord);
let field = FormField::new("Webhook URL")
    .with_async_validator(Arc::new(validator));

// Later, trigger validation
field.validate_async().await?;
```

### Validation States

```rust
pub enum ValidationState {
    Idle,           // No validation performed
    Validating,     // Async validation in progress
    Valid(Option<String>),   // Validation succeeded
    Invalid(String), // Validation failed with error
}
```

### Visual Feedback

Validation states automatically render with appropriate colors and icons:
- Idle: White, no icon
- Validating: Yellow, ⋯
- Valid: Green, ✓
- Invalid: Red, ✗

## Best Practices

### 1. Screen Implementation

- Keep render logic separate from business logic
- Use mode enums for complex screen states
- Handle all input in `handle_key()`
- Return transitions instead of mutating context directly

### 2. Widget Usage

- Prefer composition over creating new widgets
- Use builders for optional configuration
- Keep widgets focused on single responsibility
- Document custom formatting requirements

### 3. Database Access

- Always use `DatabaseService` trait, never concrete types
- Handle errors gracefully with `context.messages`
- Batch operations when possible
- Keep transactions short

### 4. Testing

- Use `MockDatabaseService` for unit tests
- Test screen transitions independently
- Test widget rendering in isolation
- Test validation logic separately from UI

### 5. Error Handling

- Display user-friendly messages via `MessageDisplay`
- Log detailed errors for debugging
- Don't panic in UI code
- Gracefully degrade on errors

## Performance Considerations

- **Lazy Loading**: Only load data when screen is entered
- **Incremental Updates**: Update only changed parts
- **Pagination**: Use `PaginatedView` for large datasets
- **Async Operations**: Don't block render loop
- **Widget Caching**: Reuse widget instances when possible

## Future Enhancements

Potential improvements for the architecture:

1. **Event Bus**: Centralized event system for cross-screen communication
2. **Command Pattern**: Undo/redo functionality
3. **Animation System**: Smooth transitions and loading indicators
4. **Theme System**: Configurable color schemes
5. **Plugin System**: Extensible screen and widget registration

## Conclusion

The reddit-notifier TUI architecture provides a solid foundation for building interactive terminal applications. Its layered design, trait-based abstractions, and comprehensive widget library make it maintainable, testable, and extensible.

For more detailed information:
- [Widget Library Guide](WIDGET_LIBRARY.md)
- [Testing Guide](TESTING_GUIDE.md)
