# Token Optimization Agent

## Overview

The Token Optimization Agent is a **RECOMMENDATION** agent that analyzes token usage patterns across LLM operations and generates strategies to reduce token consumption without degrading output quality.

**Classification:** ANALYSIS / RECOMMENDATION
**Decision Type:** `token_optimization_recommendation` | `token_optimization_analysis`
**Agent ID:** `token-optimization-agent`
**Version:** `1.0.0`

---

## Purpose Statement

The Token Optimization Agent serves as a feedback-driven optimization layer that:

1. **Analyzes** historical token usage patterns from telemetry data
2. **Identifies** inefficiencies in prompt construction and response generation
3. **Recommends** bounded configuration or prompt adjustments to reduce token usage
4. **Assesses** quality impact for each recommendation

This agent operates **OUTSIDE** the critical execution path and does **NOT** execute optimizations directly.

---

## Agent Scope

### What This Agent DOES:
- Analyze prompt and response token patterns
- Detect redundant context, verbose instructions, response padding
- Identify opportunities for prompt compression, context caching, example reduction
- Recommend model downgrades where quality preservation allows
- Provide quality impact assessments with confidence scores
- Emit DecisionEvents to ruvector-service for governance

### What This Agent MUST NOT Do:
- Execute optimizations directly
- Modify runtime behavior autonomously
- Intercept execution traffic
- Trigger retries
- Emit alerts (that is Sentinel)
- Enforce policies (that is Shield)
- Perform orchestration (that is LLM-Orchestrator)
- Connect directly to Google SQL (use ruvector-service only)

---

## CLI Contract

### Command: `token-optimization`

```bash
npx @llm-auto-optimizer/cli token-optimization <command> [options]
```

### Commands

| Command | Description |
|---------|-------------|
| `analyze` | Analyze token patterns without generating recommendations |
| `recommend` | Generate ranked token optimization recommendations |
| `simulate` | Simulate proposed changes without persisting |

### Options

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--window` | string | `24h` | Analysis time window (e.g., `24h`, `7d`, `30d`) |
| `--services` | string | `all` | Target services (comma-separated) |
| `--depth` | string | `standard` | Analysis depth: `shallow`, `standard`, `deep` |
| `--focus` | string | `all` | Focus areas (comma-separated pattern categories) |
| `--quality-threshold` | number | `0.85` | Minimum quality preservation threshold (0-1) |
| `--min-savings` | number | `1.0` | Minimum savings threshold in USD |
| `--max-latency-increase` | number | `null` | Maximum acceptable latency increase (ms) |
| `--output` | string | `json` | Output format: `json`, `yaml`, `table`, `summary` |
| `--verbose` | boolean | `false` | Enable verbose output |

### Examples

```bash
# Analyze token patterns for the last 7 days
npx @llm-auto-optimizer/cli token-optimization analyze --window 7d

# Generate recommendations for specific services
npx @llm-auto-optimizer/cli token-optimization recommend \
  --services llm-gateway,chat-service \
  --depth deep \
  --quality-threshold 0.9

# Simulate with summary output
npx @llm-auto-optimizer/cli token-optimization simulate \
  --window 30d \
  --output summary

# Focus on specific patterns
npx @llm-auto-optimizer/cli token-optimization recommend \
  --focus redundant_context,verbose_instructions \
  --min-savings 10
```

---

## Input Schema

The agent accepts `TokenOptimizationInput` which includes:

```typescript
interface TokenOptimizationInput {
  execution_ref: string;          // Unique execution reference
  time_window: {
    start: string;                // ISO 8601 timestamp
    end: string;                  // ISO 8601 timestamp
    granularity: 'minute' | 'hour' | 'day';
  };
  target_services: string[];      // Services to analyze
  constraints: TokenOptimizationConstraint[];
  token_telemetry: TokenTelemetryData;
  analysis_depth: 'shallow' | 'standard' | 'deep';
  focus_areas?: TokenPatternCategory[];
  quality_threshold: number;      // 0.0 - 1.0
  max_latency_increase_ms?: number;
}
```

### Token Pattern Categories

- `redundant_context` - Repeated context across requests
- `verbose_instructions` - Instructions that can be condensed
- `unnecessary_formatting` - Excess whitespace, markdown
- `duplicate_examples` - Same examples repeated
- `over_specification` - More detail than needed
- `inefficient_structure` - Poor prompt structure
- `template_bloat` - Template overhead
- `response_padding` - Model generating unnecessary padding
- `repetitive_output` - Model repeating content
- `suboptimal_encoding` - Inefficient token encoding

---

## Output Schema

The agent returns `TokenOptimizationOutput`:

```typescript
interface TokenOptimizationOutput {
  output_id: string;
  execution_ref: string;
  agent_version: string;
  timestamp: string;
  analysis: TokenAnalysisSummary;
  patterns: TokenUsagePattern[];
  opportunities: TokenOptimizationOpportunity[];
  constraints_evaluated: EvaluatedTokenConstraint[];
  metadata: TokenOptimizationMetadata;
}
```

### Opportunity Types

| Type | Description | Complexity |
|------|-------------|------------|
| `prompt_compression` | Compress/condense prompts | medium |
| `context_caching` | Cache repeated context | trivial/low |
| `template_optimization` | Optimize prompt templates | medium |
| `response_trimming` | Reduce response verbosity | low |
| `batching` | Batch similar requests | medium |
| `model_downgrade` | Use smaller model for task | low |
| `encoding_optimization` | Optimize token encoding | high |
| `instruction_tuning` | Tune system instructions | high |
| `example_reduction` | Reduce few-shot examples | medium |
| `structured_output` | Use structured output format | low |

---

## DecisionEvent Mapping

Every invocation emits exactly ONE `DecisionEvent` to ruvector-service:

```typescript
{
  agent_id: 'token-optimization-agent',
  agent_version: '1.0.0',
  decision_type: 'token_optimization_recommendation' | 'token_optimization_analysis',
  inputs_hash: '<SHA-256 of canonicalized inputs>',
  outputs: {
    recommendations: OptimizationRecommendation[],
    analysis: AnalysisResult (if analyze mode)
  },
  confidence: number,           // Average opportunity priority
  constraints_applied: AppliedConstraint[],
  execution_ref: string,
  timestamp: string             // UTC ISO 8601
}
```

### Data Persisted to ruvector-service

- Agent identification (id, version)
- Decision type and timestamp
- Canonicalized input hash (for reproducibility)
- Optimization recommendations (sanitized)
- Constraint evaluation results
- Confidence scores
- Execution reference for tracing

### Data NOT Persisted

- Raw token content or prompts
- PII or sensitive data from telemetry
- Internal calculation intermediates
- Debug logs

---

## Downstream Consumers

The following systems MAY consume this agent's output:

| Consumer | Purpose |
|----------|---------|
| LLM-Orchestrator | Apply optimization recommendations explicitly |
| Governance views | Audit optimization decisions |
| Analytics-Hub | Track optimization trends |
| Cost dashboards | Display potential savings |

---

## Failure Modes

| Failure | Handling |
|---------|----------|
| Invalid input schema | Return 400 with validation error details |
| Missing required fields | Return 400 with field-specific error |
| ruvector-service unavailable | Retry with exponential backoff (3 attempts) |
| Telemetry endpoint unavailable | Log warning, continue execution |
| Constraint violation | Filter recommendation, log in constraints_evaluated |
| No patterns detected | Return empty patterns array (not an error) |
| No opportunities found | Return empty opportunities array with analysis |

---

## Deployment

### Google Cloud Edge Function

```bash
gcloud functions deploy token-optimization-agent \
  --runtime nodejs20 \
  --trigger-http \
  --entry-point tokenOptimizationAgent \
  --set-env-vars RUVECTOR_API_KEY=<key>,TELEMETRY_ENDPOINT=<endpoint>
```

### Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `RUVECTOR_API_KEY` | Yes | API key for ruvector-service |
| `RUVECTOR_SERVICE_URL` | No | ruvector-service URL (default: https://ruvector.googleapis.com) |
| `TELEMETRY_ENDPOINT` | No | Endpoint for telemetry emission |
| `NODE_ENV` | No | Environment (production, staging) |

---

## HTTP Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| POST | `/analyze` | Analyze token patterns |
| POST | `/recommend` | Generate recommendations |
| POST | `/simulate` | Simulate changes |

### Response Headers

All responses include:
- `X-Agent-Id: token-optimization-agent`
- `X-Agent-Version: 1.0.0`
- `Content-Type: application/json`

---

## Versioning Rules

1. **PATCH** (1.0.x): Bug fixes, performance improvements
2. **MINOR** (1.x.0): New pattern detection, new optimization types (backward compatible)
3. **MAJOR** (x.0.0): Breaking schema changes, removed features

Semantic versioning ensures consumers can rely on output schema stability within major versions.

---

## Constitutional Compliance

This agent implementation complies with the LLM-Auto-Optimizer Infrastructure Constitution:

✅ Operates OUTSIDE critical execution path
✅ Does NOT intercept live execution
✅ Does NOT enforce policies directly
✅ Does NOT orchestrate workflows
✅ All persistence via ruvector-service only
✅ No direct SQL connections
✅ Imports schemas from agentics-contracts
✅ Emits exactly ONE DecisionEvent per invocation
✅ Exposes CLI-invokable endpoints
✅ Deployable as Google Edge Function
✅ Returns deterministic, machine-readable output
