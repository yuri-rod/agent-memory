use crate::db::MemoryDb;
use std::fs;
use std::io;
use std::path::Path;

pub fn sync_from_markdown(workspace: &Path, db: &mut MemoryDb) -> io::Result<usize> {
    let memory_md = workspace.join("MEMORY.md");
    if !memory_md.is_file() {
        return Ok(0);
    }

    let content = fs::read_to_string(&memory_md)?;
    let mut count = 0;
    let mut current_category = "context".to_string();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('#') {
            let header = trimmed.trim_start_matches('#').trim().to_lowercase();
            if header.contains("convention") || header.contains("rule") {
                current_category = "convention".to_string();
            } else if header.contains("decision") || header.contains("architecture") {
                current_category = "decision".to_string();
            } else if header.contains("gotcha") || header.contains("lesson") {
                current_category = "gotcha".to_string();
            } else if header.contains("focus") {
                current_category = "focus".to_string();
            }
            continue;
        }

        if let Some(item_text) = trimmed.strip_prefix(['-', '*', '•']) {
            let item_text = item_text.trim();
            if item_text.is_empty() {
                continue;
            }

            let (key, val) = if let Some(colon_pos) = item_text.find(':') {
                let k = item_text[..colon_pos]
                    .trim()
                    .replace("**", "")
                    .replace('`', "");
                let v = item_text[colon_pos + 1..].trim().to_string();
                (k, v)
            } else {
                let k = format!("note_{}", count + 1);
                (k, item_text.to_string())
            };

            db.save_memory(&current_category, &key, &val, &[], true)
                .map_err(io::Error::other)?;
            count += 1;
        }
    }

    Ok(count)
}

pub fn export_to_markdown(workspace: &Path, db: &MemoryDb) -> io::Result<()> {
    let items = db.list_all(None).map_err(io::Error::other)?;

    let mut md = String::from("# Project Memory Ledger\n\n");

    let mut conventions = Vec::new();
    let mut decisions = Vec::new();
    let mut gotchas = Vec::new();
    let mut other = Vec::new();

    for item in items {
        match item.category.to_lowercase().as_str() {
            "convention" => conventions.push(item),
            "decision" => decisions.push(item),
            "gotcha" => gotchas.push(item),
            _ => other.push(item),
        }
    }

    if !conventions.is_empty() {
        md.push_str("## Conventions & Rules\n");
        for c in conventions {
            md.push_str(&format!("- **{}**: {}\n", c.key, c.content));
        }
        md.push('\n');
    }

    if !decisions.is_empty() {
        md.push_str("## Architectural Decisions\n");
        for d in decisions {
            md.push_str(&format!("- **{}**: {}\n", d.key, d.content));
        }
        md.push('\n');
    }

    if !gotchas.is_empty() {
        md.push_str("## Gotchas & Platform Quirks\n");
        for g in gotchas {
            md.push_str(&format!("- **{}**: {}\n", g.key, g.content));
        }
        md.push('\n');
    }

    if !other.is_empty() {
        md.push_str("## Active Notes & Focus\n");
        for o in other {
            md.push_str(&format!("- **{}**: {}\n", o.key, o.content));
        }
        md.push('\n');
    }

    let memory_md = workspace.join("MEMORY.md");
    fs::write(memory_md, md)?;
    Ok(())
}
