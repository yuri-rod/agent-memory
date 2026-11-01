use std::env;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::MemoryDb;
use crate::injector::generate_context_header;

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

pub const MAX_JSONRPC_FRAME_BYTES: usize = 256 * 1024; // 256 KiB transport limit

fn drain_until_newline<R: BufRead>(reader: &mut R) -> io::Result<()> {
    loop {
        let available = match reader.fill_buf() {
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if available.is_empty() {
            return Ok(());
        }
        if let Some(pos) = available.iter().position(|&b| b == b'\n') {
            reader.consume(pos + 1);
            return Ok(());
        }
        let len = available.len();
        reader.consume(len);
    }
}

pub fn read_bounded_line<R: BufRead>(reader: &mut R, max_bytes: usize) -> io::Result<Option<String>> {
    let mut buf = Vec::new();
    loop {
        let available = match reader.fill_buf() {
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if available.is_empty() {
            if buf.is_empty() {
                return Ok(None);
            }
            break;
        }
        if let Some(pos) = available.iter().position(|&b| b == b'\n') {
            let consume_len = pos + 1;
            if buf.len() + pos > max_bytes {
                reader.consume(consume_len);
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("JSON-RPC request exceeds maximum frame size of {} bytes", max_bytes),
                ));
            }
            buf.extend_from_slice(&available[..pos]);
            reader.consume(consume_len);
            break;
        } else {
            let len = available.len();
            if buf.len() + len > max_bytes {
                reader.consume(len);
                let _ = drain_until_newline(reader);
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("JSON-RPC request exceeds maximum frame size of {} bytes", max_bytes),
                ));
            }
            buf.extend_from_slice(available);
            reader.consume(len);
        }
    }
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
    let s = String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some(s))
}

pub fn run_mcp_server() -> io::Result<()> {
    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin.lock());
    let mut stdout = io::stdout();

    let workspace = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut db = MemoryDb::open(&workspace).map_err(io::Error::other)?;

    loop {
        match read_bounded_line(&mut reader, MAX_JSONRPC_FRAME_BYTES) {
            Ok(Some(line)) => {
                if line.trim().is_empty() {
                    continue;
                }

                let Ok(req) = serde_json::from_str::<JsonRpcRequest>(&line) else {
                    continue;
                };

                let response = handle_request(&req, &mut db);
                if let Some(resp) = response {
                    let resp_str = serde_json::to_string(&resp)?;
                    writeln!(stdout, "{}", resp_str)?;
                    stdout.flush()?;
                }
            }
            Ok(None) => break,
            Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": {
                        "code": -32600,
                        "message": format!("Invalid request: {}", e)
                    }
                });
                let resp_str = serde_json::to_string(&err_resp)?;
                writeln!(stdout, "{}", resp_str)?;
                stdout.flush()?;
            }
            Err(e) => return Err(e),
        }
    }

    Ok(())
}

fn handle_request(req: &JsonRpcRequest, db: &mut MemoryDb) -> Option<Value> {
    let id = req.id.clone().unwrap_or(Value::Null);

    match req.method.as_str() {
        "initialize" => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "serverInfo": {
                    "name": "agent-memory",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "tools": {}
                }
            }
        })),

        "notifications/initialized" => None,

        "tools/list" => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": [
                    {
                        "name": "save_memory",
                        "description": "Store a persistent decision, convention, gotcha, or focus item across agent sessions",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "category": { "type": "string", "enum": ["decision", "convention", "gotcha", "focus", "context"], "description": "Memory category" },
                                "key": { "type": "string", "description": "Unique key identifier (e.g. auth_protocol, commit_format)" },
                                "content": { "type": "string", "description": "The exact decision or convention text" },
                                "pinned": { "type": "boolean", "description": "If true, include this entry when the context header is requested" },
                                "tags": { "type": "array", "items": { "type": "string" }, "description": "Optional search tags" }
                            },
                            "required": ["category", "key", "content"]
                        }
                    },
                    {
                        "name": "recall_memory",
                        "description": "Search or list cross-session memories, architectural decisions, and project conventions",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": { "type": "string", "description": "Optional search term across keys, content, and tags" },
                                "category": { "type": "string", "description": "Optional category filter" }
                            }
                        }
                    },
                    {
                        "name": "get_context_header",
                        "description": "Retrieve pinned conventions, decisions and recent session summaries",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "record_session_summary",
                        "description": "Record what was accomplished in this session so future sessions have immediate context",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "agent_name": { "type": "string", "description": "Agent name (e.g. claude, codex, agy)" },
                                "summary": { "type": "string", "description": "Short 1-2 sentence summary of changes accomplished" },
                                "files_touched": { "type": "array", "items": { "type": "string" }, "description": "List of relative file paths modified" }
                            },
                            "required": ["agent_name", "summary"]
                        }
                    }
                ]
            }
        })),

        "tools/call" => {
            let params = req.params.as_ref()?;
            let tool_name = params.get("name").and_then(|v| v.as_str())?;
            let args = params.get("arguments").cloned().unwrap_or(json!({}));

            match tool_name {
                "save_memory" => {
                    let category = args.get("category").and_then(|v| v.as_str()).unwrap_or("decision");
                    let key = args.get("key").and_then(|v| v.as_str()).unwrap_or("");
                    let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    let pinned = args.get("pinned").and_then(|v| v.as_bool()).unwrap_or(true);
                    let tags: Vec<String> = args
                        .get("tags")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(|s| s.to_string())).collect())
                        .unwrap_or_default();

                    match db.save_memory_with_provenance(category, key, content, &tags, pinned, "agent") {
                        Ok(_) => Some(json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [{
                                    "type": "text",
                                    "text": format!("✓ Saved memory: [{}] {}: {}", category, key, content)
                                }]
                            }
                        })),
                        Err(e) => Some(json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "isError": true,
                                "content": [{ "type": "text", "text": format!("Failed to save memory: {}", e) }]
                            }
                        })),
                    }
                }

                "recall_memory" => {
                    let query = args.get("query").and_then(|v| v.as_str());
                    let category = args.get("category").and_then(|v| v.as_str());

                    let items = if let Some(q) = query {
                        if !q.trim().is_empty() {
                            db.search(q).unwrap_or_default()
                        } else {
                            db.list_all(category).unwrap_or_default()
                        }
                    } else {
                        db.list_all(category).unwrap_or_default()
                    };

                    let mut out = String::new();
                    if items.is_empty() {
                        out.push_str("No matching memories found.");
                    } else {
                        for item in items {
                            let pin_str = if item.pinned { " [pinned]" } else { "" };
                            out.push_str(&format!(
                                "• [{}] {}: {}{} [source: {}]\n",
                                item.category, item.key, item.content, pin_str, item.provenance
                            ));
                        }
                    }

                    Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": out }]
                        }
                    }))
                }

                "get_context_header" => {
                    let header = generate_context_header(db);
                    Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": if header.is_empty() { "No pinned memories.".to_string() } else { header } }]
                        }
                    }))
                }

                "record_session_summary" => {
                    let agent_name = args.get("agent_name").and_then(|v| v.as_str()).unwrap_or("agent");
                    let summary = args.get("summary").and_then(|v| v.as_str()).unwrap_or("");
                    let files: Vec<String> = args
                        .get("files_touched")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(|s| s.to_string())).collect())
                        .unwrap_or_default();

                    match db.record_session(agent_name, summary, &files) {
                        Ok(_) => Some(json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [{ "type": "text", "text": format!("✓ Recorded session summary for {}", agent_name) }]
                            }
                        })),
                        Err(e) => Some(json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "isError": true,
                                "content": [{ "type": "text", "text": format!("Failed to record session: {}", e) }]
                            }
                        })),
                    }
                }

                _ => Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": format!("Tool '{}' not found", tool_name) }
                })),
            }
        }

        _ => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("Method '{}' not found", req.method) }
        })),
    }
}
