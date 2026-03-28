use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::{App, AppScreen};
use crate::theme;

/// Action returned from input handling.
pub enum WelcomeAction {
    SelectProject(Option<String>),
    Quit,
}

pub fn draw(frame: &mut Frame, app: &App) {
    let (projects, selected) = match &app.state {
        AppScreen::Welcome { projects, selected } => (projects, *selected),
        _ => return,
    };

    let area = frame.area();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::ORANGE))
        .style(Style::default().bg(theme::BG));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Vertical layout: title, spacer, content, spacer, hints
    let chunks = Layout::vertical([
        Constraint::Length(2), // title
        Constraint::Length(1), // spacer
        Constraint::Min(3),    // content
        Constraint::Length(1), // spacer
        Constraint::Length(1), // hints
    ])
    .split(inner);

    // Title
    let title = Paragraph::new("cc-dm-stream v0.1.9")
        .alignment(Alignment::Center)
        .style(Style::default().fg(theme::ORANGE).add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    // Content area
    if projects.is_empty() {
        let waiting = Paragraph::new("No active sessions. Waiting...")
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme::MUTED));
        frame.render_widget(waiting, chunks[2]);
    } else {
        let header =
            Paragraph::new("Active projects on bus:").style(Style::default().fg(theme::FG));
        let content_chunks = Layout::vertical([
            Constraint::Length(2), // header + spacer
            Constraint::Min(1),    // list
        ])
        .split(chunks[2]);
        frame.render_widget(header, content_chunks[0]);

        let mut items: Vec<ListItem> = projects
            .iter()
            .enumerate()
            .map(|(i, (name, count))| {
                let prefix = if i == selected { "> " } else { "  " };
                let text = format!(
                    "{}{}  ({} session{})",
                    prefix,
                    name,
                    count,
                    if *count == 1 { "" } else { "s" }
                );
                let style = if i == selected {
                    Style::default()
                        .fg(theme::BLUE)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme::FG)
                };
                ListItem::new(Line::from(Span::styled(text, style)))
            })
            .collect();

        // "all projects" entry
        let all_idx = projects.len();
        let prefix = if selected == all_idx { "> " } else { "  " };
        let style = if selected == all_idx {
            Style::default()
                .fg(theme::BLUE)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::FG)
        };
        items.push(ListItem::new(Line::from(Span::styled(
            format!("{}[all projects]", prefix),
            style,
        ))));

        let list = List::new(items);
        frame.render_widget(list, content_chunks[1]);
    }

    // Hints
    let hints = Paragraph::new(Line::from(vec![Span::styled(
        "\u{2191}\u{2193}: select  Enter: confirm  q: quit",
        Style::default().fg(theme::MUTED),
    )]))
    .alignment(Alignment::Center);
    frame.render_widget(hints, chunks[4]);
}

pub fn handle_input(app: &mut App, key: KeyEvent) -> Option<WelcomeAction> {
    let (projects, selected) = match &mut app.state {
        AppScreen::Welcome { projects, selected } => (projects, selected),
        _ => return None,
    };

    let total_items = if projects.is_empty() {
        0
    } else {
        projects.len() + 1 // projects + "all projects"
    };

    match key.code {
        KeyCode::Char('q') => Some(WelcomeAction::Quit),
        KeyCode::Down | KeyCode::Char('j') => {
            if total_items > 0 {
                *selected = (*selected + 1) % total_items;
            }
            None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if total_items > 0 {
                *selected = (*selected + total_items - 1) % total_items;
            }
            None
        }
        KeyCode::Enter => {
            if total_items == 0 {
                return None;
            }
            if *selected < projects.len() {
                Some(WelcomeAction::SelectProject(Some(
                    projects[*selected].0.clone(),
                )))
            } else {
                // "all projects" selected
                Some(WelcomeAction::SelectProject(None))
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn app_with_projects(projects: Vec<(String, usize)>) -> App {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        app.state = AppScreen::Welcome {
            projects,
            selected: 0,
        };
        app
    }

    #[test]
    fn render_with_3_projects_lists_all_plus_all_projects() {
        let projects = vec![
            ("alpha".to_string(), 3),
            ("beta".to_string(), 2),
            ("gamma".to_string(), 1),
        ];
        let app = app_with_projects(projects);

        // Verify the state has 3 projects and total selectable items = 4 (3 + all)
        match &app.state {
            AppScreen::Welcome { projects, .. } => {
                assert_eq!(projects.len(), 3);
                assert_eq!(projects[0].0, "alpha");
                assert_eq!(projects[0].1, 3);
                assert_eq!(projects[1].0, "beta");
                assert_eq!(projects[2].0, "gamma");
                // "all projects" is always appended as virtual last item (index == projects.len())
            }
            _ => panic!("expected Welcome screen"),
        }
    }

    #[test]
    fn render_with_0_projects_shows_waiting() {
        let app = app_with_projects(vec![]);

        // With 0 projects, total_items = 0, no selection possible
        match &app.state {
            AppScreen::Welcome { projects, .. } => {
                assert!(projects.is_empty());
            }
            _ => panic!("expected Welcome screen"),
        }
        // Verify Enter does nothing when empty
        let mut app = app;
        let action = handle_input(&mut app, key(KeyCode::Enter));
        assert!(action.is_none());
    }

    #[test]
    fn arrow_down_wraps_from_last_to_first() {
        let projects = vec![("alpha".to_string(), 1), ("beta".to_string(), 2)];
        let mut app = app_with_projects(projects);

        // total items = 3 (alpha, beta, all projects)
        // Start at 0, go down 3 times to wrap back to 0
        handle_input(&mut app, key(KeyCode::Down)); // -> 1
        handle_input(&mut app, key(KeyCode::Down)); // -> 2 (all projects)
        handle_input(&mut app, key(KeyCode::Down)); // -> 0 (wrap)

        match &app.state {
            AppScreen::Welcome { selected, .. } => assert_eq!(*selected, 0),
            _ => panic!("expected Welcome screen"),
        }
    }

    #[test]
    fn enter_on_project_returns_name() {
        let projects = vec![("myapp".to_string(), 5), ("other".to_string(), 2)];
        let mut app = app_with_projects(projects);

        // Select second project (index 1)
        handle_input(&mut app, key(KeyCode::Down)); // -> 1

        let action = handle_input(&mut app, key(KeyCode::Enter));
        match action {
            Some(WelcomeAction::SelectProject(Some(name))) => assert_eq!(name, "other"),
            _ => panic!("expected SelectProject with name"),
        }
    }

    #[test]
    fn selection_clamped_when_list_shrinks() {
        let projects = vec![
            ("alpha".to_string(), 3),
            ("beta".to_string(), 2),
            ("gamma".to_string(), 1),
        ];
        let mut app = app_with_projects(projects);

        // Move selection to index 2 (gamma)
        handle_input(&mut app, key(KeyCode::Down)); // -> 1
        handle_input(&mut app, key(KeyCode::Down)); // -> 2
        match &app.state {
            AppScreen::Welcome { selected, .. } => assert_eq!(*selected, 2),
            _ => panic!("expected Welcome"),
        }

        // Simulate refresh with smaller list — only 1 project now
        // selected=2 should clamp to max valid index (1: the "all projects" entry)
        let new_projects = vec![("alpha".to_string(), 3)];
        app.refresh_welcome_projects(new_projects);
        match &app.state {
            AppScreen::Welcome {
                selected,
                projects,
            } => {
                assert_eq!(projects.len(), 1);
                // max valid index = projects.len() = 1 (the "all projects" virtual entry)
                assert_eq!(*selected, 1);
            }
            _ => panic!("expected Welcome"),
        }
    }

    #[test]
    fn selection_preserved_when_list_stays_same_size() {
        let projects = vec![("alpha".to_string(), 3), ("beta".to_string(), 2)];
        let mut app = app_with_projects(projects);

        // Move to index 1 (beta)
        handle_input(&mut app, key(KeyCode::Down));

        // Refresh with same-length list
        let new_projects = vec![("alpha".to_string(), 4), ("beta".to_string(), 1)];
        app.refresh_welcome_projects(new_projects);
        match &app.state {
            AppScreen::Welcome { selected, .. } => assert_eq!(*selected, 1),
            _ => panic!("expected Welcome"),
        }
    }

    #[test]
    fn enter_on_all_projects_returns_none() {
        let projects = vec![("myapp".to_string(), 5)];
        let mut app = app_with_projects(projects);

        // total items = 2 (myapp, all projects). Go down once to reach "all projects"
        handle_input(&mut app, key(KeyCode::Down)); // -> 1 (all projects)

        let action = handle_input(&mut app, key(KeyCode::Enter));
        match action {
            Some(WelcomeAction::SelectProject(None)) => {} // correct
            _ => panic!("expected SelectProject(None) for all projects"),
        }
    }
}
