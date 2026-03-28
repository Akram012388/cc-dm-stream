use std::path::PathBuf;
use std::time::Duration;

use notify::{Config, Event, PollWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::types::AppEvent;

const BUS_FILES: &[&str] = &["bus.db", "bus.db-wal", "bus.db-shm"];

pub fn start_watcher(
    bus_dir: PathBuf,
    tx: mpsc::Sender<AppEvent>,
) -> Result<PollWatcher, notify::Error> {
    if !bus_dir.exists() {
        return Err(notify::Error::path_not_found());
    }

    let config = Config::default().with_poll_interval(Duration::from_millis(100));

    let mut watcher = PollWatcher::new(
        move |res: Result<Event, notify::Error>| {
            let event = match res {
                Ok(e) => e,
                Err(_) => return,
            };

            let is_bus_event = event.paths.iter().any(|p| {
                p.file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(|name| BUS_FILES.contains(&name))
            });

            if is_bus_event {
                let _ = tx.blocking_send(AppEvent::BusChanged);
            }
        },
        config,
    )?;

    watcher.watch(&bus_dir, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn watcher_sends_event_on_bus_file_write() {
        let dir = TempDir::new().unwrap();
        let bus_file = dir.path().join("bus.db");

        let (tx, mut rx) = mpsc::channel(16);
        let _watcher = start_watcher(dir.path().to_path_buf(), tx).unwrap();

        // Let PollWatcher complete initial scan of empty directory
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Drain any initial events from the first poll
        while rx.try_recv().is_ok() {}

        // Create bus.db — PollWatcher will detect on next poll
        fs::write(&bus_file, b"data").unwrap();

        // PollWatcher fires on 100ms intervals — allow enough time
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
        assert!(
            event.is_ok(),
            "should receive BusChanged event within timeout"
        );
    }

    #[tokio::test]
    async fn watcher_filters_non_bus_files() {
        let dir = TempDir::new().unwrap();

        let (tx, mut rx) = mpsc::channel(16);
        let _watcher = start_watcher(dir.path().to_path_buf(), tx).unwrap();

        // PollWatcher needs time to complete first poll cycle
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Write to an unrelated file
        fs::write(dir.path().join("unrelated.txt"), b"noise").unwrap();

        // Wait for several poll cycles — should NOT receive an event
        let event = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
        assert!(
            event.is_err(),
            "should NOT receive event for non-bus file writes"
        );
    }

    #[test]
    fn watcher_missing_directory_returns_error() {
        let (tx, _rx) = mpsc::channel(16);
        let result = start_watcher(PathBuf::from("/tmp/nonexistent_cc_dm_watcher_test"), tx);
        assert!(result.is_err(), "should error for missing directory");
    }
}
