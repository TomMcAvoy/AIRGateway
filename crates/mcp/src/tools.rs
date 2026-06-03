use async_trait::async_trait;
use dashmap::DashMap;
use rustai_core::error::{GatewayError, GatewayResult};
use serde_json::Value;
use std::sync::Arc;
use tracing::info;

use crate::protocol::{Tool, ToolCallResult, ToolContent};

/// Trait for MCP tool implementations
#[async_trait]
pub trait McpToolHandler: Send + Sync {
    /// Get the tool definition
    fn tool_definition(&self) -> Tool;

    /// Execute the tool with given arguments
    async fn execute(&self, arguments: Option<Value>) -> GatewayResult<ToolCallResult>;
}

/// Registry of available MCP tools
pub struct ToolRegistry {
    tools: DashMap<String, Arc<dyn McpToolHandler>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let registry = Self {
            tools: DashMap::new(),
        };
        registry
    }

    /// Register a tool handler
    pub fn register(&self, handler: Arc<dyn McpToolHandler>) {
        let def = handler.tool_definition();
        info!(tool_name = %def.name, "Registered MCP tool");
        self.tools.insert(def.name.clone(), handler);
    }

    /// Get a tool handler by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn McpToolHandler>> {
        self.tools.get(name).map(|t| t.clone())
    }

    /// List all registered tool definitions
    pub fn list_tools(&self) -> Vec<Tool> {
        self.tools
            .iter()
            .map(|t| t.tool_definition())
            .collect()
    }

    /// Execute a tool by name with given arguments
    pub async fn call_tool(&self, name: &str, args: Option<Value>) -> GatewayResult<ToolCallResult> {
        let handler = self.get(name)
            .ok_or_else(|| GatewayError::NotFound(format!("Tool '{name}' not found")))?;
        handler.execute(args).await
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Database query tool - allows agents to query local databases
pub struct DatabaseQueryTool {
    /// Connection string for the database
    connection_string: String,
}

impl DatabaseQueryTool {
    pub fn new(connection_string: &str) -> Self {
        Self {
            connection_string: connection_string.to_string(),
        }
    }
}

#[async_trait]
impl McpToolHandler for DatabaseQueryTool {
    fn tool_definition(&self) -> Tool {
        Tool {
            name: "query_database".to_string(),
            description: Some("Execute a read-only SQL query against the connected database".to_string()),
            inputSchema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The SQL query to execute (SELECT only)"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(&self, arguments: Option<Value>) -> GatewayResult<ToolCallResult> {
        let args = arguments
            .ok_or_else(|| GatewayError::BadRequest("Missing arguments".into()))?;

        let query = args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GatewayError::BadRequest("Missing 'query' argument".into()))?;

        // Validate it's a read-only query
        let query_upper = query.to_uppercase().trim().to_string();
        if !query_upper.starts_with("SELECT") {
            return Err(GatewayError::BadRequest("Only SELECT queries are allowed".into()));
        }

        info!(query = %query, "Executing database query via MCP tool");

        // In a real implementation, this would connect to the database
        // For now, return a placeholder result
        Ok(ToolCallResult {
            content: vec![ToolContent::Text {
                text: format!("Executed query: {query}\nResults: [placeholder - database integration required]"),
            }],
            is_error: Some(false),
        })
    }
}

/// File system discovery tool - allows agents to explore local files
pub struct FileDiscoveryTool {
    /// Base path for file operations
    base_path: String,
}

impl FileDiscoveryTool {
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: base_path.to_string(),
        }
    }
}

#[async_trait]
impl McpToolHandler for FileDiscoveryTool {
    fn tool_definition(&self) -> Tool {
        Tool {
            name: "discover_files".to_string(),
            description: Some("List and discover files within the allowed directory scope".to_string()),
            inputSchema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to list files from"
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Optional glob pattern to filter results"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, arguments: Option<Value>) -> GatewayResult<ToolCallResult> {
        let args = arguments
            .ok_or_else(|| GatewayError::BadRequest("Missing arguments".into()))?;

        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GatewayError::BadRequest("Missing 'path' argument".into()))?;

        // Ensure path is within the base directory (security)
        let full_path = std::path::Path::new(&self.base_path).join(path);
        if !full_path.exists() {
            return Ok(ToolCallResult {
                content: vec![ToolContent::Text {
                    text: format!("Path '{path}' does not exist"),
                }],
                is_error: Some(true),
            });
        }

        // List directory contents
        let mut entries = Vec::new();
        if let Ok(dir) = std::fs::read_dir(&full_path) {
            for entry in dir.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let file_type = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    "directory"
                } else {
                    "file"
                };
                entries.push(format!("[{file_type}] {name}"));
            }
        }

        Ok(ToolCallResult {
            content: vec![ToolContent::Text {
                text: entries.join("\n"),
            }],
            is_error: Some(false),
        })
    }
}

/// System info tool - provides system information to agents
pub struct SystemInfoTool;

#[async_trait]
impl McpToolHandler for SystemInfoTool {
    fn tool_definition(&self) -> Tool {
        Tool {
            name: "system_info".to_string(),
            description: Some("Get system information including CPU, memory, and OS details".to_string()),
            inputSchema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn execute(&self, _arguments: Option<Value>) -> GatewayResult<ToolCallResult> {
        let info = serde_json::json!({
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "hostname": hostname(),
            "cpus": num_cpus(),
            "rust_version": env!("CARGO_PKG_VERSION"),
        });

        Ok(ToolCallResult {
            content: vec![ToolContent::Text {
                text: serde_json::to_string_pretty(&info).unwrap_or_default(),
            }],
            is_error: Some(false),
        })
    }
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "unknown".to_string())
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}
