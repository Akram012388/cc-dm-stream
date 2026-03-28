use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags};

use crate::types::{parse_thread_id, MessageEntry, Priority, SessionInfo, SessionStatus};

pub fn open_bus(path: &Path) -> Result<Connection, rusqlite::Error> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
}

pub fn read_sessions(conn: &Connection) -> Result<Vec<SessionInfo>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, role, project, last_seen FROM sessions \
         WHERE status = 'active' ORDER BY registered_at ASC",
    )?;

    let now = Utc::now();
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let role: String = row.get(2)?;
        let project: String = row.get(3)?;
        let last_seen_str: String = row.get(4)?;
        Ok((id, name, role, project, last_seen_str))
    })?;

    let mut sessions = Vec::new();
    for row in rows {
        let (id, name, role, project, last_seen_str) = row?;
        let last_seen = DateTime::parse_from_rfc3339(&last_seen_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(now);
        let status = SessionStatus::from_last_seen(last_seen, now);
        sessions.push(SessionInfo {
            id,
            name,
            role,
            project,
            last_seen,
            status,
        });
    }
    Ok(sessions)
}

pub fn read_pending_messages(conn: &Connection) -> Result<Vec<MessageEntry>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, from_session, to_session, content, meta, created_at \
         FROM messages WHERE delivered = 0 ORDER BY id ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let from_session: String = row.get(1)?;
        let to_session: String = row.get(2)?;
        let content: String = row.get(3)?;
        let meta: String = row.get(4)?;
        let created_at_str: String = row.get(5)?;
        Ok((id, from_session, to_session, content, meta, created_at_str))
    })?;

    let now = Utc::now();
    let mut messages = Vec::new();
    for row in rows {
        let (id, from_session, to_session, content, meta, created_at_str) = row?;
        let priority = Priority::from_meta_json(&meta);
        let thread_id = parse_thread_id(&meta);
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(now);
        messages.push(MessageEntry {
            id,
            from_session,
            to_session,
            content,
            priority,
            thread_id,
            created_at,
        });
    }
    Ok(messages)
}

pub fn list_projects(conn: &Connection) -> Result<Vec<(String, usize)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT project, COUNT(*) FROM sessions \
         WHERE status = 'active' AND project != '' \
         GROUP BY project ORDER BY project ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        let project: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        Ok((project, count as usize))
    })?;

    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    /// Create a temp SQLite DB with cc-dm schema and return (Connection, NamedTempFile).
    /// NamedTempFile must stay in scope so the file isn't deleted.
    fn setup_test_db() -> (Connection, NamedTempFile) {
        let tmp = NamedTempFile::new().expect("failed to create temp file");
        let conn = Connection::open(tmp.path()).expect("failed to open test db");
        conn.execute_batch(
            "CREATE TABLE sessions (
                id            TEXT PRIMARY KEY,
                name          TEXT NOT NULL DEFAULT '',
                role          TEXT NOT NULL DEFAULT 'worker',
                cwd           TEXT NOT NULL DEFAULT '',
                status        TEXT NOT NULL DEFAULT 'active',
                last_seen     TEXT NOT NULL,
                registered_at TEXT NOT NULL,
                project       TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE messages (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                from_session  TEXT NOT NULL,
                to_session    TEXT NOT NULL,
                content       TEXT NOT NULL,
                meta          TEXT NOT NULL DEFAULT '{}',
                delivered     INTEGER NOT NULL DEFAULT 0,
                created_at    TEXT NOT NULL
            );",
        )
        .expect("failed to create schema");
        (conn, tmp)
    }

    // --- open_bus tests ---

    #[test]
    fn open_bus_readonly_select_works() {
        let (conn, tmp) = setup_test_db();
        drop(conn);

        let ro_conn = open_bus(tmp.path()).expect("should open read-only");
        let count: i64 = ro_conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .expect("SELECT should work");
        assert_eq!(count, 0);
    }

    #[test]
    fn open_bus_readonly_insert_fails() {
        let (conn, tmp) = setup_test_db();
        drop(conn);

        let ro_conn = open_bus(tmp.path()).expect("should open read-only");
        let result = ro_conn.execute(
            "INSERT INTO sessions (id, last_seen, registered_at) VALUES ('x', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        );
        assert!(
            result.is_err(),
            "INSERT should fail on read-only connection"
        );
    }

    #[test]
    fn open_bus_missing_file_fails() {
        let result = open_bus(Path::new("/tmp/nonexistent_cc_dm_bus_test.db"));
        assert!(result.is_err(), "should fail for missing file");
    }

    // --- read_sessions tests ---

    #[test]
    fn read_sessions_populated() {
        let (conn, _tmp) = setup_test_db();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO sessions (id, name, role, project, status, last_seen, registered_at) \
             VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?5)",
            rusqlite::params!["s1", "architect", "orchestrator", "myapp", &now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions (id, name, role, project, status, last_seen, registered_at) \
             VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?5)",
            rusqlite::params!["s2", "engineer", "developer", "myapp", &now],
        )
        .unwrap();

        let sessions = read_sessions(&conn).expect("should read sessions");
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].name, "architect");
        assert_eq!(sessions[0].role, "orchestrator");
        assert_eq!(sessions[0].project, "myapp");
        assert_eq!(sessions[1].name, "engineer");
    }

    #[test]
    fn read_sessions_empty_table() {
        let (conn, _tmp) = setup_test_db();
        let sessions = read_sessions(&conn).expect("should read sessions");
        assert!(sessions.is_empty());
    }

    // --- read_pending_messages tests ---

    #[test]
    fn read_pending_messages_ordering() {
        let (conn, _tmp) = setup_test_db();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO messages (from_session, to_session, content, meta, delivered, created_at) \
             VALUES ('alice', 'session-b', 'msg1', '{}', 0, ?1)",
            [&now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages (from_session, to_session, content, meta, delivered, created_at) \
             VALUES ('bob', 'session-a', 'msg2', '{}', 0, ?1)",
            [&now],
        )
        .unwrap();

        let msgs = read_pending_messages(&conn).expect("should read messages");
        assert_eq!(msgs.len(), 2);
        assert!(msgs[0].id < msgs[1].id, "should be ordered by ascending ID");
        assert_eq!(msgs[0].content, "msg1");
        assert_eq!(msgs[1].content, "msg2");
    }

    #[test]
    fn read_pending_messages_parses_urgent_priority() {
        let (conn, _tmp) = setup_test_db();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO messages (from_session, to_session, content, meta, delivered, created_at) \
             VALUES ('alice', 'session-b', 'urgent msg', '{\"priority\":\"urgent\"}', 0, ?1)",
            [&now],
        )
        .unwrap();

        let msgs = read_pending_messages(&conn).expect("should read messages");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].priority, Priority::Urgent);
    }

    #[test]
    fn read_pending_messages_bad_meta_defaults_to_normal() {
        let (conn, _tmp) = setup_test_db();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO messages (from_session, to_session, content, meta, delivered, created_at) \
             VALUES ('alice', 'session-b', 'normal msg', 'not json at all', 0, ?1)",
            [&now],
        )
        .unwrap();

        let msgs = read_pending_messages(&conn).expect("should read messages");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].priority, Priority::Normal);
    }

    // --- list_projects tests ---

    #[test]
    fn list_projects_grouping() {
        let (conn, _tmp) = setup_test_db();
        let now = Utc::now().to_rfc3339();
        for (id, project) in [("s1", "alpha"), ("s2", "alpha"), ("s3", "beta")] {
            conn.execute(
                "INSERT INTO sessions (id, name, role, project, status, last_seen, registered_at) \
                 VALUES (?1, ?1, 'worker', ?2, 'active', ?3, ?3)",
                rusqlite::params![id, project, &now],
            )
            .unwrap();
        }

        let projects = list_projects(&conn).expect("should list projects");
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0], ("alpha".to_string(), 2));
        assert_eq!(projects[1], ("beta".to_string(), 1));
    }

    #[test]
    fn list_projects_excludes_empty_project() {
        let (conn, _tmp) = setup_test_db();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO sessions (id, name, role, project, status, last_seen, registered_at) \
             VALUES ('s1', 'worker1', 'worker', '', 'active', ?1, ?1)",
            [&now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions (id, name, role, project, status, last_seen, registered_at) \
             VALUES ('s2', 'worker2', 'worker', 'myapp', 'active', ?1, ?1)",
            [&now],
        )
        .unwrap();

        let projects = list_projects(&conn).expect("should list projects");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].0, "myapp");
    }
}
