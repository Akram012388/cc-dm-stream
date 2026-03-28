mod app;
mod bus;
mod theme;
mod types;
mod ui;
mod watcher;

use std::io::{self, stdout};
use std::path::PathBuf;

use clap::Parser;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

use app::{App, AppScreen};
use bus::{list_projects, open_bus, read_pending_messages, read_sessions};
use types::{AppEvent, SessionStatus};
use ui::feed::{self, StreamAction};
use ui::welcome::{self, WelcomeAction};
use watcher::start_watcher;

#[derive(Parser, Debug)]
#[command(
    name = "cc-dm-stream",
    version,
    about = "Live streaming TUI for the cc-dm coordination bus"
)]
pub struct Cli {
    /// Filter to a specific project (skips welcome screen)
    #[arg(long)]
    pub project: Option<String>,
}

fn bus_dir() -> PathBuf {
    dirs::home_dir()
        .expect("could not determine home directory")
        .join(".cc-dm")
}

fn bus_path() -> PathBuf {
    bus_dir().join("bus.db")
}

/// Read bus and refresh the welcome screen project list.
fn refresh_welcome(app: &mut App, path: &std::path::Path) {
    if let Ok(conn) = open_bus(path) {
        app.stats.connected = true;
        if let Ok(projects) = list_projects(&conn) {
            app.state = AppScreen::Welcome {
                projects,
                selected: 0,
            };
        }
    }
}

/// Read bus, apply project filter, and update app state.
fn refresh_stream(app: &mut App, path: &std::path::Path) {
    match open_bus(path) {
        Ok(conn) => {
            app.stats.connected = true;
            let sessions = read_sessions(&conn).unwrap_or_default();
            let messages = read_pending_messages(&conn).unwrap_or_default();
            let filtered_msgs: Vec<_> = app
                .filtered_messages(&sessions, &messages)
                .into_iter()
                .cloned()
                .collect();
            let owned_sessions: Vec<_> = app
                .apply_project_filter(&sessions)
                .into_iter()
                .cloned()
                .collect();
            app.process_bus_update(owned_sessions, filtered_msgs);
        }
        Err(_) => {
            app.stats.connected = false;
        }
    }
}

/// Recompute SessionStatus for all in-memory sessions (no DB read).
fn recompute_statuses(app: &mut App) {
    let now = chrono::Utc::now();
    for session in app.sessions.values_mut() {
        session.status = SessionStatus::from_last_seen(session.last_seen, now);
    }
}

fn is_ctrl_c(key: &KeyEvent) -> bool {
    key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let bp = bus_path();
    let bd = bus_dir();

    let mut app = App::new(bp.clone());

    // If --project provided, set filter and go straight to stream
    if let Some(ref project) = cli.project {
        app.project_filter = Some(project.clone());
        app.state = AppScreen::Stream;
    }

    // Initial bus check
    if bp.exists() {
        match &app.state {
            AppScreen::Welcome { .. } => refresh_welcome(&mut app, &bp),
            AppScreen::Stream => refresh_stream(&mut app, &bp),
        }
    }

    let (tx, mut rx) = mpsc::channel::<AppEvent>(32);

    // Terminal setup — ratatui::init() handles raw mode + alternate screen
    let mut terminal = ratatui::init();
    execute!(stdout(), crossterm::event::EnableMouseCapture)?;

    let result = run_loop(&mut terminal, &mut app, &mut rx, &bp, &bd, tx).await;

    // Terminal teardown (always)
    execute!(stdout(), crossterm::event::DisableMouseCapture)?;
    ratatui::restore();

    result
}

async fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    rx: &mut mpsc::Receiver<AppEvent>,
    bp: &std::path::Path,
    bd: &std::path::Path,
    tx: mpsc::Sender<AppEvent>,
) -> io::Result<()> {
    let mut ticker = interval(Duration::from_secs(1));
    let mut event_stream = EventStream::new();

    // Start watcher on bus directory (if it exists)
    let mut _watcher_handle = if bd.exists() {
        start_watcher(bd.to_path_buf(), tx.clone()).ok()
    } else {
        None
    };

    loop {
        // Render
        terminal.draw(|frame| ui::draw(frame, app))?;

        tokio::select! {
            // Bus change event from watcher
            Some(_event) = rx.recv() => {
                match &app.state {
                    AppScreen::Welcome { .. } => refresh_welcome(app, bp),
                    AppScreen::Stream => refresh_stream(app, bp),
                }

                // If bus_dir just appeared and we have no watcher, start one
                if _watcher_handle.is_none() && bd.exists() {
                    _watcher_handle = start_watcher(bd.to_path_buf(), tx.clone()).ok();
                }
            }

            // 1-second tick for roster "last: Xs" display
            _ = ticker.tick() => {
                recompute_statuses(app);

                // If bus_dir appeared and we have no watcher yet, start one
                if bd.exists() {
                    if _watcher_handle.is_none() {
                        _watcher_handle = start_watcher(bd.to_path_buf(), tx.clone()).ok();
                    }
                    // Always refresh on tick as fallback if watcher dies
                    if bp.exists() {
                        match &app.state {
                            AppScreen::Welcome { .. } => {
                                if !app.stats.connected {
                                    refresh_welcome(app, bp);
                                }
                            }
                            AppScreen::Stream => refresh_stream(app, bp),
                        }
                    }
                }
            }

            // Crossterm keyboard/mouse events
            Some(Ok(event)) = event_stream.next() => {
                match event {
                    Event::Key(key) => {
                        // Global Ctrl+C handler
                        if is_ctrl_c(&key) {
                            return Ok(());
                        }

                        match &app.state {
                            AppScreen::Welcome { .. } => {
                                if let Some(action) = welcome::handle_input(app, key) {
                                    match action {
                                        WelcomeAction::Quit => return Ok(()),
                                        WelcomeAction::SelectProject(project) => {
                                            app.project_filter = project;
                                            app.state = AppScreen::Stream;
                                            refresh_stream(app, bp);
                                        }
                                    }
                                }
                            }
                            AppScreen::Stream => {
                                if let Some(StreamAction::Quit) = feed::handle_key_input(app, key) {
                                    return Ok(());
                                }
                            }
                        }
                    }
                    Event::Mouse(mouse) => {
                        if matches!(app.state, AppScreen::Stream) {
                            feed::handle_mouse_input(app, mouse);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn cli_no_args_defaults() {
        let cli = Cli::parse_from(["cc-dm-stream"]);
        assert!(cli.project.is_none());
    }

    #[test]
    fn cli_project_flag() {
        let cli = Cli::parse_from(["cc-dm-stream", "--project", "foo"]);
        assert_eq!(cli.project, Some("foo".to_string()));
    }

    #[test]
    fn clean_exit_on_q_in_stream() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        app.state = AppScreen::Stream;
        let action = feed::handle_key_input(&mut app, key(KeyCode::Char('q')));
        assert!(matches!(action, Some(StreamAction::Quit)));
    }

    #[test]
    fn clean_exit_on_ctrl_c() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(is_ctrl_c(&key));
    }

    #[test]
    fn bus_missing_at_launch_not_connected() {
        let app = App::new(PathBuf::from("/tmp/nonexistent_bus.db"));
        assert!(!app.stats.connected);
    }

    #[test]
    fn project_cli_sets_stream_screen() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        app.project_filter = Some("myapp".to_string());
        app.state = AppScreen::Stream;
        assert!(matches!(app.state, AppScreen::Stream));
        assert_eq!(app.project_filter, Some("myapp".to_string()));
    }
}
