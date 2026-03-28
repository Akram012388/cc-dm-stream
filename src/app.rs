use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use chrono::Utc;

use crate::types::{
    BusStats, FeedEntry, MessageEntry, PruneAlert, ReconnectAlert, RingBuffer, SessionInfo,
};

const FEED_CAPACITY: usize = 1000;

pub enum AppScreen {
    Welcome {
        projects: Vec<(String, usize)>,
        selected: usize,
    },
    Stream,
}

pub struct App {
    pub state: AppScreen,
    pub project_filter: Option<String>,
    pub sessions: HashMap<String, SessionInfo>,
    pub feed: RingBuffer<FeedEntry>,
    pub known_message_ids: HashSet<i64>,
    pub pruned_session_ids: HashSet<String>,
    pub stats: BusStats,
    pub scroll_offset: usize,
    pub auto_scroll: bool,
    pub visible_height: usize,
    pub total_feed_lines: usize,
    // Search state
    pub search_mode: bool,
    pub search_query: String,
    pub search_matches: Vec<usize>,
    pub search_match_index: usize,
}

impl App {
    pub fn new(bus_path: PathBuf) -> Self {
        Self {
            state: AppScreen::Welcome {
                projects: Vec::new(),
                selected: 0,
            },
            project_filter: None,
            sessions: HashMap::new(),
            feed: RingBuffer::new(FEED_CAPACITY),
            known_message_ids: HashSet::new(),
            pruned_session_ids: HashSet::new(),
            stats: BusStats {
                active_sessions: 0,
                messages_observed: 0,
                bus_path,
                connected: false,
            },
            scroll_offset: 0,
            auto_scroll: true,
            visible_height: 0,
            total_feed_lines: 0,
            search_mode: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_match_index: 0,
        }
    }

    pub fn process_bus_update(&mut self, sessions: Vec<SessionInfo>, messages: Vec<MessageEntry>) {
        let now = Utc::now();

        // Build set of incoming session IDs
        let incoming_ids: HashSet<&str> = sessions.iter().map(|s| s.id.as_str()).collect();

        // Detect pruned sessions (in our map but missing from update)
        let pruned: Vec<SessionInfo> = self
            .sessions
            .values()
            .filter(|s| !incoming_ids.contains(s.id.as_str()))
            .cloned()
            .collect();

        for session in &pruned {
            let ago = now
                .signed_duration_since(session.last_seen)
                .num_seconds()
                .max(0) as u64;
            self.feed.push(FeedEntry::PruneAlert(PruneAlert {
                session_name: session.name.clone(),
                role: session.role.clone(),
                last_seen_ago_secs: ago,
                timestamp: now,
            }));
            self.pruned_session_ids.insert(session.id.clone());
            self.sessions.remove(&session.id);
        }

        // Update/add sessions — detect reconnects
        for session in sessions {
            if self.pruned_session_ids.remove(&session.id) {
                self.feed
                    .push(FeedEntry::ReconnectAlert(ReconnectAlert {
                        session_name: session.name.clone(),
                        role: session.role.clone(),
                        timestamp: now,
                    }));
            }
            self.sessions.insert(session.id.clone(), session);
        }

        // Diff messages — only capture new ones
        for msg in messages {
            if self.known_message_ids.insert(msg.id) {
                self.stats.messages_observed += 1;
                self.feed.push(FeedEntry::Message(msg));
            }
        }

        // Update stats
        self.stats.active_sessions = self.sessions.len();
        self.stats.connected = true;

        // Re-run search if active
        if self.search_mode && !self.search_query.is_empty() {
            self.recompute_search_matches();
        }
    }

    /// Enter search mode, pausing auto-scroll.
    pub fn enter_search(&mut self) {
        self.search_mode = true;
        self.search_query.clear();
        self.search_matches.clear();
        self.search_match_index = 0;
        if self.auto_scroll {
            self.scroll_offset = self.total_feed_lines.saturating_sub(self.visible_height);
        }
        self.auto_scroll = false;
    }

    /// Exit search mode, restoring auto-scroll.
    pub fn exit_search(&mut self) {
        self.search_mode = false;
        self.search_query.clear();
        self.search_matches.clear();
        self.search_match_index = 0;
        self.auto_scroll = true;
        self.scroll_offset = self.total_feed_lines.saturating_sub(self.visible_height);
    }

    /// Recompute search matches against current feed entries.
    pub fn recompute_search_matches(&mut self) {
        let query = self.search_query.to_lowercase();
        self.search_matches = self
            .feed
            .iter()
            .enumerate()
            .filter(|(_, entry)| match entry {
                FeedEntry::Message(msg) => msg.content.to_lowercase().contains(&query),
                _ => false,
            })
            .map(|(i, _)| i)
            .collect();
        // Clamp match index
        if self.search_matches.is_empty() {
            self.search_match_index = 0;
        } else {
            self.search_match_index = self
                .search_match_index
                .min(self.search_matches.len() - 1);
        }
    }

    /// Navigate to next search match (wraps around).
    pub fn search_next(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        self.search_match_index = (self.search_match_index + 1) % self.search_matches.len();
    }

    /// Navigate to previous search match (wraps around).
    pub fn search_prev(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        self.search_match_index = (self.search_match_index + self.search_matches.len() - 1)
            % self.search_matches.len();
    }

    /// Update the welcome screen project list, preserving the current selection index.
    pub fn refresh_welcome_projects(&mut self, projects: Vec<(String, usize)>) {
        let prev_selected = match &self.state {
            AppScreen::Welcome { selected, .. } => *selected,
            _ => 0,
        };
        // Max valid index = projects.len() (the "all projects" virtual entry)
        let max_index = projects.len();
        self.state = AppScreen::Welcome {
            projects,
            selected: prev_selected.min(max_index),
        };
    }

    pub fn apply_project_filter<'a>(&self, sessions: &'a [SessionInfo]) -> Vec<&'a SessionInfo> {
        match &self.project_filter {
            None => sessions.iter().collect(),
            Some(project) => sessions.iter().filter(|s| &s.project == project).collect(),
        }
    }

    pub fn filtered_messages<'a>(
        &self,
        sessions: &[SessionInfo],
        messages: &'a [MessageEntry],
    ) -> Vec<&'a MessageEntry> {
        let filtered = self.apply_project_filter(sessions);
        let names: HashSet<&str> = filtered.iter().map(|s| s.name.as_str()).collect();
        let ids: HashSet<&str> = filtered.iter().map(|s| s.id.as_str()).collect();

        messages
            .iter()
            .filter(|m| {
                names.contains(m.from_session.as_str()) || ids.contains(m.to_session.as_str())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SessionStatus;
    use chrono::Duration;

    fn make_session(id: &str, name: &str, role: &str, project: &str, ago_secs: i64) -> SessionInfo {
        let now = Utc::now();
        let last_seen = now - Duration::seconds(ago_secs);
        SessionInfo {
            id: id.to_string(),
            name: name.to_string(),
            role: role.to_string(),
            project: project.to_string(),
            last_seen,
            status: SessionStatus::from_last_seen(last_seen, now),
        }
    }

    fn make_message(id: i64, from: &str, to: &str, content: &str) -> MessageEntry {
        MessageEntry {
            id,
            from_session: from.to_string(),
            to_session: to.to_string(),
            content: content.to_string(),
            priority: crate::types::Priority::Normal,
            thread_id: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn new_session_detected() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        let sessions = vec![make_session("s1", "architect", "orchestrator", "myapp", 5)];
        app.process_bus_update(sessions, vec![]);
        assert!(app.sessions.contains_key("s1"));
        assert_eq!(app.sessions["s1"].name, "architect");
    }

    #[test]
    fn heartbeat_update_recalculates_status() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));

        // First: session is stale
        let sessions = vec![make_session("s1", "worker", "dev", "myapp", 61)];
        app.process_bus_update(sessions, vec![]);
        assert_eq!(app.sessions["s1"].status, SessionStatus::Stale);

        // Second: session heartbeats, now active
        let sessions = vec![make_session("s1", "worker", "dev", "myapp", 5)];
        app.process_bus_update(sessions, vec![]);
        assert_eq!(app.sessions["s1"].status, SessionStatus::Active);
    }

    #[test]
    fn session_pruned_emits_alert() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));

        // First: session exists
        let sessions = vec![make_session("s1", "tester", "worker", "myapp", 62)];
        app.process_bus_update(sessions, vec![]);
        assert_eq!(app.sessions.len(), 1);

        // Second: session disappears
        app.process_bus_update(vec![], vec![]);
        assert!(app.sessions.is_empty());

        // Feed should have a PruneAlert
        let entries: Vec<_> = app.feed.iter().collect();
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            FeedEntry::PruneAlert(alert) => {
                assert_eq!(alert.session_name, "tester");
                assert_eq!(alert.role, "worker");
            }
            _ => panic!("expected PruneAlert"),
        }
    }

    #[test]
    fn pruned_session_reconnect_emits_alert() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));

        // Session appears
        let sessions = vec![make_session("s1", "tester", "worker", "myapp", 5)];
        app.process_bus_update(sessions, vec![]);

        // Session disappears (pruned)
        app.process_bus_update(vec![], vec![]);
        assert!(app.pruned_session_ids.contains("s1"));

        // Session reappears (reconnect)
        let sessions = vec![make_session("s1", "tester", "worker", "myapp", 2)];
        app.process_bus_update(sessions, vec![]);

        // Should have PruneAlert + ReconnectAlert in feed
        let entries: Vec<_> = app.feed.iter().collect();
        assert_eq!(entries.len(), 2);
        match &entries[1] {
            FeedEntry::ReconnectAlert(alert) => {
                assert_eq!(alert.session_name, "tester");
                assert_eq!(alert.role, "worker");
            }
            _ => panic!("expected ReconnectAlert, got {:?}", entries[1]),
        }
        // Pruned set should be cleared for this ID
        assert!(!app.pruned_session_ids.contains("s1"));
    }

    #[test]
    fn new_session_does_not_emit_reconnect_alert() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));

        // Brand new session (never pruned) — should NOT emit ReconnectAlert
        let sessions = vec![make_session("s1", "alice", "worker", "myapp", 5)];
        app.process_bus_update(sessions, vec![]);

        let entries: Vec<_> = app.feed.iter().collect();
        assert!(entries.is_empty(), "new session should not emit any alert");
    }

    #[test]
    fn new_message_captured() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        let messages = vec![make_message(1, "alice", "session-b", "hello")];
        app.process_bus_update(vec![], messages);

        assert!(app.known_message_ids.contains(&1));
        assert_eq!(app.feed.len(), 1);
        assert_eq!(app.stats.messages_observed, 1);
    }

    #[test]
    fn duplicate_message_ignored() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));

        let messages = vec![make_message(1, "alice", "session-b", "hello")];
        app.process_bus_update(vec![], messages);

        // Same message ID again
        let messages = vec![make_message(1, "alice", "session-b", "hello")];
        app.process_bus_update(vec![], messages);

        assert_eq!(app.feed.len(), 1);
        assert_eq!(app.stats.messages_observed, 1);
    }

    #[test]
    fn delivered_message_stays_in_known_set() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));

        let messages = vec![make_message(1, "alice", "session-b", "hello")];
        app.process_bus_update(vec![], messages);

        // Message disappears from bus (delivered) — send empty messages
        app.process_bus_update(vec![], vec![]);

        // ID should still be in known set
        assert!(app.known_message_ids.contains(&1));
    }

    #[test]
    fn ring_buffer_eviction_at_1001() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));

        for i in 1..=1001 {
            let messages = vec![make_message(i, "alice", "session-b", "msg")];
            app.process_bus_update(vec![], messages);
        }

        assert_eq!(app.feed.len(), 1000);
        assert_eq!(app.stats.messages_observed, 1001);
    }

    #[test]
    fn project_filter_on_sessions() {
        let app = App {
            project_filter: Some("myapp".to_string()),
            ..App::new(PathBuf::from("/tmp/bus.db"))
        };

        let sessions = vec![
            make_session("s1", "a", "worker", "myapp", 5),
            make_session("s2", "b", "worker", "other", 5),
            make_session("s3", "c", "worker", "myapp", 5),
        ];

        let filtered = app.apply_project_filter(&sessions);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "a");
        assert_eq!(filtered[1].name, "c");
    }

    #[test]
    fn project_filter_on_messages() {
        let app = App {
            project_filter: Some("myapp".to_string()),
            ..App::new(PathBuf::from("/tmp/bus.db"))
        };

        let sessions = vec![
            make_session("s1", "alice", "worker", "myapp", 5),
            make_session("s2", "bob", "worker", "other", 5),
        ];

        let messages = vec![
            make_message(1, "alice", "s2", "from filtered session"), // alice is in myapp
            make_message(2, "bob", "s1", "to filtered session"),     // to s1 which is in myapp
            make_message(3, "bob", "s2", "neither end in myapp"),    // bob is other, s2 is other
        ];

        let filtered = app.filtered_messages(&sessions, &messages);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].id, 1);
        assert_eq!(filtered[1].id, 2);
    }

    #[test]
    fn stats_update_after_process() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));

        let sessions = vec![
            make_session("s1", "a", "worker", "myapp", 5),
            make_session("s2", "b", "worker", "myapp", 10),
        ];
        let messages = vec![
            make_message(1, "a", "s2", "msg1"),
            make_message(2, "b", "s1", "msg2"),
        ];

        app.process_bus_update(sessions, messages);
        assert_eq!(app.stats.active_sessions, 2);
        assert_eq!(app.stats.messages_observed, 2);
        assert!(app.stats.connected);
    }

    // --- Search tests ---

    #[test]
    fn enter_search_activates_mode() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        app.auto_scroll = true;
        app.enter_search();
        assert!(app.search_mode);
        assert!(!app.auto_scroll);
        assert!(app.search_query.is_empty());
    }

    #[test]
    fn exit_search_restores_auto_scroll() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        app.enter_search();
        app.search_query = "test".to_string();
        app.exit_search();
        assert!(!app.search_mode);
        assert!(app.auto_scroll);
        assert!(app.search_query.is_empty());
        assert!(app.search_matches.is_empty());
    }

    #[test]
    fn search_finds_matching_messages() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        app.process_bus_update(
            vec![],
            vec![
                make_message(1, "a", "b", "hello world"),
                make_message(2, "a", "b", "goodbye"),
                make_message(3, "a", "b", "hello again"),
            ],
        );
        app.enter_search();
        app.search_query = "hello".to_string();
        app.recompute_search_matches();
        assert_eq!(app.search_matches.len(), 2);
        assert_eq!(app.search_matches, vec![0, 2]);
    }

    #[test]
    fn search_case_insensitive() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        app.process_bus_update(vec![], vec![make_message(1, "a", "b", "Hello World")]);
        app.enter_search();
        app.search_query = "hello".to_string();
        app.recompute_search_matches();
        assert_eq!(app.search_matches.len(), 1);
    }

    #[test]
    fn search_no_matches() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        app.process_bus_update(vec![], vec![make_message(1, "a", "b", "hello")]);
        app.enter_search();
        app.search_query = "zzz".to_string();
        app.recompute_search_matches();
        assert!(app.search_matches.is_empty());
    }

    #[test]
    fn search_next_wraps_around() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        app.process_bus_update(
            vec![],
            vec![
                make_message(1, "a", "b", "match"),
                make_message(2, "a", "b", "match"),
                make_message(3, "a", "b", "match"),
            ],
        );
        app.enter_search();
        app.search_query = "match".to_string();
        app.recompute_search_matches();
        assert_eq!(app.search_match_index, 0);
        app.search_next();
        assert_eq!(app.search_match_index, 1);
        app.search_next();
        assert_eq!(app.search_match_index, 2);
        app.search_next(); // wraps
        assert_eq!(app.search_match_index, 0);
    }

    #[test]
    fn search_prev_wraps_around() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        app.process_bus_update(
            vec![],
            vec![
                make_message(1, "a", "b", "match"),
                make_message(2, "a", "b", "match"),
            ],
        );
        app.enter_search();
        app.search_query = "match".to_string();
        app.recompute_search_matches();
        assert_eq!(app.search_match_index, 0);
        app.search_prev(); // wraps to last
        assert_eq!(app.search_match_index, 1);
    }

    #[test]
    fn new_message_during_search_updates_matches() {
        let mut app = App::new(PathBuf::from("/tmp/bus.db"));
        app.process_bus_update(vec![], vec![make_message(1, "a", "b", "hello")]);
        app.enter_search();
        app.search_query = "hello".to_string();
        app.recompute_search_matches();
        assert_eq!(app.search_matches.len(), 1);

        // New message arrives while searching
        app.process_bus_update(vec![], vec![make_message(2, "a", "b", "hello again")]);
        assert_eq!(app.search_matches.len(), 2);
    }
}
