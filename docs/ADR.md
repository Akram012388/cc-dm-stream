# cc-dm-stream — Architecture Decision Records

**Date:** 2026-03-28
**Status:** Accepted

---

## ADR-01: Read-Only SQLite Connection

**Decision:** cc-dm-stream opens the bus database in read-only mode and never writes any data.

**Context:** cc-dm is a production coordination layer. Any observer must have zero side effects on the bus. If cc-dm-stream crashes, corrupts its state, or behaves unexpectedly, the team must keep running unaffected.

**Consequence:** SQLite connection uses `SQLITE_OPEN_READONLY`. No INSERT, UPDATE, DELETE, or PRAGMA writes. The bus does not know cc-dm-stream exists.

---

## ADR-02: Event-Driven via Filesystem Watching (Not Polling)

**Decision:** Watch the `~/.cc-dm/` directory using `notify` crate (`kqueue` on macOS, `inotify` on Linux) instead of polling the database on a timer.

**Context:** Polling introduces latency equal to the poll interval. A 100ms poll means up to 100ms delay. Filesystem notification delivers sub-10ms latency from bus write to screen update. cc-dm messages are ephemeral (deleted within ~500ms), so speed matters — we must read before deletion.

**Consequence:** Directory-level watching catches WAL writes, checkpoint flushes, and edge cases. No timers, no sleep intervals in the hot path.

**2026-03-28 Update:** FSEvents (`RecommendedWatcher` / `kqueue` on macOS) does not reliably fire for `~/.cc-dm/` directory changes — zero events observed during active bus writes in production testing. Root cause is likely FSEvents not monitoring SQLite WAL-mode writes at the directory level. Switched to `notify::PollWatcher` with a 100ms poll interval, which reliably detects changes. This introduces up to 100ms latency (vs the original sub-10ms goal), but the 400ms remaining window before cc-dm's 500ms deletion cycle is comfortable. The poll interval must not exceed 200ms to maintain reliable message capture.

---

## ADR-03: Diff-Based Message Capture

**Decision:** On every filesystem event, read ALL pending messages (`WHERE delivered = 0`), diff against an in-memory set of known message IDs, and capture new entries into the ring buffer.

**Context:** cc-dm deletes messages after delivery (~500ms). The `delivered` column is never set to 1. A simple "read rows since last rowid" approach would miss messages deleted between reads. The diff approach captures messages during their ~500ms lifespan and preserves them in cc-dm-stream's ring buffer permanently (up to 1000).

**Consequence:** cc-dm-stream is the only place message history exists after delivery. The bus is the live wire; our buffer is the tape recorder. Full table scan on messages is acceptable — the table is small (messages are ephemeral, max ~50 pending at any time).

---

## ADR-04: 1000-Message Ring Buffer

**Decision:** Keep the last 1000 messages in memory. Oldest messages are evicted when the buffer is full.

**Context:** Messages must persist in cc-dm-stream after deletion from the bus. Unlimited history risks unbounded memory growth. 1000 messages covers several hours of active multi-session coordination, which is sufficient for scroll-back review.

**Consequence:** Fixed memory footprint. No disk persistence. History resets on restart (launch-forward only).

---

## ADR-05: No Heartbeat Rendering in Feed

**Decision:** Heartbeats update the session roster panel silently. They do not appear as entries in the message feed.

**Context:** Heartbeats fire every 30s per session. With 5 sessions, that's a heartbeat every 6 seconds — noise that would drown out actual messages in the feed. The roster panel's "time since last heartbeat" indicator already surfaces heartbeat health.

**Consequence:** The feed contains only messages and prune alerts. Roster reflects heartbeat state via status colour and timestamp.

---

## ADR-06: Prune Alerts as Feed Notifications

**Decision:** When a session disappears from the bus (pruned after 60s inactivity), render an alert notification in the message feed, visually distinct from normal messages.

**Context:** A pruned session may be intentional (session closed) or accidental (crash, stuck process). The alert serves as a call-to-action — the user can switch to the stale session's pane and re-register if needed. Subtle but actionable.

**Consequence:** Prune detection via sessions table diff. If a known session ID disappears, emit alert with session name, role, and time since last heartbeat. Rendered in amber with `[!]` prefix.

---

## ADR-07: Two-Panel Layout with Rich Status Bar

**Decision:** Two panels (roster left, message feed right) plus a stats-rich status bar at the bottom. No third stats column.

**Context:** The message feed is the primary content. A dedicated stats column wastes horizontal space on data that fits in a single status bar line. Two panels maximize feed real estate while keeping the roster always visible.

**Consequence:** Status bar carries: bus connection status, file path, session count, message count, keybinding hints. Stats update on every bus event.

---

## ADR-08: Tokyo Night Theme, Hardcoded

**Decision:** Hardcode the Tokyo Night colour palette. No runtime theming, no configuration.

**Context:** The target user runs WezTerm with Tokyo Night. Theming infrastructure (config parsing, colour resolution, fallbacks) is complexity with no immediate payoff. YAGNI.

**Consequence:** Colours are constants in the source. Changing the theme requires editing code. Acceptable for v0.1.0.

---

## ADR-09: Raw Message Rendering (No Broadcast Grouping)

**Decision:** Every bus row is one feed entry. No heuristic broadcast detection or message grouping.

**Context:** cc-dm broadcasts are N individual message rows with no broadcast flag in the schema. Heuristic grouping (same sender + content + close timestamps) adds complexity and is technically a guess. Raw rendering matches the "live console.log" mental model — cc-dm-stream shows exactly what the bus sees.

**Consequence:** A broadcast to 5 sessions appears as 5 lines. The user sees the truth. Simpler code, zero heuristics.

---

## ADR-10: Project Scoping with Welcome Screen

**Decision:** On launch, show a welcome screen with active projects from the bus. User manually selects one. CLI argument `--project <name>` skips the welcome screen.

**Context:** A developer may run multiple projects simultaneously. Without project scoping, all traffic is interleaved — noise. The welcome screen provides discovery. The CLI argument provides muscle memory for repeat use.

**Consequence:** Always show the picker, even for a single project. No auto-selection. Sessions with empty project tags visible only under "all projects." Filtering applied to both roster and message feed.

---

## ADR-11: Directory-Level Filesystem Watching

**Decision:** Watch the `~/.cc-dm/` directory rather than individual files (`bus.db`, `bus.db-wal`).

**Context:** SQLite WAL checkpoints move data from WAL to main DB and can truncate the WAL file. Watching only `bus.db-wal` risks blind spots during and after checkpoints. Directory-level watching catches all write events regardless of which file SQLite writes to.

**Consequence:** Slightly more filesystem events to filter (non-bus files in `~/.cc-dm/`), but robust against WAL checkpoint edge cases. Filter events to only process bus-related file changes.

---

## ADR-12: Launch-Forward Only

**Decision:** On startup, cc-dm-stream captures only events that occur after launch. No historical snapshot of existing messages.

**Context:** Messages in the bus are ephemeral — most have already been delivered and deleted. Reading "existing" messages at launch would show only undelivered (possibly stale) messages, creating a misleading initial state. Clean start is honest and simple.

**Consequence:** The roster is populated immediately from existing sessions (current state, not historical). The message feed starts empty. First message appears when the first bus write occurs after launch.

**Note:** Existing sessions ARE loaded at startup for the roster and project picker. Only the message feed starts clean.

---

## ADR-13: Manual Project Selection Always

**Decision:** The project picker is always presented, even when only one project is active. No auto-selection.

**Context:** Auto-selection for single projects is a micro-optimization that removes user agency. The picker takes one keypress. Consistency is worth more than saving one interaction.

**Consequence:** Uniform UX regardless of project count.

---

## ADR-14: Simple Scroll Mechanics

**Decision:** Mouse scroll + arrow keys for navigation. Space to jump to end. No "new messages" indicator or unread count.

**Context:** A "3 new messages" badge is UI complexity for marginal value. The user either watches live (auto-scroll) or reviews history (manual scroll). Space to catch up is the fastest path back to live. KISS.

**Consequence:** No tracking of "last viewed" position. No unread state management. Scroll position is the only state.

---

## ADR-15: Message Search — KISS Substring Matching

**Decision:** Search uses plain case-insensitive substring matching. No regex, no fuzzy matching, no ranking.

**Context:** The feed is a live stream of coordination messages. Search is for quick recall ("did someone mention X?"), not document retrieval. Regex adds complexity (escaping, error handling, performance) for a feature that 99% of the time is used as a plain text filter. Fuzzy matching adds ranking ambiguity.

**Keybinding rationale:** `Ctrl+F` (familiar) and `/` (vim convention) both enter search mode. `Ctrl+N/Ctrl+P` navigate matches during typing without consuming the character — `n/N` would conflict with typing 'n' in the query. `Esc` exits entirely (no two-phase exit). `Enter` jumps to next match (vim `/` + `n` muscle memory).

**Consequence:** Search mode is a single boolean state. Query updates trigger full re-scan of the ring buffer (max 1000 entries — instant). Match indices are feed entry positions, not visual line positions, so scroll-to-match requires entry-to-line mapping (deferred to future iteration). New messages during search auto-update the match list via `recompute_search_matches` in `process_bus_update`.

---

## ADR-16: Reconnect Alerts

**Decision:** Track pruned session IDs. When a previously pruned ID reappears, emit a green `[+] SESSION RECONNECTED` alert in the feed.

**Context:** A pruned session that reappears is a meaningful event — the session was dead and came back. This is actionable information (e.g., a crashed worker recovered). New sessions that were never pruned should NOT trigger this alert.

**Consequence:** `pruned_session_ids: HashSet<String>` on App. Prune adds to set, reconnect removes. The set grows unboundedly if sessions keep cycling, but in practice cc-dm session IDs are unique UUIDs and the set stays small.

---

## Architecture Overview

### Tech Stack

| Component | Crate | Purpose |
|-----------|-------|---------|
| Language | Rust | Ownership model, async concurrency, zero-cost abstractions |
| TUI framework | `ratatui` | Declarative terminal widgets, composable layouts |
| Terminal backend | `crossterm` | Cross-platform terminal control, raw mode, events |
| Async runtime | `tokio` | Event loop, task spawning, channel communication |
| SQLite | `rusqlite` | Read-only connection to bus.db, WAL-compatible |
| Filesystem events | `notify` | PollWatcher for directory change detection (100ms interval) |
| CLI parsing | `clap` | Argument parsing (`--project`) |

### Event Loop

```
PollWatcher polls ~/.cc-dm/ directory (100ms)
    │
    ▼ change detected
tokio task: filter event (bus-related files only)
    │
    ▼ bus change confirmed
rusqlite: read sessions table + pending messages
    │
    ▼ parse into typed events
diff against in-memory state
    │
    ├─ new sessions     → update roster
    ├─ updated sessions → update roster (heartbeat, status)
    ├─ removed sessions → prune alert → feed + remove from roster
    ├─ new messages     → append to ring buffer → feed
    └─ removed messages → mark delivered in buffer (no-op for display)
    │
    ▼
mpsc channel → render task
    │
    ▼
ratatui re-renders affected widgets
    │
    ▼
crossterm flushes diff to terminal
```

### Data Flow

```
cc-dm sessions ──write──▶ ~/.cc-dm/bus.db (WAL mode)
                                │
                    PollWatcher (100ms)
                                │
                                ▼
                        cc-dm-stream (read-only)
                                │
                    ┌───────────┼───────────┐
                    ▼           ▼           ▼
                 Roster      Feed      Status Bar
               (sessions)  (messages)  (stats)
```

### State Model

```rust
// Conceptual — not final API
struct AppState {
    project_filter: String,
    sessions: HashMap<String, SessionInfo>,  // keyed by session ID
    messages: RingBuffer<MessageEntry>,      // capacity 1000
    known_message_ids: HashSet<i64>,         // for diff detection
    stats: BusStats,
    scroll_position: ScrollState,
}
```
