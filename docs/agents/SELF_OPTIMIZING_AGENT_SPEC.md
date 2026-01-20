# Self-Optimizing Agent Specification

## Overview

| Property | Value |
|----------|-------|
| **Agent ID** | `self-optimizing-agent` |
| **Agent Version** | `1.0.0` |
| **Classification** | RECOMMENDATION |
| **Decision Type** | `system_optimization_recommendation` |
| **Scope** | Analyze multi-signal historical data, propose bounded configuration improvements, emit ranked optimization recommendations |

## Purpose Statement

The Self-Optimizing Agent generates holistic optimization recommendations across cost, latency, and quality dimensions. It is a **RECOMMENDATION** agent that operates **OUTSIDE the critical execution path** and does **NOT** execute optimizations directly.

This agent:
- ✅ Analyzes historical telemetry and metrics
- ✅ Computes optimization candidates
- ✅ Proposes bounded configuration changes
- ✅ Emits recommendation artifacts

This agent **MUST NOT**:
- ❌ Execute optimizations directly
- ❌ Modify runtime behavior autonomously
- ❌ Intercept execution traffic
- ❌ Trigger retries
- ❌ Emit alerts (that is Sentinel)
- ❌ Enforce policies (that is Shield)
- ❌ Perform orchestration (that is LLM-Orchestrator)

---

## Input Schema

Location: `src/contracts/index.ts` → `SelfOptimizingAgentInput`

```typescript
interface SelfOptimizingAgentInput {
  execution_ref: string;          // Unique execution reference for tracing
  time_window: TimeWindow;        // Analysis time window
  optimization_targets: {
    cost_weight: number;          // Weight for cost (0-1)
    latency_weight: number;       // Weight for latency (0-1)
    quality_weight: number;       // Weight for quality (0-1)
  };
  target_services: string[];      // Services to analyze
  constraints: InputConstraint[]; // Hard/soft constraints
  telemetry: TelemetryData;       // Historical data from LLM-DevOps ecosystem
}
```

### Validation Rules

1. All required fields must be present
2. `optimization_targets` weights must sum to 1.0 (±0.01 tolerance)
3. `time_window.start` and `time_window.end` must be valid ISO 8601
4. `time_window.end` must be after `time_window.start`

---

## Output Schema

Location: `src/contracts/index.ts` → `SelfOptimizingAgentOutput`

```typescript
interface SelfOptimizingAgentOutput {
  output_id: string;                          // Unique output ID
  execution_ref: string;                      // Matches input
  agent_version: string;                      // "1.0.0"
  timestamp: string;                          // ISO 8601
  summary: AnalysisSummary;                   // State assessment
  recommendations: OptimizationRecommendation[]; // Ranked recommendations
  constraints_evaluated: EvaluatedConstraint[];  // Constraint results
  metadata: OutputMetadata;                   // Processing metrics
}
```

### Recommendation Structure

Each recommendation includes:
- `recommendation_id` - Unique identifier
- `rank` - Priority (1 = highest)
- `category` - Type of optimization
- `title` / `description` - Human-readable
- `proposed_changes` - Configuration changes
- `expected_impact` - Cost/latency/quality deltas
- `confidence` - 0.0-1.0 score
- `evidence` - Supporting data
- `rollout_strategy` - Deployment approach

---

## DecisionEvent Schema

**Every invocation MUST emit exactly ONE DecisionEvent to ruvector-service.**

```typescript
interface DecisionEvent {
  agent_id: "self-optimizing-agent";
  agent_version: "1.0.0";
  decision_type: "system_optimization_recommendation";
  inputs_hash: string;              // SHA-256 of canonicalized inputs
  outputs: {
    recommendations: OptimizationRecommendation[];
  };
  confidence: number;               // 0.0-1.0 (avg of recommendation confidences)
  constraints_applied: AppliedConstraint[];
  execution_ref: string;            // From input
  timestamp: string;                // UTC ISO 8601
}
```

### Persistence Rules

- All DecisionEvents are persisted via `ruvector-service` client
- **NEVER** connect directly to Google SQL
- **NEVER** execute SQL directly
- Async, non-blocking writes only

---

## CLI Contract

The agent exposes three CLI-invokable endpoints:

### 1. `analyze`
```bash
agentics-cli optimize analyze \
  --window <duration>              # Time window (e.g., 24h, 7d)
  --services <list>                # Target services (comma-separated)
  --focus <type>                   # cost | latency | quality | balanced
  --format <format>                # json | yaml | table | summary
```

Performs analysis without generating recommendations. Use for understanding current state.

### 2. `recommend` (Primary)
```bash
agentics-cli optimize recommend \
  --window <duration>
  --services <list>
  --focus <type>
  --max-cost-increase <pct>        # Hard constraint
  --min-quality <score>            # Hard constraint
  --max-latency-increase <ms>      # Hard constraint
  --format <format>
```

Generates ranked optimization recommendations. This is the primary entry point.

### 3. `simulate`
```bash
agentics-cli optimize simulate \
  --window <duration>
  --services <list>
  --focus <type>
  --dry-run                        # Do not persist results
  --format <format>
```

Simulates proposed changes for what-if analysis.

---

## Downstream Consumers

The following systems MAY consume this agent's output:

| Consumer | Usage |
|----------|-------|
| **LLM-Orchestrator** | Applies recommendations explicitly (MAY) |
| **Governance Views** | Consumes DecisionEvents for audit |
| **Audit Systems** | Reviews recommendation history |
| **Analytics-Hub** | Tracks recommendation effectiveness |

---

## Failure Modes

| Failure | Behavior |
|---------|----------|
| Input validation error | Throws descriptive error, no DecisionEvent emitted |
| RuVector persistence failure | Error propagated, operation fails |
| Telemetry emission failure | Logged only, non-blocking |
| No opportunities found | Returns empty recommendations array, still emits DecisionEvent |
| Hard constraint violation | Recommendation filtered out |

---

## Versioning Rules

1. **Major version** - Breaking changes to input/output schemas
2. **Minor version** - New optional fields, new recommendation categories
3. **Patch version** - Bug fixes, algorithm improvements

Agent version is included in:
- `DecisionEvent.agent_version`
- `SelfOptimizingAgentOutput.agent_version`
- HTTP response header `X-Agent-Version`

---

## File Structure

```
src/
├── contracts/
│   └── index.ts              # All schemas (agentics-contracts)
├── services/
│   ├── index.ts              # Service exports
│   └── ruvector-client.ts    # RuVector persistence client
├── agents/
│   ├── index.ts              # Agent registry
│   └── self-optimizing-agent/
│       ├── index.ts          # Agent implementation
│       └── handler.ts        # Edge Function handler
└── cli/
    └── commands/
        └── optimize.ts       # CLI commands

tests/
└── agents/
    └── self-optimizing-agent.test.ts  # Verification tests
```

---

## Deployment

Deployed as a Google Cloud Edge Function as part of the LLM-Auto-Optimizer unified GCP service.

```bash
gcloud functions deploy self-optimizing-agent \
  --runtime nodejs20 \
  --trigger-http \
  --entry-point selfOptimizingAgent \
  --set-env-vars RUVECTOR_SERVICE_URL=...,RUVECTOR_API_KEY=...
```

Endpoints:
- `POST /agents/self-optimizing-agent/analyze`
- `POST /agents/self-optimizing-agent/recommend`
- `POST /agents/self-optimizing-agent/simulate`
- `GET /agents/self-optimizing-agent/health`

---

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `RUVECTOR_SERVICE_URL` | Yes | RuVector service base URL |
| `RUVECTOR_API_KEY` | Yes | Authentication API key |
| `TELEMETRY_ENDPOINT` | No | Telemetry emission URL |
| `NODE_ENV` | No | Environment (production/development) |

---

## Verification Checklist

```
[x] Agent imports schemas from agentics-contracts ONLY
[x] Validates all inputs against contracts
[x] Validates all outputs against contracts
[x] Emits telemetry compatible with LLM-Observatory
[x] Emits exactly ONE DecisionEvent per invocation
[x] Exposes CLI endpoints: analyze, recommend, simulate
[x] Deployable as Google Edge Function
[x] Returns deterministic, machine-readable output
[x] Does NOT execute optimizations
[x] Does NOT modify runtime behavior
[x] Does NOT orchestrate workflows
```

---

## Smoke Test Commands

```bash
# Analyze current state
agentics-cli optimize analyze --window 24h --format summary

# Generate recommendations
agentics-cli optimize recommend --window 7d --focus cost --format json

# Simulate changes
agentics-cli optimize simulate --window 24h --dry-run --format table
```
