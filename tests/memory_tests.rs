use std::fs;
use tempfile::TempDir;

use agent_memory::db::MemoryDb;
use agent_memory::injector::generate_context_header;
use agent_memory::sync::{export_to_markdown, sync_from_markdown};

#[test]
fn test_save_and_recall_decisions() {
    let temp = TempDir::new().unwrap();
    let mut db = MemoryDb::open(temp.path()).unwrap();

    db.save_memory(
        "decision",
        "auth_strategy",
        "Use Ed25519 asymmetric JWT signatures",
        &["auth".to_string(), "security".to_string()],
        true,
    ).unwrap();

    db.save_memory(
        "convention",
        "commit_style",
        "Use pt-BR conventional commits",
        &["git".to_string()],
        true,
    ).unwrap();

    let recalled = db.search("Ed25519").unwrap();
    assert_eq!(recalled.len(), 1);
    assert_eq!(recalled[0].key, "auth_strategy");

    let all_conventions = db.list_all(Some("convention")).unwrap();
    assert_eq!(all_conventions.len(), 1);
    assert_eq!(all_conventions[0].key, "commit_style");
}

#[test]
fn test_pinned_context_header() {
    let temp = TempDir::new().unwrap();
    let mut db = MemoryDb::open(temp.path()).unwrap();

    db.save_memory("convention", "no_em_dash", "Never use Unicode code point U+2014 anywhere", &[], true).unwrap();
    db.save_memory("decision", "database", "Use embedded SQLite in WAL mode", &[], true).unwrap();
    db.save_memory("context", "draft_note", "Temporary unpinned scratch note", &[], false).unwrap();

    let header = generate_context_header(&db);
    assert!(header.contains("<agent_memory>"));
    assert!(header.contains("no_em_dash: Never use Unicode code point U+2014 anywhere"));
    assert!(header.contains("database: Use embedded SQLite in WAL mode"));
    assert!(!header.contains("draft_note"), "Unpinned items must not be in context header");
}

#[test]
fn test_markdown_sync() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path();

    let md_content = r#"# Project Memory
## Conventions
- **no_em_dash**: Never use Unicode code point U+2014 anywhere.
- **commit_style**: Use pt-BR conventional commits.

## Architectural Decisions
- **db_engine**: SQLite via bundled rusqlite.
"#;
    fs::write(workspace.join("MEMORY.md"), md_content).unwrap();

    let mut db = MemoryDb::open(workspace).unwrap();
    let count = sync_from_markdown(workspace, &mut db).unwrap();
    assert_eq!(count, 3);

    let items = db.list_all(None).unwrap();
    assert_eq!(items.len(), 3);

    db.save_memory("gotcha", "macos_launchd", "Avoid KeepAlive without error check", &[], true).unwrap();
    export_to_markdown(workspace, &db).unwrap();

    let updated_md = fs::read_to_string(workspace.join("MEMORY.md")).unwrap();
    assert!(updated_md.contains("macos_launchd"));
    assert!(updated_md.contains("db_engine"));
}
