# cc-dm-stream — Product Requirements Document

**Date:** 2026-03-28
**Author:** Akram (visionary), Claude (architect)
**Version:** 0.1.0
**Status:** Draft

---

## What It Does

cc-dm-stream is a read-only terminal UI that streams live events from the cc-dm SQLite bus. It renders session registrations, direct messages, broadcasts, and stale session pruning as they happen — with zero polling delay.

One pane in your WezTerm workspace. Always open. The team's pulse, always visible.

## The Problem

Running a multi-session Claude Code sprint with cc-dm today:

You dispatch tasks. Agents start. Messages flow across the bus. Then silence on your end. You have no visibility into whether a session is healthy, whether a message was delivered, whether a worker finished or is stuck in a loop, whether the bus itself is behaving. You find out by manually switching into each pane and reading terminal output one session at a time.

cc-dm solved the coordination problem. cc-dm-stream solves the visibility problem.

## Target User

Developers running multi-session Claude Code workflows using cc-dm. Comfortable with terminal tools. Running WezTerm or equivalent multiplexer with multiple panes.

---

## Functional Requirements

### FR-1: Project Scoping

On launch, cc-dm-stream presents a welcome screen listing all active projects on the cc-dm bus. The user selects one project to observe. The TUI then launches filtered to that project scope.

- Always show the project picker, even for a single project
- CLI shortcut: `cc-dm-stream --project <name>` skips the welcome screen
- Sessions with empty project tags are visible only in "all projects" view
- If no active sessions exist, display a clean waiting state

### FR-2: Session Roster

Left panel displays all registered sessions for the selected project.

Per session:
- Session name (display name)
- Role
- Last heartbeat timestamp
- Time since last heartbeat (computed live)
- Status indicator:
  - Green: active (heartbeat within 45s)
  - Amber: approaching stale (45–60s since heartbeat)
  - Red: stale (>60s since heartbeat)

Roster updates silently on every bus event. Heartbeats are not rendered in the message feed.

### FR-3: Message Feed

Right panel displays a scrolling live feed of messages.

Per message:
- Timestamp
- Sender name (`from_session` — already a display name in cc-dm)
- Recipient session ID (`to_session`)
- Message content
- Priority color coding (urgent = hot accent in Tokyo Night palette)

Behaviour:
- New messages append at bottom
- Auto-scrolls to bottom when new messages arrive
- Mouse scroll and arrow keys for scroll-back
- Space key jumps to end (live tail)
- Auto-scroll pauses when user scrolls up, resumes on Space or manual scroll to bottom
- Messages are raw — every bus row is one feed entry, no broadcast grouping
- 1000-message ring buffer in memory

### FR-4: Prune Alerts

When a session disappears from the bus (pruned after 60s inactivity), an alert notification appears in the message feed:

```
[!] SESSION PRUNED: stream-engineer (worker) — last seen 62s ago
```

Rendered in amber/red, visually distinct from normal messages. Serves as a call-to-action for the user to re-register the stale session if unintentional.

### FR-5: Status Bar

Bottom edge displays:
- Bus connection status (connected / waiting)
- Bus file path
- Active session count
- Total messages observed (since launch)
- Keybinding hints (q: quit, Space: jump to end, arrows: scroll)

Updated on every bus event.

### FR-6: Graceful State Handling

| State | Behaviour |
|-------|-----------|
| Bus not found | Clean waiting screen: "Waiting for cc-dm bus at ~/.cc-dm/bus.db..." |
| Bus found, no sessions | Welcome screen with empty project list: "No active sessions. Waiting..." |
| Bus found, sessions active | Welcome screen → project picker → TUI |
| All sessions die mid-stream | Roster empties, prune alerts fire, TUI stays alive |
| Bus file deleted mid-stream | TUI transitions to waiting state |

### FR-7: Exit

`q` or `Ctrl+C` exits cleanly. No cleanup required — cc-dm-stream holds no state on the bus.

---

## Non-Functional Requirements

### NFR-1: Read-Only Invariant

cc-dm-stream never writes to the bus. Never registers as a session. Never sends a heartbeat. Never modifies any table or row. If cc-dm-stream crashes, the team keeps running. The bus does not know cc-dm-stream exists.

SQLite connection opened in read-only mode.

### NFR-2: Event-Driven, Not Polling

Filesystem event notification (`kqueue` on macOS, `inotify` on Linux) watches the `~/.cc-dm/` directory. A write to the bus triggers an immediate read and re-render. Target latency: sub-10ms from bus write to screen update.

### NFR-3: Zero Configuration

If cc-dm is installed and `~/.cc-dm/bus.db` exists, cc-dm-stream works on launch. No config file, no API keys, no setup step.

### NFR-4: Startup Time

Under 1 second from command to first render.

### NFR-5: Memory

1000-message ring buffer. Session roster in memory (bounded by active session count, practically <20). No unbounded growth.

---

## UI Specification

### Layout

```
┌──────────────┬────────────────────────────────────────┐
│   Roster     │          Message Feed                  │
│   (fixed     │          (flex width)                  │
│    width)    │                                        │
│              │                                        │
│  SESSION 1   │  12:01:03 architect → session-a1b2     │
│  ● active    │  Deploy the API changes to staging     │
│  last: 3s    │                                        │
│              │  12:01:05 planner → session-c3d4       │
│  SESSION 2   │  Review PR #42 when ready              │
│  ● active    │                                        │
│  last: 12s   │  12:01:47 worker → session-a1b2       │
│              │  Done. Tests passing.                  │
│  SESSION 3   │                                        │
│  ◌ stale     │  [!] SESSION PRUNED: tester (worker)   │
│  last: 65s   │      — last seen 62s ago               │
│              │                                        │
├──────────────┴────────────────────────────────────────┤
│ ● bus: connected | ~/.cc-dm/bus.db | sessions: 3      │
│ msgs: 47 | q: quit  Space: latest  ↑↓: scroll         │
└───────────────────────────────────────────────────────┘
```

### Welcome Screen

```
┌─────────────────────────────────────────┐
│          cc-dm-stream v0.1.0            │
│                                         │
│  Active projects on bus:                │
│                                         │
│  > cc-dm-stream  (3 sessions)           │
│    api-backend   (2 sessions)           │
│    [all projects]                       │
│                                         │
│  ↑↓: select  Enter: confirm  q: quit   │
│                                         │
└─────────────────────────────────────────┘
```

### Theme

Tokyo Night — hardcoded. Dark background, muted syntax colours, distinct accent colours for status indicators.

| Element | Colour |
|---------|--------|
| Background | #1a1b26 |
| Foreground / text | #a9b1d6 |
| Active session indicator | #9ece6a (green) |
| Approaching stale indicator | #e0af68 (amber) |
| Stale session indicator | #f7768e (red) |
| Urgent priority message | #f7768e (red) |
| Prune alert | #e0af68 (amber) |
| Sender name | #7aa2f7 (blue) |
| Timestamp | #565f89 (muted) |
| Status bar background | #16161e |
| Selected item (welcome screen) | #7aa2f7 (blue) |

---

## Out of Scope

- Sending messages (not a cc-dm client)
- Session management (not a process manager)
- Modifying cc-dm source code or schema
- Configuration files or theming
- Web UI, Electron, or browser-based rendering
- Broadcast grouping or message aggregation
- Historical message replay (launch-forward only)
- Permission relay rendering (YAGNI for v1)

---

## Distribution

- `cargo install cc-dm-stream` for Rust developers
- Pre-built binary for macOS Apple Silicon via GitHub Releases
- No Rust toolchain required for binary users

---

## Success Criteria

1. Launch → project picker → TUI in under 1 second
2. Message appears on screen within 10ms of bus write
3. Session prune detected and alerted within one filesystem event cycle
4. Zero writes to the bus under any circumstance
5. Clean exit on q/Ctrl+C with no resource leaks
6. Runs indefinitely without memory growth beyond the ring buffer
