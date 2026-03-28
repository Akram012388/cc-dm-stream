# cc-dm-stream — Implementation Plan

**Date:** 2026-03-28
**Scope:** Full v0.1.0 implementation
**Approach:** Bottom-up, test-first per phase
**Branch:** `main`

---

## Build Order

Phases are sequential — each builds on the previous. Tests are written before implementation within each phase.

---

## Phase 1: Types + Theme + CLI

**Goal:** Define all shared types, colour constants, and CLI argument parsing. Zero runtime behaviour. This is the foundation everything else imports.

### Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point stub, CLI parsing with clap |
| `src/types.rs` | `SessionInfo`, `MessageEntry`, `BusStats`, `AppEvent`, `ScrollState`, `RingBuffer<T>` |
| `src/theme.rs` | Tokyo Night colour constants as `ratatui::style::Color` values |

### Types to Define

```rust
struct SessionInfo {
    id: String,
    name: String,
    role: String,
    project: String,
    last_seen: DateTime<Utc>,     // parsed from ISO 8601
    status: SessionStatus,        // computed from last_seen
}

enum SessionStatus {
    Active,          // < 45s since heartbeat
    ApproachingStale, // 45–60s
    Stale,           // > 60s
}

struct MessageEntry {
    id: i64,
    from_session: String,         // display name
    to_session: String,           // session ID
    content: String,
    priority: Priority,           // parsed from meta JSON
    created_at: DateTime<Utc>,
}

enum Priority {
    Normal,
    Urgent,
    Low,
}

struct PruneAlert {
    session_name: String,
    role: String,
    last_seen_ago_secs: u64,
    timestamp: DateTime<Utc>,
}

// Feed entries are either messages or alerts
enum FeedEntry {
    Message(MessageEntry),
    PruneAlert(PruneAlert),
}

struct BusStats {
    active_sessions: usize,
    messages_observed: u64,       // total since launch
    bus_path: PathBuf,
    connected: bool,
}

struct RingBuffer<T> {
    items: VecDeque<T>,
    capacity: usize,              // 1000
}

// Events sent from watcher task to render task
enum AppEvent {
    BusChanged,                   // filesystem event detected
    Tick,                         // 1-second render refresh
    Input(crossterm::event::Event), // keyboard/mouse
}
```

### Tests

| Test | Verification |
|------|-------------|
| `SessionStatus` from timestamp | Active/approaching/stale boundaries at 44s, 45s, 59s, 60s, 61s |
| `RingBuffer` push within capacity | Items appended, len correct |
| `RingBuffer` push at capacity | Oldest evicted, len stays at capacity |
| `RingBuffer` iteration order | Oldest first, newest last |
| `Priority` from meta JSON | Parses "urgent", "low", missing key defaults to Normal |
| CLI parsing: no args | Default state, no project filter |
| CLI parsing: `--project foo` | Project filter set to "foo" |
| Tokyo Night colours | Constants match expected hex values |

---

## Phase 2: Bus Reader

**Goal:** Read-only SQLite queries against `~/.cc-dm/bus.db`. All database access lives here. No filesystem watching yet — just pure query functions.

### Files

| File | Purpose |
|------|---------|
| `src/bus.rs` | Open connection, query sessions, query messages, diff logic |

### Functions

```rust
fn open_bus(path: &Path) -> Result<Connection>
  // SQLITE_OPEN_READONLY | SQLITE_OPEN_NO_MUTEX
  // Returns error if file doesn't exist (caller handles gracefully)

fn read_sessions(conn: &Connection) -> Result<Vec<SessionInfo>>
  // SELECT id, name, role, project, last_seen FROM sessions WHERE status = 'active'

fn read_pending_messages(conn: &Connection) -> Result<Vec<MessageEntry>>
  // SELECT * FROM messages WHERE delivered = 0 ORDER BY id ASC
  // Parses meta JSON for priority

fn list_projects(conn: &Connection) -> Result<Vec<(String, usize)>>
  // SELECT project, COUNT(*) FROM sessions WHERE status = 'active' AND project != ''
  // GROUP BY project ORDER BY project ASC
  // Returns (project_name, session_count)
```

### Tests

| Test | Verification |
|------|-------------|
| `open_bus` read-only | Connection opens, SELECT works, INSERT fails |
| `read_sessions` with populated table | Returns correct session count and fields |
| `read_sessions` with empty table | Returns empty vec, no error |
| `read_pending_messages` ordering | Messages returned by ascending ID |
| `read_pending_messages` meta parsing | Urgent priority parsed from `{"priority":"urgent"}` |
| `read_pending_messages` bad meta JSON | Defaults to Normal priority |
| `list_projects` grouping | Correct project names and counts |
| `list_projects` excludes empty project | Sessions with `project = ''` not listed |

**Note:** Tests create a temp SQLite DB, insert test data, run queries. Tests clean up temp files.

---

## Phase 3: Filesystem Watcher

**Goal:** Watch `~/.cc-dm/` directory for changes. Filter to bus-related files. Send events through a channel.

### Files

| File | Purpose |
|------|---------|
| `src/watcher.rs` | Spawn notify watcher, filter events, send through mpsc |

### Functions

```rust
fn start_watcher(
    bus_dir: PathBuf,
    tx: mpsc::Sender<AppEvent>,
) -> Result<notify::RecommendedWatcher>
  // Watches bus_dir for write/modify events
  // Filters to bus.db, bus.db-wal, bus.db-shm
  // Sends AppEvent::BusChanged on match
```

### Tests

| Test | Verification |
|------|-------------|
| Watcher sends event on file write | Write to watched dir, receive AppEvent::BusChanged |
| Watcher filters non-bus files | Write to unrelated file in dir, no event sent |
| Watcher handles missing directory | Returns error, does not panic |

---

## Phase 4: App State + Diff Logic

**Goal:** The core brain. Holds all state, processes bus reads, computes diffs, emits feed entries and roster updates.

### Files

| File | Purpose |
|------|---------|
| `src/app.rs` | `App` struct, state transitions, diff engine |

### Struct

```rust
struct App {
    state: AppScreen,             // Welcome or Stream
    project_filter: Option<String>,
    sessions: HashMap<String, SessionInfo>,
    feed: RingBuffer<FeedEntry>,
    known_message_ids: HashSet<i64>,
    stats: BusStats,
    scroll_offset: usize,
    auto_scroll: bool,
}

enum AppScreen {
    Welcome { projects: Vec<(String, usize)>, selected: usize },
    Stream,
}
```

### Key Methods

```rust
fn process_bus_update(&mut self, sessions: Vec<SessionInfo>, messages: Vec<MessageEntry>)
  // 1. Diff sessions → detect new, updated, removed
  // 2. Removed sessions → emit PruneAlert to feed
  // 3. Diff messages → detect new (not in known_message_ids)
  // 4. New messages → add to feed ring buffer, add ID to known set
  // 5. Update stats

fn apply_project_filter(&self, sessions: &[SessionInfo]) -> Vec<SessionInfo>
  // Filter by project_filter match

fn filtered_messages(&self, sessions: &[SessionInfo], messages: &[MessageEntry]) -> Vec<MessageEntry>
  // Include messages where from_session name or to_session ID belongs to a filtered session
```

### Tests

| Test | Verification |
|------|-------------|
| New session detected | Session not in map → added to roster |
| Heartbeat update | Session `last_seen` changed → status recalculated |
| Session pruned | Session in map but missing from read → PruneAlert in feed |
| New message captured | Message ID not in known set → added to feed and known set |
| Duplicate message ignored | Message ID already in known set → not added again |
| Message delivered (disappeared) | Message ID in known set but not in read → ID stays in known set (no action) |
| Ring buffer eviction | Push 1001st message → oldest evicted, len = 1000 |
| Project filter on sessions | Only sessions with matching project returned |
| Project filter on messages | Only messages involving filtered sessions returned |
| Stats update | Session count and message count accurate after process |

---

## Phase 5: TUI — Welcome Screen

**Goal:** The project picker. List active projects, arrow key selection, Enter to confirm, q to quit.

### Files

| File | Purpose |
|------|---------|
| `src/ui/mod.rs` | Layout router (Welcome vs Stream screen) |
| `src/ui/welcome.rs` | Welcome screen render + input handling |

### Behaviour

- Title: `cc-dm-stream v0.1.0`
- List: project names with session counts + "[all projects]" at end
- Arrow keys move selection highlight
- Enter confirms → transition to Stream screen
- q quits

### Tests

| Test | Verification |
|------|-------------|
| Render with 3 projects | All three listed with counts, plus "all projects" |
| Render with 0 projects | "No active sessions. Waiting..." message |
| Arrow down wraps | From last item, wraps to first |
| Enter on selection | Returns selected project name |
| Enter on "all projects" | Returns None (no filter) |

---

## Phase 6: TUI — Main Stream View

**Goal:** The two-panel layout + status bar. Roster left, feed right, stats bottom.

### Files

| File | Purpose |
|------|---------|
| `src/ui/roster.rs` | Session roster panel |
| `src/ui/feed.rs` | Message feed with scroll |
| `src/ui/status_bar.rs` | Bottom stats bar |

### Roster Panel

- Fixed width (~20-25 chars)
- Per session: name, role, status dot (coloured), "last: Xs"
- Sorted by registration order (as returned by bus query)

### Feed Panel

- Flex width (remaining space)
- Per entry: timestamp, sender → recipient, content (wrapped)
- PruneAlerts rendered with `[!]` prefix in amber
- Urgent messages highlighted in red
- Scroll: mouse wheel, arrow keys, Space = jump to end
- Auto-scroll when at bottom, pauses when scrolled up

### Status Bar

- Left: connection indicator + bus path
- Centre: `sessions: N | msgs: N`
- Right: `q: quit  Space: latest  ↑↓: scroll`

### Tests

| Test | Verification |
|------|-------------|
| Roster renders session with active status | Green dot present |
| Roster renders stale session | Red dot present |
| Feed renders message entry | Timestamp, sender, recipient, content visible |
| Feed renders prune alert | `[!]` prefix, amber styling |
| Feed renders urgent message | Red accent styling |
| Status bar shows session count | Matches actual count |
| Scroll state: at bottom | auto_scroll = true |
| Scroll state: scrolled up | auto_scroll = false |
| Space key | Resets scroll to bottom, auto_scroll = true |

---

## Phase 7: Integration + Main Loop

**Goal:** Wire everything together. The `main.rs` event loop that connects watcher → bus → app → ui.

### Main Loop

```
1. Parse CLI args
2. Check if bus exists → if not, show waiting state
3. If bus exists, query projects → show welcome screen
4. On project selection → start watcher + enter stream screen
5. Event loop:
   a. AppEvent::BusChanged → read bus, process diffs, render
   b. AppEvent::Tick (1s) → recompute session statuses, render
   c. AppEvent::Input → handle keys/mouse, render
   d. q / Ctrl+C → clean exit
```

**Note on the 1-second tick:** The roster shows "last: Xs ago" — this counter must update even without bus events. A 1-second tokio interval sends `AppEvent::Tick` to refresh the display. This is NOT database polling. No SQLite reads on tick — just recompute `SessionStatus` from in-memory `last_seen` timestamps and re-render.

### Files

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI parsing, event loop orchestration, terminal setup/teardown |

### Tests

| Test | Verification |
|------|-------------|
| Clean exit on q | Terminal restored, no panic |
| Clean exit on Ctrl+C | Terminal restored, no panic |
| Bus missing at launch | Waiting state displayed, no crash |
| Bus appears after launch | Transitions to welcome screen |

---

## Summary

| Phase | Files | Tests | Depends On |
|-------|-------|-------|------------|
| 1. Types + Theme + CLI | `types.rs`, `theme.rs`, `main.rs` | 8 | — |
| 2. Bus Reader | `bus.rs` | 8 | Phase 1 |
| 3. Filesystem Watcher | `watcher.rs` | 3 | Phase 1 |
| 4. App State + Diff | `app.rs` | 10 | Phase 1, 2 |
| 5. Welcome Screen | `ui/mod.rs`, `ui/welcome.rs` | 5 | Phase 1, 4 |
| 6. Stream View | `ui/roster.rs`, `ui/feed.rs`, `ui/status_bar.rs` | 9 | Phase 1, 4 |
| 7. Integration | `main.rs` | 4 | All |
| **Total** | **12 files** | **47 tests** | |
