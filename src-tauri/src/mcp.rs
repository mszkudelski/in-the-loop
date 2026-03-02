use crate::db::{Database, Item, TodoWithBindings};
use crate::services::url_parser;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

// ── JSON-RPC types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, code: i64, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message }),
        }
    }
}

// ── MCP handler ─────────────────────────────────────────────────────

pub struct McpHandler {
    db: Database,
}

impl McpHandler {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn handle_message(&self, request: &JsonRpcRequest) -> Option<JsonRpcResponse> {
        match request.method.as_str() {
            "initialize" => Some(self.handle_initialize(request.id.clone())),
            "notifications/initialized" => None, // notification, no response
            "tools/list" => Some(self.handle_tools_list(request.id.clone())),
            "tools/call" => Some(self.handle_tools_call(request.id.clone(), &request.params)),
            _ => Some(JsonRpcResponse::error(
                request.id.clone(),
                -32601,
                format!("Method not found: {}", request.method),
            )),
        }
    }

    fn handle_initialize(&self, id: Option<Value>) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "in-the-loop",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
    }

    fn handle_tools_list(&self, id: Option<Value>) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id,
            json!({
                "tools": [
                    {
                        "name": "list_items",
                        "description": "List tracked work items (PRs, Slack threads, GitHub Actions, etc.) from In The Loop. Returns active items by default.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "include_archived": {
                                    "type": "boolean",
                                    "description": "If true, return archived items instead of active ones. Default: false"
                                }
                            }
                        }
                    },
                    {
                        "name": "list_todos",
                        "description": "List all todos from In The Loop, including subtasks and bound work items.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "add_item",
                        "description": "Add a new work item to track in In The Loop. Supports Slack thread URLs, GitHub PR URLs, and GitHub Actions run URLs.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "url": {
                                    "type": "string",
                                    "description": "URL of the item to track (Slack thread, GitHub PR, or GitHub Action run)"
                                },
                                "title": {
                                    "type": "string",
                                    "description": "Optional custom title. If omitted, a title is auto-generated from the URL."
                                }
                            },
                            "required": ["url"]
                        }
                    },
                    {
                        "name": "add_todo",
                        "description": "Add a new todo to In The Loop. Optionally set a planned date and parent todo for subtasks.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "title": {
                                    "type": "string",
                                    "description": "Title of the todo"
                                },
                                "planned_date": {
                                    "type": "string",
                                    "description": "Optional planned date in YYYY-MM-DD format"
                                },
                                "parent_id": {
                                    "type": "string",
                                    "description": "Optional parent todo ID to create this as a subtask"
                                }
                            },
                            "required": ["title"]
                        }
                    },
                    {
                        "name": "bind_todo_to_item",
                        "description": "Bind a todo to a tracked work item in In The Loop, creating a relationship between them.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "todo_id": {
                                    "type": "string",
                                    "description": "ID of the todo"
                                },
                                "item_id": {
                                    "type": "string",
                                    "description": "ID of the tracked work item"
                                }
                            },
                            "required": ["todo_id", "item_id"]
                        }
                    }
                ]
            }),
        )
    }

    fn handle_tools_call(&self, id: Option<Value>, params: &Option<Value>) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(id, -32602, "Missing params".to_string());
            }
        };

        let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        match tool_name {
            "list_items" => self.tool_list_items(id, &arguments),
            "list_todos" => self.tool_list_todos(id),
            "add_item" => self.tool_add_item(id, &arguments),
            "add_todo" => self.tool_add_todo(id, &arguments),
            "bind_todo_to_item" => self.tool_bind_todo_to_item(id, &arguments),
            _ => JsonRpcResponse::error(id, -32602, format!("Unknown tool: {}", tool_name)),
        }
    }

    fn tool_list_items(&self, id: Option<Value>, arguments: &Value) -> JsonRpcResponse {
        let include_archived = arguments
            .get("include_archived")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        match self.db.get_items(include_archived) {
            Ok(items) => {
                let text = format_items(&items);
                JsonRpcResponse::success(
                    id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": text
                        }]
                    }),
                )
            }
            Err(e) => JsonRpcResponse::success(
                id,
                json!({
                    "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                    "isError": true
                }),
            ),
        }
    }

    fn tool_list_todos(&self, id: Option<Value>) -> JsonRpcResponse {
        match self.db.get_todos() {
            Ok(todos) => {
                let text = format_todos(&todos);
                JsonRpcResponse::success(
                    id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": text
                        }]
                    }),
                )
            }
            Err(e) => JsonRpcResponse::success(
                id,
                json!({
                    "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                    "isError": true
                }),
            ),
        }
    }

    fn tool_add_item(&self, id: Option<Value>, arguments: &Value) -> JsonRpcResponse {
        let url = match arguments.get("url").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => {
                return JsonRpcResponse::success(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": "Error: 'url' parameter is required" }],
                        "isError": true
                    }),
                );
            }
        };

        let custom_title = arguments.get("title").and_then(|v| v.as_str());

        let parsed = match url_parser::parse_url(url) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::success(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": format!("Error parsing URL: {}", e) }],
                        "isError": true
                    }),
                );
            }
        };

        let title = custom_title
            .map(|t| t.to_string())
            .unwrap_or(parsed.suggested_title);

        let item = Item {
            id: Uuid::new_v4().to_string(),
            item_type: parsed.item_type,
            title: title.clone(),
            url: Some(url.to_string()),
            status: "waiting".to_string(),
            previous_status: None,
            metadata: serde_json::to_string(&parsed.metadata).unwrap_or_else(|_| "{}".to_string()),
            last_checked_at: None,
            last_updated_at: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            archived: false,
            archived_at: None,
            polling_interval_override: None,
            checked: false,
        };

        match self.db.add_item(&item) {
            Ok(_) => JsonRpcResponse::success(
                id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Added item: {} (type: {}, id: {})", title, item.item_type, item.id)
                    }]
                }),
            ),
            Err(e) => JsonRpcResponse::success(
                id,
                json!({
                    "content": [{ "type": "text", "text": format!("Error adding item: {}", e) }],
                    "isError": true
                }),
            ),
        }
    }

    fn tool_add_todo(&self, id: Option<Value>, arguments: &Value) -> JsonRpcResponse {
        let title = match arguments.get("title").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => {
                return JsonRpcResponse::success(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": "Error: 'title' parameter is required" }],
                        "isError": true
                    }),
                );
            }
        };

        let planned_date = arguments.get("planned_date").and_then(|v| v.as_str());
        let parent_id = arguments.get("parent_id").and_then(|v| v.as_str());

        let todo = crate::db::Todo {
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            status: "open".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            planned_date: planned_date.map(|s| s.to_string()),
            parent_id: parent_id.map(|s| s.to_string()),
        };

        match self.db.add_todo(&todo) {
            Ok(_) => JsonRpcResponse::success(
                id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Added todo: {} (id: {})", todo.title, todo.id)
                    }]
                }),
            ),
            Err(e) => JsonRpcResponse::success(
                id,
                json!({
                    "content": [{ "type": "text", "text": format!("Error adding todo: {}", e) }],
                    "isError": true
                }),
            ),
        }
    }

    fn tool_bind_todo_to_item(&self, id: Option<Value>, arguments: &Value) -> JsonRpcResponse {
        let todo_id = match arguments.get("todo_id").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => {
                return JsonRpcResponse::success(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": "Error: 'todo_id' parameter is required" }],
                        "isError": true
                    }),
                );
            }
        };

        let item_id = match arguments.get("item_id").and_then(|v| v.as_str()) {
            Some(i) => i,
            None => {
                return JsonRpcResponse::success(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": "Error: 'item_id' parameter is required" }],
                        "isError": true
                    }),
                );
            }
        };

        match self.db.bind_todo_to_item(todo_id, item_id) {
            Ok(_) => JsonRpcResponse::success(
                id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Bound todo {} to item {}", todo_id, item_id)
                    }]
                }),
            ),
            Err(e) => JsonRpcResponse::success(
                id,
                json!({
                    "content": [{ "type": "text", "text": format!("Error binding: {}", e) }],
                    "isError": true
                }),
            ),
        }
    }
}

// ── Formatting helpers ──────────────────────────────────────────────

fn format_items(items: &[Item]) -> String {
    if items.is_empty() {
        return "No items found.".to_string();
    }

    let mut out = format!("Found {} item(s):\n\n", items.len());
    for item in items {
        out.push_str(&format!(
            "- [{}] {} (type: {})\n  Status: {}\n",
            item.id, item.title, item.item_type, item.status
        ));
        if let Some(ref url) = item.url {
            out.push_str(&format!("  URL: {}\n", url));
        }
        if let Some(ref updated) = item.last_updated_at {
            out.push_str(&format!("  Updated: {}\n", updated));
        }
        out.push('\n');
    }
    out
}

fn format_todos(todos: &[TodoWithBindings]) -> String {
    if todos.is_empty() {
        return "No todos found.".to_string();
    }

    let mut out = format!("Found {} todo(s):\n\n", todos.len());
    for tw in todos {
        format_todo_entry(&mut out, tw, 0);
    }
    out
}

fn format_todo_entry(out: &mut String, tw: &TodoWithBindings, indent: usize) {
    let prefix = "  ".repeat(indent);
    let status_icon = match tw.todo.status.as_str() {
        "done" => "✓",
        "open" => "○",
        _ => "·",
    };

    out.push_str(&format!(
        "{}{} {} [id: {}] ({})\n",
        prefix, status_icon, tw.todo.title, tw.todo.id, tw.todo.status
    ));

    if let Some(ref date) = tw.todo.planned_date {
        out.push_str(&format!("{}  Planned: {}\n", prefix, date));
    }

    for item in &tw.bound_items {
        out.push_str(&format!(
            "{}  ↳ {} [{}]\n",
            prefix, item.title, item.status
        ));
    }

    for sub in &tw.subtasks {
        format_todo_entry(out, sub, indent + 1);
    }
}
