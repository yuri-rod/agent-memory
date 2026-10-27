use chrono::Utc;
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

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

        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                category TEXT NOT NULL,
                key TEXT NOT NULL UNIQUE,
                content TEXT NOT NULL,
                tags TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                pinned INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;

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
        let now = Utc::now().to_rfc3339();
        let tags_str = tags.join(",");
        let pinned_int = if pinned { 1 } else { 0 };

        self.conn.query_row(
            "INSERT INTO memories (category, key, content, tags, created_at, updated_at, pinned)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)
             ON CONFLICT(key) DO UPDATE SET
                category = excluded.category,
                content = excluded.content,
                tags = excluded.tags,
                updated_at = excluded.updated_at,
                pinned = excluded.pinned
             RETURNING id",
            params![category, key, content, tags_str, now, pinned_int],
            |row| row.get(0),
        )
    }

    pub fn get_pinned(&self) -> Result<Vec<MemoryItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, category, key, content, tags, created_at, updated_at, pinned
             FROM memories WHERE pinned = 1 ORDER BY category, key",
        )?;

        let rows = stmt.query_map([], map_row_to_memory)?;
        let mut items = Vec::new();
        for r in rows {
            items.push(r?);
        }
        Ok(items)
    }

    pub fn list_all(&self, category_filter: Option<&str>) -> Result<Vec<MemoryItem>> {
        if let Some(cat) = category_filter {
            let mut stmt = self.conn.prepare(
                "SELECT id, category, key, content, tags, created_at, updated_at, pinned FROM memories WHERE category = ?1 ORDER BY pinned DESC, updated_at DESC",
            )?;
            let rows = stmt.query_map(params![cat], map_row_to_memory)?;
            let mut items = Vec::new();
            for r in rows {
                items.push(r?);
            }
            Ok(items)
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT id, category, key, content, tags, created_at, updated_at, pinned FROM memories ORDER BY pinned DESC, category, updated_at DESC",
            )?;
            let rows = stmt.query_map([], map_row_to_memory)?;
            let mut items = Vec::new();
            for r in rows {
                items.push(r?);
            }
            Ok(items)
        }
    }

    pub fn search(&self, query_str: &str) -> Result<Vec<MemoryItem>> {
        let pattern = format!("%{}%", query_str);
        let mut stmt = self.conn.prepare(
            "SELECT id, category, key, content, tags, created_at, updated_at, pinned
             FROM memories
             WHERE key LIKE ?1 OR content LIKE ?1 OR tags LIKE ?1
             ORDER BY pinned DESC, updated_at DESC",
        )?;

        let rows = stmt.query_map(params![pattern], map_row_to_memory)?;
        let mut items = Vec::new();
        for r in rows {
            items.push(r?);
        }
        Ok(items)
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

        let mut items = Vec::new();
        for r in rows {
            items.push(r?);
        }
        Ok(items)
    }
}

fn map_row_to_memory(row: &rusqlite::Row) -> Result<MemoryItem> {
    let tags_str: String = row.get(4)?;
    let tags = if tags_str.is_empty() {
        Vec::new()
    } else {
        tags_str.split(',').map(|s| s.trim().to_string()).collect()
    };

    Ok(MemoryItem {
        id: row.get(0)?,
        category: row.get(1)?,
        key: row.get(2)?,
        content: row.get(3)?,
        tags,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        pinned: row.get::<_, i32>(7)? == 1,
    })
}
