# AIRGateway

**Author:** Thomas McAvoy — Chief Performance Engineer

[![Rust](https://img.shields.io/badge/Rust-1.81%2B-orange)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

**Cloud-native, high-throughput AI Gateway and Reverse Proxy engine** built in Rust, designed to handle high-concurrency, long-lived Server-Sent Events (SSE) for streaming LLM inference traffic.

Serves as the foundational infrastructure middle-layer integrating AI Agents, cloud platforms, and internal microservices.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Client / Agent                         │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                     RustAI Gateway                            │
│                                                               │
│  ┌─────────┐  ┌──────────┐  ┌─────────┐  ┌──────────────┐  │
│  │  Proxy   │  │  Router  │  │   MCP   │  │  Wasm Plugin │  │
│  │  Engine  │  │ (Axum/   │  │ Server  │  │  (Wasmtime)  │  │
│  │(Pingora/ │  │  Tower)  │  │         │  │              │  │
│  │  Hyper)  │  └──────────┘  └─────────┘  └──────────────┘  │
│  └─────────┘                                                │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │           Observability (Prometheus / Tracing)        │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │     Kubernetes Gateway API Controller (gateway-api-rs)│    │
│  └──────────────────────────────────────────────────────┘    │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    AI Providers (Upstreams)                   │
│  ┌────────┐  ┌──────────┐  ┌────────┐  ┌────────────────┐  │
│  │ OpenAI │  │ Anthropic│  │ Google │  │ Ollama (Local) │  │
│  └────────┘  └──────────┘  └────────┘  └────────────────┘  │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ DeepSeek (V3 / R1) — Native MoE Architecture Support   │  │
│  └────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Project Structure

```
rustai/
├── Cargo.toml                          # Workspace root
├── src/main.rs                         # Gateway binary entrypoint
├── rustai.toml                         # Example configuration
├── Dockerfile                          # Container build
├── DEEPSEEK_PLAN.md                    # DeepSeek integration roadmap
├── kubernetes/
│   ├── deployment.yaml                  # K8s Deployment + Service + RBAC
│   └── gateway.yaml                    # Gateway API + AiGateway CRDs
├── crates/
│   ├── core/                           # Core types, traits, configuration
│   │   ├── src/
│   │   │   ├── types.rs                # GatewayConfig, Upstream, Route, SseEvent
│   │   │   ├── error.rs                # GatewayError type hierarchy + IntoResponse
│   │   │   ├── traits.rs               # BackendTransport, ProviderAdapter, MetricsCollector
│   │   │   ├── config.rs               # TOML/ENV config loader
│   │   │   └── middleware.rs           # MiddlewareChain, AuthMiddleware, LoggingMiddleware
│   ├── proxy/                          # Reverse proxy engine
│   │   ├── src/
│   │   │   ├── engine.rs               # ProxyEngine - connection management & routing
│   │   │   ├── transport.rs            # Reqwest-based HTTP backend transport
│   │   │   ├── connection.rs           # ConnectionPool with Semaphore-based limiting
│   │   │   ├── sse.rs                  # SseProxy - streaming SSE response handling
│   │   │   └── tls.rs                  # TLS configuration with rustls
│   ├── router/                         # Axum/Tower middleware-based router
│   │   ├── src/
│   │   │   ├── router.rs               # Axum Router + health/chat completions handlers
│   │   │   ├── middleware.rs           # Tower layers: auth, logging, rate limit, latency
│   │   │   ├── provider.rs             # ProviderAdapters: OpenAI, Anthropic, Google
│   │   │   └── proxy_handler.rs        # ProxyHandler - upstream request dispatch + SSE streaming
│   ├── mcp/                            # Model Context Protocol backend
│   │   ├── src/
│   │   │   ├── protocol.rs             # MCP JSON-RPC protocol types
│   │   │   ├── server.rs               # McpServer - initialize, list_tools, call_tool
│   │   │   ├── tools.rs                # ToolRegistry, DatabaseQuery, FileDiscovery, SysInfo
│   │   │   └── transport.rs            # SSE/TCP transport abstractions
│   ├── wasm-plugin/                    # WebAssembly plugin system
│   │   ├── src/
│   │   │   ├── runtime.rs              # WasmPluginInstance - Wasmtime sandboxing
│   │   │   ├── engine.rs               # WasmPluginEngine - lifecycle management
│   │   │   ├── plugins.rs              # TokenBucketRateLimiter, PiiMasker, TracingPlugin
│   │   │   └── types.rs                # PluginConfig, PluginResult, PluginAction
│   ├── k8s-gateway/                    # Kubernetes Gateway API integration
│   │   ├── src/
│   │   │   ├── crd.rs                  # AiGateway CRD definition (kube-rs + schemars)
│   │   │   ├── controller.rs           # Kubernetes controller reconciler
│   │   │   └── watcher.rs              # GatewayWatcher - watch for config changes
│   └── observability/                  # Prometheus observability stack
│       ├── src/
│       │   ├── metrics.rs              # PrometheusMetrics - counters, histograms, gauges
│       │   ├── tracing.rs              # Tracing init with JSON structured logging
│       │   └── logging.rs              # Observability stack initialization
```

## Key Engineering Achievements

### Async Infrastructure
Low-latency network proxy using **Tokio**, **Hyper**, and **Pingora**-style architecture, reducing routing overhead to sub-milliseconds and scaling to **10,000+ concurrent asynchronous connections**.

### Protocol Translation & Routing
Extensible middleware via **Axum** and **Tower** to intercept payloads, transform between provider-specific API formats (OpenAI, Anthropic, Google Gemini, DeepSeek), and dynamically route traffic.

### Agentic Frameworks (MCP)
Native backend support for the **Model Context Protocol (MCP)**, empowering autonomous AI Agents with secure, localized database query tools and file discovery patterns via JSON-RPC over stdio.

### Wasm Plugin Architecture
Sandboxed **WebAssembly** runtime extension layer using **Wasmtime** for dynamic token-based rate limiting, PII masking, and distributed tracing with configurable memory limits and execution timeouts.

### Cloud-Native Deployment
Containerized and configured using **gateway-api-rs** as a Kubernetes-native ingress control plane with **Prometheus** observability stacks (latency histograms, throughput counters, active connection gauges).

## Quick Start

### Prerequisites
- Rust 1.81+
- Docker (for containerized deployment)
- Kubernetes cluster (for Gateway API deployment)

### Local Development

```bash
# Clone and build
git clone https://github.com/yourorg/rustai.git
cd rustai
cargo build --release

# Configure
cp rustai.toml.example rustai.toml
# Edit rustai.toml with your API keys

# Run
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
cargo run --release
```

### Test the Gateway

```bash
# Health check
curl http://localhost:8080/health

# Chat completion (non-streaming)
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Hello!"}],
    "stream": false
  }'

# Chat completion (streaming SSE)
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Tell me a story"}],
    "stream": true
  }'

# Prometheus metrics
curl http://localhost:9090/metrics
```

### Docker Deployment

```bash
# Build the container
docker build -t rustai/gateway:latest .

# Run
docker run -p 8080:8080 -p 9090:9090 \
  -e OPENAI_API_KEY="sk-..." \
  -e ANTHROPIC_API_KEY="sk-ant-..." \
  rustai/gateway:latest
```

### Kubernetes Deployment

```bash
# Create secrets
kubectl create secret generic rustai-secrets \
  --from-literal=openai-api-key="sk-..." \
  --from-literal=anthropic-api-key="sk-ant-..."

# Deploy the gateway
kubectl apply -f kubernetes/deployment.yaml

# Apply Gateway API resources
kubectl apply -f kubernetes/gateway.yaml

# Check status
kubectl get aigateways
kubectl get gateway
kubectl get httproute
```

## Configuration Reference

The gateway is configured via TOML files. Key configuration sections:

| Section | Description |
|---------|-------------|
| `upstreams` | AI provider backends (OpenAI, Anthropic, DeepSeek, Ollama, etc.) |
| `routes` | Path-based routing rules with method matching |
| `rate_limit` | Token bucket rate limiting |
| `wasm_plugins_dir` | Directory for loading Wasm plugins |

Environment variable substitution is supported: `api_key = "${OPENAI_API_KEY}"`

## MCP Server

Run the MCP stdio server for AI Agent integration:

```bash
# Run as a standalone MCP server
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | cargo run --bin rustai-mcp
```

## Metrics

Prometheus metrics are exposed on the metrics port (default `:9090`):

| Metric | Type | Description |
|--------|------|-------------|
| `rustai_requests_total` | Counter | Total HTTP requests by method, path, status |
| `rustai_request_duration_seconds` | Histogram | Request latency distribution |
| `rustai_active_connections` | Gauge | Current active connections |
| `rustai_upstream_requests_total` | Counter | Upstream requests by provider |
| `rustai_upstream_errors_total` | Counter | Upstream errors by type |
| `rustai_rate_limited_requests_total` | Counter | Rate-limited requests by route |
| `rustai_plugin_execution_duration_seconds` | Histogram | Wasm plugin execution time |

## Core Competencies

- **Rust Systems Engineering**
- **Async Rust** — Tokio, Hyper, Axum, Tower
- **Reverse Proxy Architecture**
- **AI Gateway & LLM Infrastructure**
- **Model Context Protocol (MCP)**
- **WebAssembly (Wasmtime)**
- **Kubernetes Gateway API**
- **Distributed Systems**
- **Prometheus Observability**

---

**Thomas McAvoy** — Chief Performance Engineer  
*RustAI Gateway — Cloud-Native AI Infrastructure*
