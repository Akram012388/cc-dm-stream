# cc-dm-stream

Live streaming TUI for the [cc-dm](https://github.com/Akram012388/cc-dm) coordination bus.

Your agents are talking. cc-dm-stream lets you watch.

## What It Does

A Rust-native terminal UI that watches the cc-dm SQLite bus in real time and renders every event — session registrations, direct messages, broadcasts, stale session pruning — as it happens, with zero polling delay.

```
┌──────────────┬────────────────────────────────────────┐
│   Roster     │          Message Feed                  │
│              │                                        │
│  planner     │  12:01:03 architect → session-a1b2     │
│  ● active    │  Deploy the API changes to staging     │
│  last: 3s    │                                        │
│              │  12:01:05 planner → session-c3d4       │
│  worker      │  Review PR #42 when ready              │
│  ● active    │                                        │
│  last: 12s   │  12:01:47 worker → session-a1b2       │
│              │  Done. Tests passing.                  │
│  tester      │                                        │
│  ◌ stale     │  [!] SESSION PRUNED: tester (worker)   │
│  last: 65s   │      — last seen 62s ago               │
│              │                                        │
├──────────────┴────────────────────────────────────────┤
│ ● connected | ~/.cc-dm/bus.db | sessions: 3 | msgs: 47│
└───────────────────────────────────────────────────────┘
```

## Requirements

- [cc-dm](https://github.com/Akram012388/cc-dm) installed and running
- macOS or Linux

## Install

```bash
# Shell installer (macOS / Linux)
curl -sSL https://raw.githubusercontent.com/Akram012388/cc-dm-stream/main/install.sh | sh

# Cargo
cargo install cc-dm-stream

# npm
npm install -g cc-dm-stream

# Homebrew
brew tap Akram012388/cc-dm-stream && brew install cc-dm-stream
```

Or download a pre-built binary from [Releases](https://github.com/Akram012388/cc-dm-stream/releases).

## Usage

```bash
# Launch with project picker
cc-dm-stream

# Skip picker, go straight to a project
cc-dm-stream --project my-project
```

## How It Works

cc-dm-stream is a **read-only observer**. It never writes to the bus, never registers as a session, never sends a heartbeat. The bus does not know it exists.

It watches the `~/.cc-dm/` directory for filesystem events (`kqueue` on macOS, `inotify` on Linux). When cc-dm writes to the bus, cc-dm-stream reads the change and renders it — sub-10ms from bus write to screen update.

## Relationship to cc-dm

```
cc-dm          ← protocol and bus (TypeScript, SQLite)
cc-dm-stream   ← live streaming TUI (Rust, read-only observer)
```

cc-dm-stream is an observer in the ecosystem, not a participant.

## Part of the cc-dm ecosystem

| Project | Description |
|---------|-------------|
| [cc-dm](https://github.com/Akram012388/cc-dm) | Peer-to-peer coordination protocol for Claude Code sessions |
| [cc-dm-stream](https://github.com/Akram012388/cc-dm-stream) | Live streaming TUI observer (this repo) |

## Author

**Akram Ahmed** — [@CodeAkram](https://x.com/CodeAkram)

## License

MIT
