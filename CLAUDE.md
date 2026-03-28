# cc-dm-stream

Live streaming TUI for the cc-dm coordination bus. Read-only observer — never writes to the bus.

## Stack

- **Rust** (2021 edition)
- **ratatui** — terminal UI framework
- **crossterm** — terminal backend
- **tokio** — async runtime
- **rusqlite** — SQLite read-only access (WAL compatible)
- **notify** — filesystem event watching (kqueue on macOS, inotify on Linux)
- **clap** — CLI argument parsing

## Architecture

```
src/
├── main.rs          # entry point, CLI parsing, app bootstrap
├── app.rs           # AppState, event handling, state transitions
├── bus.rs           # SQLite read-only queries, diff logic
├── watcher.rs       # filesystem event watching, event filtering
├── ui/
│   ├── mod.rs       # layout composition (two-panel + status bar)
│   ├── roster.rs    # session roster panel (left)
│   ├── feed.rs      # message feed panel (right)
│   ├── status_bar.rs # stats bar (bottom)
│   └── welcome.rs   # project picker screen
├── theme.rs         # Tokyo Night colour constants
└── types.rs         # shared types (SessionInfo, MessageEntry, BusStats, etc.)
```

## Key Implementation Details

### The Bus

cc-dm stores sessions and messages in `~/.cc-dm/bus.db` (SQLite, WAL mode).

**Sessions table:** `id TEXT PK, name, role, cwd, project, status, last_seen, registered_at`
**Messages table:** `id INTEGER PK AUTOINCREMENT, from_session (display name), to_session (session ID), content, meta JSON, delivered (always 0), created_at`

**Critical:** Messages are ephemeral. They are DELETED after delivery (~500ms), not marked as delivered. The `delivered` column is never set to 1. cc-dm-stream must catch messages during their brief existence.

### SQLite Connection

```rust
use rusqlite::{Connection, OpenFlags};

// Read-only, no mutex (single-threaded access from watcher task)
let conn = Connection::open_with_flags(
    &bus_path,
    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
)?;
```

### Terminal Init (ratatui 0.30+)

```rust
// ratatui::init() handles crossterm raw mode, alternate screen, panic hook
let mut terminal = ratatui::init();
// ... app loop ...
ratatui::restore(); // always call on exit
```

### Diff-Based Message Capture

On every filesystem event:
1. Read ALL pending messages (`SELECT * FROM messages WHERE delivered = 0 ORDER BY id ASC`)
2. Compare message IDs against the in-memory `known_message_ids` set
3. New IDs → capture into ring buffer, add to known set
4. IDs in known set but missing from query → message was delivered (no action needed for display)

This works because filesystem notifications arrive in <10ms, and cc-dm's poll loop deletes messages every ~500ms. The 490ms window is comfortable.

### Session Roster Diffing

On every filesystem event:
1. Read full sessions table (`SELECT id, name, role, cwd, project, last_seen FROM sessions WHERE status = 'active'`)
2. Compare against in-memory sessions map
3. New session IDs → add to roster
4. Changed `last_seen` → update heartbeat display
5. Missing session IDs → emit prune alert to feed, remove from roster

### Event Loop Architecture

Two tokio tasks communicating via `mpsc` channel:

**Watcher task:**
- `notify` watches `~/.cc-dm/` directory
- Filters events to bus-related files (`bus.db`, `bus.db-wal`, `bus.db-shm`)
- On relevant event: reads SQLite, diffs state, sends typed events through channel

**Render task:**
- Receives events from channel
- Updates `AppState`
- Calls ratatui draw cycle
- Handles keyboard/mouse input via crossterm events

Neither task blocks the other.

### Stale Session Detection

Computed locally from `last_seen` timestamps — does not wait for cc-dm to prune:
- Green: `now - last_seen < 45s`
- Amber: `45s <= now - last_seen < 60s`
- Red: `now - last_seen >= 60s`

Prune alert fires when a session disappears from the sessions table entirely (deleted by cc-dm's 60s cleanup).

### Ring Buffer

Fixed capacity 1000. When full, oldest entry is evicted. This is the only place message history exists after cc-dm deletes delivered messages.

### Project Filtering

All queries to sessions and messages are filtered by the selected project. Sessions with matching `project` field are included. Messages where `from_session` name matches a roster session or `to_session` ID matches a roster session are included.

## cc-dm Constants (Reference)

| Constant | Value |
|----------|-------|
| Bus path | `~/.cc-dm/bus.db` |
| Heartbeat interval | 30s |
| Session expiry | 60s |
| Message expiry | 15s (undelivered) |
| Poll interval | 500ms |
| `from_session` | Display name (NOT session ID) |
| `to_session` | Session ID |

## Do's

1. Open SQLite in read-only mode (`SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX`)
2. Handle all bus states gracefully (missing, empty, active, mid-deletion)
3. Use the diff approach for message capture — never assume rows persist
4. Filter filesystem events to bus-related files before querying SQLite
5. Keep the render cycle fast — only redraw changed widgets

## Don'ts

1. Never write to the bus — no INSERT, UPDATE, DELETE, PRAGMA writes
2. Never register as a session or send heartbeats
3. Never poll on a timer — use filesystem events exclusively
4. Never assume messages persist — they are deleted within ~500ms
5. Never join `from_session` against `sessions.id` — it's a display name, not an ID
6. Never auto-select a project in the welcome screen — always require manual selection
7. Never add theming configuration — Tokyo Night is hardcoded
8. Never render heartbeats in the message feed

## Testing

```bash
cargo test              # unit tests
cargo test -- --ignored # integration tests (requires bus.db)
cargo clippy            # lint
cargo fmt --check       # format check
```

## Running

```bash
# development
cargo run

# with project filter
cargo run -- --project cc-dm-stream

# release build
cargo build --release
./target/release/cc-dm-stream
```
