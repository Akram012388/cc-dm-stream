use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;

use crate::theme;
use crate::types::{SessionInfo, SessionStatus};

/// Build a styled Line for a single roster entry.
pub fn roster_item_line(session: &SessionInfo) -> Line<'_> {
    let (dot, dot_color) = match session.status {
        SessionStatus::Active => ("●", theme::GREEN),
        SessionStatus::ApproachingStale => ("●", theme::AMBER),
        SessionStatus::Stale => ("●", theme::RED),
    };

    let ago = chrono::Utc::now()
        .signed_duration_since(session.last_seen)
        .num_seconds()
        .max(0);

    Line::from(vec![
        Span::styled(format!("{} ", dot), Style::default().fg(dot_color)),
        Span::styled(
            &session.name,
            Style::default().fg(theme::FG).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" ({})", session.role),
            Style::default().fg(theme::MUTED),
        ),
        Span::styled(format!(" {}s", ago), Style::default().fg(theme::MUTED)),
    ])
}

pub fn draw(frame: &mut Frame, area: Rect, sessions: &[SessionInfo]) {
    let block = Block::default()
        .title(" Roster ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::MUTED))
        .style(Style::default().bg(theme::BG))
        .padding(ratatui::widgets::Padding::horizontal(1));

    let items: Vec<ListItem> = sessions
        .iter()
        .map(|s| ListItem::new(roster_item_line(s)))
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn make_session(name: &str, role: &str, status: SessionStatus, ago_secs: i64) -> SessionInfo {
        let now = Utc::now();
        SessionInfo {
            id: format!("session-{}", name),
            name: name.to_string(),
            role: role.to_string(),
            project: "test".to_string(),
            last_seen: now - Duration::seconds(ago_secs),
            status,
        }
    }

    #[test]
    fn roster_active_session_has_green_dot() {
        let session = make_session("alice", "worker", SessionStatus::Active, 5);
        let line = roster_item_line(&session);
        let dot_span = &line.spans[0];
        assert!(dot_span.content.contains('●'));
        assert_eq!(dot_span.style.fg, Some(theme::GREEN));
    }

    #[test]
    fn roster_stale_session_has_red_dot() {
        let session = make_session("bob", "worker", SessionStatus::Stale, 65);
        let line = roster_item_line(&session);
        let dot_span = &line.spans[0];
        assert!(dot_span.content.contains('●'));
        assert_eq!(dot_span.style.fg, Some(theme::RED));
    }
}
