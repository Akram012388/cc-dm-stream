pub mod welcome;

use ratatui::Frame;

use crate::app::{App, AppScreen};

pub fn draw(frame: &mut Frame, app: &App) {
    match &app.state {
        AppScreen::Welcome { .. } => welcome::draw(frame, app),
        AppScreen::Stream => {} // Phase 6
    }
}
