use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use clap::{Parser, Subcommand};
use colored::*;

use agent_memory::db::MemoryDb;
use agent_memory::injector::generate_context_header;
use agent_memory::mcp::run_mcp_server;
use agent_memory::sync::{export_to_markdown, sync_from_markdown};

#[derive(Parser)]
#[command(name = "agent-memory", version, about = "Local project memory for coding agents")]
struct Cli {
    #[arg(short = 'C', long = "directory", global = true, help = "Workspace directory")]
    directory: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Save a persistent decision, convention, gotcha, or focus note")]
    Remember {
        #[arg(help = "Category (decision, convention, gotcha, focus, context)")]
        category: String,

        #[arg(help = "Unique identifier key (e.g. auth_protocol, commit_format)")]
        key: String,

        #[arg(help = "Memory description content")]
        content: String,

        #[arg(long = "unpinned", help = "Keep this entry out of the context header")]
        unpinned: bool,

        #[arg(short = 't', long = "tags", help = "Comma-separated search tags")]
        tags: Option<String>,
    },

    #[command(about = "Search or recall recorded decisions and conventions")]
    Recall {
        #[arg(help = "Optional search query across keys, content, and tags")]
        query: Option<String>,

        #[arg(short = 'c', long = "category", help = "Filter by category")]
        category: Option<String>,
    },

    #[command(about = "List all memories in this workspace")]
    List {
        #[arg(short = 'c', long = "category", help = "Filter by category")]
        category: Option<String>,
    },

    #[command(about = "Output pinned memories and recent summaries for prompt context")]
    Header,

    #[command(about = "Delete a memory by key or ID")]
    Forget {
        #[arg(help = "Key name or numeric ID to delete")]
        target: String,
    },

    #[command(about = "Synchronize SQLite database with workspace MEMORY.md")]
    Sync {
        #[arg(long = "export", help = "Export SQLite memories to MEMORY.md")]
        export: bool,
    },

    #[command(about = "Start JSON-RPC Model Context Protocol (MCP) server over stdio")]
    Mcp,

    #[command(about = "Automatically configure Antigravity, Claude Code, and Codex")]
    Install,
}

fn find_workspace(start_dir: &Path) -> PathBuf {
    let mut curr = start_dir.to_path_buf();
    loop {
        if curr.join(".git").exists() || curr.join(".agent-memory").exists() || curr.join("MEMORY.md").exists() {
            return curr;
        }
        if !curr.pop() {
            break;
        }
    }
    start_dir.to_path_buf()
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let current_dir = env::current_dir()?;
    let workspace = cli.directory.unwrap_or_else(|| find_workspace(&current_dir));

    match cli.command {
        Commands::Remember { category, key, content, unpinned, tags } => {
            let mut db = MemoryDb::open(&workspace).map_err(io::Error::other)?;
            let tags_vec: Vec<String> = tags
                .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();

            db.save_memory(&category, &key, &content, &tags_vec, !unpinned)
                .map_err(io::Error::other)?;

            println!(
                "{} Saved memory [{}]: {} -> {}{}",
                "✓".green().bold(),
                category.cyan(),
                key.bold(),
                content,
                if !unpinned { " (pinned in header)".dimmed() } else { "".normal() }
            );
        }

        Commands::Recall { query, category } => {
            let db = MemoryDb::open(&workspace).map_err(io::Error::other)?;
            let items = if let Some(q) = query {
                db.search(&q).map_err(io::Error::other)?
            } else {
                db.list_all(category.as_deref()).map_err(io::Error::other)?
            };

            if items.is_empty() {
                println!("No matching memories found.");
                return Ok(());
            }

            println!("\n{}", "=== Recalled Memories ===".cyan().bold());
            for item in items {
                let pin_str = if item.pinned { " [pinned]".yellow() } else { "".normal() };
                println!("• [{}] {}: {}{}", item.category.cyan(), item.key.bold(), item.content, pin_str);
            }
            println!();
        }

        Commands::List { category } => {
            let db = MemoryDb::open(&workspace).map_err(io::Error::other)?;
            let items = db.list_all(category.as_deref()).map_err(io::Error::other)?;

            if items.is_empty() {
                println!("No memories recorded in this workspace.");
                return Ok(());
            }

            println!("\n{}", format!("=== Workspace Memories ({}) ===", workspace.display()).cyan().bold());
            for item in items {
                let pin_str = if item.pinned { " ★".yellow().bold() } else { "".normal() };
                println!("  #{:<2} [{}] {}: {}{}", item.id, item.category.cyan(), item.key.bold(), item.content, pin_str);
            }
            println!();
        }

        Commands::Header => {
            let db = MemoryDb::open(&workspace).map_err(io::Error::other)?;
            let header = generate_context_header(&db);
            if !header.is_empty() {
                print!("{}", header);
            } else {
                println!("No pinned memories in this workspace.");
            }
        }

        Commands::Forget { target } => {
            let mut db = MemoryDb::open(&workspace).map_err(io::Error::other)?;
            let count = db.delete_memory(&target).map_err(io::Error::other)?;
            if count > 0 {
                println!("{} Removed memory '{}'", "✓".green().bold(), target);
            } else {
                println!("Memory '{}' not found.", target);
            }
        }

        Commands::Sync { export } => {
            let mut db = MemoryDb::open(&workspace).map_err(io::Error::other)?;
            if export {
                export_to_markdown(&workspace, &db)?;
                println!("{} Exported memories to MEMORY.md", "✓".green().bold());
            } else {
                let count = sync_from_markdown(&workspace, &mut db)?;
                println!("{} Synchronized {} items from MEMORY.md into SQLite", "✓".green().bold(), count);
            }
        }

        Commands::Mcp => {
            env::set_current_dir(&workspace)?;
            run_mcp_server()?;
        }

        Commands::Install => {
            install_globally()?;
        }
    }

    Ok(())
}

fn install_globally() -> io::Result<()> {
    let Some(home) = std::env::var("HOME").ok().map(PathBuf::from) else {
        return Err(io::Error::new(io::ErrorKind::NotFound, "HOME not found"));
    };

    println!("\n{}", "=== Installing agent-memory across CLI Agent Environments ===".cyan().bold());

    // 1. Antigravity MCP config
    let agy_mcp_path = home.join(".gemini").join("config").join("mcp_config.json");
    if agy_mcp_path.exists() {
        if let Ok(content) = fs::read_to_string(&agy_mcp_path) {
            if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(servers) = val.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
                    servers.insert(
                        "agent-memory".to_string(),
                        serde_json::json!({
                            "command": "/Users/yuri/.local/bin/agent-memory",
                            "args": ["mcp"]
                        }),
                    );
                    let _ = fs::write(&agy_mcp_path, serde_json::to_string_pretty(&val).unwrap());
                    println!("{} Configured in Google Antigravity (AGY)", "✓".green().bold());
                }
            }
        }
    }

    // 2. Claude Code MCP config (~/.claude.json)
    let claude_json_path = home.join(".claude.json");
    if claude_json_path.exists() {
        if let Ok(content) = fs::read_to_string(&claude_json_path) {
            if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(servers) = val.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
                    servers.insert(
                        "agent-memory".to_string(),
                        serde_json::json!({
                            "command": "/Users/yuri/.local/bin/agent-memory",
                            "args": ["mcp"]
                        }),
                    );
                    let _ = fs::write(&claude_json_path, serde_json::to_string_pretty(&val).unwrap());
                    println!("{} Configured in Claude Code CLI (~/.claude.json)", "✓".green().bold());
                }
            }
        }
    }

    // 3. Codex config (~/.codex/config.toml)
    let codex_toml_path = home.join(".codex").join("config.toml");
    if codex_toml_path.exists() {
        if let Ok(mut content) = fs::read_to_string(&codex_toml_path) {
            if !content.contains("[mcp_servers.agent-memory]") {
                content.push_str("\n[mcp_servers.agent-memory]\ncommand = \"/Users/yuri/.local/bin/agent-memory\"\nargs = [\"mcp\"]\nenabled = true\n");
                let _ = fs::write(&codex_toml_path, content);
                println!("{} Configured in Codex CLI (~/.codex/config.toml)", "✓".green().bold());
            }
        }
    }

    println!("\n{} agent-memory is ready and auto-connected everywhere!\n", "Success:".green().bold());
    Ok(())
}
