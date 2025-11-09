# Data Flow and Interface Specifications

## Overview

This document details the data flow patterns, API interfaces, and integration points for the LLM Auto-Optimizer system.

---

## Table of Contents

1. [Data Flow Patterns](#1-data-flow-patterns)
2. [API Interfaces](#2-api-interfaces)
3. [Event Schemas](#3-event-schemas)
4. [Integration Patterns](#4-integration-patterns)
5. [Client SDKs](#5-client-sdks)

---

## 1. Data Flow Patterns

### 1.1 Request Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    Request Processing Flow                   │
└─────────────────────────────────────────────────────────────┘

User Request
    │
    ▼
┌─────────────────────┐
│  Application        │
│  Load Balancer      │
└──────────┬──────────┘
           │
           ├─────────────────────────────────────────┐
           │                                         │
           ▼                                         ▼
    ┌─────────────┐                         ┌─────────────┐
    │ App Instance│                         │ Optimizer   │
    │     #1      │                         │  (Sidecar)  │
    └──────┬──────┘                         └──────┬──────┘
           │                                        │
           │ 1. Get Current Config ◄───────────────┤
           │                                        │
           │ 2. Config Response ────────────────────►
           │                                        │
           ▼                                        │
    ┌─────────────┐                                │
    │ LLM Provider│                                │
    │  (Anthropic)│                                │
    └──────┬──────┘                                │
           │                                        │
           │ 3. LLM Response                        │
           │                                        │
           ▼                                        │
    ┌─────────────┐                                │
    │  Response   │                                │
    │  to User    │                                │
    └─────────────┘                                │
           │                                        │
           │ 4. Report Metrics ─────────────────────►
           │                                        │
           │                                        ▼
           │                              ┌─────────────────┐
           │                              │  Metrics        │
           │                              │  Aggregation    │
           │                              └────────┬────────┘
           │                                       │
           │                                       ▼
           │                              ┌─────────────────┐
           │                              │  Time-Series DB │
           │                              └─────────────────┘
```

### 1.2 Optimization Flow

```
┌─────────────────────────────────────────────────────────────┐
│                  Optimization Decision Flow                  │
└─────────────────────────────────────────────────────────────┘

Timer/Trigger Event
    │
    ▼
┌─────────────────────┐
│  Control Loop       │
│  Scheduler          │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────┐
│                    OBSERVE Phase                             │
│                                                               │
│  ┌──────────────┐        ┌──────────────┐                   │
│  │ Query Metrics│───────►│  Aggregate   │                   │
│  │  from DB     │        │  & Enrich    │                   │
│  └──────────────┘        └──────┬───────┘                   │
│                                  │                            │
└──────────────────────────────────┼────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────┐
│                    ORIENT Phase                              │
│                                                               │
│  ┌──────────────┐        ┌──────────────┐                   │
│  │   Analyze    │───────►│   Detect     │                   │
│  │   Trends     │        │  Anomalies   │                   │
│  └──────┬───────┘        └──────┬───────┘                   │
│         │                       │                            │
│         └───────────┬───────────┘                            │
│                     │                                         │
│                     ▼                                         │
│         ┌──────────────────────┐                             │
│         │  Compare to Baseline │                             │
│         └──────────┬───────────┘                             │
│                    │                                          │
└────────────────────┼──────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                    DECIDE Phase                              │
│                                                               │
│  ┌──────────────┐        ┌──────────────┐                   │
│  │  Evaluate    │───────►│   Generate   │                   │
│  │   Rules      │        │   ML Plan    │                   │
│  └──────┬───────┘        └──────┬───────┘                   │
│         │                       │                            │
│         └───────────┬───────────┘                            │
│                     │                                         │
│                     ▼                                         │
│         ┌──────────────────────┐                             │
│         │  Validate Constraints│                             │
│         └──────────┬───────────┘                             │
│                    │                                          │
│                    ▼                                          │
│         ┌──────────────────────┐                             │
│         │  Select Best Plan    │                             │
│         └──────────┬───────────┘                             │
│                    │                                          │
└────────────────────┼──────────────────────────────────────────┘
                     │
                     ├──── No Action ────► IDLE
                     │
                     └──── Has Plan ────►
                                          │
                                          ▼
┌─────────────────────────────────────────────────────────────┐
│                     ACT Phase                                │
│                                                               │
│  ┌──────────────┐        ┌──────────────┐                   │
│  │   Update     │───────►│   Deploy     │                   │
│  │   Config     │        │   Canary     │                   │
│  └──────────────┘        └──────┬───────┘                   │
│                                  │                            │
│                                  ▼                            │
│                       ┌──────────────────┐                   │
│                       │  Monitor Health  │                   │
│                       └────────┬─────────┘                   │
│                                │                              │
│                    ┌───────────┴────────────┐                │
│                    │                        │                │
│                    ▼                        ▼                │
│            ┌──────────────┐        ┌──────────────┐         │
│            │   Success    │        │   Rollback   │         │
│            │   Complete   │        │   Triggered  │         │
│            └──────────────┘        └──────────────┘         │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

### 1.3 Metrics Pipeline

```
┌─────────────────────────────────────────────────────────────┐
│                    Metrics Data Pipeline                     │
└─────────────────────────────────────────────────────────────┘

Application
    │
    │ HTTP POST /metrics
    │
    ▼
┌─────────────────────┐
│  Metrics Ingestion  │
│      API            │
└──────────┬──────────┘
           │
           │ Validate & Parse
           │
           ▼
┌─────────────────────┐
│   Ring Buffer       │
│   (In-Memory)       │
└──────────┬──────────┘
           │
           │ Batch (100 events or 30s)
           │
           ▼
┌─────────────────────┐
│   Enrichment        │
│   Pipeline          │
└──────────┬──────────┘
           │
           ├──────────────────────────────┐
           │                              │
           ▼                              ▼
┌─────────────────────┐        ┌─────────────────────┐
│  Time-Series DB     │        │   Redis Cache       │
│  (Long-term)        │        │   (Recent data)     │
└─────────────────────┘        └─────────────────────┘
           │                              │
           │                              │
           └──────────┬───────────────────┘
                      │
                      │ Query for Analysis
                      │
                      ▼
            ┌─────────────────────┐
            │  Analysis Engine    │
            └─────────────────────┘
```

### 1.4 Configuration Update Flow

```
┌─────────────────────────────────────────────────────────────┐
│                Configuration Update Flow                     │
└─────────────────────────────────────────────────────────────┘

Decision Engine
    │
    │ Proposed Config
    │
    ▼
┌─────────────────────┐
│  Constraint         │
│  Validator          │
└──────────┬──────────┘
           │
           │ Valid?
           │
           ├──── No ────► Reject & Alert
           │
           │ Yes
           ▼
┌─────────────────────┐
│  Create Version     │
│  in Database        │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  Deployment         │
│  Manager            │
└──────────┬──────────┘
           │
           │ Canary Strategy
           │
           ├──── Step 1: 5% ────►
           │
           │ Monitor (5 min)
           │
           ├──────┬─────────────────┐
           │      │                 │
           │  Healthy          Unhealthy
           │      │                 │
           │      ▼                 ▼
           │   Next Step       Rollback
           │
           └──── Step 2: 25% ────►
           │
           └──── Step 3: 50% ────►
           │
           └──── Step 4: 100% ───►
                      │
                      ▼
            ┌─────────────────────┐
            │  Update Active      │
            │  Config Flag        │
            └──────────┬──────────┘
                       │
                       ▼
            ┌─────────────────────┐
            │  Publish Event      │
            │  to Applications    │
            └──────────┬──────────┘
                       │
                       ▼
            ┌─────────────────────┐
            │  Applications       │
            │  Fetch New Config   │
            └─────────────────────┘
```

---

## 2. API Interfaces

### 2.1 Metrics Ingestion API

**Endpoint**: `POST /api/v1/metrics`

**Request**:
```typescript
interface MetricsRequest {
  // Batch of metrics
  metrics: Array<{
    timestamp: number;
    service: string;
    endpoint: string;
    model: string;
    provider: string;

    // Performance
    latencyMs: number;
    tokensInput: number;
    tokensOutput: number;
    tokensTotal: number;

    // Cost
    costUsd: number;

    // Status
    statusCode: number;
    errorType?: string;
    retryCount?: number;

    // Optional tags
    tags?: Record<string, string>;
  }>;
}
```

**Response**:
```typescript
interface MetricsResponse {
  accepted: number;
  rejected: number;
  errors?: Array<{
    index: number;
    reason: string;
  }>;
}
```

**Example**:
```bash
curl -X POST http://optimizer:8080/api/v1/metrics \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "metrics": [
      {
        "timestamp": 1699564800000,
        "service": "chat-service",
        "endpoint": "/api/chat",
        "model": "claude-sonnet-4",
        "provider": "anthropic",
        "latencyMs": 1234,
        "tokensInput": 150,
        "tokensOutput": 500,
        "tokensTotal": 650,
        "costUsd": 0.0325,
        "statusCode": 200
      }
    ]
  }'
```

### 2.2 Configuration API

**Get Current Configuration**

`GET /api/v1/config/current`

```typescript
interface ConfigResponse {
  version: string;
  config: LLMConfig;
  appliedAt: number;
  appliedBy: string;
}
```

**Get Configuration for Service**

`GET /api/v1/config/service/{serviceName}`

```typescript
interface ServiceConfigResponse {
  service: string;
  config: LLMConfig;
  overrides?: Record<string, any>;
}
```

**Manual Configuration Update**

`POST /api/v1/config/update`

```typescript
interface ConfigUpdateRequest {
  config: LLMConfig;
  reason: string;
  deploymentStrategy?: 'immediate' | 'canary' | 'gradual';
  validateOnly?: boolean;
}

interface ConfigUpdateResponse {
  success: boolean;
  version?: string;
  validationResult: ValidationResult;
  deploymentId?: string;
}
```

### 2.3 Optimization API

**Trigger Manual Optimization**

`POST /api/v1/optimize/trigger`

```typescript
interface OptimizationRequest {
  services?: string[];  // Specific services, or all if empty
  objectives?: Objective[];
  constraints?: Constraint[];
  dryRun?: boolean;
}

interface OptimizationResponse {
  optimizationId: string;
  status: 'queued' | 'running' | 'completed' | 'failed';
  plan?: OptimizationPlan;
  eta?: number;  // ms until completion
}
```

**Get Optimization Status**

`GET /api/v1/optimize/{optimizationId}`

```typescript
interface OptimizationStatusResponse {
  id: string;
  status: 'queued' | 'running' | 'completed' | 'failed';
  progress: number;  // 0-100
  currentPhase: 'observe' | 'analyze' | 'decide' | 'act';
  result?: OptimizationResult;
  error?: string;
}
```

### 2.4 Analytics API

**Get Performance Metrics**

`GET /api/v1/analytics/metrics`

```typescript
interface MetricsQuery {
  services?: string[];
  metrics: string[];  // ['latency.p95', 'cost.total', etc.]
  timeRange: {
    start: number;
    end: number;
  };
  aggregation?: 'avg' | 'sum' | 'min' | 'max' | 'p95' | 'p99';
  groupBy?: string[];  // ['service', 'model', etc.]
}

interface MetricsQueryResponse {
  metrics: Array<{
    metric: string;
    dataPoints: Array<{
      timestamp: number;
      value: number;
      dimensions?: Record<string, string>;
    }>;
  }>;
}
```

**Get Optimization History**

`GET /api/v1/analytics/history`

```typescript
interface HistoryQuery {
  limit?: number;
  offset?: number;
  services?: string[];
  timeRange?: {
    start: number;
    end: number;
  };
}

interface HistoryResponse {
  total: number;
  events: OptimizationEvent[];
}
```

**Get Baselines**

`GET /api/v1/analytics/baselines`

```typescript
interface BaselineResponse {
  baselines: Array<{
    name: string;
    description: string;
    createdAt: number;
    metrics: Record<string, number>;
    sampleSize: number;
  }>;
}
```

### 2.5 Admin API

**Health Check**

`GET /api/v1/health`

```typescript
interface HealthResponse {
  status: 'healthy' | 'degraded' | 'unhealthy';
  version: string;
  uptime: number;
  checks: Array<{
    name: string;
    healthy: boolean;
    message?: string;
    latency?: number;
  }>;
}
```

**System Status**

`GET /api/v1/status`

```typescript
interface StatusResponse {
  mode: 'sidecar' | 'standalone' | 'daemon';
  controlLoop: {
    state: ControlLoopState;
    lastRun: number;
    nextRun: number;
  };
  deployments: {
    active: number;
    pending: number;
  };
  metrics: {
    bufferSize: number;
    ingestionRate: number;  // events/second
    lastFlush: number;
  };
  resources: {
    memoryUsageMb: number;
    cpuUsagePercent: number;
  };
}
```

**Pause/Resume Optimization**

`POST /api/v1/admin/pause`
`POST /api/v1/admin/resume`

```typescript
interface PauseResponse {
  paused: boolean;
  pausedAt?: number;
  reason?: string;
}
```

**Emergency Stop**

`POST /api/v1/admin/emergency-stop`

```typescript
interface EmergencyStopRequest {
  reason: string;
}

interface EmergencyStopResponse {
  stopped: boolean;
  safeMode: boolean;
  timestamp: number;
}
```

---

## 3. Event Schemas

### 3.1 Internal Events

**Config Change Event**

```typescript
interface ConfigChangeEvent {
  id: string;
  type: 'config.changed';
  timestamp: number;
  data: {
    previousVersion: string;
    newVersion: string;
    previousConfig: LLMConfig;
    newConfig: LLMConfig;
    changeType: 'manual' | 'automatic';
    changedBy: string;
    reason: string;
  };
}
```

**Optimization Complete Event**

```typescript
interface OptimizationCompleteEvent {
  id: string;
  type: 'optimization.complete';
  timestamp: number;
  data: {
    optimizationId: string;
    decision: 'optimize' | 'skip' | 'rollback';
    duration: number;
    changesApplied: boolean;
    improvements?: Record<string, number>;
  };
}
```

**Deployment Event**

```typescript
interface DeploymentEvent {
  id: string;
  type: 'deployment.started' | 'deployment.progressed' | 'deployment.completed' | 'deployment.failed';
  timestamp: number;
  data: {
    deploymentId: string;
    configVersion: string;
    strategy: string;
    progress: number;
    currentStep?: string;
    error?: string;
  };
}
```

**Rollback Event**

```typescript
interface RollbackEvent {
  id: string;
  type: 'rollback.triggered' | 'rollback.completed';
  timestamp: number;
  data: {
    deploymentId: string;
    fromVersion: string;
    toVersion: string;
    reason: string;
    automatic: boolean;
    triggers?: Array<{
      metric: string;
      threshold: number;
      actual: number;
    }>;
  };
}
```

**Anomaly Detected Event**

```typescript
interface AnomalyEvent {
  id: string;
  type: 'anomaly.detected';
  timestamp: number;
  data: {
    metric: string;
    value: number;
    expectedRange: { min: number; max: number };
    severity: 'low' | 'medium' | 'high' | 'critical';
    confidence: number;
    detectorType: string;
  };
}
```

### 3.2 Webhook Events

The optimizer can send webhook notifications for important events:

**Webhook Payload**

```typescript
interface WebhookPayload {
  event: Event;
  metadata: {
    optimizer: {
      version: string;
      mode: string;
      instanceId: string;
    };
    timestamp: number;
  };
}
```

**Webhook Configuration**

```yaml
webhooks:
  - url: https://api.example.com/webhooks/optimizer
    events:
      - config.changed
      - optimization.complete
      - deployment.completed
      - deployment.failed
      - rollback.triggered
    headers:
      Authorization: "Bearer <token>"
    retryPolicy:
      maxRetries: 3
      backoff: exponential
```

---

## 4. Integration Patterns

### 4.1 Pull-Based Integration

Application periodically polls for configuration updates:

```typescript
// Application code
import { OptimizerClient } from 'llm-auto-optimizer/client';

const client = new OptimizerClient({
  endpoint: 'http://optimizer:8080',
  pollInterval: 30000  // 30 seconds
});

// Start polling
client.startConfigSync((newConfig) => {
  console.log('Config updated:', newConfig);
  updateLLMClient(newConfig);
});

// Use current config
const config = client.getCurrentConfig();
```

### 4.2 Push-Based Integration

Optimizer pushes updates via webhooks or events:

```typescript
// Application code
import { createServer } from 'http';

const server = createServer((req, res) => {
  if (req.url === '/webhooks/optimizer' && req.method === 'POST') {
    let body = '';
    req.on('data', chunk => body += chunk);
    req.on('end', () => {
      const event = JSON.parse(body);

      if (event.type === 'config.changed') {
        updateLLMClient(event.data.newConfig);
      }

      res.writeHead(200);
      res.end();
    });
  }
});

server.listen(3000);
```

### 4.3 Sidecar Integration

Direct localhost communication:

```typescript
// Application code
import axios from 'axios';

const OPTIMIZER_ENDPOINT = 'http://localhost:9090';

async function getLLMConfig(): Promise<LLMConfig> {
  const response = await axios.get(`${OPTIMIZER_ENDPOINT}/api/v1/config/current`);
  return response.data.config;
}

async function reportMetrics(metrics: MetricEvent): Promise<void> {
  await axios.post(`${OPTIMIZER_ENDPOINT}/api/v1/metrics`, {
    metrics: [metrics]
  });
}

// Use in application
const config = await getLLMConfig();
const response = await callLLM(prompt, config);
await reportMetrics({
  timestamp: Date.now(),
  service: 'my-service',
  latencyMs: response.latency,
  // ... other metrics
});
```

### 4.4 gRPC Integration

For high-performance scenarios:

```protobuf
// optimizer.proto
syntax = "proto3";

package optimizer;

service OptimizerService {
  rpc GetConfig(ConfigRequest) returns (ConfigResponse);
  rpc ReportMetrics(MetricsRequest) returns (MetricsResponse);
  rpc StreamConfig(ConfigStreamRequest) returns (stream ConfigUpdate);
}

message ConfigRequest {
  string service = 1;
  string endpoint = 2;
}

message ConfigResponse {
  string version = 1;
  LLMConfig config = 2;
}

message MetricsRequest {
  repeated Metric metrics = 1;
}

message ConfigUpdate {
  string version = 1;
  LLMConfig config = 2;
  int64 timestamp = 3;
}
```

---

## 5. Client SDKs

### 5.1 TypeScript/JavaScript SDK

```typescript
import { OptimizerClient } from 'llm-auto-optimizer';

const client = new OptimizerClient({
  endpoint: 'http://optimizer:8080',
  apiKey: process.env.OPTIMIZER_API_KEY,
  timeout: 5000,
  retries: 3
});

// Get configuration
const config = await client.getConfig();

// Get configuration for specific service
const serviceConfig = await client.getConfig({
  service: 'chat-service',
  endpoint: '/api/chat'
});

// Report metrics
await client.reportMetrics({
  timestamp: Date.now(),
  service: 'chat-service',
  endpoint: '/api/chat',
  model: 'claude-sonnet-4',
  provider: 'anthropic',
  latencyMs: 1234,
  tokensInput: 150,
  tokensOutput: 500,
  tokensTotal: 650,
  costUsd: 0.0325,
  statusCode: 200
});

// Batch report
await client.reportMetricsBatch([
  { /* metric 1 */ },
  { /* metric 2 */ },
  { /* metric 3 */ }
]);

// Report quality evaluation
await client.reportQuality({
  timestamp: Date.now(),
  service: 'chat-service',
  requestId: 'req-123',
  overallScore: 92,
  dimensionScores: {
    accuracy: 95,
    relevance: 90,
    coherence: 90
  },
  evaluationMethod: 'llm-as-judge'
});

// Subscribe to config updates
client.on('configUpdate', (newConfig) => {
  console.log('Configuration updated:', newConfig);
  updateLLMSettings(newConfig);
});

// Start automatic sync
await client.startSync({ interval: 30000 });

// Query metrics
const metrics = await client.queryMetrics({
  services: ['chat-service'],
  metrics: ['latency.p95', 'cost.total'],
  timeRange: {
    start: Date.now() - 3600000,  // 1 hour ago
    end: Date.now()
  },
  aggregation: 'avg',
  groupBy: ['model']
});

// Get optimization history
const history = await client.getHistory({
  limit: 10,
  services: ['chat-service']
});

// Trigger manual optimization
const result = await client.triggerOptimization({
  services: ['chat-service'],
  objectives: [
    { metric: 'cost', goal: 'minimize', weight: 0.7 },
    { metric: 'quality', goal: 'maximize', weight: 0.3 }
  ],
  dryRun: true
});
```

### 5.2 Python SDK

```python
from llm_auto_optimizer import OptimizerClient

client = OptimizerClient(
    endpoint="http://optimizer:8080",
    api_key=os.environ.get("OPTIMIZER_API_KEY"),
    timeout=5.0,
    retries=3
)

# Get configuration
config = client.get_config()

# Service-specific config
service_config = client.get_config(
    service="chat-service",
    endpoint="/api/chat"
)

# Report metrics
client.report_metrics({
    "timestamp": int(time.time() * 1000),
    "service": "chat-service",
    "endpoint": "/api/chat",
    "model": "claude-sonnet-4",
    "provider": "anthropic",
    "latencyMs": 1234,
    "tokensInput": 150,
    "tokensOutput": 500,
    "tokensTotal": 650,
    "costUsd": 0.0325,
    "statusCode": 200
})

# Batch report
client.report_metrics_batch([
    { # metric 1 },
    { # metric 2 },
    { # metric 3 }
])

# Context manager for automatic metric reporting
with client.track_request(
    service="chat-service",
    endpoint="/api/chat"
) as tracker:
    response = call_llm(prompt, config)
    tracker.set_tokens(
        input=response.usage.input_tokens,
        output=response.usage.output_tokens
    )
    tracker.set_cost(response.cost)
    # Metrics automatically reported on exit

# Query metrics
metrics = client.query_metrics(
    services=["chat-service"],
    metrics=["latency.p95", "cost.total"],
    time_range={
        "start": int((time.time() - 3600) * 1000),
        "end": int(time.time() * 1000)
    },
    aggregation="avg",
    group_by=["model"]
)

# Subscribe to updates
def on_config_update(new_config):
    print(f"Config updated: {new_config}")
    update_llm_settings(new_config)

client.subscribe_config_updates(on_config_update)
```

### 5.3 Go SDK

```go
package main

import (
    "context"
    "time"
    optimizer "github.com/llm-auto-optimizer/go-sdk"
)

func main() {
    client := optimizer.NewClient(optimizer.Config{
        Endpoint: "http://optimizer:8080",
        APIKey:   os.Getenv("OPTIMIZER_API_KEY"),
        Timeout:  5 * time.Second,
        Retries:  3,
    })

    // Get configuration
    config, err := client.GetConfig(context.Background())
    if err != nil {
        log.Fatal(err)
    }

    // Service-specific config
    serviceConfig, err := client.GetConfigForService(
        context.Background(),
        "chat-service",
        "/api/chat",
    )

    // Report metrics
    err = client.ReportMetrics(context.Background(), optimizer.Metrics{
        Timestamp:    time.Now().UnixMilli(),
        Service:      "chat-service",
        Endpoint:     "/api/chat",
        Model:        "claude-sonnet-4",
        Provider:     "anthropic",
        LatencyMs:    1234,
        TokensInput:  150,
        TokensOutput: 500,
        TokensTotal:  650,
        CostUsd:      0.0325,
        StatusCode:   200,
    })

    // Batch report
    err = client.ReportMetricsBatch(context.Background(), []optimizer.Metrics{
        // metrics...
    })

    // Helper for automatic tracking
    tracker := client.NewRequestTracker("chat-service", "/api/chat")
    defer func() {
        tracker.SetTokens(150, 500)
        tracker.SetCost(0.0325)
        tracker.Report(context.Background())
    }()

    response := callLLM(prompt, config)

    // Subscribe to config updates
    updates, err := client.SubscribeConfigUpdates(context.Background())
    if err != nil {
        log.Fatal(err)
    }

    go func() {
        for update := range updates {
            log.Printf("Config updated: %+v", update)
            updateLLMSettings(update.Config)
        }
    }()

    // Query metrics
    metrics, err := client.QueryMetrics(context.Background(), optimizer.MetricsQuery{
        Services: []string{"chat-service"},
        Metrics:  []string{"latency.p95", "cost.total"},
        TimeRange: optimizer.TimeRange{
            Start: time.Now().Add(-1 * time.Hour).UnixMilli(),
            End:   time.Now().UnixMilli(),
        },
        Aggregation: "avg",
        GroupBy:     []string{"model"},
    })
}
```

---

## 6. Error Handling

### 6.1 Error Response Format

All API errors follow this format:

```typescript
interface ErrorResponse {
  error: {
    code: string;
    message: string;
    details?: any;
    retryable: boolean;
  };
  requestId: string;
  timestamp: number;
}
```

### 6.2 Error Codes

```typescript
enum ErrorCode {
  // Client errors (4xx)
  INVALID_REQUEST = 'INVALID_REQUEST',
  UNAUTHORIZED = 'UNAUTHORIZED',
  FORBIDDEN = 'FORBIDDEN',
  NOT_FOUND = 'NOT_FOUND',
  CONFLICT = 'CONFLICT',
  RATE_LIMITED = 'RATE_LIMITED',

  // Server errors (5xx)
  INTERNAL_ERROR = 'INTERNAL_ERROR',
  DATABASE_ERROR = 'DATABASE_ERROR',
  SERVICE_UNAVAILABLE = 'SERVICE_UNAVAILABLE',
  TIMEOUT = 'TIMEOUT',

  // Domain-specific errors
  CONSTRAINT_VIOLATION = 'CONSTRAINT_VIOLATION',
  OPTIMIZATION_FAILED = 'OPTIMIZATION_FAILED',
  DEPLOYMENT_FAILED = 'DEPLOYMENT_FAILED',
  ROLLBACK_FAILED = 'ROLLBACK_FAILED'
}
```

### 6.3 Retry Logic

Recommended retry logic for clients:

```typescript
async function withRetry<T>(
  fn: () => Promise<T>,
  options: {
    maxRetries: number;
    backoff: 'linear' | 'exponential';
    initialDelay: number;
  }
): Promise<T> {
  let lastError: Error;

  for (let attempt = 0; attempt <= options.maxRetries; attempt++) {
    try {
      return await fn();
    } catch (error) {
      lastError = error;

      // Don't retry non-retryable errors
      if (!error.retryable) {
        throw error;
      }

      // Don't retry on last attempt
      if (attempt === options.maxRetries) {
        break;
      }

      // Calculate delay
      const delay = options.backoff === 'exponential'
        ? options.initialDelay * Math.pow(2, attempt)
        : options.initialDelay * (attempt + 1);

      await sleep(delay);
    }
  }

  throw lastError;
}
```

---

**End of Data Flow and Interface Specifications**
