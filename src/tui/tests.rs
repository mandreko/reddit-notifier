//! Integration tests for TUI screen navigation flows
//!
//! These tests verify that screen navigation works correctly and that
//! state is properly managed across screen transitions.

#[cfg(test)]
mod navigation_tests {
    use crate::services::mock_database::MockDatabaseService;
    use crate::tui::app::{App, Screen};
    use crate::tui::screens::{
        endpoints::EndpointsMode, subscriptions::SubscriptionsMode,
    };
    use crate::tui::state::Navigable;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::sync::Arc;

    /// Helper to create a test database service
    fn create_test_db() -> Arc<MockDatabaseService> {
        Arc::new(MockDatabaseService::new())
    }

    /// Helper to create a KeyEvent from a KeyCode
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[tokio::test]
    async fn test_main_menu_to_subscriptions_navigation() {
        let db = create_test_db();
        let mut app = App::new(db).expect("Failed to create app");

        // Start at main menu
        assert_eq!(app.context.current_screen, Screen::MainMenu);

        // Navigate to first menu item (Manage Subscriptions)
        app.states.main_menu_state.set_selected(0);

        // Press Enter to go to subscriptions
        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.context.current_screen, Screen::Subscriptions);

        // Press Esc to go back
        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.context.current_screen, Screen::MainMenu);
    }

    #[tokio::test]
    async fn test_main_menu_to_endpoints_navigation() {
        let db = create_test_db();
        let mut app = App::new(db).expect("Failed to create app");

        // Navigate to Manage Endpoints (second item)
        app.states.main_menu_state.set_selected(1);

        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.context.current_screen, Screen::Endpoints);

        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.context.current_screen, Screen::MainMenu);
    }

    #[tokio::test]
    async fn test_main_menu_to_test_notification_navigation() {
        let db = create_test_db();
        let mut app = App::new(db).expect("Failed to create app");

        // Navigate to Test Notification (third item)
        app.states.main_menu_state.set_selected(2);

        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.context.current_screen, Screen::TestNotification);

        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.context.current_screen, Screen::MainMenu);
    }

    #[tokio::test]
    async fn test_main_menu_to_logs_navigation() {
        let db = create_test_db();
        let mut app = App::new(db).expect("Failed to create app");

        // Navigate to View Logs (fourth item)
        app.states.main_menu_state.set_selected(3);

        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.context.current_screen, Screen::Logs);

        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.context.current_screen, Screen::MainMenu);
    }

    #[tokio::test]
    async fn test_quit_from_main_menu() {
        let db = create_test_db();
        let mut app = App::new(db).expect("Failed to create app");

        assert!(!app.context.should_quit);

        // Press 'q' at main menu
        app.handle_key(key(KeyCode::Char('q')))
            .await
            .expect("Failed to handle key");

        assert!(app.context.should_quit);
    }

    #[tokio::test]
    async fn test_quit_from_main_menu_via_selection() {
        let db = create_test_db();
        let mut app = App::new(db).expect("Failed to create app");

        // Navigate to Quit (fifth item)
        app.states.main_menu_state.set_selected(4);

        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");

        assert!(app.context.should_quit);
    }

    #[tokio::test]
    async fn test_main_menu_navigation_wraps() {
        let db = create_test_db();
        let mut app = App::new(db).expect("Failed to create app");

        // Start at first item
        assert_eq!(app.states.main_menu_state.selected(), 0);

        // Go up should wrap to last item
        app.states.main_menu_state.previous();
        assert_eq!(app.states.main_menu_state.selected(), 4);

        // Go down should wrap to first item
        app.states.main_menu_state.next();
        assert_eq!(app.states.main_menu_state.selected(), 0);
    }

    #[tokio::test]
    async fn test_message_display_clears_on_key() {
        let db = create_test_db();
        let mut app = App::new(db).expect("Failed to create app");

        // Set an error message
        app.context.messages.set_error("Test error".to_string());
        assert!(app.context.messages.has_message());

        // Go to subscriptions screen
        app.goto_screen(Screen::Subscriptions);

        // Any key should clear the message
        app.handle_key(key(KeyCode::Char('x')))
            .await
            .expect("Failed to handle key");

        assert!(!app.context.messages.has_message());
    }

    #[tokio::test]
    async fn test_subscriptions_mode_defaults_to_list() {
        let db = create_test_db();
        let app = App::new(db).expect("Failed to create app");

        assert_eq!(app.states.subscriptions_state.mode, SubscriptionsMode::List);
    }

    #[tokio::test]
    async fn test_subscriptions_create_mode_entry() {
        let db = create_test_db();
        let mut app = App::new(db).expect("Failed to create app");
        app.goto_screen(Screen::Subscriptions);

        // Press 'n' to enter create mode
        app.handle_key(key(KeyCode::Char('n')))
            .await
            .expect("Failed to handle key");

        assert!(matches!(
            app.states.subscriptions_state.mode,
            SubscriptionsMode::Creating(_)
        ));

        // Press Esc to cancel
        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.states.subscriptions_state.mode, SubscriptionsMode::List);
    }

    #[tokio::test]
    async fn test_subscriptions_creating_accepts_valid_chars() {
        let db = create_test_db();
        let mut app = App::new(db).expect("Failed to create app");
        app.goto_screen(Screen::Subscriptions);

        // Enter create mode
        app.handle_key(key(KeyCode::Char('n')))
            .await
            .expect("Failed to handle key");

        // Type some valid characters
        app.handle_key(key(KeyCode::Char('t')))
            .await
            .expect("Failed to handle key");
        app.handle_key(key(KeyCode::Char('e')))
            .await
            .expect("Failed to handle key");
        app.handle_key(key(KeyCode::Char('s')))
            .await
            .expect("Failed to handle key");
        app.handle_key(key(KeyCode::Char('t')))
            .await
            .expect("Failed to handle key");

        // Check the input buffer
        if let SubscriptionsMode::Creating(input) = &app.states.subscriptions_state.mode {
            assert_eq!(input.value(), "test");
        } else {
            panic!("Expected Creating mode");
        }

        // Test backspace
        app.handle_key(key(KeyCode::Backspace))
            .await
            .expect("Failed to handle key");

        if let SubscriptionsMode::Creating(input) = &app.states.subscriptions_state.mode {
            assert_eq!(input.value(), "tes");
        } else {
            panic!("Expected Creating mode");
        }
    }

    #[tokio::test]
    async fn test_endpoints_mode_defaults_to_list() {
        let db = create_test_db();
        let app = App::new(db).expect("Failed to create app");

        assert!(matches!(app.states.endpoints_state.mode, EndpointsMode::List));
    }

    #[tokio::test]
    async fn test_endpoints_create_mode_entry() {
        let db = create_test_db();
        let mut app = App::new(db).expect("Failed to create app");
        app.goto_screen(Screen::Endpoints);

        // Press 'n' to enter create mode
        app.handle_key(key(KeyCode::Char('n')))
            .await
            .expect("Failed to handle key");

        assert!(matches!(
            app.states.endpoints_state.mode,
            EndpointsMode::Creating(_)
        ));

        // Press Esc to cancel
        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");

        assert!(matches!(app.states.endpoints_state.mode, EndpointsMode::List));
    }

    #[tokio::test]
    async fn test_screen_transition_preserves_state() {
        let db = create_test_db();
        let mut app = App::new(db).expect("Failed to create app");

        // Navigate to second item in main menu
        app.states.main_menu_state.next();
        app.states.main_menu_state.next();
        assert_eq!(app.states.main_menu_state.selected(), 2);

        // Go to another screen and back
        app.goto_screen(Screen::Subscriptions);
        app.goto_screen(Screen::MainMenu);

        // State should be preserved
        assert_eq!(app.states.main_menu_state.selected(), 2);
    }

    #[tokio::test]
    async fn test_navigable_trait_on_subscriptions() {
        let db = create_test_db();
        let app = App::new(db).expect("Failed to create app");

        // Test Navigable trait methods
        assert_eq!(app.states.subscriptions_state.selected(), 0);
        assert!(app.states.subscriptions_state.is_empty());
    }

    #[tokio::test]
    async fn test_navigable_trait_on_endpoints() {
        let db = create_test_db();
        let app = App::new(db).expect("Failed to create app");

        assert_eq!(app.states.endpoints_state.selected(), 0);
        assert!(app.states.endpoints_state.is_empty());
    }

    #[tokio::test]
    async fn test_navigable_trait_on_test_notification() {
        let db = create_test_db();
        let app = App::new(db).expect("Failed to create app");

        assert_eq!(app.states.test_notification_state.selected(), 0);
        assert!(app.states.test_notification_state.is_empty());
    }

    #[tokio::test]
    async fn test_message_display_integration() {
        let db = create_test_db();
        let mut app = App::new(db).expect("Failed to create app");

        // Initially no message
        assert!(!app.context.messages.has_message());

        // Set error
        app.context.messages.set_error("Error message".to_string());
        assert!(app.context.messages.has_message());

        // Success should clear error
        app.context.messages.set_success("Success message".to_string());
        assert!(app.context.messages.has_message());

        // Clear should remove message
        app.context.messages.clear();
        assert!(!app.context.messages.has_message());
    }

    #[tokio::test]
    async fn test_multiple_screen_transitions() {
        let db = create_test_db();
        let mut app = App::new(db).expect("Failed to create app");

        // MainMenu -> Subscriptions
        app.states.main_menu_state.set_selected(0);
        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.context.current_screen, Screen::Subscriptions);

        // Subscriptions -> MainMenu
        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.context.current_screen, Screen::MainMenu);

        // MainMenu -> Endpoints
        app.states.main_menu_state.set_selected(1);
        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.context.current_screen, Screen::Endpoints);

        // Endpoints -> MainMenu
        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.context.current_screen, Screen::MainMenu);

        // MainMenu -> TestNotification
        app.states.main_menu_state.set_selected(2);
        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.context.current_screen, Screen::TestNotification);

        // TestNotification -> MainMenu
        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.context.current_screen, Screen::MainMenu);

        // MainMenu -> Logs
        app.states.main_menu_state.set_selected(3);
        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.context.current_screen, Screen::Logs);

        // Logs -> MainMenu
        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.context.current_screen, Screen::MainMenu);
    }

    #[tokio::test]
    async fn test_q_only_quits_from_main_menu() {
        let db = create_test_db();
        let mut app = App::new(db).expect("Failed to create app");

        // 'q' on subscriptions screen shouldn't quit
        app.goto_screen(Screen::Subscriptions);
        app.handle_key(key(KeyCode::Char('q')))
            .await
            .expect("Failed to handle key");
        assert!(!app.context.should_quit);

        // 'q' on endpoints screen shouldn't quit
        app.goto_screen(Screen::Endpoints);
        app.handle_key(key(KeyCode::Char('q')))
            .await
            .expect("Failed to handle key");
        assert!(!app.context.should_quit);

        // 'q' on main menu SHOULD quit
        app.goto_screen(Screen::MainMenu);
        app.handle_key(key(KeyCode::Char('q')))
            .await
            .expect("Failed to handle key");
        assert!(app.context.should_quit);
    }

    #[tokio::test]
    async fn test_app_initial_state() {
        let db = create_test_db();
        let app = App::new(db).expect("Failed to create app");

        // Verify initial state
        assert_eq!(app.context.current_screen, Screen::MainMenu);
        assert!(!app.context.should_quit);
        assert!(!app.context.messages.has_message());
        assert_eq!(app.states.main_menu_state.selected(), 0);
        assert_eq!(app.states.subscriptions_state.selected(), 0);
        assert_eq!(app.states.endpoints_state.selected(), 0);
        assert_eq!(app.states.test_notification_state.selected(), 0);
    }
}
