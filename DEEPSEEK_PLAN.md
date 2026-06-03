# DeepSeek Integration Plan

**Author:** Thomas McAvoy — Chief Performance Engineer

---

## 1. Strategic Overview

This document outlines the complete plan to integrate **DeepSeek** (V3 and R1 models) into the RustAI Gateway as a first-class AI provider. DeepSeek's Mixture-of-Experts (MoE) architecture presents unique routing, performance, and observability requirements that must be addressed at the gateway layer.

### Why DeepSeek?

| Factor | Benefit |
|--------|---------|
| **MoE Architecture** | 671B total params, 37B activated per token — requires smart request distribution |
| **Cost Efficiency** | ~$0.14/M input tokens (V3) — 10-20x cheaper than GPT-4 |
| **Open Weights** | Self-hostable, enables air-gapped deployments |
| **R1 Reasoning** | Chain-of-thought reasoning model with 128k context window |
| **API Compatibility** | OpenAI-compatible API format — minimal adapter overhead |

### Key Performance Targets

| Metric | Target |
|--------|--------|
| P50 Latency (non-streaming) | < 500ms |
| P99 Latency (non-streaming) | < 2s |
| Streaming TTFT (Time-to-First-Token) | < 200ms |
| Concurrent Connections | 10,000+ |
| Throughput (tokens/sec) | > 1,000 TPS per upstream |
| Error Rate (5xx) | < 0.1% |

---

## 2. Enhancement Inventory

### Phase 1: Core DeepSeek Provider Support

| # | Enhancement | Priority | Effort | Description |
|---|-------------|----------|--------|-------------|
| 1.1 | DeepSeek Provider Adapter | P0 | 2 days | Implement [`DeepSeekAdapter`](crates/router/src/provider.rs) matching OpenAI API with DeepSeek-specific headers (e.g., `x-deepseek-model`) |
| 1.2 | MoE-Aware Connection Pooling | P0 | 3 days | DeepSeek's MoE architecture benefits from persistent connections — implement connection stickiness and weighted pool distribution |
| 1.3 | R1 Reasoning Stream Parser | P1 | 2 days | R1 emits chain-of-thought tokens before final answer — parse and optionally stream intermediate reasoning |
| 1.4 | DeepSeek-Specific Error Mapping | P1 | 1 day | Map DeepSeek error codes (rate limits, quota, model overload) to standardized GatewayError variants |
| 1.5 | Token Counting Middleware | P2 | 2 days | Implement accurate token counting for DeepSeek models (MoE-aware), emit as `x-request-tokens` header |

### Phase 2: Performance Optimization for MoE

| # | Enhancement | Priority | Effort | Description |
|---|-------------|----------|--------|-------------|
| 2.1 | Adaptive Connection Pooling | P0 | 4 days | Dynamic pool sizing based on MoE expert utilization — DeepSeek activates different experts per request type |
| 2.2 | Request Batching (Dynamic Batching) | P0 | 5 days | Implement request coalescing: batch compatible requests into single DeepSeek calls to maximize expert utilization |
| 2.3 | Speculative Decoding Support | P1 | 5 days | Add draft model integration for speculative decoding — use smaller model (e.g., DeepSeek-Chat-1.5B) as drafter |
| 2.4 | Prefix Caching at Gateway | P1 | 4 days | Cache common prompt prefixes (system prompts, few-shot examples) to reduce redundant processing |
| 2.5 | KV-Cache-Aware Load Balancing | P2 | 3 days | Route requests to upstream instances where relevant KV cache is already warm |
| 2.6 | Expert-Aware Request Routing | P2 | 5 days | MoE expert routing: analyze request patterns and route to instances with pre-loaded experts |

### Phase 3: Observability & Monitoring

| # | Enhancement | Priority | Effort | Description |
|---|-------------|----------|--------|-------------|
| 3.1 | MoE-Specific Metrics | P1 | 3 days | Add Prometheus metrics for: expert utilization ratio, activated experts per request, batch efficiency |
| 3.2 | Token-Level Latency Tracing | P1 | 3 days | OpenTelemetry spans for TTFT, inter-token latency, end-to-end streaming duration |
| 3.3 | Cost Attribution Per Model/MoE | P2 | 2 days | Track token costs broken down by model variant (V3 vs R1), enabling chargeback/showback |
| 3.4 | DeepSeek Health Probes | P1 | 2 days | Custom health check for DeepSeek upstreams — probe model availability, expert load, queue depth |
| 3.5 | Anomaly Detection Dashboard | P2 | 4 days | Grafana dashboard for: latency anomalies, expert imbalance, routing hot spots, error rate spikes |

### Phase 4: Advanced Routing & Failover

| # | Enhancement | Priority | Effort | Description |
|---|-------------|----------|--------|-------------|
| 4.1 | Model-Aware Route Selection | P0 | 3 days | Route based on model capabilities: V3 for chat, R1 for reasoning, fallback to OpenAI if DeepSeek degraded |
| 4.2 | Circuit Breaker for DeepSeek | P1 | 2 days | Implement circuit breaker pattern: track error rates, open circuit when threshold exceeded, gradual recovery |
| 4.3 | A/B Model Testing | P2 | 3 days | Route percentage of traffic to different model versions for performance comparison |
| 4.4 | Geographic Routing | P2 | 3 days | Route based on client region: DeepSeek servers in China vs OpenAI in US, with latency-based selection |
| 4.5 | Content-Based Routing | P2 | 4 days | Route based on content type: math/reasoning -> R1, general chat -> V3, code -> DeepSeek-Coder |

### Phase 5: Security & Governance

| # | Enhancement | Priority | Effort | Description |
|---|-------------|----------|--------|-------------|
| 5.1 | DeepSeek API Key Vault Integration | P0 | 2 days | Integrate with HashiCorp Vault / AWS Secrets Manager for DeepSeek API key management |
| 5.2 | Input/Output Guardrails for DeepSeek | P1 | 3 days | Content filtering specific to DeepSeek models — PII masking, prompt injection detection |
| 5.3 | Audit Logging for DeepSeek Requests | P1 | 2 days | Comprehensive audit trail: model, tokens, cost, latency, user, timestamp |
| 5.4 | Rate Limiting per Model Tier | P1 | 2 days | Different rate limits for V3 vs R1 — R1 is compute-heavy, V3 is cost-optimized |
| 5.5 | Data Residency Controls | P2 | 2 days | Ensure DeepSeek requests route through compliant regions (GDPR, SOC2, FedRAMP) |

### Phase 6: Infrastructure & Deployment

| # | Enhancement | Priority | Effort | Description |
|---|-------------|----------|--------|-------------|
| 6.1 | DeepSeek Upstream Config Template | P0 | 1 day | Example [`rustai.toml`](rustai.toml) configuration for DeepSeek upstreams (self-hosted + API) |
| 6.2 | DeepSeek Gateway API CRD | P1 | 2 days | Kubernetes [`AiGateway`](crates/k8s-gateway/src/crd.rs) custom resource for DeepSeek with model-specific fields |
| 6.3 | Self-Hosted Deployment Guide | P1 | 2 days | Docker Compose + Kubernetes manifests for deploying DeepSeek models alongside the gateway |
| 6.4 | Auto-Scaling for MoE Workloads | P2 | 4 days | KEDA-based auto-scaling: scale gateway replicas based on DeepSeek queue depth and latency |
| 6.5 | Canary Deployment Pipeline | P2 | 3 days | ArgoCD / Flux pipeline for canary deployments of DeepSeek adapter changes |

---

## 3. Sprint Roadmap

### Sprint 1: DeepSeek Core (2 weeks)
**Focus:** Basic connectivity, provider adapter, connection management

```
Sprint 1 Backlog
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
[P0] 1.1 DeepSeek Provider Adapter              ┃ 2 days
[P0] 1.2 MoE-Aware Connection Pooling           ┃ 3 days
[P0] 4.1 Model-Aware Route Selection            ┃ 3 days
[P0] 5.1 API Key Vault Integration              ┃ 2 days
[P0] 6.1 DeepSeek Config Template               ┃ 1 day
─────────────────────────────────────────────────────────
Total: 11 days  │  Buffer: 3 days  │  Sprint: 14 days
```

**Definition of Done:**
- [`cargo check`] passes with DeepSeek adapter
- `curl` test against DeepSeek API returns streaming response
- DeepSeek upstream configurable via `rustai.toml`
- API key rotation via environment variable
- Prometheus metrics show DeepSeek-specific labels

### Sprint 2: Performance & Streaming (2 weeks)
**Focus:** R1 reasoning streams, latency optimization, token metrics

```
Sprint 2 Backlog
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
[P1] 1.3 R1 Reasoning Stream Parser             ┃ 2 days
[P2] 1.5 Token Counting Middleware              ┃ 2 days
[P0] 2.1 Adaptive Connection Pooling            ┃ 4 days
[P0] 2.2 Request Batching                       ┃ 5 days
[P1] 3.2 Token-Level Latency Tracing            ┃ 3 days
─────────────────────────────────────────────────────────
Total: 16 days  │  Buffer: 2 days  │  Sprint: 18 days
```

**Definition of Done:**
- R1 chain-of-thought streaming works end-to-end
- Dynamic batching achieves > 2x throughput improvement
- OpenTelemetry traces show TTFT, inter-token latency
- `x-request-tokens` header present on all responses

### Sprint 3: Observability & Resilience (2 weeks)
**Focus:** Metrics, health probes, circuit breaker, failover

```
Sprint 3 Backlog
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
[P1] 3.1 MoE-Specific Prometheus Metrics        ┃ 3 days
[P2] 3.3 Cost Attribution                       ┃ 2 days
[P1] 3.4 DeepSeek Health Probes                 ┃ 2 days
[P1] 4.2 Circuit Breaker                        ┃ 2 days
[P1] 5.2 Input/Output Guardrails                ┃ 3 days
[P1] 5.3 Audit Logging                          ┃ 2 days
─────────────────────────────────────────────────────────
Total: 14 days  │  Buffer: 4 days  │  Sprint: 18 days
```

**Definition of Done:**
- Grafana dashboard with DeepSeek-specific panels
- Circuit breaker protects upstream from cascading failures
- Audit logs capture all DeepSeek requests with model info
- Content guardrails actively filter PII

### Sprint 4: Advanced Features (2 weeks)
**Focus:** Speculative decoding, prefix caching, canary deployments

```
Sprint 4 Backlog
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
[P1] 2.3 Speculative Decoding Support           ┃ 5 days
[P1] 2.4 Prefix Caching                         ┃ 4 days
[P2] 2.6 Expert-Aware Routing                   ┃ 5 days
[P2] 3.5 Anomaly Detection Dashboard            ┃ 4 days
[P2] 4.3 A/B Model Testing                      ┃ 3 days
[P2] 6.4 Auto-Scaling for MoE                   ┃ 4 days
─────────────────────────────────────────────────────────
Total: 25 days  │  Buffer: 5 days  │  Sprint: 30 days
```

**Definition of Done:**
- Speculative decoding yields > 1.5x throughput improvement
- Prefix caching reduces latency for repeated system prompts
- Anomaly detection alerts on expert imbalance
- Auto-scaling responds to DeepSeek queue depth

### Sprint 5: Production Hardening (2 weeks)
**Focus:** Kubernetes integration, geographic routing, data residency

```
Sprint 5 Backlog
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
[P1] 6.2 DeepSeek Gateway API CRD               ┃ 2 days
[P1] 6.3 Self-Hosted Deployment Guide           ┃ 2 days
[P2] 4.4 Geographic Routing                     ┃ 3 days
[P2] 4.5 Content-Based Routing                  ┃ 4 days
[P1] 5.4 Rate Limiting per Model Tier           ┃ 2 days
[P2] 6.5 Canary Deployment Pipeline             ┃ 3 days
─────────────────────────────────────────────────────────
Total: 16 days  │  Buffer: 2 days  │  Sprint: 18 days
```

**Definition of Done:**
- `kubectl apply -f deepseek-gateway.yaml` provisions DeepSeek upstream
- Geographic routing minimizes latency for global deployments
- Content-based routing correctly dispatches V3 vs R1
- Canary pipeline deploys adapter changes with zero downtime

---

## 4. Architecture Changes

### Provider Adapter Implementation

```
Current (OpenAI):
  Request ──► OpenAIAdapter ──► POST api.openai.com/v1/chat/completions

New (DeepSeek):
  Request ──► DeepSeekAdapter ──► POST api.deepseek.com/v1/chat/completions
                                       │
                                       ├── Header: x-deepseek-model → "deepseek-chat"
                                       ├── Header: x-deepseek-reasoning → "true" (R1)
                                       └── Body: stream_options: { include_usage: true }
```

The [`DeepSeekAdapter`](crates/router/src/provider.rs:1) will implement the same [`ProviderAdapter`](crates/core/src/traits.rs:1) trait:

```rust
pub struct DeepSeekAdapter;  // ~50 lines of translation logic

impl ProviderAdapter for DeepSeekAdapter {
    fn provider(&self) -> Provider { Provider::Custom("deepseek".into()) }
    fn translate_request(&self, request: LlmRequest) -> GatewayResult<Value> { ... }
    fn translate_response(&self, response: Value) -> GatewayResult<Value> { ... }
    fn parse_sse_chunk(&self, chunk: &[u8]) -> GatewayResult<Option<SseEvent>> { ... }
}
```

### Key Architectural Decisions

| Decision | Rationale |
|----------|-----------|
| **Reuse OpenAI adapter structure** | DeepSeek API is OpenAI-compatible — minimal delta |
| **New MoE-aware connection pool** | DeepSeek's MoE benefits from sticky connections to warm expert instances |
| **Separate R1 stream parser** | R1 emits reasoning tokens as distinct SSE events before final content |
| **Model-aware routing at gateway** | Avoids round-trip to DeepSeek to determine model capabilities |
| **Circuit breaker per upstream** | DeepSeek API can experience regional instability |

---

## 5. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| DeepSeek API rate limits (100 RPM free tier) | High | Medium | Implement tiered rate limiting, burst queue |
| DeepSeek China-based servers latency | Medium | High | Geographic routing, self-hosted DeepSeek option |
| DeepSeek API format changes | Low | Medium | Version-pinned adapter, integration tests |
| MoE expert imbalance degrading throughput | Medium | Medium | Expert-aware routing, pool monitoring |
| R1 reasoning stream parsing errors | Medium | Low | Graceful fallback to non-reasoning stream |
| DeepSeek model deprecation | Low | Low | Configurable model field, easy provider swap |

---

## 6. Success Metrics

### Performance Benchmarks (Pre/Post)

| Metric | Current (OpenAI) | Target (DeepSeek) | Measurement |
|--------|-----------------|-------------------|-------------|
| P50 TTFT | 150ms | 200ms | `rustai_upstream_latency_seconds` |
| P50 End-to-End | 800ms | 1.2s | `rustai_request_duration_seconds` |
| Throughput | 500 TPS | 1,000 TPS | `rustai_requests_total` rate |
| Cost/Tokens | $0.01/1K | $0.00014/1K | Cost attribution metrics |
| Error Rate | 0.05% | < 0.1% | `rustai_upstream_errors_total` |

### Go/No-Go Gates per Sprint

| Sprint | Gate | Criteria |
|--------|------|----------|
| Sprint 1 | Basic connectivity | `curl` test passes, no compilation errors |
| Sprint 2 | Streaming quality | R1 reasoning renders correctly, latency within 2x of OpenAI |
| Sprint 3 | Observability | Dashboards show all DeepSeek metrics, alerts configured |
| Sprint 4 | Performance | Throughput > 800 TPS, P99 < 2s |
| Sprint 5 | Production readiness | K8s deployment, canary, rollback tested |

---

## 7. Quick Start: DeepSeek Integration

```toml
# rustai.toml — DeepSeek upstream configuration
[[upstreams]]
id = "deepseek-api"
provider = "deepseek"
protocol = "rest"
base_url = "https://api.deepseek.com"
api_key = "${DEEPSEEK_API_KEY}"
timeout_secs = 60
max_connections = 50
headers = { x-deepseek-model = "deepseek-chat" }

[[upstreams]]
id = "deepseek-r1"
provider = "deepseek"
protocol = "rest"
base_url = "https://api.deepseek.com"
api_key = "${DEEPSEEK_API_KEY}"
timeout_secs = 120  # R1 takes longer for reasoning
max_connections = 25
headers = { x-deepseek-model = "deepseek-reasoner" }

[[routes]]
id = "deepseek-chat"
path_pattern = "/v1/chat/completions"
methods = ["POST"]
upstreams = ["deepseek-api", "deepseek-r1"]
# Auto-route: general chat → deepseek-chat, reasoning → deepseek-r1
```

```bash
# Test DeepSeek integration
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "X-Provider: deepseek" \
  -d '{
    "model": "deepseek-chat",
    "messages": [{"role": "user", "content": "Hello, DeepSeek!"}],
    "stream": true
  }'
```

---

**Thomas McAvoy** — Chief Performance Engineer  
*RustAI Gateway — DeepSeek Integration Program*
