use crate::protocol::{
    methods, InitializeRequest, InitializeResult, JsonRpcMessage, ServerCapabilities,
    ImplementationInfo, ToolsCapability, ResourcesCapability, PromptsCapability,
};
use crate::tools::ToolRegistry;
use rustai_core::error::{GatewayError, GatewayResult};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// MCP server that handles JSON-RPC messages for AI Agent interactions
pub struct McpServer {
    registry: Arc<ToolRegistry>,
    server_info: ImplementationInfo,
}

impl McpServer {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            server_info: ImplementationInfo {
                name: "rustai-mcp-server".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        }
    }

    /// Handle an incoming JSON-RPC message
    pub async fn handle_message(&self, message: JsonRpcMessage) -> JsonRpcMessage {
        let method = match &message.method {
            Some(m) => m.as_str(),
            None => {
                return JsonRpcMessage::error(
                    message.id,
                    -32600,
                    "Method not specified",
                );
            }
        };

        let params = message.params.clone().unwrap_or(serde_json::Value::Null);

        match method {
            methods::INITIALIZE => self.handle_initialize(params).await,
            methods::LIST_TOOLS => self.handle_list_tools(message.id),
            methods::CALL_TOOL => self.handle_call_tool(params, message.id).await,
            methods::PING => self.handle_ping(message.id),
            _ => JsonRpcMessage::error(
                message.id,
                -32601,
                &format!("Method '{method}' not found"),
            ),
        }
    }

    /// Handle initialize request
    async fn handle_initialize(&self, params: serde_json::Value) -> JsonRpcMessage {
        let _init: InitializeRequest = match serde_json::from_value(params) {
            Ok(req) => req,
            Err(e) => {
                return JsonRpcMessage::error(None, -32602, &format!("Invalid params: {e}"));
            }
        };

        info!("MCP client initialized: {} v{}", _init.client_info.name, _init.client_info.version);

        let result = InitializeResult {
            protocol_version: crate::protocol::MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(true),
                }),
                resources: Some(ResourcesCapability {
                    subscribe: Some(true),
                    list_changed: Some(true),
                }),
                prompts: Some(PromptsCapability {
                    list_changed: Some(true),
                }),
                logging: Some(serde_json::json!({})),
            },
            server_info: self.server_info.clone(),
        };

        JsonRpcMessage::response(
            serde_json::json!(1),
            serde_json::to_value(result).unwrap_or_default(),
        )
    }

    /// Handle tools/list request
    fn handle_list_tools(&self, id: Option<serde_json::Value>) -> JsonRpcMessage {
        let tools = self.registry.list_tools();
        let result = serde_json::json!({
            "tools": tools
        });
        JsonRpcMessage::response(id.unwrap_or(serde_json::json!(null)), result)
    }

    /// Handle tools/call request
    async fn handle_call_tool(
        &self,
        params: serde_json::Value,
        id: Option<serde_json::Value>,
    ) -> JsonRpcMessage {
        let name = params.get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let arguments = params.get("arguments").cloned();

        let name = match name {
            Some(n) => n,
            None => {
                return JsonRpcMessage::error(
                    id,
                    -32602,
                    "Missing 'name' in tool call params",
                );
            }
        };

        match self.registry.call_tool(&name, arguments).await {
            Ok(result) => {
                let value = serde_json::to_value(result).unwrap_or_default();
                JsonRpcMessage::response(id.unwrap_or(serde_json::json!(null)), value)
            }
            Err(e) => {
                JsonRpcMessage::error(id, -32000, &e.to_string())
            }
        }
    }

    /// Handle ping request
    fn handle_ping(&self, id: Option<serde_json::Value>) -> JsonRpcMessage {
        JsonRpcMessage::response(
            id.unwrap_or(serde_json::json!(null)),
            serde_json::json!({}),
        )
    }
}

/// Run an MCP server session over stdio transport
pub async fn run_stdio_server(registry: Arc<ToolRegistry>) -> GatewayResult<()> {
    let server = McpServer::new(registry);
    let (tx, mut rx) = mpsc::channel::<String>(256);

    // Spawn stdin reader
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;
        let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    debug!("MCP stdin closed");
                    break;
                }
                Ok(_) => {
                    let msg = std::mem::take(&mut line);
                    if tx_clone.send(msg).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    error!("MCP stdin read error: {e}");
                    break;
                }
            }
        }
    });

    // Process messages
    while let Some(line) = rx.recv().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        match serde_json::from_str::<JsonRpcMessage>(&line) {
            Ok(message) => {
                let response = server.handle_message(message).await;
                let resp_json = serde_json::to_string(&response).unwrap_or_default();
                println!("{}", resp_json);
            }
            Err(e) => {
                let error_resp = JsonRpcMessage::error(
                    None,
                    -32700,
                    &format!("Parse error: {e}"),
                );
                let resp_json = serde_json::to_string(&error_resp).unwrap_or_default();
                println!("{}", resp_json);
            }
        }
    }

    info!("MCP stdio server shutting down");
    Ok(())
}
