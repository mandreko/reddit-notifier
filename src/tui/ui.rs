use ratatui::Frame;

use crate::services::DatabaseService;
use super::app::{App, Screen};
use super::screens;

pub fn render<D: DatabaseService>(frame: &mut Frame, app: &App<D>) {
    match app.context.current_screen {
        Screen::MainMenu => screens::main_menu::render(frame, app),
        Screen::Subscriptions => screens::subscriptions::render(frame, app),
        Screen::Endpoints => screens::endpoints::render(frame, app),
        Screen::TestNotification => screens::test_notification::render(frame, app),
        Screen::Logs => screens::logs::render(frame, app),
    }
}
