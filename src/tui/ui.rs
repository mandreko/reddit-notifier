use ratatui::Frame;

use super::app::{App, Screen};
use super::screens;

pub fn render(frame: &mut Frame, app: &App) {
    match app.current_screen {
        Screen::MainMenu => screens::main_menu::render(frame, app),
        Screen::Subscriptions => screens::subscriptions::render(frame, app),
        Screen::Endpoints => screens::endpoints::render(frame, app),
        Screen::TestNotification => screens::test_notification::render(frame, app),
        Screen::Logs => screens::logs::render(frame, app),
    }
}
