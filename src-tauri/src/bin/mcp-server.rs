use in_the_loop_lib::db::Database;
use in_the_loop_lib::mcp::{JsonRpcRequest, McpHandler};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

fn get_db_path() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    PathBuf::from(home)
        .join("Library/Application Support/com.intheloop.app")
        .join("in-the-loop.db")
}

fn main() {
    let db_path = get_db_path();
    if !db_path.exists() {
        eprintln!(
            "Database not found at {}. Is In The Loop installed and has been run at least once?",
            db_path.display()
        );
        std::process::exit(1);
    }

    let db = Database::new(db_path).expect("Failed to open database");
    let handler = McpHandler::new(db);

    let stdin = io::stdin();
    let stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to parse JSON-RPC request: {}", e);
                continue;
            }
        };

        if let Some(response) = handler.handle_message(&request) {
            let json = serde_json::to_string(&response).expect("Failed to serialize response");
            let mut out = stdout.lock();
            writeln!(out, "{}", json).expect("Failed to write to stdout");
            out.flush().expect("Failed to flush stdout");
        }
    }
}
