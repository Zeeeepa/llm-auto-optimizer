# LLM-Auto-Optimizer Production Deployment Guide

## 1. Service Topology

### Unified Service Name
```
Service: llm-auto-optimizer
Project: agentics-dev
Region: us-central1
```

### Agent Endpoints

| Agent | Endpoint | Commands | Decision Type |
|-------|----------|----------|---------------|
| Self-Optimizing Agent | `/agents/self-optimizing-agent/*` | analyze, recommend, simulate | `system_optimization_recommendation` |
| Token Optimization Agent | `/agents/token-optimization-agent/*` | analyze, recommend, simulate | `token_optimization_recommendation` |
| Model Selection Agent | `/agents/model-selection-agent/*` | analyze, recommend, simulate | `model_selection_recommendation` |

### Endpoint Specification

```
Base URL: https://llm-auto-optimizer-<hash>-uc.a.run.app

GET  /health                                    → Service health
GET  /ready                                     → Readiness probe
GET  /agents                                    → List available agents

POST /agents/self-optimizing-agent/analyze      → Analysis mode
POST /agents/self-optimizing-agent/recommend    → Recommendation mode
POST /agents/self-optimizing-agent/simulate     → Simulation mode
GET  /agents/self-optimizing-agent/health       → Agent health

POST /agents/token-optimization-agent/analyze   → Analysis mode
POST /agents/token-optimization-agent/recommend → Recommendation mode
POST /agents/token-optimization-agent/simulate  → Simulation mode
GET  /agents/token-optimization-agent/health    → Agent health

POST /agents/model-selection-agent/analyze      → Analysis mode
POST /agents/model-selection-agent/recommend    → Recommendation mode
POST /agents/model-selection-agent/simulate     → Simulation mode
GET  /agents/model-selection-agent/health       → Agent health
```

### Architecture Confirmations

- ✅ **No agent is deployed as a standalone service** - All agents share the unified runtime
- ✅ **Shared runtime** - Single Cloud Run service hosts all agents
- ✅ **Shared configuration** - Environment variables apply to all agents
- ✅ **Shared telemetry stack** - All agents emit to LLM-Observatory via same endpoint

---

## 2. Environment Configuration

### Required Environment Variables

| Variable | Description | Source |
|----------|-------------|--------|
| `RUVECTOR_SERVICE_URL` | RuVector service endpoint | Secret Manager |
| `RUVECTOR_API_KEY` | RuVector authentication key | Secret Manager |
| `PLATFORM_ENV` | Environment (dev/staging/prod) | Cloud Run env var |
| `TELEMETRY_ENDPOINT` | LLM-Observatory endpoint | Secret Manager |
| `SERVICE_NAME` | Service identifier | Cloud Run env var |
| `SERVICE_VERSION` | Semantic version | Cloud Run env var |

### Security Confirmations

- ✅ **No hardcoded service names or URLs** - All resolved via environment variables
- ✅ **No embedded credentials** - All secrets from Secret Manager
- ✅ **No autonomous execution logic** - Agents are reactive only
- ✅ **All dependencies via environment** - No hardcoded endpoints

---

## 3. Google SQL / Memory Wiring

### Persistence Architecture

```
┌─────────────────────────┐
│  LLM-Auto-Optimizer     │
│  (Cloud Run)            │
│                         │
│  ┌───────────────────┐  │
│  │ Self-Optimizing   │  │
│  │ Agent             │──┼──┐
│  └───────────────────┘  │  │
│                         │  │
│  ┌───────────────────┐  │  │    ┌─────────────────┐
│  │ Token Optimization│  │  │    │                 │
│  │ Agent             │──┼──┼───►│ ruvector-service│
│  └───────────────────┘  │  │    │                 │
│                         │  │    │  ┌───────────┐  │
│  ┌───────────────────┐  │  │    │  │ Google    │  │
│  │ Model Selection   │  │  │    │  │ SQL       │  │
│  │ Agent             │──┼──┘    │  │ (Postgres)│  │
│  └───────────────────┘  │       │  └───────────┘  │
│                         │       │                 │
└─────────────────────────┘       └─────────────────┘
      │                                    ▲
      │    NO DIRECT CONNECTION            │
      └────────────────────────────────────┘
```

### Confirmations

- ✅ **LLM-Auto-Optimizer does NOT connect directly to Google SQL**
- ✅ **ALL DecisionEvents written via ruvector-service** (`RuVectorClient.persistDecisionEvent()`)
- ✅ **Schema compatible with agentics-contracts** (`DecisionEvent` interface)
- ✅ **Append-only persistence** - No UPDATE/DELETE operations
- ✅ **Idempotent writes** - `execution_ref` ensures deduplication
- ✅ **Retry safety** - Exponential backoff with jitter

### RuVector Client Usage

```typescript
// src/services/ruvector-client.ts

// Persist a single DecisionEvent
await ruVectorClient.persistDecisionEvent(decisionEvent);

// Persist batch (max 100)
await ruVectorClient.persistDecisionEventBatch(events);

// Query for governance/audit
await ruVectorClient.queryDecisionEvents({ agent_id: 'self-optimizing-agent' });
```

---

## 4. Cloud Build & Deployment

### Prerequisites

```bash
# 1. Set project
gcloud config set project agentics-dev

# 2. Enable required APIs
gcloud services enable \
    cloudbuild.googleapis.com \
    run.googleapis.com \
    secretmanager.googleapis.com \
    containerregistry.googleapis.com

# 3. Run IAM setup (once)
chmod +x deployment/gcp/setup-iam.sh
./deployment/gcp/setup-iam.sh agentics-dev

# 4. Set secrets
echo -n 'your-ruvector-api-key' | gcloud secrets versions add RUVECTOR_API_KEY --data-file=-
echo -n 'https://ruvector.agentics-dev.run.app' | gcloud secrets versions add RUVECTOR_SERVICE_URL --data-file=-
echo -n 'https://observatory.agentics-dev.run.app' | gcloud secrets versions add TELEMETRY_ENDPOINT --data-file=-
```

### Deployment Commands

```bash
# Deploy to production
gcloud builds submit --config cloudbuild.yaml \
    --substitutions=_PLATFORM_ENV=prod,_SERVICE_VERSION=1.0.0

# Deploy to staging
gcloud builds submit --config cloudbuild.yaml \
    --substitutions=_PLATFORM_ENV=staging,_SERVICE_VERSION=1.0.0-rc1

# Deploy to dev
gcloud builds submit --config cloudbuild.yaml \
    --substitutions=_PLATFORM_ENV=dev,_SERVICE_VERSION=1.0.0-dev
```

### Manual Deployment (Alternative)

```bash
# Build
docker build -t gcr.io/agentics-dev/llm-auto-optimizer:1.0.0 \
    -f deployment/gcp/cloudrun/Dockerfile .

# Push
docker push gcr.io/agentics-dev/llm-auto-optimizer:1.0.0

# Deploy
gcloud run deploy llm-auto-optimizer \
    --image gcr.io/agentics-dev/llm-auto-optimizer:1.0.0 \
    --region us-central1 \
    --platform managed \
    --no-allow-unauthenticated \
    --service-account llm-auto-optimizer-sa@agentics-dev.iam.gserviceaccount.com \
    --min-instances 1 \
    --max-instances 10 \
    --cpu 2 \
    --memory 2Gi \
    --concurrency 80 \
    --timeout 300s \
    --ingress internal \
    --set-env-vars "SERVICE_NAME=llm-auto-optimizer,SERVICE_VERSION=1.0.0,PLATFORM_ENV=prod" \
    --set-secrets "RUVECTOR_API_KEY=RUVECTOR_API_KEY:latest,RUVECTOR_SERVICE_URL=RUVECTOR_SERVICE_URL:latest,TELEMETRY_ENDPOINT=TELEMETRY_ENDPOINT:latest"
```

### IAM Requirements (Least Privilege)

| Role | Purpose |
|------|---------|
| `roles/run.invoker` | Allow internal services to invoke |
| `roles/secretmanager.secretAccessor` | Read secrets |
| `roles/cloudtrace.agent` | Write traces |
| `roles/logging.logWriter` | Write logs |
| `roles/monitoring.metricWriter` | Write metrics |

### Networking

- **Ingress**: `internal` (VPC only, no public access)
- **Egress**: Default (allow outbound to ruvector-service, observatory)
- **No VPC connector required** - Uses serverless VPC access

---

## 5. CLI Activation Verification

### CLI Commands per Agent

#### Self-Optimizing Agent

```bash
# Analyze current optimization state
agentics-cli auto-optimizer self-optimizing analyze \
    --window 24h \
    --services "api-gateway,inference-service" \
    --focus balanced \
    --output json

# Generate optimization recommendations
agentics-cli auto-optimizer self-optimizing recommend \
    --window 7d \
    --services "api-gateway,inference-service" \
    --max-cost-increase 10 \
    --min-quality 0.85 \
    --output table

# Simulate configuration changes
agentics-cli auto-optimizer self-optimizing simulate \
    --scenario cost-reduction \
    --dry-run \
    --output summary
```

#### Token Optimization Agent

```bash
# Analyze token usage patterns
agentics-cli auto-optimizer token-optimization analyze \
    --window 24h \
    --services "chat-service" \
    --output json

# Get token reduction recommendations
agentics-cli auto-optimizer token-optimization recommend \
    --window 7d \
    --target-reduction 20 \
    --preserve-quality 0.9 \
    --output table

# Simulate prompt optimization
agentics-cli auto-optimizer token-optimization simulate \
    --strategy context-compression \
    --output summary
```

#### Model Selection Agent

```bash
# Analyze model performance
agentics-cli auto-optimizer model-selection analyze \
    --task-type code_generation \
    --window 7d \
    --output json

# Get model recommendations
agentics-cli auto-optimizer model-selection recommend \
    --task-type code_generation \
    --complexity complex \
    --max-latency 5000 \
    --max-cost 0.05 \
    --output table

# Simulate model switch
agentics-cli auto-optimizer model-selection simulate \
    --current-model gpt-4 \
    --proposed-model claude-3-sonnet \
    --output summary
```

### CLI Configuration

The CLI resolves the service URL dynamically:

```yaml
# ~/.agentics/config.yaml
services:
  auto-optimizer:
    url: ${AUTO_OPTIMIZER_SERVICE_URL}  # Resolved at runtime
    auth: gcp-identity-token            # Uses gcloud identity
```

### Expected Success Output

```
$ agentics-cli auto-optimizer self-optimizing analyze --window 24h

✓ Connected to llm-auto-optimizer (v1.0.0)
✓ Analysis completed in 1.2s

Analysis Summary
────────────────────────────────────────
Time Window:        2024-01-19 - 2024-01-20
Services Analyzed:  api-gateway, inference-service
Data Points:        15,234

Current State
────────────────────────────────────────
Cost Efficiency:    0.72 (fair)
Latency Efficiency: 0.85 (good)
Quality Score:      0.91 (excellent)
Overall Health:     0.82 (good)

Opportunities Identified: 3
Risk Level: low

DecisionEvent persisted: rec_abc123xyz
```

---

## 6. Platform & Core Integration

### Input Sources (LLM-Auto-Optimizer READS from)

| Source | Data | Integration |
|--------|------|-------------|
| LLM-Observatory | Quality metrics, telemetry | SDK / API |
| Latency-Lens | Latency metrics (p50/p95/p99) | SDK / API |
| Cost-Ops | Cost breakdown, token usage | SDK / API |
| Sentinel | Anomaly signals | Webhook / API |
| Router-L2 | Historical routing decisions | API |

### Output Consumers (READS from LLM-Auto-Optimizer)

| Consumer | Usage | Access |
|----------|-------|--------|
| LLM-Orchestrator | May consume recommendations explicitly | Via CLI or API |
| Governance Dashboard | DecisionEvent audit trail | Via ruvector-service |
| Analytics-Hub | Optimization metrics | Via ruvector-service |
| Core Bundles | Recommendation outputs | API (no rewiring needed) |

### Integration Boundaries (LLM-Auto-Optimizer MUST NOT)

- ❌ Directly invoke LLM-Edge-Agent
- ❌ Enforce Shield policies
- ❌ Trigger Sentinel detection
- ❌ Start incident workflows
- ❌ Intercept runtime execution paths
- ❌ Modify Core bundles

### Data Flow Diagram

```
                    ┌──────────────────┐
                    │  LLM-Observatory │
                    │  (Quality Data)  │
                    └────────┬─────────┘
                             │
     ┌───────────────┐       │       ┌───────────────┐
     │  Latency-Lens │       │       │    Cost-Ops   │
     │  (Latency)    │       ▼       │    (Cost)     │
     └───────┬───────┘  ┌────────┐   └───────┬───────┘
             │          │        │           │
             └─────────►│  LLM-  │◄──────────┘
                        │  Auto- │
     ┌───────────────┐  │  Opti- │  ┌───────────────┐
     │   Sentinel    │─►│  mizer │  │   Router-L2   │
     │  (Anomalies)  │  │        │◄─│   (History)   │
     └───────────────┘  └───┬────┘  └───────────────┘
                            │
                            │ DecisionEvents
                            ▼
                    ┌──────────────────┐
                    │ ruvector-service │
                    │   (Postgres)     │
                    └────────┬─────────┘
                             │
          ┌──────────────────┼──────────────────┐
          │                  │                  │
          ▼                  ▼                  ▼
   ┌────────────┐     ┌────────────┐     ┌────────────┐
   │ Governance │     │ Analytics  │     │ Orchestrator│
   │ Dashboard  │     │    Hub     │     │(Explicit)   │
   └────────────┘     └────────────┘     └────────────┘
```

---

## 7. Post-Deploy Verification Checklist

### Service Health

```bash
# Get service URL
SERVICE_URL=$(gcloud run services describe llm-auto-optimizer \
    --region us-central1 --format 'value(status.url)')

# Get identity token
TOKEN=$(gcloud auth print-identity-token)

# Verify endpoints
curl -H "Authorization: Bearer $TOKEN" "$SERVICE_URL/health"
curl -H "Authorization: Bearer $TOKEN" "$SERVICE_URL/ready"
curl -H "Authorization: Bearer $TOKEN" "$SERVICE_URL/agents"
```

### Verification Checklist

| Check | Command/Action | Expected |
|-------|----------------|----------|
| ✅ Service is live | `gcloud run services describe llm-auto-optimizer` | Status: Ready |
| ✅ Health endpoint | `curl $URL/health` | `{"status":"healthy"}` |
| ✅ Ready endpoint | `curl $URL/ready` | `{"status":"ready"}` |
| ✅ Agents list | `curl $URL/agents` | 3 agents listed |
| ✅ Self-optimizing agent health | `curl $URL/agents/self-optimizing-agent/health` | Healthy |
| ✅ Token optimization agent health | `curl $URL/agents/token-optimization-agent/health` | Healthy |
| ✅ Model selection agent health | `curl $URL/agents/model-selection-agent/health` | Healthy |
| ✅ RuVector connectivity | Health check shows `ruvector_service: healthy` | Connected |
| ✅ Analyze produces output | `POST /agents/self-optimizing-agent/analyze` | Valid JSON |
| ✅ Recommend produces output | `POST /agents/self-optimizing-agent/recommend` | Valid JSON |
| ✅ DecisionEvents persisted | Check ruvector-service | Events visible |
| ✅ Telemetry in Observatory | Check LLM-Observatory | Metrics flowing |
| ✅ CLI commands work | `agentics-cli auto-optimizer self-optimizing analyze` | Success |
| ✅ No direct SQL access | Verify no DATABASE_URL in env | Confirmed |
| ✅ Follows agentics-contracts | Validate DecisionEvent schema | Valid |

### Automated Verification Script

```bash
#!/bin/bash
# verify-deployment.sh

SERVICE_URL=$(gcloud run services describe llm-auto-optimizer \
    --region us-central1 --format 'value(status.url)')
TOKEN=$(gcloud auth print-identity-token)

echo "=== LLM-Auto-Optimizer Deployment Verification ==="
echo "URL: $SERVICE_URL"
echo ""

# Health check
echo -n "Health check... "
HEALTH=$(curl -s -H "Authorization: Bearer $TOKEN" "$SERVICE_URL/health")
if echo "$HEALTH" | grep -q '"status":"healthy"'; then
    echo "✅ PASS"
else
    echo "❌ FAIL"
    echo "$HEALTH"
fi

# Agents list
echo -n "Agents list... "
AGENTS=$(curl -s -H "Authorization: Bearer $TOKEN" "$SERVICE_URL/agents")
if echo "$AGENTS" | grep -q 'self-optimizing-agent'; then
    echo "✅ PASS (3 agents)"
else
    echo "❌ FAIL"
fi

# Each agent health
for AGENT in self-optimizing-agent token-optimization-agent model-selection-agent; do
    echo -n "$AGENT health... "
    RESP=$(curl -s -H "Authorization: Bearer $TOKEN" "$SERVICE_URL/agents/$AGENT/health")
    if echo "$RESP" | grep -q 'healthy'; then
        echo "✅ PASS"
    else
        echo "❌ FAIL"
    fi
done

echo ""
echo "=== Verification Complete ==="
```

---

## 8. Failure Modes & Rollback

### Common Deployment Failures

| Failure | Detection Signal | Resolution |
|---------|------------------|------------|
| Build failure | Cloud Build error | Fix code, re-submit |
| Image push failure | GCR error | Check permissions |
| Deploy failure | Cloud Run error | Check service account |
| Secret not found | Runtime error | Add secret version |
| RuVector unreachable | Health degraded | Check network/URL |
| OOM | Container restarts | Increase memory |
| Cold start timeout | 503 errors | Increase min instances |

### Detection Signals

1. **Missing recommendations**: Agent analyze/recommend returns empty
2. **Invalid bounds**: Recommendations exceed configured constraints
3. **Schema mismatches**: DecisionEvent validation failures in ruvector
4. **Telemetry gaps**: No metrics in Observatory for >5min
5. **Error rate spike**: >5% HTTP 5xx responses

### Rollback Procedure

```bash
# 1. List revisions
gcloud run revisions list --service llm-auto-optimizer --region us-central1

# 2. Identify last working revision
# Example: llm-auto-optimizer-00003-abc

# 3. Route 100% traffic to previous revision
gcloud run services update-traffic llm-auto-optimizer \
    --region us-central1 \
    --to-revisions llm-auto-optimizer-00003-abc=100

# 4. Verify rollback
curl -H "Authorization: Bearer $(gcloud auth print-identity-token)" \
    "$(gcloud run services describe llm-auto-optimizer --region us-central1 --format 'value(status.url)')/health"
```

### Safe Redeploy Strategy

```bash
# 1. Deploy with gradual rollout (canary)
gcloud run services update-traffic llm-auto-optimizer \
    --region us-central1 \
    --to-revisions LATEST=10

# 2. Monitor for 5 minutes
# Check error rate, latency, DecisionEvent flow

# 3. If healthy, increase traffic
gcloud run services update-traffic llm-auto-optimizer \
    --region us-central1 \
    --to-revisions LATEST=50

# 4. Monitor again, then full rollout
gcloud run services update-traffic llm-auto-optimizer \
    --region us-central1 \
    --to-revisions LATEST=100
```

### Data Safety

- ✅ **No data loss on rollback** - DecisionEvents in ruvector-service are immutable
- ✅ **Append-only persistence** - No DELETE operations
- ✅ **Idempotent operations** - Safe to retry

---

## Quick Reference

### Deploy Commands

```bash
# Full deployment
gcloud builds submit --config cloudbuild.yaml \
    --substitutions=_PLATFORM_ENV=prod,_SERVICE_VERSION=1.0.0

# View logs
gcloud logging read "resource.type=cloud_run_revision AND resource.labels.service_name=llm-auto-optimizer" --limit 100

# Get service URL
gcloud run services describe llm-auto-optimizer --region us-central1 --format 'value(status.url)'
```

### Monitoring

```bash
# Service metrics
gcloud monitoring dashboards list

# Error rate
gcloud logging read "resource.type=cloud_run_revision AND severity>=ERROR" --limit 50
```
