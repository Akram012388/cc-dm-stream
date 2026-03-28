pub mod feed;
pub mod roster;
pub mod status_bar;
pub mod welcome;

use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

use crate::app::{App, AppScreen};

pub fn draw(frame: &mut Frame, app: &App) {
    match &app.state {
        AppScreen::Welcome { .. } => welcome::draw(frame, app),
        AppScreen::Stream => draw_stream(frame, app),
    }
}

fn draw_stream(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Vertical split: main panels + status bar
    let vertical = Layout::vertical([
        Constraint::Min(3),    // main content
        Constraint::Length(1), // status bar
    ])
    .split(area);

    // Horizontal split: roster (fixed) + feed (flex)
    let horizontal = Layout::horizontal([
        Constraint::Length(25), // roster
        Constraint::Min(20),    // feed
    ])
    .split(vertical[0]);

    // Collect sessions sorted by name for stable ordering
    let mut sessions: Vec<_> = app.sessions.values().cloned().collect();
    sessions.sort_by(|a, b| a.name.cmp(&b.name));

    roster::draw(
        frame,
        horizontal[0],
        &sessions,
        app.project_filter.as_deref(),
    );
    feed::draw(frame, horizontal[1], app);
    status_bar::draw(frame, vertical[1], &app.stats);
}
