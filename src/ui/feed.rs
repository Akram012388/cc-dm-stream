use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::theme;
use crate::types::{FeedEntry, MessageEntry, Priority, PruneAlert};

/// Build styled Lines for a message (owned, 'static lifetime).
pub fn message_lines(msg: &MessageEntry) -> Vec<Line<'static>> {
    let ts = msg
        .created_at
        .with_timezone(&chrono::Local)
        .format("[%H:%M:%S]")
        .to_string();

    let content_color = match msg.priority {
        Priority::Urgent => theme::RED,
        _ => theme::FG,
    };

    let header = Line::from(vec![
        Span::styled(format!("{} ", ts), Style::default().fg(theme::MUTED)),
        Span::styled(
            msg.from_session.clone(),
            Style::default()
                .fg(theme::BLUE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" \u{2192} {}:", msg.to_session),
            Style::default().fg(theme::MUTED),
        ),
    ]);

    let content = Line::from(vec![
        Span::styled("  ".to_string(), Style::default()),
        Span::styled(msg.content.clone(), Style::default().fg(content_color)),
    ]);

    vec![header, content, Line::from("")]
}

/// Build styled Lines for a prune alert (owned).
pub fn prune_alert_lines(alert: &PruneAlert) -> Vec<Line<'static>> {
    let style = Style::default()
        .fg(theme::AMBER)
        .add_modifier(Modifier::BOLD);

    vec![
        Line::from(vec![Span::styled(
            format!(
                "[!] SESSION PRUNED: {} ({}) \u{2014} last seen {}s ago",
                alert.session_name, alert.role, alert.last_seen_ago_secs
            ),
            style,
        )]),
        Line::from(""),
    ]
}

/// Build all visual lines for the entire feed (owned).
pub fn build_feed_lines(app: &App) -> Vec<Line<'static>> {
    app.feed
        .iter()
        .flat_map(|entry| match entry {
            FeedEntry::Message(msg) => message_lines(msg),
            FeedEntry::PruneAlert(alert) => prune_alert_lines(alert),
        })
        .collect()
}

pub fn draw(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .title(" Feed ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::MUTED))
        .style(Style::default().bg(theme::BG))
        .padding(ratatui::widgets::Padding::horizontal(1));

    let inner = block.inner(area);
    let inner_height = inner.height as usize;

    let all_lines = build_feed_lines(app);
    let total_lines = all_lines.len();

    // Store for scroll input handlers
    app.visible_height = inner_height;
    app.total_feed_lines = total_lines;

    // Compute scroll offset in visual lines
    let scroll_y = if app.auto_scroll {
        total_lines.saturating_sub(inner_height)
    } else {
        app.scroll_offset
    };

    let paragraph = Paragraph::new(all_lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false })
        .scroll((scroll_y as u16, 0));
    frame.render_widget(paragraph, area);
}

pub enum StreamAction {
    Quit,
}

pub fn handle_key_input(app: &mut App, key: KeyEvent) -> Option<StreamAction> {
    // Sync scroll_offset to match rendered position when auto_scrolling
    if app.auto_scroll {
        app.scroll_offset = app.total_feed_lines.saturating_sub(app.visible_height);
    }

    match key.code {
        KeyCode::Char('q') => return Some(StreamAction::Quit),
        KeyCode::Up | KeyCode::Char('k') => {
            app.scroll_offset = app.scroll_offset.saturating_sub(1);
            app.auto_scroll = false;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let max_offset = app.total_feed_lines.saturating_sub(app.visible_height);
            app.scroll_offset = (app.scroll_offset + 1).min(max_offset);
            if app.scroll_offset >= max_offset {
                app.auto_scroll = true;
            }
        }
        KeyCode::Char(' ') => {
            app.scroll_offset = app.total_feed_lines.saturating_sub(app.visible_height);
            app.auto_scroll = true;
        }
        _ => {}
    }
    None
}

pub fn handle_mouse_input(app: &mut App, mouse: MouseEvent) {
    // Sync scroll_offset to match rendered position when auto_scrolling
    if app.auto_scroll {
        app.scroll_offset = app.total_feed_lines.saturating_sub(app.visible_height);
    }

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            app.scroll_offset = app.scroll_offset.saturating_sub(3);
            app.auto_scroll = false;
        }
        MouseEventKind::ScrollDown => {
            let max_offset = app.total_feed_lines.saturating_sub(app.visible_height);
            app.scroll_offset = (app.scroll_offset + 3).min(max_offset);
            if app.scroll_offset >= max_offset {
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
    fn message_format_has_header_content_blank() {
        let msg = make_msg(1, "alice", "session-b", "hello world", Priority::Normal);
        let lines = message_lines(&msg);
        assert_eq!(lines.len(), 3); // header, content, blank
                                    // Header has timestamp and sender
        assert!(lines[0].spans[0].content.contains(':'));
        assert_eq!(*lines[0].spans[1].content, *"alice");
        // Header has session ID
        let header_text: String = lines[0]
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();
        assert!(header_text.contains("session-b"));
        // Content is indented
        assert_eq!(*lines[1].spans[0].content, *"  ");
        assert_eq!(*lines[1].spans[1].content, *"hello world");
        // Blank separator
        assert!(lines[2].spans.is_empty());
    }

    #[test]
    fn prune_alert_has_prefix_and_amber() {
        let alert = PruneAlert {
            session_name: "tester".to_string(),
            role: "worker".to_string(),
            last_seen_ago_secs: 62,
            timestamp: Utc::now(),
        };
        let lines = prune_alert_lines(&alert);
        assert_eq!(lines.len(), 2); // alert + blank
        assert!(lines[0].spans[0].content.contains("[!]"));
        assert_eq!(lines[0].spans[0].style.fg, Some(theme::AMBER));
    }

    #[test]
    fn urgent_message_has_red_content() {
        let msg = make_msg(1, "alice", "session-b", "URGENT!", Priority::Urgent);
        let lines = message_lines(&msg);
        assert_eq!(lines[1].spans[1].style.fg, Some(theme::RED));
    }

    #[test]
    fn scroll_up_moves_by_visual_line() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        app.visible_height = 10;
        app.total_feed_lines = 30;
        for i in 1..=10 {
            app.process_bus_update(vec![], vec![make_msg(i, "a", "b", "msg", Priority::Normal)]);
        }

        // auto_scroll true → sync: offset = 30 - 10 = 20, then Up → 19
        handle_key_input(&mut app, key(KeyCode::Up));
        assert!(!app.auto_scroll);
        assert_eq!(app.scroll_offset, 19);
    }

    #[test]
    fn space_resets_to_bottom() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        app.visible_height = 10;
        app.total_feed_lines = 30;
        app.scroll_offset = 5;
        app.auto_scroll = false;

        handle_key_input(&mut app, key(KeyCode::Char(' ')));
        assert!(app.auto_scroll);
        assert_eq!(app.scroll_offset, 20);
    }

    #[test]
    fn scroll_down_at_bottom_enables_auto_scroll() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        app.visible_height = 10;
        app.total_feed_lines = 30;
        app.scroll_offset = 19;
        app.auto_scroll = false;

        handle_key_input(&mut app, key(KeyCode::Down));
        assert_eq!(app.scroll_offset, 20);
        assert!(app.auto_scroll);
    }

    #[test]
    fn status_bar_session_count_matches() {
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
