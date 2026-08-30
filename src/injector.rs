use crate::db::MemoryDb;

pub fn generate_context_header(db: &MemoryDb) -> String {
    let Ok(pinned) = db.get_pinned() else {
        return String::new();
    };

    let Ok(recent_sessions) = db.get_recent_sessions(2) else {
        return String::new();
    };

    if pinned.is_empty() && recent_sessions.is_empty() {
        return String::new();
    }

    let mut conventions = Vec::new();
    let mut decisions = Vec::new();
    let mut gotchas = Vec::new();
    let mut other = Vec::new();

    for item in pinned {
        match item.category.to_lowercase().as_str() {
            "convention" | "rule" => conventions.push(item),
            "decision" | "architecture" => decisions.push(item),
            "gotcha" | "lesson" | "platform" => gotchas.push(item),
            _ => other.push(item),
        }
    }

    let mut output = String::new();
    output.push_str("<agent_memory>\n");

    if !conventions.is_empty() {
        output.push_str("[Pinned Project Conventions]\n");
        for c in conventions {
            output.push_str(&format!("• {}: {}\n", c.key, c.content));
        }
        output.push('\n');
    }

    if !decisions.is_empty() {
        output.push_str("[Architectural Decisions]\n");
        for d in decisions {
            output.push_str(&format!("• {}: {}\n", d.key, d.content));
        }
        output.push('\n');
    }

    if !gotchas.is_empty() {
        output.push_str("[Platform Gotchas & Lessons]\n");
        for g in gotchas {
            output.push_str(&format!("• {}: {}\n", g.key, g.content));
        }
        output.push('\n');
    }

    if !other.is_empty() {
        output.push_str("[Active Focus & Notes]\n");
        for o in other {
            output.push_str(&format!("• {}: {}\n", o.key, o.content));
        }
        output.push('\n');
    }

    if !recent_sessions.is_empty() {
        output.push_str("[Recent Session Context]\n");
        for s in recent_sessions {
            let files_str = if !s.files_touched.is_empty() {
                format!(" (Files: {})", s.files_touched.join(", "))
            } else {
                String::new()
            };
            output.push_str(&format!("• [{}] {}{}\n", s.agent_name, s.summary, files_str));
        }
        output.push('\n');
    }

    output.push_str("</agent_memory>\n");
    output
}
