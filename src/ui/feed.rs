use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;

use crate::app::App;
use crate::theme;
use crate::types::{FeedEntry, MessageEntry, Priority, PruneAlert};

const CONT_INDENT: &str = "           "; // 11 chars, aligns with after [HH:MM:SS]

/// Split text into chunks that fit within max_width.
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 || text.is_empty() {
        return vec![text.to_string()];
    }
    text.chars()
        .collect::<Vec<_>>()
        .chunks(max_width)
        .map(|c| c.iter().collect())
        .collect()
}

/// Build styled Lines for a message feed entry, wrapping content to width.
pub fn message_lines(msg: &MessageEntry, width: usize) -> Vec<Line<'_>> {
    let ts = msg.created_at.format("[%H:%M:%S]").to_string();
    let content_color = match msg.priority {
        Priority::Urgent => theme::RED,
        _ => theme::FG,
    };

    let prefix = format!("{} {} \u{2192} {}: ", ts, msg.from_session, msg.to_session);
    let prefix_len = prefix.len();

    let first_line_width = width.saturating_sub(prefix_len);
    let cont_width = width.saturating_sub(CONT_INDENT.len());

    if first_line_width == 0 || msg.content.len() <= first_line_width {
        // Fits on one line
        return vec![Line::from(vec![
            Span::styled(format!("{} ", ts), Style::default().fg(theme::MUTED)),
            Span::styled(
                &msg.from_session,
                Style::default()
                    .fg(theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" \u{2192} {}: ", msg.to_session),
                Style::default().fg(theme::MUTED),
            ),
            Span::styled(&msg.content, Style::default().fg(content_color)),
        ])];
    }

    let content_chars: Vec<char> = msg.content.chars().collect();
    let first_chunk: String = content_chars[..first_line_width].iter().collect();
    let rest: String = content_chars[first_line_width..].iter().collect();

    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{} ", ts), Style::default().fg(theme::MUTED)),
        Span::styled(
            &msg.from_session,
            Style::default()
                .fg(theme::BLUE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" \u{2192} {}: ", msg.to_session),
            Style::default().fg(theme::MUTED),
        ),
        Span::styled(first_chunk, Style::default().fg(content_color)),
    ])];

    for chunk in wrap_text(&rest, cont_width) {
        lines.push(Line::from(vec![
            Span::styled(CONT_INDENT.to_string(), Style::default()),
            Span::styled(chunk, Style::default().fg(content_color)),
        ]));
    }

    lines
}

/// Build a styled Line for a message (single-line, for tests).
pub fn message_line(msg: &MessageEntry) -> Line<'_> {
    message_lines(msg, usize::MAX).into_iter().next().unwrap()
}

/// Build styled Lines for a prune alert, wrapping to width.
pub fn prune_alert_lines(alert: &PruneAlert, width: usize) -> Vec<Line<'_>> {
    let text = format!(
        "[!] SESSION PRUNED: {} ({}) \u{2014} last seen {}s ago",
        alert.session_name, alert.role, alert.last_seen_ago_secs
    );
    let style = Style::default()
        .fg(theme::AMBER)
        .add_modifier(Modifier::BOLD);

    wrap_text(&text, width)
        .into_iter()
        .map(|chunk| Line::from(vec![Span::styled(chunk, style)]))
        .collect()
}

/// Build a styled Line for a prune alert (single-line, for tests).
pub fn prune_alert_line(alert: &PruneAlert) -> Line<'_> {
    prune_alert_lines(alert, usize::MAX)
        .into_iter()
        .next()
        .unwrap()
}

/// Build styled Lines for any feed entry, wrapping to width.
pub fn feed_entry_lines(entry: &FeedEntry, width: usize) -> Vec<Line<'_>> {
    match entry {
        FeedEntry::Message(msg) => message_lines(msg, width),
        FeedEntry::PruneAlert(alert) => prune_alert_lines(alert, width),
    }
}

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Feed ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::MUTED))
        .style(Style::default().bg(theme::BG))
        .padding(ratatui::widgets::Padding::horizontal(1));

    let inner = block.inner(area);
    let inner_height = inner.height as usize;
    let inner_width = inner.width as usize;
    let feed_len = app.feed.len();

    // Compute effective scroll offset
    let offset = if app.auto_scroll {
        feed_len.saturating_sub(inner_height)
    } else {
        app.scroll_offset
    };

    let items: Vec<ListItem> = app
        .feed
        .iter()
        .skip(offset)
        .take(inner_height)
        .map(|entry| ListItem::new(feed_entry_lines(entry, inner_width)))
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

pub enum StreamAction {
    Quit,
}

pub fn handle_key_input(app: &mut App, key: KeyEvent) -> Option<StreamAction> {
    let feed_len = app.feed.len();

    // Sync scroll_offset when transitioning out of auto_scroll
    if app.auto_scroll {
        app.scroll_offset = feed_len.saturating_sub(1);
    }

    match key.code {
        KeyCode::Char('q') => return Some(StreamAction::Quit),
        KeyCode::Up | KeyCode::Char('k') => {
            app.scroll_offset = app.scroll_offset.saturating_sub(1);
            app.auto_scroll = false;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.scroll_offset = (app.scroll_offset + 1).min(feed_len.saturating_sub(1));
            if app.scroll_offset >= feed_len.saturating_sub(1) {
                app.auto_scroll = true;
            }
        }
        KeyCode::Char(' ') => {
            app.scroll_offset = feed_len.saturating_sub(1);
            app.auto_scroll = true;
        }
        _ => {}
    }
    None
}

pub fn handle_mouse_input(app: &mut App, mouse: MouseEvent) {
    let feed_len = app.feed.len();

    // Sync scroll_offset when transitioning out of auto_scroll
    if app.auto_scroll {
        app.scroll_offset = feed_len.saturating_sub(1);
    }

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            app.scroll_offset = app.scroll_offset.saturating_sub(3);
            app.auto_scroll = false;
        }
        MouseEventKind::ScrollDown => {
            app.scroll_offset = (app.scroll_offset + 3).min(feed_len.saturating_sub(1));
            if app.scroll_offset >= feed_len.saturating_sub(1) {
                app.auto_scroll = true;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crossterm::event::KeyModifiers;
    use std::path::PathBuf;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn make_msg(id: i64, from: &str, to: &str, content: &str, priority: Priority) -> MessageEntry {
        MessageEntry {
            id,
            from_session: from.to_string(),
            to_session: to.to_string(),
            content: content.to_string(),
            priority,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn feed_message_entry_has_timestamp_sender_content() {
        let msg = make_msg(1, "alice", "session-b", "hello world", Priority::Normal);
        let line = message_line(&msg);
        // Should have 4 spans: timestamp, sender, arrow+recipient, content
        assert_eq!(line.spans.len(), 4);
        assert!(line.spans[0].content.contains(':')); // timestamp [HH:MM:SS]
        assert_eq!(*line.spans[1].content, *"alice"); // sender
        assert!(line.spans[2].content.contains("session-b")); // recipient
        assert_eq!(*line.spans[3].content, *"hello world"); // content
    }

    #[test]
    fn feed_prune_alert_has_prefix_and_amber() {
        let alert = PruneAlert {
            session_name: "tester".to_string(),
            role: "worker".to_string(),
            last_seen_ago_secs: 62,
            timestamp: Utc::now(),
        };
        let line = prune_alert_line(&alert);
        assert_eq!(line.spans.len(), 1);
        assert!(line.spans[0].content.contains("[!]"));
        assert!(line.spans[0].content.contains("tester"));
        assert_eq!(line.spans[0].style.fg, Some(theme::AMBER));
    }

    #[test]
    fn long_message_wraps_to_multiple_lines() {
        let long_content = "a".repeat(200);
        let msg = make_msg(1, "alice", "session-b", &long_content, Priority::Normal);
        let lines = message_lines(&msg, 60);
        assert!(
            lines.len() > 1,
            "long message should wrap to multiple lines"
        );
        // First line has prefix spans + first content chunk
        assert_eq!(lines[0].spans.len(), 4);
        // Continuation lines have indent + content
        assert_eq!(lines[1].spans.len(), 2);
    }

    #[test]
    fn short_message_stays_single_line() {
        let msg = make_msg(1, "alice", "session-b", "short", Priority::Normal);
        let lines = message_lines(&msg, 80);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn feed_urgent_message_has_red_content() {
        let msg = make_msg(1, "alice", "session-b", "URGENT!", Priority::Urgent);
        let line = message_line(&msg);
        // Content span (index 3) should be red
        assert_eq!(line.spans[3].style.fg, Some(theme::RED));
    }

    #[test]
    fn scroll_at_bottom_auto_scroll_true() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        // auto_scroll starts true
        assert!(app.auto_scroll);

        // Add some messages
        for i in 1..=5 {
            app.process_bus_update(vec![], vec![make_msg(i, "a", "b", "msg", Priority::Normal)]);
        }
        // Still auto_scroll after adding messages
        assert!(app.auto_scroll);
    }

    #[test]
    fn scroll_up_disables_auto_scroll() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        for i in 1..=10 {
            app.process_bus_update(vec![], vec![make_msg(i, "a", "b", "msg", Priority::Normal)]);
        }
        app.scroll_offset = 9; // at bottom

        handle_key_input(&mut app, key(KeyCode::Up));
        assert!(!app.auto_scroll);
        assert_eq!(app.scroll_offset, 8);
    }

    #[test]
    fn space_resets_to_bottom() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        for i in 1..=10 {
            app.process_bus_update(vec![], vec![make_msg(i, "a", "b", "msg", Priority::Normal)]);
        }
        app.scroll_offset = 3;
        app.auto_scroll = false;

        handle_key_input(&mut app, key(KeyCode::Char(' ')));
        assert!(app.auto_scroll);
        assert_eq!(app.scroll_offset, 9); // feed.len() - 1
    }

    #[test]
    fn status_bar_session_count_matches() {
        // This tests the data, not the render — status bar reads from app.stats
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        use crate::types::SessionStatus;
        use chrono::Duration;
        let now = Utc::now();
        let sessions = vec![
            crate::types::SessionInfo {
                id: "s1".to_string(),
                name: "a".to_string(),
                role: "w".to_string(),
                project: "p".to_string(),
                last_seen: now - Duration::seconds(5),
                status: SessionStatus::Active,
            },
            crate::types::SessionInfo {
                id: "s2".to_string(),
                name: "b".to_string(),
                role: "w".to_string(),
                project: "p".to_string(),
                last_seen: now - Duration::seconds(10),
                status: SessionStatus::Active,
            },
        ];
        app.process_bus_update(sessions, vec![]);
        assert_eq!(app.stats.active_sessions, 2);
    }
}
