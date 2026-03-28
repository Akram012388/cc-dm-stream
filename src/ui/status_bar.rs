use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::theme;
use crate::types::BusStats;

pub fn draw(frame: &mut Frame, area: Rect, stats: &BusStats) {
    let chunks = Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Percentage(34),
        Constraint::Percentage(33),
    ])
    .split(area);

    let bg = Style::default().bg(theme::STATUS_BAR_BG);

    // Left: connection status + bus path
    let (indicator, indicator_color) = if stats.connected {
        ("\u{25cf} connected", theme::GREEN)
    } else {
        ("\u{25cb} waiting", theme::RED)
    };
    let left = Paragraph::new(Line::from(vec![
        Span::styled(indicator, Style::default().fg(indicator_color)),
        Span::styled(
            format!(" | {}", stats.bus_path.display()),
            Style::default().fg(theme::MUTED),
        ),
    ]))
    .style(bg);
    frame.render_widget(left, chunks[0]);

    // Center: session + message counts
    let center = Paragraph::new(Line::from(vec![Span::styled(
        format!(
            "sessions: {} | msgs: {}",
            stats.active_sessions, stats.messages_observed
        ),
        Style::default().fg(theme::FG),
    )]))
    .alignment(Alignment::Center)
    .style(bg);
    frame.render_widget(center, chunks[1]);

    // Right: keybinding hints
    let right = Paragraph::new(Line::from(vec![Span::styled(
        "q: quit  Space: latest  \u{2191}\u{2193}: scroll",
        Style::default().fg(theme::MUTED),
    )]))
    .alignment(Alignment::Right)
    .style(bg);
    frame.render_widget(right, chunks[2]);
}
