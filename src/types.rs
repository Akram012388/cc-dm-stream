use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::path::PathBuf;

// --- Session ---

#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    Active,
    ApproachingStale,
    Stale,
}

impl SessionStatus {
    pub fn from_last_seen(last_seen: DateTime<Utc>, now: DateTime<Utc>) -> Self {
        let elapsed = now.signed_duration_since(last_seen).num_seconds();
        if elapsed < 45 {
            SessionStatus::Active
        } else if elapsed < 60 {
            SessionStatus::ApproachingStale
        } else {
            SessionStatus::Stale
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub name: String,
    pub role: String,
    pub project: String,
    pub last_seen: DateTime<Utc>,
    pub status: SessionStatus,
}

// --- Priority ---

#[derive(Debug, Clone, PartialEq)]
pub enum Priority {
    Normal,
    Urgent,
    Low,
}

impl Priority {
    pub fn from_meta_json(json_str: &str) -> Self {
        // Simple parsing: look for "priority":"<value>" in the JSON string
        if let Some(start) = json_str.find("\"priority\"") {
            let rest = &json_str[start..];
            if rest.contains("\"urgent\"") {
                return Priority::Urgent;
            }
            if rest.contains("\"low\"") {
                return Priority::Low;
            }
        }
        Priority::Normal
    }
}

// --- Message ---

#[derive(Debug, Clone)]
pub struct MessageEntry {
    pub id: i64,
    pub from_session: String,
    pub to_session: String,
    pub content: String,
    pub priority: Priority,
    pub created_at: DateTime<Utc>,
}

// --- Prune Alert ---

#[derive(Debug, Clone)]
pub struct PruneAlert {
    pub session_name: String,
    pub role: String,
    pub last_seen_ago_secs: u64,
    pub timestamp: DateTime<Utc>,
}

// --- Feed ---

#[derive(Debug, Clone)]
pub enum FeedEntry {
    Message(MessageEntry),
    PruneAlert(PruneAlert),
}

// --- Bus Stats ---

#[derive(Debug, Clone)]
pub struct BusStats {
    pub active_sessions: usize,
    pub messages_observed: u64,
    pub bus_path: PathBuf,
    pub connected: bool,
}

// --- Ring Buffer ---

#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    items: VecDeque<T>,
    capacity: usize,
}

impl<T> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            items: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, item: T) {
        if self.items.len() == self.capacity {
            self.items.pop_front();
        }
        self.items.push_back(item);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }
}

// --- App Event ---

pub enum AppEvent {
    BusChanged,
    Tick,
    Input(crossterm::event::Event),
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    // --- SessionStatus tests ---

    #[test]
    fn session_status_active_at_44s() {
        let now = Utc::now();
        let last_seen = now - Duration::seconds(44);
        assert_eq!(
            SessionStatus::from_last_seen(last_seen, now),
            SessionStatus::Active
        );
    }

    #[test]
    fn session_status_approaching_stale_at_45s() {
        let now = Utc::now();
        let last_seen = now - Duration::seconds(45);
        assert_eq!(
            SessionStatus::from_last_seen(last_seen, now),
            SessionStatus::ApproachingStale
        );
    }

    #[test]
    fn session_status_approaching_stale_at_59s() {
        let now = Utc::now();
        let last_seen = now - Duration::seconds(59);
        assert_eq!(
            SessionStatus::from_last_seen(last_seen, now),
            SessionStatus::ApproachingStale
        );
    }

    #[test]
    fn session_status_stale_at_60s() {
        let now = Utc::now();
        let last_seen = now - Duration::seconds(60);
        assert_eq!(
            SessionStatus::from_last_seen(last_seen, now),
            SessionStatus::Stale
        );
    }

    #[test]
    fn session_status_stale_at_61s() {
        let now = Utc::now();
        let last_seen = now - Duration::seconds(61);
        assert_eq!(
            SessionStatus::from_last_seen(last_seen, now),
            SessionStatus::Stale
        );
    }

    // --- RingBuffer tests ---

    #[test]
    fn ring_buffer_push_within_capacity() {
        let mut buf = RingBuffer::new(5);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        assert_eq!(buf.len(), 3);
        let items: Vec<&i32> = buf.iter().collect();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn ring_buffer_push_at_capacity_evicts_oldest() {
        let mut buf = RingBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        buf.push(4); // evicts 1
        assert_eq!(buf.len(), 3);
        let items: Vec<&i32> = buf.iter().collect();
        assert_eq!(items, vec![&2, &3, &4]);
    }

    #[test]
    fn ring_buffer_iteration_order_oldest_first() {
        let mut buf = RingBuffer::new(5);
        buf.push(10);
        buf.push(20);
        buf.push(30);
        let items: Vec<&i32> = buf.iter().collect();
        assert_eq!(items, vec![&10, &20, &30]);
    }

    // --- Priority tests ---

    #[test]
    fn priority_parses_urgent() {
        let json = r#"{"priority":"urgent","message_type":"task"}"#;
        assert_eq!(Priority::from_meta_json(json), Priority::Urgent);
    }

    #[test]
    fn priority_parses_low() {
        let json = r#"{"priority":"low"}"#;
        assert_eq!(Priority::from_meta_json(json), Priority::Low);
    }

    #[test]
    fn priority_defaults_to_normal_when_missing() {
        let json = r#"{"message_type":"status"}"#;
        assert_eq!(Priority::from_meta_json(json), Priority::Normal);
    }

    #[test]
    fn priority_defaults_to_normal_on_empty_string() {
        assert_eq!(Priority::from_meta_json(""), Priority::Normal);
    }

    #[test]
    fn priority_defaults_to_normal_on_unknown_value() {
        let json = r#"{"priority":"critical"}"#;
        assert_eq!(Priority::from_meta_json(json), Priority::Normal);
    }
}
