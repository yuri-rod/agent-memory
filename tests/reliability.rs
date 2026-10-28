use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use agent_memory::db::MemoryDb;
use agent_memory::sync::{export_to_markdown, sync_from_markdown};
use rusqlite::Connection;
use tempfile::TempDir;

fn save_over_mcp(cwd: &Path, directory: Option<&Path>) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_agent-memory"));
    command.current_dir(cwd);
    if let Some(directory) = directory {
        command.arg("-C").arg(directory);
    }
    let mut child = command
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(
        b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"save_memory\",\"arguments\":{\"category\":\"decision\",\"key\":\"storage\",\"content\":\"Use SQLite\"}}}\n",
    ).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(response["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Saved memory"));
}

#[test]
fn mcp_uses_explicit_workspace() {
    let cwd = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    save_over_mcp(cwd.path(), Some(workspace.path()));

    let db = MemoryDb::open(workspace.path()).unwrap();
    assert_eq!(db.search("storage").unwrap().len(), 1);
    assert!(!cwd.path().join(".agent-memory").exists());
}

#[test]
fn mcp_uses_discovered_workspace_from_subdirectory() {
    let workspace = TempDir::new().unwrap();
    fs::create_dir(workspace.path().join(".git")).unwrap();
    let nested = workspace.path().join("src");
    fs::create_dir(&nested).unwrap();
    save_over_mcp(&nested, None);

    let db = MemoryDb::open(workspace.path()).unwrap();
    assert_eq!(db.search("storage").unwrap().len(), 1);
    assert!(!nested.join(".agent-memory").exists());
}

#[test]
fn markdown_import_accepts_unicode_bullets() {
    let workspace = TempDir::new().unwrap();
    fs::write(
        workspace.path().join("MEMORY.md"),
        "## Decisions\n• **café**: Utiliser SQLite\n",
    )
    .unwrap();
    let mut db = MemoryDb::open(workspace.path()).unwrap();

    assert_eq!(sync_from_markdown(workspace.path(), &mut db).unwrap(), 1);
    let items = db.search("café").unwrap();
    assert_eq!(items[0].content, "Utiliser SQLite");
}

#[test]
fn markdown_import_reports_database_write_failure() {
    let workspace = TempDir::new().unwrap();
    fs::write(
        workspace.path().join("MEMORY.md"),
        "- storage: Use SQLite\n",
    )
    .unwrap();
    let mut db = MemoryDb::open(workspace.path()).unwrap();
    let conn = Connection::open(&db.db_path).unwrap();
    conn.execute_batch("CREATE TRIGGER reject_write BEFORE INSERT ON memories BEGIN SELECT RAISE(FAIL, 'synthetic write failure'); END;").unwrap();

    let error = sync_from_markdown(workspace.path(), &mut db).unwrap_err();
    assert!(error.to_string().contains("synthetic write failure"));
    assert!(db.list_all(None).unwrap().is_empty());
}

#[test]
fn markdown_export_reports_database_read_failure_and_preserves_file() {
    let workspace = TempDir::new().unwrap();
    let memory_md = workspace.path().join("MEMORY.md");
    fs::write(&memory_md, "existing notes\n").unwrap();
    let db = MemoryDb::open(workspace.path()).unwrap();
    let conn = Connection::open(&db.db_path).unwrap();
    conn.execute_batch("DROP TABLE memories;").unwrap();

    assert!(export_to_markdown(workspace.path(), &db).is_err());
    assert_eq!(fs::read_to_string(memory_md).unwrap(), "existing notes\n");
}

#[test]
fn updating_a_memory_returns_its_original_id() {
    let workspace = TempDir::new().unwrap();
    let mut db = MemoryDb::open(workspace.path()).unwrap();
    let first = db
        .save_memory("decision", "storage", "SQLite", &[], true)
        .unwrap();
    let second = db
        .save_memory("convention", "tests", "cargo test", &[], true)
        .unwrap();
    assert_ne!(first, second);
    drop(db);

    let mut db = MemoryDb::open(workspace.path()).unwrap();
    let updated = db
        .save_memory("decision", "storage", "Bundled SQLite", &[], true)
        .unwrap();
    assert_eq!(updated, first);
    assert_eq!(db.list_all(None).unwrap().len(), 2);
    assert_eq!(db.search("storage").unwrap()[0].content, "Bundled SQLite");
}

#[test]
fn row_conversion_errors_are_not_silently_dropped() {
    let workspace = TempDir::new().unwrap();
    let mut db = MemoryDb::open(workspace.path()).unwrap();
    db.save_memory("decision", "broken", "synthetic entry", &[], true)
        .unwrap();
    db.record_session("test", "synthetic session", &[]).unwrap();

    let conn = Connection::open(&db.db_path).unwrap();
    conn.execute_batch(
        "UPDATE memories SET tags = x'80'; UPDATE sessions SET files_touched = x'80';",
    )
    .unwrap();

    assert!(db.get_pinned().is_err());
    assert!(db.list_all(None).is_err());
    assert!(db.list_all(Some("decision")).is_err());
    assert!(db.search("broken").is_err());
    assert!(db.get_recent_sessions(2).is_err());
}
