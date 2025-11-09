# LLM DevOps Ecosystem - Quick Reference Guide

**For:** Architecture Team and Developers
**Version:** 1.0
**Date:** 2025-11-09

---

## Document Overview

This quick reference provides essential information for integrating with the LLM DevOps ecosystem.

**Main Documents:**
- **ECOSYSTEM_RESEARCH.md** - Comprehensive research and specifications
- **INTEGRATION_EXAMPLES.md** - Code templates and implementation patterns
- **QUICK_REFERENCE.md** - This document (quick lookup)

---

## Component URLs and Endpoints

### Production Endpoints

| Component | gRPC Endpoint | REST API | Purpose |
|-----------|---------------|----------|---------|
| **LLM-Observatory** | `grpc://observatory:4317` | `https://observatory/api/v1` | Metrics & Telemetry |
| **LLM-Orchestrator** | `grpc://orchestrator:50051` | `https://orchestrator/api/v1` | Request Routing |
| **LLM-Sentinel** | `grpc://sentinel:50052` | `https://sentinel/api/v1` | Anomaly Detection |
| **LLM-Governance** | `grpc://governance:50053` | `https://governance/api/v1` | Policy & Compliance |
| **LLM-Registry** | `grpc://registry:50054` | `https://registry/api/v1` | Model Metadata |
| **LLM-Auto-Optimizer** | `grpc://auto-optimizer:50051` | `https://auto-optimizer/api/v1` | Optimization |

### Kafka Topics

| Topic | Purpose | Publishers | Subscribers |
|-------|---------|------------|-------------|
| `llm.requests` | Request events | Orchestrator | Observatory, Governance |
| `llm.responses` | Response events | Orchestrator | Observatory, Sentinel |
| `llm.metrics` | Metric events | Observatory | Auto-Optimizer, Sentinel |
| `llm.anomalies` | Anomaly events | Sentinel | Auto-Optimizer, Ops Team |
| `llm.optimizations` | Optimization events | Auto-Optimizer | Observatory, Registry |
| `llm.governance.violations` | Policy violations | Governance | Ops Team, Security |
| `llm.governance.alerts` | Budget/policy alerts | Governance | Auto-Optimizer, Finance |

---

## Authentication

### API Keys

```bash
# Set environment variables
export OBSERVATORY_API_KEY="obs_xxx"
export GOVERNANCE_API_KEY="gov_xxx"
export REGISTRY_API_KEY="reg_xxx"
```

### JWT Tokens

```bash
# Get token
curl -X POST https://auth.llm-platform.internal/token \
  -H "Content-Type: application/json" \
  -d '{"client_id":"your-client-id","client_secret":"your-secret"}'

# Use token
curl https://auto-optimizer/api/v1/optimize/prompt \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"prompt":"..."}'
```

---

## Common Integration Patterns

### 1. Send Telemetry to Observatory

```typescript
import { OTLPTraceExporter } from '@opentelemetry/exporter-trace-otlp-grpc';

const exporter = new OTLPTraceExporter({
  url: 'grpc://observatory:4317',
  headers: { 'x-api-key': process.env.OBSERVATORY_API_KEY }
});
```

### 2. Publish CloudEvent

```typescript
const event = {
  specversion: '1.0',
  type: 'io.llm-platform.optimization.applied',
  source: '/auto-optimizer',
  id: uuidv4(),
  time: new Date().toISOString(),
  datacontenttype: 'application/json',
  data: { /* your data */ }
};

await kafka.send({
  topic: 'llm.optimizations',
  messages: [{ value: JSON.stringify(event) }]
});
```

### 3. Call gRPC Service

```typescript
import * as grpc from '@grpc/grpc-js';

const client = new AutoOptimizerService(
  'auto-optimizer:50051',
  grpc.credentials.createInsecure()
);

client.optimizePrompt({
  request_id: 'req-123',
  prompt: 'Your prompt here',
  target_model: 'gpt-4',
  optimization_type: 'COMPRESSION'
}, (error, response) => {
  if (error) console.error(error);
  else console.log(response);
});
```

### 4. Query Metrics

```bash
curl "https://observatory/api/v1/metrics/query?metric_name=gen_ai.client.token.usage&start_time=2025-11-09T00:00:00Z&end_time=2025-11-09T23:59:59Z&aggregation=sum" \
  -H "Authorization: Bearer <token>"
```

### 5. Check Policy Compliance

```bash
curl -X POST https://governance/api/v1/policies/evaluate \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "context": {"team_id": "team-123"},
    "llm_request": {
      "model": "gpt-4",
      "estimated_cost_usd": 0.025
    }
  }'
```

---

## OpenTelemetry Semantic Conventions

### Trace Attributes (GenAI)

```typescript
{
  'gen_ai.system': 'openai',                    // Provider
  'gen_ai.request.model': 'gpt-4',              // Model ID
  'gen_ai.request.temperature': 0.7,            // Temperature
  'gen_ai.request.max_tokens': 500,             // Max tokens
  'gen_ai.response.finish_reasons': ['stop'],   // Finish reason
  'gen_ai.usage.completion_tokens': 245,        // Output tokens
  'gen_ai.usage.prompt_tokens': 128,            // Input tokens
  'gen_ai.usage.total_tokens': 373              // Total tokens
}
```

### Custom LLM Attributes

```typescript
{
  'llm.request_id': 'req_abc123',                    // Request ID
  'llm.user_id': 'user_456',                         // User ID
  'llm.application_name': 'chatbot-v2',              // App name
  'optimization.enabled': true,                       // Optimization flag
  'optimization.technique': 'prompt_compression',     // Technique used
  'optimization.tokens_saved': 250,                   // Tokens saved
  'optimization.cost_saved_usd': 0.0075,             // Cost saved
  'optimization.compression_ratio': 0.5               // Compression ratio
}
```

### Metric Names

```typescript
// Standard metrics
'gen_ai.client.token.usage'           // Token usage counter
'gen_ai.client.operation.duration'    // Operation duration histogram

// Custom metrics
'llm.optimization.runs'                // Optimization runs counter
'llm.optimization.duration'            // Optimization duration histogram
'llm.optimization.tokens_saved'        // Tokens saved counter
'llm.optimization.cost_saved'          // Cost saved counter (USD)
```

---

## CloudEvents Schema

### Basic Structure

```json
{
  "specversion": "1.0",
  "type": "io.llm-platform.{component}.{event}",
  "source": "/{component}/{subcomponent}",
  "id": "unique-event-id",
  "time": "2025-11-09T10:15:30.123Z",
  "datacontenttype": "application/json",
  "subject": "resource/resource-id",
  "data": { /* event data */ }
}
```

### Event Type Patterns

| Pattern | Example |
|---------|---------|
| Request lifecycle | `io.llm-platform.request.{received\|routed\|completed}` |
| Optimization | `io.llm-platform.optimization.{applied\|failed}` |
| Experiment | `io.llm-platform.experiment.{started\|completed}` |
| Anomaly | `io.llm-platform.sentinel.{alert\|anomaly-resolved}` |
| Governance | `io.llm-platform.governance.{policy-violation\|budget-alert}` |

---

## Model Capabilities Reference

### Required Capabilities Schema

```json
{
  "min_context_window": 32000,
  "features": [
    "text_generation",
    "function_calling",
    "json_mode",
    "vision",
    "streaming"
  ],
  "reasoning_capability": "advanced"
}
```

### Constraints Schema

```json
{
  "max_cost_per_request_usd": 0.05,
  "max_latency_p95_ms": 3000,
  "required_compliance": ["gdpr", "soc2"],
  "allowed_regions": ["us-east-1", "eu-west-1"]
}
```

---

## Cost Calculation

### Token-Based Pricing

```typescript
function calculateCost(tokens: {input: number, output: number}, model: string): number {
  const pricing = {
    'gpt-4': { input: 0.03, output: 0.06 },      // per 1K tokens
    'gpt-3.5-turbo': { input: 0.0015, output: 0.002 },
    'claude-3-opus': { input: 0.015, output: 0.075 },
    'claude-3-sonnet': { input: 0.003, output: 0.015 }
  };

  const rates = pricing[model] || { input: 0.01, output: 0.03 };
  return (tokens.input / 1000 * rates.input) +
         (tokens.output / 1000 * rates.output);
}
```

### Cost Savings Calculation

```typescript
function calculateSavings(
  originalTokens: number,
  optimizedTokens: number,
  model: string
): number {
  const tokensSaved = originalTokens - optimizedTokens;
  const costPer1k = model.includes('gpt-4') ? 0.03 : 0.01;
  return (tokensSaved / 1000) * costPer1k;
}
```

---

## Optimization Techniques

### Prompt Compression

```typescript
{
  technique: 'llmlingua',
  compressionRatio: 0.5,          // Target 50% compression
  minPromptLength: 500,            // Skip if < 500 tokens
  preserveQuestions: true,         // Keep questions intact
  expectedSavings: '30-50%'        // Cost reduction
}
```

### Model Cascading

```typescript
{
  technique: 'model_cascade',
  primaryModel: 'gpt-3.5-turbo',
  fallbackModel: 'gpt-4',
  escalationThreshold: {
    confidenceScore: 0.85,
    complexityScore: 0.7
  },
  expectedSavings: '40-60%'
}
```

### Semantic Caching

```typescript
{
  technique: 'semantic_cache',
  similarityThreshold: 0.92,      // 92% similarity required
  cacheTTL: 600,                  // 10 minutes
  expectedHitRate: '30-40%',
  expectedSavings: '20-35%'
}
```

---

## Error Handling

### Standard Error Response

```json
{
  "error": {
    "code": "OPTIMIZATION_FAILED",
    "message": "Prompt compression resulted in quality degradation",
    "details": {
      "quality_score": 0.65,
      "threshold": 0.85,
      "recommendation": "Try lower compression ratio"
    },
    "request_id": "req-abc123",
    "timestamp": "2025-11-09T10:15:30.123Z"
  }
}
```

### Error Codes

| Code | Description | Action |
|------|-------------|--------|
| `OPTIMIZATION_FAILED` | Optimization unsuccessful | Retry with different parameters |
| `POLICY_VIOLATION` | Request violates policy | Check governance rules |
| `BUDGET_EXCEEDED` | Budget limit reached | Contact finance team |
| `MODEL_UNAVAILABLE` | Requested model offline | Try alternative model |
| `INVALID_REQUEST` | Malformed request | Fix request format |
| `RATE_LIMIT_EXCEEDED` | Too many requests | Implement backoff |

---

## Health Checks

### Service Health Endpoints

```bash
# Liveness probe (is service running?)
curl http://auto-optimizer:8080/health

# Readiness probe (is service ready to accept traffic?)
curl http://auto-optimizer:8080/ready

# Metrics endpoint
curl http://auto-optimizer:9090/metrics
```

### Expected Responses

```json
// Healthy
{"status": "ok", "timestamp": "2025-11-09T10:15:30.123Z"}

// Unhealthy
{
  "status": "degraded",
  "checks": {
    "database": "ok",
    "kafka": "degraded",
    "registry": "ok"
  }
}
```

---

## Rate Limits

### Default Limits

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/optimize/prompt` | 100 req/min | Per API key |
| `/optimize/model-selection` | 200 req/min | Per API key |
| `/experiments` | 10 req/min | Per API key |
| `/metrics/query` | 500 req/min | Per API key |

### Rate Limit Headers

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 87
X-RateLimit-Reset: 1699564800
```

---

## Monitoring Queries

### Prometheus Queries

```promql
# Request rate
rate(gen_ai_client_requests_total[5m])

# Token usage
sum by (model) (gen_ai_client_token_usage)

# Optimization success rate
rate(llm_optimization_runs{status="success"}[5m]) /
rate(llm_optimization_runs[5m])

# Cost savings
sum(llm_optimization_cost_saved_total)

# P95 latency
histogram_quantile(0.95,
  rate(gen_ai_client_operation_duration_bucket[5m]))
```

### Alert Rules

```yaml
groups:
- name: llm_optimization
  rules:
  - alert: OptimizationFailureRate
    expr: rate(llm_optimization_runs{status="failure"}[5m]) > 0.1
    for: 5m
    annotations:
      summary: "High optimization failure rate"

  - alert: BudgetThresholdExceeded
    expr: llm_cost_total > llm_budget_limit * 0.8
    annotations:
      summary: "Budget threshold exceeded"
```

---

## Configuration Examples

### Environment Variables

```bash
# Required
OTEL_EXPORTER_OTLP_ENDPOINT=grpc://observatory:4317
KAFKA_BROKERS=kafka-0:9092,kafka-1:9092
REGISTRY_URL=https://registry.llm-platform.internal
GOVERNANCE_URL=https://governance.llm-platform.internal

# Optional
OPTIMIZATION_CACHE_TTL=600
OPTIMIZATION_MIN_PROMPT_LENGTH=500
OPTIMIZATION_DEFAULT_COMPRESSION_RATIO=0.5
EXPERIMENT_DEFAULT_DURATION_HOURS=168
```

### Configuration File

```yaml
# config/auto-optimizer.yaml
server:
  grpc_port: 50051
  http_port: 8080
  metrics_port: 9090

telemetry:
  export_endpoint: grpc://observatory:4317
  export_interval_seconds: 5
  sample_rate: 1.0

optimization:
  prompt_compression:
    enabled: true
    default_ratio: 0.5
    min_length: 500
    preserve_questions: true

  semantic_caching:
    enabled: true
    similarity_threshold: 0.92
    ttl_seconds: 600

  model_cascading:
    enabled: true
    default_primary: gpt-3.5-turbo
    default_fallback: gpt-4

experiments:
  default_duration_hours: 168
  min_traffic_percentage: 5
  max_concurrent_experiments: 10
```

---

## Troubleshooting

### Common Issues

**Issue: Telemetry not appearing**
```bash
# Check connection
curl grpc://observatory:4317

# Verify API key
echo $OBSERVATORY_API_KEY

# Check logs
kubectl logs -f deployment/auto-optimizer
```

**Issue: Events not being consumed**
```bash
# Check Kafka connection
kafka-console-consumer --bootstrap-server kafka:9092 --topic llm.optimizations

# Verify consumer group
kafka-consumer-groups --bootstrap-server kafka:9092 --describe --group auto-optimizer
```

**Issue: Policy violations**
```bash
# Check policy configuration
curl https://governance/api/v1/policies \
  -H "Authorization: Bearer <token>"

# Evaluate specific request
curl -X POST https://governance/api/v1/policies/evaluate \
  -H "Content-Type: application/json" \
  -d '{"context": {...}, "llm_request": {...}}'
```

---

## Best Practices

### 1. Always Instrument Operations

```typescript
await instrumentLLMRequest('operation.name', attributes, async (span) => {
  // Your operation
  span.setAttributes({ result: 'success' });
});
```

### 2. Use CloudEvents for All Events

```typescript
const event: CloudEvent = {
  specversion: '1.0',
  type: 'io.llm-platform.your.event',
  source: '/your-component',
  id: uuidv4(),
  time: new Date().toISOString(),
  data: { /* your data */ }
};
```

### 3. Implement Circuit Breakers

```typescript
const breaker = new CircuitBreaker(async () => {
  return await callExternalService();
}, {
  timeout: 3000,
  errorThresholdPercentage: 50,
  resetTimeout: 30000
});
```

### 4. Add Retry Logic

```typescript
async function retryWithBackoff<T>(
  fn: () => Promise<T>,
  maxRetries: number = 3
): Promise<T> {
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await fn();
    } catch (error) {
      if (i === maxRetries - 1) throw error;
      await delay(Math.pow(2, i) * 1000);
    }
  }
  throw new Error('Max retries exceeded');
}
```

### 5. Validate Inputs

```typescript
import Joi from 'joi';

const schema = Joi.object({
  prompt: Joi.string().required().min(10).max(100000),
  model: Joi.string().required(),
  optimization_type: Joi.string().valid('compression', 'enhancement')
});

const { error, value } = schema.validate(request);
if (error) throw new Error(error.message);
```

---

## Support and Resources

### Documentation
- Main Research: `ECOSYSTEM_RESEARCH.md`
- Code Examples: `INTEGRATION_EXAMPLES.md`
- API Docs: `https://docs.llm-platform.internal`

### Contacts
- Platform Team: platform-team@company.com
- LLMOps Team: llmops@company.com
- On-call: Slack #llm-platform-oncall

### External Resources
- OpenTelemetry: https://opentelemetry.io/docs/
- CloudEvents: https://cloudevents.io/
- gRPC: https://grpc.io/docs/
- Kafka: https://kafka.apache.org/documentation/

---

**Quick Reference v1.0** | Last Updated: 2025-11-09
