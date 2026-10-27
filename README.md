# agent-memory

A local SQLite ledger for project decisions, conventions and session summaries that coding agents can retrieve through a CLI or MCP.

Use it to carry a decision such as a database choice into the next coding session. Entries belong to a workspace, have a unique key, and can be pinned into a context header. Your agent must request that header or receive it in its prompt; the tool does not enforce instructions or automatically load context into every client.

## Try it with a temporary workspace

Build with the stable Rust toolchain and a C compiler for bundled SQLite:

```bash
git clone https://github.com/yuri-rod/agent-memory.git
cd agent-memory
cargo build --locked --release

demo_workspace=$(mktemp -d)
./target/release/agent-memory -C "$demo_workspace" remember decision storage "Use bundled SQLite"
./target/release/agent-memory -C "$demo_workspace" remember convention tests "Run cargo test --locked"
./target/release/agent-memory -C "$demo_workspace" header
```

The header command prints:

```text
<agent_memory>
[Pinned Project Conventions]
• tests: Run cargo test --locked

[Architectural Decisions]
• storage: Use bundled SQLite

</agent_memory>
```

Search the same workspace and export a readable copy:

```bash
./target/release/agent-memory -C "$demo_workspace" recall storage
./target/release/agent-memory -C "$demo_workspace" sync --export
cat "$demo_workspace/MEMORY.md"
```

To install the binary on your Cargo executable path:

```bash
cargo install --path . --locked
```

## Workspace and entry behavior

`-C /absolute/path/to/project` selects the workspace for memory commands, including `mcp`. Without it, the CLI searches the current directory and its parents for `.git`, `.agent-memory` or `MEMORY.md`, falling back to the current directory. The database lives at `.agent-memory/memory.sqlite`.

- `remember CATEGORY KEY CONTENT` inserts or updates the entry with that key. Entries are pinned by default; add `--unpinned` for notes that should stay out of the header. `--tags storage,local` adds searchable tags.
- `recall QUERY` searches keys, content and tags using SQLite `LIKE`. `%` and `_` act as wildcards. `list --category decision` filters entries by category. The category option on `recall` applies when no query is supplied.
- `header` includes pinned entries grouped by category and the two most recent recorded session summaries. Its size depends on your saved text; there is no token budget.
- `forget KEY_OR_ID` deletes one entry. Numeric arguments are interpreted as IDs, so use nonnumeric keys.

Entries and summaries are ordinary local text stored in SQLite. Keep `.agent-memory/` out of version control when it contains private notes. Review exported Markdown before committing it.

## Markdown import and export

`agent-memory -C "$demo_workspace" sync` imports supported bullet lines from `MEMORY.md`. Recognized section headings select categories, and each `key: value` bullet updates that key. For example:

```markdown
## Architectural decisions
- **storage**: Use bundled SQLite

## Conventions
• **tests**: Run cargo test --locked
```

Import pins entries and clears their tags. It does not delete database entries omitted from the file. A database error stops import and returns an error; entries imported before that error remain saved.

`sync --export` replaces `MEMORY.md` with a grouped view of all entries, including unpinned ones. It does not preserve handwritten prose, tags, pin state, timestamps or session summaries. It writes directly to the destination, so keep a separate copy of important handwritten notes before exporting. This is a text import/export format, not a lossless database backup.

## MCP integration

Configure a local stdio server in your client with the absolute binary path and an explicit workspace. For clients using a `mcpServers` JSON object:

```json
{
  "mcpServers": {
    "agent-memory": {
      "command": "/absolute/path/to/agent-memory",
      "args": ["-C", "/absolute/path/to/project", "mcp"]
    }
  }
}
```

The server exposes `save_memory`, `recall_memory`, `get_context_header` and `record_session_summary`. Ask the client to call `get_context_header` when beginning work, and save only reviewed decisions or summaries. The server does not scan agent conversation files or use a hosted inference service. It assumes a trusted local client, has no request-size limit and does not enforce every declared argument constraint. Saved text appears in the header without XML escaping.

The `install` helper uses a fixed executable path and edits existing configuration files in your home directory. Use explicit client configuration for other installations. Client-specific startup hooks and automatic context loading are not implemented.

## Implementation and checks

- [SQLite storage](src/db.rs) uses parameterized queries, unique memory keys and upserts that return the stored entry ID. SQLite is bundled with the binary; there is no database service to deploy.
- [Context generation](src/injector.rs) groups pinned entries and recent session summaries into a prompt header.
- [Markdown conversion](src/sync.rs) and [MCP transport](src/mcp.rs) share the same storage implementation.
- [Core tests](tests/memory_tests.rs) cover storage, pinned headers and Markdown conversion. [Regression tests](tests/reliability.rs) exercise explicit and discovered MCP workspaces, Unicode import, SQLite read/write failures and updates across database connections, using temporary directories.

```bash
cargo check --locked
cargo test --locked
cargo build --locked --release
```

[CI](.github/workflows/ci.yml) checks and tests on Linux and macOS. Additional local diagnostics are `cargo fmt --all -- --check` and `cargo clippy --locked --all-targets -- -D warnings`; CI does not currently enforce them.

To contribute a fix, include a reproducible case and a regression test using synthetic data. Keep database files and real conversation content out of fixtures.

## License

MIT License.
