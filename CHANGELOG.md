# Changelog

## 0.1.2 - 2026-10-28

- Collect SQLite query results with the standard iterator API, preserving row order
  and error propagation while removing five repeated loops.
- Cover malformed stored values across pinned, filtered, search and session queries.

## 0.1.1 - 2026-10-27

- Use the explicitly selected or discovered project workspace for MCP storage.
- Import Unicode bullet markers without panicking and report SQLite import/export failures.
- Return the existing entry ID when updating a memory across database connections.
- Report the package version during MCP initialization.
- Add workspace and persistence regressions, plus a runnable temporary-workspace example and precise Markdown import/export and client-configuration limits.
