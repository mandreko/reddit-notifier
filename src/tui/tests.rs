//! Integration tests for TUI screen navigation flows
//!
//! These tests verify that screen navigation works correctly and that
//! state is properly managed across screen transitions.

#[cfg(test)]
mod navigation_tests {
    use crate::tui::app::{App, Screen};
    use crate::tui::screens::{
        endpoints::EndpointsMode, subscriptions::SubscriptionsMode,
    };
    use crate::tui::state::Navigable;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use sqlx::SqlitePool;

    /// Helper to create a test database pool
    async fn create_test_pool() -> SqlitePool {
        SqlitePool::connect(":memory:")
            .await
            .expect("Failed to create test pool")
    }

    /// Helper to create a KeyEvent from a KeyCode
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[tokio::test]
    async fn test_main_menu_to_subscriptions_navigation() {
        let pool = create_test_pool().await;
        let mut app = App::new(pool).expect("Failed to create app");

        // Start at main menu
        assert_eq!(app.current_screen, Screen::MainMenu);

        // Navigate to first menu item (Manage Subscriptions)
        app.main_menu_state.set_selected(0);

        // Press Enter to go to subscriptions
        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.current_screen, Screen::Subscriptions);

        // Press Esc to go back
        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.current_screen, Screen::MainMenu);
    }

    #[tokio::test]
    async fn test_main_menu_to_endpoints_navigation() {
        let pool = create_test_pool().await;
        let mut app = App::new(pool).expect("Failed to create app");

        // Navigate to Manage Endpoints (second item)
        app.main_menu_state.set_selected(1);

        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.current_screen, Screen::Endpoints);

        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.current_screen, Screen::MainMenu);
    }

    #[tokio::test]
    async fn test_main_menu_to_test_notification_navigation() {
        let pool = create_test_pool().await;
        let mut app = App::new(pool).expect("Failed to create app");

        // Navigate to Test Notification (third item)
        app.main_menu_state.set_selected(2);

        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.current_screen, Screen::TestNotification);

        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.current_screen, Screen::MainMenu);
    }

    #[tokio::test]
    async fn test_main_menu_to_logs_navigation() {
        let pool = create_test_pool().await;
        let mut app = App::new(pool).expect("Failed to create app");

        // Navigate to View Logs (fourth item)
        app.main_menu_state.set_selected(3);

        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.current_screen, Screen::Logs);

        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.current_screen, Screen::MainMenu);
    }

    #[tokio::test]
    async fn test_quit_from_main_menu() {
        let pool = create_test_pool().await;
        let mut app = App::new(pool).expect("Failed to create app");

        assert!(!app.should_quit);

        // Press 'q' at main menu
        app.handle_key(key(KeyCode::Char('q')))
            .await
            .expect("Failed to handle key");

        assert!(app.should_quit);
    }

    #[tokio::test]
    async fn test_quit_from_main_menu_via_selection() {
        let pool = create_test_pool().await;
        let mut app = App::new(pool).expect("Failed to create app");

        // Navigate to Quit (fifth item)
        app.main_menu_state.set_selected(4);

        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");

        assert!(app.should_quit);
    }

    #[tokio::test]
    async fn test_main_menu_navigation_wraps() {
        let pool = create_test_pool().await;
        let mut app = App::new(pool).expect("Failed to create app");

        // Start at first item
        assert_eq!(app.main_menu_state.selected(), 0);

        // Go up should wrap to last item
        app.main_menu_state.previous();
        assert_eq!(app.main_menu_state.selected(), 4);

        // Go down should wrap to first item
        app.main_menu_state.next();
        assert_eq!(app.main_menu_state.selected(), 0);
    }

    #[tokio::test]
    async fn test_message_display_clears_on_key() {
        let pool = create_test_pool().await;
        let mut app = App::new(pool).expect("Failed to create app");

        // Set an error message
        app.messages.set_error("Test error".to_string());
        assert!(app.messages.has_message());

        // Go to subscriptions screen
        app.current_screen = Screen::Subscriptions;

        // Any key should clear the message
        app.handle_key(key(KeyCode::Char('x')))
            .await
            .expect("Failed to handle key");

        assert!(!app.messages.has_message());
    }

    #[tokio::test]
    async fn test_subscriptions_mode_defaults_to_list() {
        let pool = create_test_pool().await;
        let app = App::new(pool).expect("Failed to create app");

        assert_eq!(app.subscriptions_state.mode, SubscriptionsMode::List);
    }

    #[tokio::test]
    async fn test_subscriptions_create_mode_entry() {
        let pool = create_test_pool().await;
        let mut app = App::new(pool).expect("Failed to create app");
        app.current_screen = Screen::Subscriptions;

        // Press 'n' to enter create mode
        app.handle_key(key(KeyCode::Char('n')))
            .await
            .expect("Failed to handle key");

        assert!(matches!(
            app.subscriptions_state.mode,
            SubscriptionsMode::Creating(_)
        ));

        // Press Esc to cancel
        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");

        assert_eq!(app.subscriptions_state.mode, SubscriptionsMode::List);
    }

    #[tokio::test]
    async fn test_subscriptions_creating_accepts_valid_chars() {
        let pool = create_test_pool().await;
        let mut app = App::new(pool).expect("Failed to create app");
        app.current_screen = Screen::Subscriptions;

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
        if let SubscriptionsMode::Creating(input) = &app.subscriptions_state.mode {
            assert_eq!(input, "test");
        } else {
            panic!("Expected Creating mode");
        }

        // Test backspace
        app.handle_key(key(KeyCode::Backspace))
            .await
            .expect("Failed to handle key");

        if let SubscriptionsMode::Creating(input) = &app.subscriptions_state.mode {
            assert_eq!(input, "tes");
        } else {
            panic!("Expected Creating mode");
        }
    }

    #[tokio::test]
    async fn test_endpoints_mode_defaults_to_list() {
        let pool = create_test_pool().await;
        let app = App::new(pool).expect("Failed to create app");

        assert!(matches!(app.endpoints_state.mode, EndpointsMode::List));
    }

    #[tokio::test]
    async fn test_endpoints_create_mode_entry() {
        let pool = create_test_pool().await;
        let mut app = App::new(pool).expect("Failed to create app");
        app.current_screen = Screen::Endpoints;

        // Press 'n' to enter create mode
        app.handle_key(key(KeyCode::Char('n')))
            .await
            .expect("Failed to handle key");

        assert!(matches!(
            app.endpoints_state.mode,
            EndpointsMode::Creating(_)
        ));

        // Press Esc to cancel
        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");

        assert!(matches!(app.endpoints_state.mode, EndpointsMode::List));
    }

    #[tokio::test]
    async fn test_screen_transition_preserves_state() {
        let pool = create_test_pool().await;
        let mut app = App::new(pool).expect("Failed to create app");

        // Navigate to second item in main menu
        app.main_menu_state.next();
        app.main_menu_state.next();
        assert_eq!(app.main_menu_state.selected(), 2);

        // Go to another screen and back
        app.current_screen = Screen::Subscriptions;
        app.current_screen = Screen::MainMenu;

        // State should be preserved
        assert_eq!(app.main_menu_state.selected(), 2);
    }

    #[tokio::test]
    async fn test_navigable_trait_on_subscriptions() {
        let pool = create_test_pool().await;
        let app = App::new(pool).expect("Failed to create app");

        // Test Navigable trait methods
        assert_eq!(app.subscriptions_state.selected(), 0);
        assert!(app.subscriptions_state.is_empty());
    }

    #[tokio::test]
    async fn test_navigable_trait_on_endpoints() {
        let pool = create_test_pool().await;
        let app = App::new(pool).expect("Failed to create app");

        assert_eq!(app.endpoints_state.selected(), 0);
        assert!(app.endpoints_state.is_empty());
    }

    #[tokio::test]
    async fn test_navigable_trait_on_test_notification() {
        let pool = create_test_pool().await;
        let app = App::new(pool).expect("Failed to create app");

        assert_eq!(app.test_notification_state.selected(), 0);
        assert!(app.test_notification_state.is_empty());
    }

    #[tokio::test]
    async fn test_message_display_integration() {
        let pool = create_test_pool().await;
        let mut app = App::new(pool).expect("Failed to create app");

        // Initially no message
        assert!(!app.messages.has_message());

        // Set error
        app.messages.set_error("Error message".to_string());
        assert!(app.messages.has_message());

        // Success should clear error
        app.messages.set_success("Success message".to_string());
        assert!(app.messages.has_message());

        // Clear should remove message
        app.messages.clear();
        assert!(!app.messages.has_message());
    }

    #[tokio::test]
    async fn test_multiple_screen_transitions() {
        let pool = create_test_pool().await;
        let mut app = App::new(pool).expect("Failed to create app");

        // MainMenu -> Subscriptions
        app.main_menu_state.set_selected(0);
        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.current_screen, Screen::Subscriptions);

        // Subscriptions -> MainMenu
        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.current_screen, Screen::MainMenu);

        // MainMenu -> Endpoints
        app.main_menu_state.set_selected(1);
        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.current_screen, Screen::Endpoints);

        // Endpoints -> MainMenu
        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.current_screen, Screen::MainMenu);

        // MainMenu -> TestNotification
        app.main_menu_state.set_selected(2);
        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.current_screen, Screen::TestNotification);

        // TestNotification -> MainMenu
        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.current_screen, Screen::MainMenu);

        // MainMenu -> Logs
        app.main_menu_state.set_selected(3);
        app.handle_key(key(KeyCode::Enter))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.current_screen, Screen::Logs);

        // Logs -> MainMenu
        app.handle_key(key(KeyCode::Esc))
            .await
            .expect("Failed to handle key");
        assert_eq!(app.current_screen, Screen::MainMenu);
    }

    #[tokio::test]
    async fn test_q_only_quits_from_main_menu() {
        let pool = create_test_pool().await;
        let mut app = App::new(pool).expect("Failed to create app");

        // 'q' on subscriptions screen shouldn't quit
        app.current_screen = Screen::Subscriptions;
        app.handle_key(key(KeyCode::Char('q')))
            .await
            .expect("Failed to handle key");
        assert!(!app.should_quit);

        // 'q' on endpoints screen shouldn't quit
        app.current_screen = Screen::Endpoints;
        app.handle_key(key(KeyCode::Char('q')))
            .await
            .expect("Failed to handle key");
        assert!(!app.should_quit);

        // 'q' on main menu SHOULD quit
        app.current_screen = Screen::MainMenu;
        app.handle_key(key(KeyCode::Char('q')))
            .await
            .expect("Failed to handle key");
        assert!(app.should_quit);
    }

    #[tokio::test]
    async fn test_app_initial_state() {
        let pool = create_test_pool().await;
        let app = App::new(pool).expect("Failed to create app");

        // Verify initial state
        assert_eq!(app.current_screen, Screen::MainMenu);
        assert!(!app.should_quit);
        assert!(!app.messages.has_message());
        assert_eq!(app.main_menu_state.selected(), 0);
        assert_eq!(app.subscriptions_state.selected(), 0);
        assert_eq!(app.endpoints_state.selected(), 0);
        assert_eq!(app.test_notification_state.selected(), 0);
    }
}
