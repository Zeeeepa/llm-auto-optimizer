# LLM Auto-Optimizer: Implementation Templates

This document provides actionable templates, checklists, and starter code structures for implementing each phase of the roadmap.

---

## MVP Phase Templates

### 1. Project Structure Template

```
llm-auto-optimizer/
├── src/
│   ├── core/
│   │   ├── feedback-loop.ts         # Main feedback loop orchestrator
│   │   ├── metrics-collector.ts     # Metrics collection
│   │   ├── threshold-engine.ts      # Threshold-based optimization
│   │   └── model-selector.ts        # Model selection logic
│   ├── integrations/
│   │   ├── claude-api-client.ts     # Claude API wrapper
│   │   └── metrics-storage.ts       # Metrics persistence
│   ├── config/
│   │   ├── thresholds.yaml          # Threshold configurations
│   │   └── models.yaml              # Model definitions
│   └── utils/
│       ├── logger.ts                # Structured logging
│       └── error-handler.ts         # Error handling utilities
├── tests/
│   ├── unit/
│   ├── integration/
│   └── scenarios/
│       ├── cost-optimization.test.ts
│       ├── quality-escalation.test.ts
│       └── latency-optimization.test.ts
├── docker/
│   ├── Dockerfile
│   └── docker-compose.yml
├── docs/
│   ├── README.md
│   ├── CONFIGURATION.md
│   └── API.md
└── examples/
    ├── basic-usage.ts
    └── custom-thresholds.ts
```

### 2. Core Data Models

```typescript
// src/core/models.ts

export interface Metric {
  id: string;
  timestamp: number;
  requestId: string;
  modelUsed: string;

  // Performance metrics
  latencyMs: number;
  tokensInput: number;
  tokensOutput: number;
  totalTokens: number;

  // Quality metrics
  qualityScore?: number;  // 0-100
  accuracy?: number;      // 0-1
  relevance?: number;     // 0-1

  // Cost metrics
  costUSD: number;

  // Error tracking
  errorOccurred: boolean;
  errorType?: string;

  // Context
  queryComplexity?: 'simple' | 'medium' | 'complex';
  domain?: string;
}

export interface ModelConfig {
  id: string;
  name: string;
  apiEndpoint: string;
  costPer1kTokensInput: number;
  costPer1kTokensOutput: number;
  averageLatencyMs: number;
  capabilities: string[];
  tier: 'fast' | 'balanced' | 'advanced';
}

export interface OptimizationDecision {
  timestamp: number;
  requestId: string;
  originalModel: string;
  selectedModel: string;
  reason: string;
  triggeredBy: 'threshold' | 'quality' | 'cost' | 'latency';
  confidence: number;  // 0-1
  expectedImprovement: {
    costReduction?: number;
    latencyReduction?: number;
    qualityIncrease?: number;
  };
}

export interface ThresholdConfig {
  name: string;
  enabled: boolean;

  // Trigger conditions
  maxLatencyMs?: number;
  minQualityScore?: number;
  maxCostPerQuery?: number;
  minAccuracy?: number;

  // Actions
  preferredModel?: string;
  fallbackModel?: string;
  escalationModel?: string;
}
```

### 3. Feedback Loop Engine Template

```typescript
// src/core/feedback-loop.ts

import { Metric, OptimizationDecision, ModelConfig } from './models';
import { MetricsCollector } from './metrics-collector';
import { ThresholdEngine } from './threshold-engine';
import { ModelSelector } from './model-selector';

export class FeedbackLoopEngine {
  private metricsCollector: MetricsCollector;
  private thresholdEngine: ThresholdEngine;
  private modelSelector: ModelSelector;

  constructor(config: FeedbackLoopConfig) {
    this.metricsCollector = new MetricsCollector(config.metricsConfig);
    this.thresholdEngine = new ThresholdEngine(config.thresholds);
    this.modelSelector = new ModelSelector(config.models);
  }

  async processRequest(request: LLMRequest): Promise<LLMResponse> {
    // 1. Collect baseline metrics
    const startTime = Date.now();

    // 2. Select optimal model
    const modelDecision = await this.selectModel(request);

    // 3. Execute request
    const response = await this.executeRequest(request, modelDecision.selectedModel);

    // 4. Collect post-execution metrics
    const metric = await this.metricsCollector.collect({
      request,
      response,
      modelUsed: modelDecision.selectedModel,
      latencyMs: Date.now() - startTime,
    });

    // 5. Store decision and metrics
    await this.storeMetrics(metric, modelDecision);

    // 6. Trigger optimization if needed
    await this.evaluateOptimizationTriggers(metric);

    return response;
  }

  private async selectModel(request: LLMRequest): Promise<OptimizationDecision> {
    // Get historical performance for this request type
    const history = await this.metricsCollector.getHistory({
      domain: request.domain,
      complexity: this.estimateComplexity(request),
    });

    // Check threshold violations
    const thresholdViolations = this.thresholdEngine.evaluate(history);

    // Select model based on thresholds and history
    return this.modelSelector.select(request, history, thresholdViolations);
  }

  private estimateComplexity(request: LLMRequest): 'simple' | 'medium' | 'complex' {
    // Simple heuristics for MVP
    const inputLength = request.prompt.length;
    const hasContext = request.context && request.context.length > 0;
    const requiresReasoning = request.systemPrompt?.includes('reasoning') ||
                              request.systemPrompt?.includes('analysis');

    if (inputLength > 2000 || requiresReasoning) return 'complex';
    if (inputLength > 500 || hasContext) return 'medium';
    return 'simple';
  }

  private async executeRequest(
    request: LLMRequest,
    modelId: string
  ): Promise<LLMResponse> {
    // Delegate to Claude API client
    // Implementation depends on integration layer
  }

  private async evaluateOptimizationTriggers(metric: Metric): Promise<void> {
    // Check if we should adjust thresholds or model preferences
    // This is where future RL/A/B testing hooks in
  }
}
```

### 4. Threshold Engine Template

```typescript
// src/core/threshold-engine.ts

export class ThresholdEngine {
  private thresholds: ThresholdConfig[];

  evaluate(metrics: Metric[]): ThresholdViolation[] {
    const violations: ThresholdViolation[] = [];

    for (const threshold of this.thresholds) {
      if (!threshold.enabled) continue;

      const recentMetrics = this.getRecentWindow(metrics);
      const aggregated = this.aggregateMetrics(recentMetrics);

      // Check latency threshold
      if (threshold.maxLatencyMs && aggregated.avgLatencyMs > threshold.maxLatencyMs) {
        violations.push({
          threshold: threshold.name,
          type: 'latency',
          current: aggregated.avgLatencyMs,
          limit: threshold.maxLatencyMs,
          severity: this.calculateSeverity(aggregated.avgLatencyMs, threshold.maxLatencyMs),
        });
      }

      // Check quality threshold
      if (threshold.minQualityScore && aggregated.avgQualityScore < threshold.minQualityScore) {
        violations.push({
          threshold: threshold.name,
          type: 'quality',
          current: aggregated.avgQualityScore,
          limit: threshold.minQualityScore,
          severity: this.calculateSeverity(threshold.minQualityScore, aggregated.avgQualityScore),
        });
      }

      // Check cost threshold
      if (threshold.maxCostPerQuery && aggregated.avgCost > threshold.maxCostPerQuery) {
        violations.push({
          threshold: threshold.name,
          type: 'cost',
          current: aggregated.avgCost,
          limit: threshold.maxCostPerQuery,
          severity: this.calculateSeverity(aggregated.avgCost, threshold.maxCostPerQuery),
        });
      }
    }

    return violations;
  }

  private getRecentWindow(metrics: Metric[], windowMs: number = 300000): Metric[] {
    const cutoff = Date.now() - windowMs;  // Default 5-minute window
    return metrics.filter(m => m.timestamp > cutoff);
  }

  private aggregateMetrics(metrics: Metric[]): AggregatedMetrics {
    if (metrics.length === 0) {
      return this.getDefaultAggregates();
    }

    return {
      avgLatencyMs: this.average(metrics.map(m => m.latencyMs)),
      avgCost: this.average(metrics.map(m => m.costUSD)),
      avgQualityScore: this.average(metrics.filter(m => m.qualityScore).map(m => m.qualityScore!)),
      errorRate: metrics.filter(m => m.errorOccurred).length / metrics.length,
      totalRequests: metrics.length,
    };
  }

  private calculateSeverity(actual: number, threshold: number): 'low' | 'medium' | 'high' {
    const ratio = actual / threshold;
    if (ratio > 2.0 || ratio < 0.5) return 'high';
    if (ratio > 1.5 || ratio < 0.67) return 'medium';
    return 'low';
  }
}
```

### 5. Configuration File Templates

**thresholds.yaml**:
```yaml
thresholds:
  - name: fast-response
    enabled: true
    maxLatencyMs: 2000
    preferredModel: claude-3-haiku-20240307

  - name: cost-optimization
    enabled: true
    maxCostPerQuery: 0.01
    minQualityScore: 75
    preferredModel: claude-3-haiku-20240307
    escalationModel: claude-3-5-sonnet-20241022

  - name: high-quality
    enabled: true
    minQualityScore: 85
    minAccuracy: 0.9
    escalationModel: claude-sonnet-4-5-20250929

  - name: error-escalation
    enabled: true
    maxErrorRate: 0.05
    escalationModel: claude-sonnet-4-5-20250929
```

**models.yaml**:
```yaml
models:
  - id: claude-3-haiku-20240307
    name: Claude 3 Haiku
    tier: fast
    costPer1kTokensInput: 0.00025
    costPer1kTokensOutput: 0.00125
    averageLatencyMs: 800
    capabilities:
      - simple_qa
      - summarization
      - translation

  - id: claude-3-5-sonnet-20241022
    name: Claude 3.5 Sonnet
    tier: balanced
    costPer1kTokensInput: 0.003
    costPer1kTokensOutput: 0.015
    averageLatencyMs: 1500
    capabilities:
      - complex_reasoning
      - code_generation
      - analysis
      - simple_qa

  - id: claude-sonnet-4-5-20250929
    name: Claude Sonnet 4.5
    tier: advanced
    costPer1kTokensInput: 0.003
    costPer1kTokensOutput: 0.015
    averageLatencyMs: 2000
    capabilities:
      - advanced_reasoning
      - multi_step_planning
      - complex_code
      - all
```

### 6. Docker Deployment Template

**Dockerfile**:
```dockerfile
FROM node:20-alpine

WORKDIR /app

# Install dependencies
COPY package*.json ./
RUN npm ci --only=production

# Copy application
COPY dist/ ./dist/
COPY config/ ./config/

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=40s \
  CMD node -e "require('http').get('http://localhost:3000/health', (r) => process.exit(r.statusCode === 200 ? 0 : 1))"

# Run as non-root
USER node

EXPOSE 3000

CMD ["node", "dist/index.js"]
```

**docker-compose.yml**:
```yaml
version: '3.8'

services:
  optimizer:
    build: .
    ports:
      - "3000:3000"
    environment:
      - CLAUDE_API_KEY=${CLAUDE_API_KEY}
      - METRICS_STORAGE_URL=postgres://metrics:password@postgres:5432/optimizer
      - LOG_LEVEL=info
    depends_on:
      - postgres
      - redis
    volumes:
      - ./config:/app/config:ro
    restart: unless-stopped

  postgres:
    image: postgres:16-alpine
    environment:
      - POSTGRES_DB=optimizer
      - POSTGRES_USER=metrics
      - POSTGRES_PASSWORD=password
    volumes:
      - postgres_data:/var/lib/postgresql/data
    ports:
      - "5432:5432"

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data

volumes:
  postgres_data:
  redis_data:
```

### 7. MVP Validation Test Template

```typescript
// tests/scenarios/cost-optimization.test.ts

describe('MVP Validation: Cost Optimization', () => {
  let feedbackLoop: FeedbackLoopEngine;

  beforeAll(async () => {
    // Initialize with cost-optimization thresholds
    feedbackLoop = new FeedbackLoopEngine({
      thresholds: [
        {
          name: 'cost-optimization',
          enabled: true,
          maxCostPerQuery: 0.01,
          minQualityScore: 75,
          preferredModel: 'claude-3-haiku-20240307',
        }
      ],
      models: loadModels(),
      metricsConfig: { storage: 'memory' },
    });
  });

  test('should switch to Haiku for simple queries', async () => {
    // Scenario: 100 simple Q&A queries
    const queries = generateSimpleQueries(100);
    const results: OptimizationDecision[] = [];

    for (const query of queries) {
      const response = await feedbackLoop.processRequest(query);
      results.push(response.optimizationDecision);
    }

    // Expectations
    const haikuUsage = results.filter(r => r.selectedModel.includes('haiku')).length;
    expect(haikuUsage).toBeGreaterThan(80); // 80%+ use Haiku

    // Calculate cost savings
    const baselineCost = calculateBaselineCost(queries, 'claude-3-5-sonnet');
    const actualCost = results.reduce((sum, r) => sum + r.actualCost, 0);
    const savings = (baselineCost - actualCost) / baselineCost;

    expect(savings).toBeGreaterThan(0.30); // 30%+ cost reduction
  });

  test('should maintain quality within 5% degradation', async () => {
    // Generate queries with known expected quality
    const queries = generateQueriesWithGroundTruth(100);

    const results = await Promise.all(
      queries.map(q => feedbackLoop.processRequest(q))
    );

    // Compare quality scores
    const avgQuality = average(results.map(r => r.metric.qualityScore));
    const baselineQuality = 85; // From manual evaluation

    expect(avgQuality).toBeGreaterThan(baselineQuality * 0.95); // < 5% degradation
  });
});
```

---

## Beta Phase Templates

### 1. A/B Testing Framework

```typescript
// src/optimization/ab-testing.ts

export class ABTestingFramework {
  private experiments: Map<string, Experiment> = new Map();

  async createExperiment(config: ExperimentConfig): Promise<Experiment> {
    const experiment: Experiment = {
      id: generateId(),
      name: config.name,
      variants: config.variants,
      trafficSplit: config.trafficSplit,
      startTime: Date.now(),
      endTime: config.durationHours ? Date.now() + config.durationHours * 3600000 : undefined,
      status: 'running',
      results: {
        variantMetrics: new Map(),
        significanceLevel: config.significanceLevel || 0.05,
      },
    };

    this.experiments.set(experiment.id, experiment);
    return experiment;
  }

  async routeRequest(request: LLMRequest): Promise<string> {
    // Find active experiments for this request type
    const activeExperiments = Array.from(this.experiments.values())
      .filter(e => e.status === 'running' && this.matchesExperiment(request, e));

    if (activeExperiments.length === 0) {
      return this.getDefaultModel(request);
    }

    // Use first matching experiment (could be more sophisticated)
    const experiment = activeExperiments[0];
    return this.selectVariant(experiment, request.userId || request.sessionId);
  }

  private selectVariant(experiment: Experiment, userId: string): string {
    // Consistent hashing for user assignment
    const hash = this.hashUserId(userId, experiment.id);
    let cumulative = 0;

    for (const [variant, split] of Object.entries(experiment.trafficSplit)) {
      cumulative += split;
      if (hash < cumulative) {
        return variant;
      }
    }

    return experiment.variants[0]; // Fallback
  }

  async recordResult(
    experimentId: string,
    variant: string,
    metric: Metric
  ): Promise<void> {
    const experiment = this.experiments.get(experimentId);
    if (!experiment) return;

    if (!experiment.results.variantMetrics.has(variant)) {
      experiment.results.variantMetrics.set(variant, []);
    }

    experiment.results.variantMetrics.get(variant)!.push(metric);

    // Check if we have statistical significance
    if (this.shouldEvaluate(experiment)) {
      await this.evaluateExperiment(experiment);
    }
  }

  private async evaluateExperiment(experiment: Experiment): Promise<void> {
    const variants = Array.from(experiment.results.variantMetrics.entries());

    if (variants.length < 2) return; // Need at least 2 variants

    // Perform statistical tests (simplified)
    const [control, treatment] = variants;
    const tTestResult = this.performTTest(control[1], treatment[1]);

    if (tTestResult.pValue < experiment.results.significanceLevel) {
      // We have a winner!
      const winner = tTestResult.mean1 > tTestResult.mean2 ? control[0] : treatment[0];

      experiment.status = 'completed';
      experiment.results.winner = winner;
      experiment.results.confidenceLevel = 1 - tTestResult.pValue;

      await this.notifyExperimentComplete(experiment);
    }
  }

  private performTTest(samples1: Metric[], samples2: Metric[]): TTestResult {
    // Implement t-test for metric comparison
    // Could use a library like jStat or simple-statistics
    // Compare cost, latency, or quality metrics
  }
}
```

### 2. Reinforcement Learning Pipeline

```typescript
// src/optimization/rl-pipeline.ts

export class RLOptimizationPipeline {
  private agent: RLAgent;
  private replayBuffer: ExperienceReplayBuffer;
  private featureExtractor: FeatureExtractor;

  constructor(config: RLConfig) {
    this.agent = new RLAgent(config.agentConfig);
    this.replayBuffer = new ExperienceReplayBuffer(config.bufferSize);
    this.featureExtractor = new FeatureExtractor(config.features);
  }

  async selectModel(request: LLMRequest, context: RequestContext): Promise<string> {
    // Extract state features
    const state = this.featureExtractor.extract(request, context);

    // Epsilon-greedy exploration
    if (Math.random() < this.agent.epsilon) {
      return this.exploreRandomModel();
    }

    // Exploit learned policy
    return this.agent.selectAction(state);
  }

  async recordExperience(
    request: LLMRequest,
    modelSelected: string,
    metric: Metric,
    context: RequestContext
  ): Promise<void> {
    const state = this.featureExtractor.extract(request, context);
    const nextState = this.featureExtractor.extract(null, { ...context, lastMetric: metric });

    const reward = this.calculateReward(metric, context.userPreferences);

    const experience: Experience = {
      state,
      action: modelSelected,
      reward,
      nextState,
      done: true, // Single-step episodes for MVP
    };

    this.replayBuffer.add(experience);

    // Trigger training if buffer is sufficiently full
    if (this.replayBuffer.size() >= this.agent.config.minExperiencesForTraining) {
      await this.train();
    }
  }

  private calculateReward(metric: Metric, preferences: UserPreferences): number {
    // Multi-objective reward function
    const costWeight = preferences.costWeight || 0.4;
    const latencyWeight = preferences.latencyWeight || 0.3;
    const qualityWeight = preferences.qualityWeight || 0.3;

    // Normalize metrics to [0, 1]
    const costScore = 1 - Math.min(metric.costUSD / 0.1, 1); // Cap at $0.10
    const latencyScore = 1 - Math.min(metric.latencyMs / 5000, 1); // Cap at 5s
    const qualityScore = (metric.qualityScore || 75) / 100;

    return costWeight * costScore +
           latencyWeight * latencyScore +
           qualityWeight * qualityScore;
  }

  private async train(): Promise<void> {
    // Sample mini-batch from replay buffer
    const batch = this.replayBuffer.sample(this.agent.config.batchSize);

    // Update agent (Q-learning, policy gradient, etc.)
    const loss = await this.agent.update(batch);

    // Decay exploration rate
    this.agent.decayEpsilon();

    // Log training metrics
    await this.logTrainingMetrics({ loss, epsilon: this.agent.epsilon });
  }
}

class FeatureExtractor {
  extract(request: LLMRequest | null, context: RequestContext): StateVector {
    if (!request) {
      // Terminal state
      return this.getTerminalState();
    }

    return {
      // Request features
      promptLength: request.prompt.length,
      hasContext: request.context ? 1 : 0,
      estimatedComplexity: this.estimateComplexity(request),

      // Historical features
      recentAvgLatency: context.recentAvgLatency || 0,
      recentAvgCost: context.recentAvgCost || 0,
      recentQuality: context.recentQuality || 0,

      // Time features
      hourOfDay: new Date().getHours() / 24,
      dayOfWeek: new Date().getDay() / 7,

      // User preference features
      userCostSensitivity: context.userPreferences?.costWeight || 0.5,
      userLatencySensitivity: context.userPreferences?.latencyWeight || 0.5,
    };
  }
}
```

### 3. Kubernetes Helm Chart Template

```yaml
# helm/llm-auto-optimizer/values.yaml

replicaCount: 3

image:
  repository: llm-auto-optimizer
  pullPolicy: IfNotPresent
  tag: "1.0.0"

service:
  type: ClusterIP
  port: 3000
  annotations:
    prometheus.io/scrape: "true"
    prometheus.io/port: "3000"
    prometheus.io/path: "/metrics"

ingress:
  enabled: true
  className: "nginx"
  annotations:
    cert-manager.io/cluster-issuer: "letsencrypt-prod"
  hosts:
    - host: optimizer.example.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: optimizer-tls
      hosts:
        - optimizer.example.com

autoscaling:
  enabled: true
  minReplicas: 3
  maxReplicas: 10
  targetCPUUtilizationPercentage: 70
  targetMemoryUtilizationPercentage: 80

resources:
  limits:
    cpu: 1000m
    memory: 1Gi
  requests:
    cpu: 500m
    memory: 512Mi

postgresql:
  enabled: true
  auth:
    database: optimizer
    username: optimizer
  primary:
    persistence:
      enabled: true
      size: 20Gi

redis:
  enabled: true
  auth:
    enabled: true
  master:
    persistence:
      enabled: true
      size: 8Gi

config:
  claudeApiKey: ""  # Set via secret
  logLevel: info
  metricsRetentionDays: 30

  thresholds:
    - name: cost-optimization
      enabled: true
      maxCostPerQuery: 0.01

monitoring:
  prometheus:
    enabled: true
  grafana:
    dashboards:
      enabled: true
```

---

## v1.0 Phase Templates

### 1. Security Audit Checklist

```markdown
# Security Audit Checklist - LLM Auto-Optimizer

## Authentication & Authorization
- [ ] API keys stored securely (no hardcoding)
- [ ] API key rotation mechanism implemented
- [ ] Rate limiting per API key
- [ ] RBAC implemented for multi-tenant deployments
- [ ] OAuth 2.0 / OIDC integration tested
- [ ] Session management secure (HTTPOnly, Secure cookies)

## Data Security
- [ ] Data at rest encryption (AES-256)
- [ ] Data in transit encryption (TLS 1.3)
- [ ] Secrets management (Vault, KMS)
- [ ] PII detection and masking
- [ ] Audit logs for data access
- [ ] Data retention policies enforced

## Input Validation
- [ ] All user inputs validated
- [ ] SQL injection prevention (parameterized queries)
- [ ] XSS prevention (output encoding)
- [ ] SSRF prevention (URL validation)
- [ ] File upload validation (if applicable)
- [ ] Request size limits enforced

## Dependency Security
- [ ] All dependencies scanned (Snyk, npm audit)
- [ ] No critical/high vulnerabilities
- [ ] Dependency pinning in package.json
- [ ] Regular dependency updates scheduled
- [ ] SBOM (Software Bill of Materials) generated

## Container Security
- [ ] Base image from trusted source
- [ ] Image scanned for vulnerabilities (Trivy)
- [ ] Running as non-root user
- [ ] Minimal attack surface (alpine, distroless)
- [ ] No secrets in image layers
- [ ] Resource limits configured

## Network Security
- [ ] Network policies configured (K8s)
- [ ] Service mesh security (mTLS)
- [ ] Firewall rules reviewed
- [ ] CORS properly configured
- [ ] CSP headers set
- [ ] HTTPS enforced

## Compliance
- [ ] GDPR requirements addressed
- [ ] Data deletion capability
- [ ] Privacy policy documented
- [ ] Audit trail immutable
- [ ] Compliance reporting available

## Penetration Testing
- [ ] OWASP Top 10 vulnerabilities checked
- [ ] API security testing (fuzzing)
- [ ] Privilege escalation attempts
- [ ] DDoS resistance tested
- [ ] Findings documented and remediated
```

### 2. Performance Benchmark Suite

```typescript
// tests/benchmarks/performance.bench.ts

import { benchmark } from 'k6';

export const options = {
  scenarios: {
    // Constant load
    constant_load: {
      executor: 'constant-arrival-rate',
      rate: 1000, // 1000 RPS
      duration: '5m',
      preAllocatedVUs: 100,
      maxVUs: 200,
    },

    // Ramp up
    ramp_up: {
      executor: 'ramping-arrival-rate',
      startRate: 100,
      stages: [
        { duration: '2m', target: 500 },
        { duration: '5m', target: 1000 },
        { duration: '2m', target: 2000 },
        { duration: '5m', target: 2000 },
        { duration: '2m', target: 0 },
      ],
      preAllocatedVUs: 200,
      maxVUs: 500,
    },

    // Spike test
    spike_test: {
      executor: 'ramping-arrival-rate',
      startRate: 100,
      stages: [
        { duration: '30s', target: 100 },
        { duration: '10s', target: 5000 }, // Spike
        { duration: '30s', target: 100 },
      ],
      preAllocatedVUs: 500,
      maxVUs: 1000,
    },
  },

  thresholds: {
    'http_req_duration{scenario:constant_load}': ['p(95)<100', 'p(99)<200'],
    'http_req_duration{scenario:ramp_up}': ['p(95)<150', 'p(99)<300'],
    'http_req_failed': ['rate<0.01'], // <1% errors
    'iteration_duration': ['p(95)<500'],
  },
};

export default function () {
  const payload = {
    prompt: generateRandomPrompt(),
    context: generateContext(),
    preferences: {
      costWeight: 0.4,
      latencyWeight: 0.3,
      qualityWeight: 0.3,
    },
  };

  const response = http.post('http://optimizer:3000/api/optimize', JSON.stringify(payload), {
    headers: { 'Content-Type': 'application/json' },
  });

  check(response, {
    'status is 200': (r) => r.status === 200,
    'latency < 100ms': (r) => r.timings.duration < 100,
    'has optimization decision': (r) => JSON.parse(r.body).optimizationDecision !== undefined,
  });
}
```

### 3. Migration Guide Template

```markdown
# Migration Guide: Beta to v1.0

## Overview
This guide helps you migrate from LLM Auto-Optimizer Beta to v1.0.

**Estimated Time**: 1-2 hours
**Downtime**: < 5 minutes (with blue-green deployment)

## Breaking Changes

### 1. Configuration Format
**Beta**:
```yaml
thresholds:
  latency: 2000
  cost: 0.01
```

**v1.0**:
```yaml
thresholds:
  - name: latency-threshold
    enabled: true
    maxLatencyMs: 2000
  - name: cost-threshold
    enabled: true
    maxCostPerQuery: 0.01
```

**Migration**: Use the conversion script:
```bash
./scripts/convert-config.sh config/beta-thresholds.yaml > config/v1-thresholds.yaml
```

### 2. API Endpoints
**Deprecated**: `POST /optimize` (removed in v1.0)
**New**: `POST /api/v1/optimize`

**Migration**: Update client code to use new endpoint.

### 3. Metrics Schema
The `quality_score` field is now required (was optional in Beta).

**Migration**: Backfill missing quality scores:
```sql
UPDATE metrics SET quality_score = 75 WHERE quality_score IS NULL;
```

## Pre-Migration Checklist

- [ ] Back up current database
- [ ] Review new features and configuration options
- [ ] Test migration in staging environment
- [ ] Update client applications to v1.0 SDK
- [ ] Schedule maintenance window

## Migration Steps

### Step 1: Data Backup
```bash
# Backup PostgreSQL
pg_dump -h localhost -U optimizer optimizer > backup-$(date +%Y%m%d).sql

# Backup Redis (if using)
redis-cli SAVE
cp /var/lib/redis/dump.rdb backup-redis-$(date +%Y%m%d).rdb
```

### Step 2: Deploy v1.0
```bash
# Blue-green deployment
kubectl apply -f k8s/v1.0/deployment-green.yaml

# Wait for pods to be ready
kubectl wait --for=condition=ready pod -l app=optimizer,version=v1.0

# Switch traffic
kubectl patch service optimizer -p '{"spec":{"selector":{"version":"v1.0"}}}'
```

### Step 3: Run Migrations
```bash
# Database schema migrations
npm run migrate:up

# Configuration migration
./scripts/migrate-config.sh
```

### Step 4: Validation
```bash
# Health check
curl http://optimizer:3000/health

# Run smoke tests
npm run test:smoke

# Verify metrics collection
curl http://optimizer:3000/api/v1/metrics | jq .
```

### Step 5: Rollback (if needed)
```bash
# Revert traffic to beta
kubectl patch service optimizer -p '{"spec":{"selector":{"version":"beta"}}}'

# Restore database
psql -h localhost -U optimizer optimizer < backup-20250109.sql
```

## Post-Migration

- [ ] Monitor error rates and latency
- [ ] Verify all integrations working
- [ ] Update documentation links
- [ ] Decommission beta deployment (after 1 week)

## Support
For issues, contact: support@example.com or #optimizer-support on Slack
```

### 4. Production Runbook Template

```markdown
# LLM Auto-Optimizer - Production Runbook

## Service Overview
- **Service Name**: LLM Auto-Optimizer
- **On-Call**: [PagerDuty rotation link]
- **Dashboard**: [Grafana link]
- **Logs**: [Log aggregation link]

## Common Incidents

### High Latency (p99 > 200ms)

**Symptoms**:
- p99 latency spikes in Grafana
- Slow response times reported by users

**Diagnosis**:
```bash
# Check current latency
curl http://optimizer:3000/metrics | grep request_duration_p99

# Check Claude API status
curl https://status.anthropic.com/api/v2/status.json

# Check database query performance
kubectl exec -it postgres-0 -- psql -U optimizer -c "SELECT * FROM pg_stat_statements ORDER BY mean_exec_time DESC LIMIT 10;"
```

**Resolution**:
1. Check if Claude API is degraded (external)
2. Scale up replicas: `kubectl scale deployment optimizer --replicas=10`
3. Enable caching temporarily: `kubectl set env deployment/optimizer CACHE_ENABLED=true`
4. If database is slow, run VACUUM: `kubectl exec -it postgres-0 -- vacuumdb -U optimizer -d optimizer`

**Escalation**: If latency persists > 30 min, page on-call engineer

---

### Claude API Rate Limit Exceeded

**Symptoms**:
- 429 errors in logs
- Requests failing with "rate limit" message

**Diagnosis**:
```bash
# Check error rate
kubectl logs -l app=optimizer | grep "rate limit" | wc -l

# Check current rate
curl http://optimizer:3000/api/v1/stats
```

**Resolution**:
1. Enable request queuing: `kubectl set env deployment/optimizer QUEUE_ENABLED=true`
2. Increase backoff: `kubectl set env deployment/optimizer RETRY_BACKOFF_MS=5000`
3. Contact Anthropic support for quota increase (if persistent)

---

### Database Connection Pool Exhausted

**Symptoms**:
- "too many connections" errors
- Slow database operations

**Diagnosis**:
```bash
kubectl exec -it postgres-0 -- psql -U optimizer -c "SELECT count(*) FROM pg_stat_activity;"
```

**Resolution**:
1. Increase pool size: `kubectl set env deployment/optimizer DB_POOL_SIZE=50`
2. Restart pods: `kubectl rollout restart deployment/optimizer`
3. Kill idle connections: `kubectl exec -it postgres-0 -- psql -U optimizer -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE state = 'idle' AND state_change < NOW() - INTERVAL '10 minutes';"`

---

## Deployment Procedures

### Rolling Update
```bash
# Update image
kubectl set image deployment/optimizer optimizer=llm-auto-optimizer:v1.1.0

# Monitor rollout
kubectl rollout status deployment/optimizer

# If issues, rollback
kubectl rollout undo deployment/optimizer
```

### Canary Deployment
```bash
# Deploy canary
kubectl apply -f k8s/canary-deployment.yaml

# Route 10% traffic
kubectl apply -f k8s/canary-service.yaml

# Monitor for 30 minutes
# If successful, proceed with full rollout
```

## Maintenance

### Database Backup
```bash
# Manual backup
kubectl exec -it postgres-0 -- pg_dump -U optimizer optimizer | gzip > backup-$(date +%Y%m%d).sql.gz

# Restore
gunzip -c backup-20250109.sql.gz | kubectl exec -i postgres-0 -- psql -U optimizer optimizer
```

### Log Rotation
```bash
# Logs automatically rotated via fluent-bit
# Manual cleanup if needed
kubectl exec -it optimizer-pod -- find /var/log -name "*.log" -mtime +7 -delete
```

## Contacts
- **Engineering Lead**: Jane Doe (jane@example.com)
- **DevOps On-Call**: PagerDuty rotation
- **Product Manager**: John Smith (john@example.com)
```

---

## Quick Start Checklists

### MVP Sprint 1 Kickoff Checklist
- [ ] Repository created and team has access
- [ ] CI/CD pipeline configured (GitHub Actions)
- [ ] Development environment setup (docker-compose)
- [ ] Claude API key obtained and configured
- [ ] Project structure created (see template above)
- [ ] First sprint planning complete
- [ ] Daily standup scheduled

### Beta Launch Checklist
- [ ] MVP exit criteria met
- [ ] Beta user list confirmed (5-10 users)
- [ ] Deployment guides written for each cloud
- [ ] Monitoring dashboard configured
- [ ] Support channel created (Slack)
- [ ] Beta feedback form prepared
- [ ] Rollback plan tested

### v1.0 GA Launch Checklist
- [ ] Security audit passed
- [ ] All documentation complete
- [ ] Performance benchmarks published
- [ ] Support system operational
- [ ] Release notes finalized
- [ ] Community announcement prepared
- [ ] Press release (if applicable)
- [ ] Monitoring alerts configured
- [ ] On-call rotation established

---

**Document Version**: 1.0
**Last Updated**: 2025-11-09
**Owner**: Roadmap Planning Agent
