# agent-memory

[![CI](https://github.com/yuri-rod/agent-memory/actions/workflows/ci.yml/badge.svg)](https://github.com/yuri-rod/agent-memory/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/yuri-rod/agent-memory?color=blue)](https://github.com/yuri-rod/agent-memory/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

**Universal, local-first cross-session memory and architectural decision ledger for coding agents.**

`agent-memory` eliminates cross-session amnesia in tools like Codex, Claude Code, Google Antigravity (AGY), Aider, and Cursor. It preserves architectural decisions, project conventions, and platform gotchas in a fast local SQLite ledger, automatically providing a dense, <150 token context header at the start of every new session.

```bash
# Remember architectural decisions and conventions
$ agent-memory remember decision auth "Use Ed25519 asymmetric JWT signatures"
$ agent-memory remember convention commit_style "Follow pt-BR conventional commits"
$ agent-memory remember gotcha macos_launchd "KeepAlive crash loop if script exits 0"

# Search and recall instantly
$ agent-memory recall auth
$ agent-memory list

# Print dense context header for LLM injection
$ agent-memory header

# Sync two-way with local repository MEMORY.md
$ agent-memory sync

# Auto-configure across AGY, Claude Code, and Codex
$ agent-memory install
```

---

## Why Agent Memory?

1. **Stops repetitive questioning**: Coding agents will no longer ask you the same architectural trade-offs across terminal sessions.
2. **Strict rule enforcement**: Pinned project conventions (like formatting rules, banned syntax, or commit conventions) are always present in the context header.
3. **Zero cloud or Docker dependencies**: Single compiled Rust binary with bundled SQLite. Zero external servers or Python environments required.
4. **Markdown two-way sync**: Edit `MEMORY.md` by hand or let your agent record decisions via MCP, both stay in sync.
5. **Universal auto-loader**: Integrates natively via Model Context Protocol (MCP) with Claude Code, Codex, and Google Antigravity.

---

## Injected Context Header Format

When an agent starts a session, `agent-memory` supplies an ultra-compact XML context header:

```xml
<agent_memory>
[Pinned Project Conventions]
• commit_style: Follow pt-BR conventional commits
• no_em_dash: Never use Unicode code point U+2014 anywhere

[Architectural Decisions]
• auth: Use Ed25519 asymmetric JWT signatures
• database: Embedded SQLite in WAL mode

[Platform Gotchas & Lessons]
• macos_launchd: KeepAlive crash loop if script exits 0

[Recent Session Context]
• [claude] Implemented JWT middleware and auth tests (Files: src/auth.rs, tests/auth_test.rs)
</agent_memory>
```

---

## CLI Command Reference

| Command | Description | Example |
| :--- | :--- | :--- |
| `remember` | Store a decision, rule, or gotcha | `agent-memory remember decision db "SQLite WAL"` |
| `recall` | Search memories across keys, content, and tags | `agent-memory recall auth` |
| `list` | List all memories in workspace | `agent-memory list` |
| `header` | Output compact context header | `agent-memory header` |
| `forget` | Delete a memory by key or ID | `agent-memory forget auth` |
| `sync` | Two-way sync with workspace `MEMORY.md` | `agent-memory sync` |
| `mcp` | Start Model Context Protocol stdio server | `agent-memory mcp` |
| `install` | Auto-configure AGY, Claude Code, and Codex | `agent-memory install` |

---

## Installation

### Via Cargo

```bash
cargo install --git https://github.com/yuri-rod/agent-memory.git
```

### From Source

```bash
git clone https://github.com/yuri-rod/agent-memory.git
cd agent-memory
cargo build --release
cp target/release/agent-memory /usr/local/bin/
agent-memory install
```

---

## License

MIT License.
