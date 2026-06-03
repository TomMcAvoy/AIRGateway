use crate::protocol::JsonRpcMessage;
use rustai_core::error::{GatewayError, GatewayResult};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{debug, error, info};

/// MCP transport layer abstraction
#[async_trait::async_trait]
pub trait McpTransport: Send + Sync {
    /// Send a JSON-RPC message
    async fn send(&self, message: &JsonRpcMessage) -> GatewayResult<()>;

    /// Receive a JSON-RPC message
    async fn receive(&self) -> GatewayResult<JsonRpcMessage>;
}

/// SSE-based transport for MCP over HTTP
pub struct SseTransport;

impl SseTransport {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl McpTransport for SseTransport {
    async fn send(&self, message: &JsonRpcMessage) -> GatewayResult<()> {
        let json = serde_json::to_string(message)
            .map_err(|e| GatewayError::Internal(format!("JSON serialization error: {e}")))?;
        debug!("SSE transport send: {json}");
        Ok(())
    }

    async fn receive(&self) -> GatewayResult<JsonRpcMessage> {
        Err(GatewayError::Internal("SSE receive not implemented".into()))
    }
}

/// TCP-based transport for MCP - sends and receives JSON-RPC messages
pub struct TcpTransport {
    writer: tokio::sync::Mutex<tokio::net::tcp::OwnedWriteHalf>,
    reader: tokio::sync::Mutex<BufReader<tokio::net::tcp::OwnedReadHalf>>,
}

impl TcpTransport {
    pub async fn connect(addr: &str) -> GatewayResult<Self> {
        let stream = TcpStream::connect(addr).await
            .map_err(|e| GatewayError::UpstreamConnection(format!("TCP connect error: {e}")))?;

        let (reader_half, writer_half) = stream.into_split();
        let reader = BufReader::new(reader_half);

        info!("MCP TCP transport connected to {addr}");
        Ok(Self {
            writer: tokio::sync::Mutex::new(writer_half),
            reader: tokio::sync::Mutex::new(reader),
        })
    }
}

#[async_trait::async_trait]
impl McpTransport for TcpTransport {
    async fn send(&self, message: &JsonRpcMessage) -> GatewayResult<()> {
        let mut json = serde_json::to_string(message)
            .map_err(|e| GatewayError::Internal(format!("JSON serialization error: {e}")))?;
        json.push('\n');

        let mut writer = self.writer.lock().await;
        writer.write_all(json.as_bytes()).await
            .map_err(|e| GatewayError::UpstreamConnection(format!("Write error: {e}")))?;

        Ok(())
    }

    async fn receive(&self) -> GatewayResult<JsonRpcMessage> {
        let mut line = String::new();
        let mut reader = self.reader.lock().await;
        reader.read_line(&mut line).await
            .map_err(|e| GatewayError::UpstreamConnection(format!("Read error: {e}")))?;

        if line.is_empty() {
            return Err(GatewayError::UpstreamConnection("Connection closed".into()));
        }

        serde_json::from_str(&line)
            .map_err(|e| GatewayError::Internal(format!("JSON parse error: {e}")))
    }
}
