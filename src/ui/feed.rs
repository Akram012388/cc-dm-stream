use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use std::collections::HashMap;

use crate::app::App;
use crate::theme;
use crate::types::{FeedEntry, MessageEntry, Priority, PruneAlert, ReconnectAlert};

/// Split a styled text into multiple Lines that fit within max_width.
fn wrap_styled(text: &str, style: Style, max_width: usize) -> Vec<Line<'static>> {
    if max_width == 0 || text.is_empty() {
        return vec![Line::from(Span::styled(text.to_string(), style))];
    }
    text.chars()
        .collect::<Vec<_>>()
        .chunks(max_width)
        .map(|chunk| Line::from(Span::styled(chunk.iter().collect::<String>(), style)))
        .collect()
}

/// Build styled Lines for a message, pre-wrapped to width.
/// `session_names` maps session IDs to display names for resolving `to_session`.
pub fn message_lines(
    msg: &MessageEntry,
    width: usize,
    session_names: &HashMap<String, String>,
) -> Vec<Line<'static>> {
    let ts = msg
        .created_at
        .with_timezone(&chrono::Local)
        .format("[%H:%M:%S]")
        .to_string();

    let content_color = match msg.priority {
        Priority::Urgent => theme::RED,
        _ => theme::FG,
    };

    let to_display = session_names
        .get(&msg.to_session)
        .cloned()
        .unwrap_or_else(|| msg.to_session.clone());

    let mut header_spans = vec![
        Span::styled(format!("{} ", ts), Style::default().fg(theme::MUTED)),
        Span::styled(
            msg.from_session.clone(),
            Style::default()
                .fg(theme::BLUE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" \u{2192} {}:", to_display),
            Style::default().fg(theme::MUTED),
        ),
    ];

    if let Some(ref tid) = msg.thread_id {
        header_spans.push(Span::styled(
            format!(" [{}]", tid),
            Style::default().fg(theme::MUTED),
        ));
    }

    let header = Line::from(header_spans);

    // Pre-wrap content to (width - 2) for the "  " indent
    let content_width = width.saturating_sub(2);
    let content_style = Style::default().fg(content_color);

    let mut lines = vec![header];

    let chunks: Vec<String> = if content_width > 0 && !msg.content.is_empty() {
        msg.content
            .chars()
            .collect::<Vec<_>>()
            .chunks(content_width)
            .map(|c| c.iter().collect())
            .collect()
    } else {
        vec![msg.content.clone()]
    };

    for chunk in chunks {
        lines.push(Line::from(vec![
            Span::styled("  ".to_string(), Style::default()),
            Span::styled(chunk, content_style),
        ]));
    }

    lines.push(Line::from("")); // blank separator
    lines
}

/// Build styled Lines for a prune alert, pre-wrapped to width.
pub fn prune_alert_lines(alert: &PruneAlert, width: usize) -> Vec<Line<'static>> {
    let text = format!(
        "[!] SESSION PRUNED: {} ({}) \u{2014} last seen {}s ago",
        alert.session_name, alert.role, alert.last_seen_ago_secs
    );
    let style = Style::default()
        .fg(theme::AMBER)
        .add_modifier(Modifier::BOLD);

    let mut lines = wrap_styled(&text, style, width);
    lines.push(Line::from("")); // blank separator
    lines
}

/// Build styled Lines for a reconnect alert, pre-wrapped to width.
pub fn reconnect_alert_lines(alert: &ReconnectAlert, width: usize) -> Vec<Line<'static>> {
    let text = format!(
        "[+] SESSION RECONNECTED: {} ({})",
        alert.session_name, alert.role
    );
    let style = Style::default()
        .fg(theme::GREEN)
        .add_modifier(Modifier::BOLD);

    let mut lines = wrap_styled(&text, style, width);
    lines.push(Line::from("")); // blank separator
    lines
}

/// Build a session_id → display_name lookup from the app's sessions map.
fn session_name_map(app: &App) -> HashMap<String, String> {
    app.sessions
        .iter()
        .map(|(id, info)| (id.clone(), info.name.clone()))
        .collect()
}

/// Build all visual lines for the entire feed, pre-wrapped to width.
pub fn build_feed_lines(app: &App, width: usize) -> Vec<Line<'static>> {
    let names = session_name_map(app);
    app.feed
        .iter()
        .flat_map(|entry| match entry {
            FeedEntry::Message(msg) => message_lines(msg, width, &names),
            FeedEntry::PruneAlert(alert) => prune_alert_lines(alert, width),
            FeedEntry::ReconnectAlert(alert) => reconnect_alert_lines(alert, width),
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
    let inner_width = inner.width as usize;

    // Pre-wrap all lines to panel width — total_lines = actual visual lines
    let all_lines = build_feed_lines(app, inner_width);
    let total_lines = all_lines.len();

    // Store for scroll input handlers
    app.visible_height = inner_height;
    app.total_feed_lines = total_lines;

    // Compute scroll offset in visual lines — exact match to rendered content
    let scroll_y = if app.auto_scroll {
        total_lines.saturating_sub(inner_height)
    } else {
        app.scroll_offset
    };

    // No .wrap() — content is pre-wrapped, scroll math is exact
    let paragraph = Paragraph::new(all_lines)
        .block(block)
        .scroll((scroll_y as u16, 0));
    frame.render_widget(paragraph, area);
}

pub enum StreamAction {
    Quit,
}

pub fn handle_key_input(app: &mut App, key: KeyEvent) -> Option<StreamAction> {
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
            thread_id: None,
            created_at: Utc::now(),
        }
    }

    fn make_session_names() -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();
        map.insert("session-b".to_string(), "bob".to_string());
        map
    }

    #[test]
    fn message_format_has_header_content_blank() {
        let names = make_session_names();
        let msg = make_msg(1, "alice", "session-b", "hello world", Priority::Normal);
        let lines = message_lines(&msg, 80, &names);
        assert!(lines.len() >= 3); // header, content, blank
        assert!(lines[0].spans[0].content.contains(':'));
        assert_eq!(*lines[0].spans[1].content, *"alice");
        let header_text: String = lines[0]
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();
        assert!(header_text.contains("bob"));
        assert_eq!(*lines[1].spans[0].content, *"  ");
        assert!(lines[1].spans[1].content.contains("hello world"));
        assert!(lines.last().unwrap().spans.is_empty());
    }

    #[test]
    fn to_session_resolved_to_display_name() {
        let names = make_session_names();
        let msg = make_msg(1, "alice", "session-b", "hi", Priority::Normal);
        let lines = message_lines(&msg, 80, &names);
        let header_text: String = lines[0]
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();
        assert!(
            header_text.contains("bob"),
            "header should show resolved name 'bob', got: {}",
            header_text
        );
        assert!(
            !header_text.contains("session-b"),
            "header should NOT show raw ID when resolved"
        );
    }

    #[test]
    fn to_session_fallback_to_raw_id() {
        let names = std::collections::HashMap::new(); // empty — no resolution
        let msg = make_msg(1, "alice", "unknown-id", "hi", Priority::Normal);
        let lines = message_lines(&msg, 80, &names);
        let header_text: String = lines[0]
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();
        assert!(
            header_text.contains("unknown-id"),
            "header should fallback to raw ID, got: {}",
            header_text
        );
    }

    #[test]
    fn thread_id_rendered_on_header() {
        let names = make_session_names();
        let mut msg = make_msg(1, "alice", "session-b", "hi", Priority::Normal);
        msg.thread_id = Some("design-review".to_string());
        let lines = message_lines(&msg, 80, &names);
        let header_text: String = lines[0]
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();
        assert!(
            header_text.contains("[design-review]"),
            "header should contain thread label, got: {}",
            header_text
        );
    }

    #[test]
    fn no_thread_id_no_label() {
        let names = make_session_names();
        let msg = make_msg(1, "alice", "session-b", "hi", Priority::Normal);
        let lines = message_lines(&msg, 80, &names);
        // Header has exactly 3 spans when no thread_id (timestamp, sender, arrow+recipient)
        assert_eq!(
            lines[0].spans.len(),
            3,
            "header should have 3 spans without thread_id, got {}",
            lines[0].spans.len()
        );
    }

    #[test]
    fn long_content_wraps_to_width() {
        let names = make_session_names();
        let long_content = "a".repeat(100);
        let msg = make_msg(1, "alice", "session-b", &long_content, Priority::Normal);
        let lines = message_lines(&msg, 30, &names);
        // content_width = 28, 100/28 = 4 content lines + header + blank = 6+
        assert!(
            lines.len() > 3,
            "long content should produce extra wrapped lines"
        );
    }

    #[test]
    fn reconnect_alert_has_prefix_and_green() {
        let alert = crate::types::ReconnectAlert {
            session_name: "tester".to_string(),
            role: "worker".to_string(),
            timestamp: Utc::now(),
        };
        let lines = reconnect_alert_lines(&alert, 80);
        assert!(lines.len() >= 2);
        assert!(lines[0].spans[0].content.contains("[+]"));
        assert_eq!(lines[0].spans[0].style.fg, Some(theme::GREEN));
    }

    #[test]
    fn prune_alert_has_prefix_and_amber() {
        let alert = PruneAlert {
            session_name: "tester".to_string(),
            role: "worker".to_string(),
            last_seen_ago_secs: 62,
            timestamp: Utc::now(),
        };
        let lines = prune_alert_lines(&alert, 80);
        assert!(lines.len() >= 2);
        assert!(lines[0].spans[0].content.contains("[!]"));
        assert_eq!(lines[0].spans[0].style.fg, Some(theme::AMBER));
    }

    #[test]
    fn urgent_message_has_red_content() {
        let names = make_session_names();
        let msg = make_msg(1, "alice", "session-b", "URGENT!", Priority::Urgent);
        let lines = message_lines(&msg, 80, &names);
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
