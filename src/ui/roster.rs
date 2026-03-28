use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;

use crate::theme;
use crate::types::{SessionInfo, SessionStatus};

/// Truncate a string to max_len, appending "…" if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 1 {
        "\u{2026}".to_string()
    } else {
        let mut result: String = s.chars().take(max_len - 1).collect();
        result.push('\u{2026}');
        result
    }
}

/// Build a styled Line for a single roster entry, fitting within width.
pub fn roster_item_line(session: &SessionInfo, width: usize) -> Line<'_> {
    let (dot, dot_color) = match session.status {
        SessionStatus::Active => ("\u{25cf}", theme::GREEN),
        SessionStatus::ApproachingStale => ("\u{25cf}", theme::AMBER),
        SessionStatus::Stale => ("\u{25cf}", theme::RED),
    };

    let ago = chrono::Utc::now()
        .signed_duration_since(session.last_seen)
        .num_seconds()
        .max(0);

    let time_str = format!(" {}s", ago);
    // Fixed parts: "● " (2) + time_str
    let fixed_len = 2 + time_str.len();

    if width <= fixed_len {
        // Panel too narrow — just show dot and time
        return Line::from(vec![
            Span::styled(format!("{} ", dot), Style::default().fg(dot_color)),
            Span::styled(time_str, Style::default().fg(theme::MUTED)),
        ]);
    }

    let available = width - fixed_len;
    // Format: "name (role)" — need at least name
    let role_part = format!(" ({})", session.role);
    let full_len = session.name.len() + role_part.len();

    let (display_name, display_role) = if full_len <= available {
        // Everything fits
        (session.name.clone(), role_part)
    } else if session.name.len() + 4 <= available {
        // Name fits, truncate role. Reserve space for " (…)"
        let role_budget = available - session.name.len() - 3; // " (" + ")"
        (
            session.name.clone(),
            format!(" ({})", truncate(&session.role, role_budget)),
        )
    } else {
        // Truncate name, drop role entirely
        (truncate(&session.name, available), String::new())
    };

    Line::from(vec![
        Span::styled(format!("{} ", dot), Style::default().fg(dot_color)),
        Span::styled(
            display_name,
            Style::default().fg(theme::FG).add_modifier(Modifier::BOLD),
        ),
        Span::styled(display_role, Style::default().fg(theme::MUTED)),
        Span::styled(time_str, Style::default().fg(theme::MUTED)),
    ])
}

pub fn draw(frame: &mut Frame, area: Rect, sessions: &[SessionInfo], project_filter: Option<&str>) {
    let title = match project_filter {
        Some(name) => format!(" {} ", name),
        None => " Roster ".to_string(),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::MUTED))
        .style(Style::default().bg(theme::BG))
        .padding(ratatui::widgets::Padding::horizontal(1));

    let inner_width = block.inner(area).width as usize;

    let items: Vec<ListItem> = sessions
        .iter()
        .map(|s| ListItem::new(roster_item_line(s, inner_width)))
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
        let line = roster_item_line(&session, 40);
        let dot_span = &line.spans[0];
        assert!(dot_span.content.contains('\u{25cf}'));
        assert_eq!(dot_span.style.fg, Some(theme::GREEN));
    }

    #[test]
    fn roster_stale_session_has_red_dot() {
        let session = make_session("bob", "worker", SessionStatus::Stale, 65);
        let line = roster_item_line(&session, 40);
        let dot_span = &line.spans[0];
        assert!(dot_span.content.contains('\u{25cf}'));
        assert_eq!(dot_span.style.fg, Some(theme::RED));
    }

    #[test]
    fn roster_truncates_long_role() {
        let session = make_session(
            "alice",
            "code-reviewer-specialist",
            SessionStatus::Active,
            5,
        );
        let line = roster_item_line(&session, 25);
        // Role should be truncated, name preserved
        let name_span = &line.spans[1];
        assert_eq!(*name_span.content, *"alice");
        let role_span = &line.spans[2];
        assert!(role_span.content.contains('\u{2026}'));
    }

    #[test]
    fn roster_width_const_is_30() {
        assert_eq!(super::super::ROSTER_WIDTH, 30);
    }

    #[test]
    fn roster_fits_name_and_role_at_width_30() {
        // At width 30, "alice (worker) 5s" should fit without truncation
        let session = make_session("alice", "worker", SessionStatus::Active, 5);
        let line = roster_item_line(&session, 28); // inner width after border+padding
        let name_span = &line.spans[1];
        assert_eq!(*name_span.content, *"alice");
        let role_span = &line.spans[2];
        assert!(
            role_span.content.contains("worker"),
            "role should not be truncated at width 28"
        );
    }

    #[test]
    fn roster_truncates_name_when_very_narrow() {
        let session = make_session("frontend-specialist", "worker", SessionStatus::Active, 5);
        let line = roster_item_line(&session, 15);
        // Name should be truncated, role dropped
        let name_span = &line.spans[1];
        assert!(name_span.content.contains('\u{2026}'));
    }
}
