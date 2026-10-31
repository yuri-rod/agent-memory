use chrono::Utc;
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_KEY_BYTES: usize = 256;
pub const MAX_CONTENT_BYTES: usize = 64 * 1024; // 64 KiB
pub const MAX_CATEGORY_BYTES: usize = 64;
pub const MAX_TAG_BYTES: usize = 64;
pub const MAX_TAGS_COUNT: usize = 20;

pub const MAX_AGENT_NAME_BYTES: usize = 128;
pub const MAX_SUMMARY_BYTES: usize = 4096;
pub const MAX_FILES_TOUCHED_COUNT: usize = 50;
pub const MAX_FILE_PATH_BYTES: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: i64,
    pub category: String,
    pub key: String,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub pinned: bool,
    #[serde(default = "default_provenance")]
    pub provenance: String,
}

fn default_provenance() -> String {
    "manual".to_string()
}

pub fn validate_memory_input(key: &str, content: &str) -> std::result::Result<(), String> {
    validate_memory_full_input("decision", key, content, &[])
}

pub fn validate_memory_full_input(
    category: &str,
    key: &str,
    content: &str,
    tags: &[String],
) -> std::result::Result<(), String> {
    let cat_trimmed = category.trim();
    if cat_trimmed.is_empty() || category.len() > MAX_CATEGORY_BYTES {
        return Err(format!(
            "Memory category cannot be empty and must not exceed {} bytes",
            MAX_CATEGORY_BYTES
        ));
    }
    if category.chars().any(|c| c == '\n' || c == '\r' || c.is_control()) {
        return Err("Memory category contains invalid control characters".to_string());
    }

    let key_trimmed = key.trim();
    if key_trimmed.is_empty() {
        return Err("Memory key cannot be empty or whitespace only".to_string());
    }
    if key.len() > MAX_KEY_BYTES {
        return Err(format!(
            "Memory key exceeds maximum size of {} bytes (got {} bytes)",
            MAX_KEY_BYTES,
            key.len()
        ));
    }
    if key.chars().any(|c| c == '\n' || c == '\r' || c.is_control()) {
        return Err("Memory key contains invalid newline or control characters".to_string());
    }
    if content.len() > MAX_CONTENT_BYTES {
        return Err(format!(
            "Memory content exceeds maximum size of {} bytes (got {} bytes)",
            MAX_CONTENT_BYTES,
            content.len()
        ));
    }
    if tags.len() > MAX_TAGS_COUNT {
        return Err(format!("Tags count exceeds maximum of {}", MAX_TAGS_COUNT));
    }
    for tag in tags {
        if tag.len() > MAX_TAG_BYTES || tag.chars().any(|c| c == '\n' || c == '\r' || c.is_control()) {
            return Err("Tag exceeds maximum size of 64 bytes or contains control characters".to_string());
        }
    }
    Ok(())
}

pub fn validate_session_input(
    agent_name: &str,
    summary: &str,
    files: &[String],
) -> std::result::Result<(), String> {
    let name_trimmed = agent_name.trim();
    if name_trimmed.is_empty() {
        return Err("Agent name cannot be empty".to_string());
    }
    if agent_name.len() > MAX_AGENT_NAME_BYTES {
        return Err(format!(
            "Agent name exceeds maximum size of {} bytes",
            MAX_AGENT_NAME_BYTES
        ));
    }
    if agent_name.chars().any(|c| c == '\n' || c == '\r' || c.is_control()) {
        return Err("Agent name contains invalid control characters".to_string());
    }

    let summary_trimmed = summary.trim();
    if summary_trimmed.is_empty() {
        return Err("Session summary cannot be empty".to_string());
    }
    if summary.len() > MAX_SUMMARY_BYTES {
        return Err(format!(
            "Session summary exceeds maximum size of {} bytes",
            MAX_SUMMARY_BYTES
        ));
    }

    if files.len() > MAX_FILES_TOUCHED_COUNT {
        return Err(format!(
            "Files touched count exceeds maximum of {}",
            MAX_FILES_TOUCHED_COUNT
        ));
    }
    for file in files {
        if file.len() > MAX_FILE_PATH_BYTES {
            return Err(format!(
                "File path exceeds maximum size of {} bytes",
                MAX_FILE_PATH_BYTES
            ));
        }
        if file.chars().any(|c| c == '\n' || c == '\r' || c.is_control()) {
            return Err("File path contains invalid control characters".to_string());
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionItem {
    pub id: i64,
    pub agent_name: String,
    pub summary: String,
    pub files_touched: Vec<String>,
    pub timestamp: String,
}

pub struct MemoryDb {
    conn: Connection,
    pub db_path: PathBuf,
}

impl MemoryDb {
    pub fn open(workspace: &Path) -> Result<Self> {
        let mem_dir = workspace.join(".agent-memory");
        if !mem_dir.exists() {
            let _ = fs::create_dir_all(&mem_dir);
        }

        let db_path = mem_dir.join("memory.sqlite");
        let conn = Connection::open(&db_path)?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "NORMAL");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                category TEXT NOT NULL,
                key TEXT NOT NULL UNIQUE,
                content TEXT NOT NULL,
                tags TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                pinned INTEGER NOT NULL DEFAULT 0,
                provenance TEXT NOT NULL DEFAULT 'manual'
            )",
            [],
        )?;

        // Ensure backward compatibility by applying schema migration if needed
        let _ = conn.execute(
            "ALTER TABLE memories ADD COLUMN provenance TEXT NOT NULL DEFAULT 'manual'",
            [],
        );

        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_name TEXT NOT NULL,
                summary TEXT NOT NULL,
                files_touched TEXT NOT NULL,
                timestamp TEXT NOT NULL
            )",
            [],
        )?;

        Ok(Self { conn, db_path })
    }

    pub fn save_memory(
        &mut self,
        category: &str,
        key: &str,
        content: &str,
        tags: &[String],
        pinned: bool,
    ) -> Result<i64> {
        self.save_memory_with_provenance(category, key, content, tags, pinned, "manual")
    }

    pub fn save_memory_with_provenance(
        &mut self,
        category: &str,
        key: &str,
        content: &str,
        tags: &[String],
        pinned: bool,
        provenance: &str,
    ) -> Result<i64> {
        if let Err(e) = validate_memory_full_input(category, key, content, tags) {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                e,
            ))));
        }

        let now = Utc::now().to_rfc3339();
        let tags_str = tags.join(",");
        let pinned_int = if pinned { 1 } else { 0 };

        self.conn.query_row(
            "INSERT INTO memories (category, key, content, tags, created_at, updated_at, pinned, provenance)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7)
             ON CONFLICT(key) DO UPDATE SET
                category = excluded.category,
                content = excluded.content,
                tags = excluded.tags,
                updated_at = excluded.updated_at,
                pinned = excluded.pinned,
                provenance = excluded.provenance
             RETURNING id",
            params![category, key, content, tags_str, now, pinned_int, provenance],
            |row| row.get(0),
        )
    }

    pub fn get_pinned(&self) -> Result<Vec<MemoryItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, category, key, content, tags, created_at, updated_at, pinned, provenance
             FROM memories WHERE pinned = 1 ORDER BY category, key",
        )?;

        let rows = stmt.query_map([], map_row_to_memory)?;
        rows.collect()
    }

    pub fn list_all(&self, category_filter: Option<&str>) -> Result<Vec<MemoryItem>> {
        if let Some(cat) = category_filter {
            let mut stmt = self.conn.prepare(
                "SELECT id, category, key, content, tags, created_at, updated_at, pinned, provenance FROM memories WHERE category = ?1 ORDER BY pinned DESC, updated_at DESC",
            )?;
            let rows = stmt.query_map(params![cat], map_row_to_memory)?;
            rows.collect()
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT id, category, key, content, tags, created_at, updated_at, pinned, provenance FROM memories ORDER BY pinned DESC, category, updated_at DESC",
            )?;
            let rows = stmt.query_map([], map_row_to_memory)?;
            rows.collect()
        }
    }

    pub fn search(&self, query_str: &str) -> Result<Vec<MemoryItem>> {
        let pattern = format!("%{}%", query_str);
        let mut stmt = self.conn.prepare(
            "SELECT id, category, key, content, tags, created_at, updated_at, pinned, provenance
             FROM memories
             WHERE key LIKE ?1 OR content LIKE ?1 OR tags LIKE ?1
             ORDER BY pinned DESC, updated_at DESC",
        )?;

        let rows = stmt.query_map(params![pattern], map_row_to_memory)?;
        rows.collect()
    }

    pub fn delete_memory(&mut self, id_or_key: &str) -> Result<usize> {
        if let Ok(id) = id_or_key.parse::<i64>() {
            self.conn
                .execute("DELETE FROM memories WHERE id = ?1", params![id])
        } else {
            self.conn
                .execute("DELETE FROM memories WHERE key = ?1", params![id_or_key])
        }
    }

    pub fn record_session(
        &mut self,
        agent_name: &str,
        summary: &str,
        files: &[String],
    ) -> Result<i64> {
        if let Err(e) = validate_session_input(agent_name, summary, files) {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                e,
            ))));
        }

        let now = Utc::now().to_rfc3339();
        let files_str = files.join(",");

        self.conn.execute(
            "INSERT INTO sessions (agent_name, summary, files_touched, timestamp)
             VALUES (?1, ?2, ?3, ?4)",
            params![agent_name, summary, files_str, now],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_recent_sessions(&self, limit: usize) -> Result<Vec<SessionItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent_name, summary, files_touched, timestamp
             FROM sessions ORDER BY id DESC LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let files_str: String = row.get(3)?;
            let files = if files_str.is_empty() {
                Vec::new()
            } else {
                files_str.split(',').map(|s| s.trim().to_string()).collect()
            };

            Ok(SessionItem {
                id: row.get(0)?,
                agent_name: row.get(1)?,
                summary: row.get(2)?,
                files_touched: files,
                timestamp: row.get(4)?,
            })
        })?;

        rows.collect()
    }
}

fn map_row_to_memory(row: &rusqlite::Row) -> Result<MemoryItem> {
    let tags_str: String = row.get(4)?;
    let tags = if tags_str.is_empty() {
        Vec::new()
    } else {
        tags_str.split(',').map(|s| s.trim().to_string()).collect()
    };

    let provenance = row
        .get::<_, Option<String>>(8)?
        .unwrap_or_else(|| "manual".to_string());

    Ok(MemoryItem {
        id: row.get(0)?,
        category: row.get(1)?,
        key: row.get(2)?,
        content: row.get(3)?,
        tags,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        pinned: row.get::<_, i32>(7)? == 1,
        provenance,
    })
}
