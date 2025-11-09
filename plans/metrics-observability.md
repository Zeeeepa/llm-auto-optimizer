# Metrics and Observability Framework

## Overview

Comprehensive metrics, logging, and observability strategy for monitoring and debugging the LLM Auto-Optimizer system in production.

---

## 1. Metrics Taxonomy

### 1.1 Metric Categories

| Category | Purpose | Examples | Collection Frequency |
|----------|---------|----------|---------------------|
| **Business Metrics** | Track business value and ROI | Cost savings, quality improvements, user satisfaction | Hourly |
| **System Metrics** | Monitor system health | Request rate, error rate, latency percentiles | Real-time (1s) |
| **Optimization Metrics** | Track optimization effectiveness | Experiment win rate, convergence speed, regret | Per experiment |
| **Model Metrics** | Monitor LLM performance | Token usage, cache hit rate, model availability | Per request |
| **Infrastructure Metrics** | Track resource utilization | CPU, memory, network, storage | Real-time (10s) |

---

## 2. Core Metrics Definition

### 2.1 Business Metrics

```typescript
interface BusinessMetrics {
  // Cost metrics
  totalCost: {
    value: number;           // Total spend in dollars
    period: 'hour' | 'day' | 'month';
    breakdown: {
      tokenCost: number;
      computeCost: number;
      infrastructureCost: number;
    };
  };

  costSavings: {
    absolute: number;        // Dollars saved vs baseline
    relative: number;        // Percentage reduction
    confidence: number;      // Statistical confidence (0-1)
  };

  // Quality metrics
  qualityImprovement: {
    baseline: number;        // Pre-optimization quality (0-100)
    current: number;         // Current quality (0-100)
    improvement: number;     // Percentage improvement
    confidence: number;      // Statistical confidence
  };

  // User satisfaction
  userSatisfaction: {
    nps: number;            // Net Promoter Score (-100 to 100)
    csat: number;           // Customer Satisfaction (0-100)
    thumbsUpRate: number;   // Explicit positive feedback rate
    thumbsDownRate: number; // Explicit negative feedback rate
  };

  // ROI metrics
  roi: {
    optimizationCost: number;    // Cost to run optimization
    valueDerived: number;        // Total value gained
    roiMultiple: number;         // valueDerived / optimizationCost
    paybackPeriod: number;       // Days to break even
  };
}
```

### 2.2 System Metrics

```typescript
interface SystemMetrics {
  // Request metrics
  requests: {
    total: number;
    successful: number;
    failed: number;
    rate: number;              // Requests per second
    successRate: number;       // Percentage (0-100)
  };

  // Latency metrics (all in milliseconds)
  latency: {
    mean: number;
    median: number;
    p75: number;
    p90: number;
    p95: number;
    p99: number;
    p999: number;
    max: number;
  };

  // Error metrics
  errors: {
    total: number;
    rate: number;              // Errors per second
    errorRate: number;         // Percentage (0-100)
    byType: Map<string, number>;
    recentErrors: Array<{
      timestamp: Date;
      type: string;
      message: string;
      stackTrace?: string;
    }>;
  };

  // Availability
  availability: {
    uptime: number;            // Percentage (0-100)
    uptimeSeconds: number;
    downtimeSeconds: number;
    incidentCount: number;
    mtbf: number;              // Mean time between failures
    mttr: number;              // Mean time to recovery
  };
}
```

### 2.3 Optimization Metrics

```typescript
interface OptimizationMetrics {
  // A/B Testing metrics
  abTesting: {
    activeExperiments: number;
    completedExperiments: number;
    winRate: number;           // Percentage of tests with clear winner
    avgImprovementWhenWon: number;  // Average improvement of winners
    avgSampleSize: number;
    avgDurationHours: number;
    experimentsRolledBack: number;
  };

  // Bandit metrics
  bandit: {
    totalPulls: number;
    explorationRate: number;   // Percentage (0-100)
    cumulativeRegret: number;
    avgReward: number;
    convergenceTime: number;   // Hours to convergence
    armStatistics: Array<{
      armId: string;
      pulls: number;
      avgReward: number;
      confidence: [number, number];  // 95% CI
    }>;
  };

  // Parameter optimization
  parameterOptimization: {
    parametersOptimized: string[];
    optimizationFrequency: number;  // Updates per hour
    stabilityScore: number;         // 0-1 (higher = more stable)
    performanceGain: number;        // Percentage improvement
  };

  // Cost optimization
  costOptimization: {
    budgetUtilization: number;      // Percentage (0-100)
    budgetViolations: number;
    modelSwitches: number;          // Times switched to cheaper model
    avgCostPerRequest: number;
    costTrend: 'increasing' | 'stable' | 'decreasing';
  };
}
```

### 2.4 Model Metrics

```typescript
interface ModelMetrics {
  // Token usage
  tokens: {
    input: number;
    output: number;
    total: number;
    avgPerRequest: number;
    cached: number;
    cacheHitRate: number;      // Percentage (0-100)
  };

  // Model utilization
  modelUsage: Map<string, {
    requestCount: number;
    tokenCount: number;
    totalCost: number;
    avgLatency: number;
    avgQuality: number;
  }>;

  // Model availability
  modelHealth: Map<string, {
    isAvailable: boolean;
    latency: number;
    errorRate: number;
    lastChecked: Date;
  }>;

  // Quality metrics per model
  qualityMetrics: Map<string, {
    accuracy: number;
    relevance: number;
    coherence: number;
    completeness: number;
    composite: number;
  }>;
}
```

---

## 3. Metrics Collection Architecture

### 3.1 Collection Pipeline

```typescript
class MetricsCollector {
  private metricsBuffer: Map<string, MetricPoint[]> = new Map();
  private flushInterval: number = 10000; // 10 seconds
  private storage: MetricsStorage;

  constructor(storage: MetricsStorage) {
    this.storage = storage;
    this.startFlushTimer();
  }

  // Record a metric point
  recordMetric(
    name: string,
    value: number,
    tags?: Record<string, string>,
    timestamp?: Date
  ): void {
    const point: MetricPoint = {
      name,
      value,
      tags: tags || {},
      timestamp: timestamp || new Date(),
    };

    if (!this.metricsBuffer.has(name)) {
      this.metricsBuffer.set(name, []);
    }

    this.metricsBuffer.get(name)!.push(point);

    // Immediate flush for critical metrics
    if (this.isCriticalMetric(name)) {
      this.flush();
    }
  }

  // Increment a counter
  incrementCounter(
    name: string,
    amount: number = 1,
    tags?: Record<string, string>
  ): void {
    this.recordMetric(name, amount, tags);
  }

  // Record a gauge (current value)
  recordGauge(
    name: string,
    value: number,
    tags?: Record<string, string>
  ): void {
    this.recordMetric(`gauge.${name}`, value, tags);
  }

  // Record a histogram (for latency, etc.)
  recordHistogram(
    name: string,
    value: number,
    tags?: Record<string, string>
  ): void {
    this.recordMetric(`histogram.${name}`, value, tags);
  }

  // Record a distribution (pre-aggregated percentiles)
  recordDistribution(
    name: string,
    values: number[],
    tags?: Record<string, string>
  ): void {
    const percentiles = this.calculatePercentiles(values);

    for (const [p, value] of Object.entries(percentiles)) {
      this.recordMetric(
        `${name}.p${p}`,
        value,
        tags
      );
    }
  }

  // Flush metrics to storage
  private async flush(): Promise<void> {
    const metricsToFlush = new Map(this.metricsBuffer);
    this.metricsBuffer.clear();

    try {
      await this.storage.write(metricsToFlush);
    } catch (error) {
      console.error('Failed to flush metrics:', error);
      // Re-buffer failed metrics
      for (const [name, points] of metricsToFlush) {
        if (!this.metricsBuffer.has(name)) {
          this.metricsBuffer.set(name, []);
        }
        this.metricsBuffer.get(name)!.push(...points);
      }
    }
  }

  private startFlushTimer(): void {
    setInterval(() => this.flush(), this.flushInterval);
  }

  private isCriticalMetric(name: string): boolean {
    const criticalMetrics = [
      'error',
      'crash',
      'budget_violation',
      'degradation_detected',
    ];
    return criticalMetrics.some(critical => name.includes(critical));
  }

  private calculatePercentiles(values: number[]): Record<string, number> {
    const sorted = [...values].sort((a, b) => a - b);
    const percentiles = [50, 75, 90, 95, 99, 99.9];

    const result: Record<string, number> = {};

    for (const p of percentiles) {
      const index = Math.ceil((p / 100) * sorted.length) - 1;
      result[p.toString()] = sorted[Math.max(0, index)];
    }

    return result;
  }
}
```

### 3.2 Instrumentation Example

```typescript
class InstrumentedOptimizer {
  private metrics: MetricsCollector;

  async processRequest(request: Request): Promise<Response> {
    const startTime = Date.now();

    // Track request count
    this.metrics.incrementCounter('requests.total', 1, {
      userId: request.userId,
      taskType: request.taskType,
    });

    try {
      // Select variant (A/B testing)
      const variant = this.selectVariant(request);
      this.metrics.incrementCounter('ab_test.assignment', 1, {
        experimentId: variant.experimentId,
        variantId: variant.id,
      });

      // Select parameters (bandit)
      const params = this.bandit.selectArm();
      this.metrics.recordGauge('bandit.selected_temperature', params.value, {
        armId: params.parameterId,
      });

      // Execute LLM request
      const response = await this.executeLLM(request, variant, params);

      // Record latency
      const latency = Date.now() - startTime;
      this.metrics.recordHistogram('request.latency', latency, {
        modelId: response.modelId,
        success: 'true',
      });

      // Record cost
      this.metrics.recordGauge('request.cost', response.cost, {
        modelId: response.modelId,
      });

      // Record tokens
      this.metrics.incrementCounter('tokens.input', response.inputTokens, {
        modelId: response.modelId,
      });
      this.metrics.incrementCounter('tokens.output', response.outputTokens, {
        modelId: response.modelId,
      });

      // Record quality
      const quality = await this.evaluateQuality(response);
      this.metrics.recordGauge('response.quality', quality, {
        modelId: response.modelId,
      });

      // Success
      this.metrics.incrementCounter('requests.successful', 1);

      return response;

    } catch (error) {
      // Record error
      const latency = Date.now() - startTime;
      this.metrics.recordHistogram('request.latency', latency, {
        success: 'false',
        errorType: error.constructor.name,
      });

      this.metrics.incrementCounter('requests.failed', 1, {
        errorType: error.constructor.name,
      });

      this.metrics.incrementCounter('errors.total', 1, {
        errorType: error.constructor.name,
        errorMessage: error.message,
      });

      throw error;
    }
  }
}
```

---

## 4. Logging Strategy

### 4.1 Structured Logging

```typescript
enum LogLevel {
  DEBUG = 'debug',
  INFO = 'info',
  WARN = 'warn',
  ERROR = 'error',
  CRITICAL = 'critical',
}

interface LogEntry {
  timestamp: Date;
  level: LogLevel;
  message: string;
  context: Record<string, any>;
  requestId?: string;
  userId?: string;
  experimentId?: string;
  stackTrace?: string;
}

class StructuredLogger {
  private serviceName: string;
  private environment: string;

  constructor(serviceName: string, environment: string) {
    this.serviceName = serviceName;
    this.environment = environment;
  }

  log(
    level: LogLevel,
    message: string,
    context?: Record<string, any>
  ): void {
    const entry: LogEntry = {
      timestamp: new Date(),
      level,
      message,
      context: {
        service: this.serviceName,
        environment: this.environment,
        ...context,
      },
    };

    // Add trace context if available
    if (context?.requestId) {
      entry.requestId = context.requestId;
    }

    // Output as JSON for structured log ingestion
    console.log(JSON.stringify(entry));

    // Send to log aggregation service
    this.sendToLogAggregator(entry);
  }

  debug(message: string, context?: Record<string, any>): void {
    this.log(LogLevel.DEBUG, message, context);
  }

  info(message: string, context?: Record<string, any>): void {
    this.log(LogLevel.INFO, message, context);
  }

  warn(message: string, context?: Record<string, any>): void {
    this.log(LogLevel.WARN, message, context);
  }

  error(message: string, error?: Error, context?: Record<string, any>): void {
    const logContext = {
      ...context,
      error: {
        name: error?.name,
        message: error?.message,
        stack: error?.stack,
      },
    };

    this.log(LogLevel.ERROR, message, logContext);
  }

  critical(message: string, error?: Error, context?: Record<string, any>): void {
    const logContext = {
      ...context,
      error: {
        name: error?.name,
        message: error?.message,
        stack: error?.stack,
      },
    };

    this.log(LogLevel.CRITICAL, message, logContext);

    // Critical logs trigger immediate alerts
    this.triggerAlert(message, logContext);
  }

  private sendToLogAggregator(entry: LogEntry): void {
    // Send to log service (e.g., Datadog, Splunk, CloudWatch)
    // Implementation depends on chosen service
  }

  private triggerAlert(message: string, context: Record<string, any>): void {
    // Trigger PagerDuty, Slack, etc.
    // Implementation depends on alerting infrastructure
  }
}
```

### 4.2 Logging Best Practices

```typescript
class OptimizationEngine {
  private logger: StructuredLogger;

  async runOptimization(experimentId: string): Promise<void> {
    // Start of operation
    this.logger.info('Starting optimization', {
      experimentId,
      operation: 'run_optimization',
    });

    try {
      // Log key decision points
      const variant = await this.selectVariant(experimentId);
      this.logger.debug('Variant selected', {
        experimentId,
        variantId: variant.id,
        allocationWeight: variant.allocationWeight,
      });

      // Log performance metrics
      const startTime = Date.now();
      const result = await this.executeOptimization(variant);
      const duration = Date.now() - startTime;

      this.logger.info('Optimization completed', {
        experimentId,
        variantId: variant.id,
        duration,
        improvement: result.improvement,
      });

      // Log warnings for concerning patterns
      if (result.improvement < 0.05) {
        this.logger.warn('Low improvement detected', {
          experimentId,
          improvement: result.improvement,
          recommendation: 'Consider stopping experiment',
        });
      }

    } catch (error) {
      // Log errors with full context
      this.logger.error('Optimization failed', error as Error, {
        experimentId,
        operation: 'run_optimization',
      });

      throw error;
    }
  }

  private async handleDegradation(metrics: Metrics): Promise<void> {
    // Critical events get critical logs
    this.logger.critical('Performance degradation detected', undefined, {
      metrics: {
        successRate: metrics.successRate,
        baseline: metrics.baseline,
        degradation: metrics.baseline - metrics.successRate,
      },
      action: 'initiating_rollback',
    });

    await this.rollback();
  }
}
```

---

## 5. Distributed Tracing

### 5.1 Trace Context Propagation

```typescript
interface TraceContext {
  traceId: string;
  spanId: string;
  parentSpanId?: string;
  sampled: boolean;
}

class TracingProvider {
  private serviceName: string;

  constructor(serviceName: string) {
    this.serviceName = serviceName;
  }

  // Create a new trace
  startTrace(operationName: string): Span {
    const traceId = this.generateTraceId();
    const spanId = this.generateSpanId();

    return new Span({
      traceId,
      spanId,
      operationName,
      serviceName: this.serviceName,
      startTime: Date.now(),
    });
  }

  // Create a child span
  startSpan(
    operationName: string,
    parentContext: TraceContext
  ): Span {
    return new Span({
      traceId: parentContext.traceId,
      spanId: this.generateSpanId(),
      parentSpanId: parentContext.spanId,
      operationName,
      serviceName: this.serviceName,
      startTime: Date.now(),
    });
  }

  private generateTraceId(): string {
    return this.randomHex(32);
  }

  private generateSpanId(): string {
    return this.randomHex(16);
  }

  private randomHex(length: number): string {
    const bytes = new Uint8Array(length / 2);
    crypto.getRandomValues(bytes);
    return Array.from(bytes)
      .map(b => b.toString(16).padStart(2, '0'))
      .join('');
  }
}

class Span {
  private context: TraceContext;
  private operationName: string;
  private serviceName: string;
  private startTime: number;
  private endTime?: number;
  private tags: Map<string, any> = new Map();
  private logs: Array<{ timestamp: number; fields: Record<string, any> }> = [];

  constructor(config: {
    traceId: string;
    spanId: string;
    parentSpanId?: string;
    operationName: string;
    serviceName: string;
    startTime: number;
  }) {
    this.context = {
      traceId: config.traceId,
      spanId: config.spanId,
      parentSpanId: config.parentSpanId,
      sampled: true,
    };
    this.operationName = config.operationName;
    this.serviceName = config.serviceName;
    this.startTime = config.startTime;
  }

  // Add metadata to span
  setTag(key: string, value: any): void {
    this.tags.set(key, value);
  }

  // Log an event within the span
  log(fields: Record<string, any>): void {
    this.logs.push({
      timestamp: Date.now(),
      fields,
    });
  }

  // Mark span as finished
  finish(): void {
    this.endTime = Date.now();

    // Send to tracing backend (e.g., Jaeger, Zipkin)
    this.sendToTracingBackend();
  }

  // Get context for propagation
  getContext(): TraceContext {
    return this.context;
  }

  private sendToTracingBackend(): void {
    const spanData = {
      traceId: this.context.traceId,
      spanId: this.context.spanId,
      parentSpanId: this.context.parentSpanId,
      operationName: this.operationName,
      serviceName: this.serviceName,
      startTime: this.startTime,
      duration: this.endTime! - this.startTime,
      tags: Object.fromEntries(this.tags),
      logs: this.logs,
    };

    // Send to tracing service
    // Implementation depends on chosen backend
    console.log('[Trace]', JSON.stringify(spanData));
  }
}
```

### 5.2 Instrumented Request Flow

```typescript
class TracedOptimizer {
  private tracer: TracingProvider;

  async processRequest(request: Request): Promise<Response> {
    // Start trace
    const trace = this.tracer.startTrace('process_request');
    trace.setTag('user_id', request.userId);
    trace.setTag('task_type', request.taskType);

    try {
      // A/B test selection
      const abSpan = this.tracer.startSpan('ab_test_selection', trace.getContext());
      const variant = await this.selectVariant(request);
      abSpan.setTag('variant_id', variant.id);
      abSpan.setTag('experiment_id', variant.experimentId);
      abSpan.finish();

      // Bandit selection
      const banditSpan = this.tracer.startSpan('bandit_selection', trace.getContext());
      const params = await this.bandit.selectArm();
      banditSpan.setTag('arm_id', params.parameterId);
      banditSpan.setTag('temperature', params.value);
      banditSpan.finish();

      // LLM call
      const llmSpan = this.tracer.startSpan('llm_call', trace.getContext());
      llmSpan.setTag('model_id', variant.modelId);
      llmSpan.setTag('temperature', params.value);

      const response = await this.callLLM(request, variant, params);

      llmSpan.setTag('tokens_input', response.inputTokens);
      llmSpan.setTag('tokens_output', response.outputTokens);
      llmSpan.setTag('cost', response.cost);
      llmSpan.finish();

      // Quality evaluation
      const evalSpan = this.tracer.startSpan('quality_evaluation', trace.getContext());
      const quality = await this.evaluateQuality(response);
      evalSpan.setTag('quality_score', quality);
      evalSpan.finish();

      trace.setTag('success', true);
      trace.finish();

      return response;

    } catch (error) {
      trace.setTag('error', true);
      trace.setTag('error_type', error.constructor.name);
      trace.log({
        event: 'error',
        message: error.message,
        stack: error.stack,
      });
      trace.finish();

      throw error;
    }
  }
}
```

---

## 6. Dashboards and Visualization

### 6.1 Key Dashboard Layouts

#### System Health Dashboard

```
┌─────────────────────────────────────────────────────────────┐
│ SYSTEM HEALTH                                               │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Request Rate: ████████████ 1,234 req/s  ↑ 5%             │
│  Success Rate: ████████████ 99.2%         ↑ 0.1%           │
│  Avg Latency:  ████████████ 234ms         ↓ 12ms           │
│                                                             │
│  [Latency Percentiles Graph - 24h]                         │
│   500ms┤                                                    │
│   400ms┤         ╭─╮                                        │
│   300ms┤      ╭──╯ ╰─╮                                      │
│   200ms┤   ╭──╯      ╰──╮                                   │
│   100ms┤───╯            ╰────                               │
│        └──────────────────────────────                      │
│         0h   6h   12h  18h  24h                             │
│                                                             │
│  [Error Rate by Type - Last 1h]                            │
│   Timeout:        ██ 12                                     │
│   Rate Limit:     ████ 45                                   │
│   Model Error:    █ 3                                       │
│   Budget:         ████████ 89                               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

#### Optimization Performance Dashboard

```
┌─────────────────────────────────────────────────────────────┐
│ OPTIMIZATION PERFORMANCE                                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Active A/B Tests: 3                                        │
│  ┌───────────────────────────────────────┐                 │
│  │ Experiment: prompt-optimization-v5    │                 │
│  │ Progress: ████████████░░░░░ 73%       │                 │
│  │ Leader: variant-b (+8.2%)             │                 │
│  │ Confidence: 87% (need 95%)            │                 │
│  └───────────────────────────────────────┘                 │
│                                                             │
│  Bandit Performance (Last 24h)                              │
│  Temperature: 0.7 ████████████ 68% selection               │
│  Temperature: 0.3 ████░░░░░░░░ 22% selection               │
│  Temperature: 1.0 ██░░░░░░░░░░ 10% selection               │
│                                                             │
│  [Cost Trend - 7 days]                                      │
│   $500┤                                                     │
│   $400┤╲                                                    │
│   $300┤ ╲                                                   │
│   $200┤  ╲___                                               │
│   $100┤      ╲____                                          │
│      $0└─────────────                                       │
│         D1  D2  D3  D4  D5  D6  D7                          │
│                                                             │
│  Savings: $1,234 (-23%)                                     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 6.2 Alert Definitions

```typescript
interface AlertRule {
  name: string;
  condition: string;
  threshold: number;
  duration: number;        // Seconds condition must be true
  severity: 'info' | 'warning' | 'critical';
  channels: string[];      // Where to send alert
}

const ALERT_RULES: AlertRule[] = [
  {
    name: 'High Error Rate',
    condition: 'error_rate > threshold',
    threshold: 0.05,       // 5%
    duration: 300,         // 5 minutes
    severity: 'critical',
    channels: ['pagerduty', 'slack'],
  },
  {
    name: 'High Latency',
    condition: 'latency_p95 > threshold',
    threshold: 5000,       // 5 seconds
    duration: 600,         // 10 minutes
    severity: 'warning',
    channels: ['slack'],
  },
  {
    name: 'Budget Exceeded',
    condition: 'daily_spend > threshold',
    threshold: 1000,       // $1000
    duration: 0,           // Immediate
    severity: 'critical',
    channels: ['pagerduty', 'email'],
  },
  {
    name: 'Quality Degradation',
    condition: 'quality_score < threshold',
    threshold: 75,         // Below 75/100
    duration: 1800,        // 30 minutes
    severity: 'warning',
    channels: ['slack'],
  },
  {
    name: 'Optimization Stuck',
    condition: 'no_improvement_for > threshold',
    threshold: 86400,      // 24 hours
    duration: 0,
    severity: 'info',
    channels: ['slack'],
  },
];
```

---

## 7. Monitoring Best Practices

### 7.1 SLI/SLO Definition

```typescript
interface ServiceLevelIndicator {
  name: string;
  description: string;
  measurement: string;
  target: number;
  window: string;
}

const SERVICE_LEVEL_INDICATORS: ServiceLevelIndicator[] = [
  {
    name: 'Availability',
    description: 'Percentage of successful requests',
    measurement: 'successful_requests / total_requests',
    target: 99.9,          // 99.9% availability
    window: '30d',         // 30-day rolling window
  },
  {
    name: 'Latency',
    description: '95th percentile response time',
    measurement: 'p95(request_duration)',
    target: 1000,          // 1000ms
    window: '24h',
  },
  {
    name: 'Quality',
    description: 'Average quality score',
    measurement: 'avg(quality_score)',
    target: 80,            // 80/100
    window: '7d',
  },
  {
    name: 'Cost Efficiency',
    description: 'Cost per successful request',
    measurement: 'total_cost / successful_requests',
    target: 0.10,          // $0.10 per request
    window: '24h',
  },
];

interface ServiceLevelObjective {
  sli: string;
  target: number;
  window: string;
  errorBudget: number;
}

const SERVICE_LEVEL_OBJECTIVES: ServiceLevelObjective[] = [
  {
    sli: 'Availability',
    target: 99.9,
    window: '30d',
    errorBudget: 43.2,     // minutes per 30 days
  },
  {
    sli: 'Latency',
    target: 1000,          // ms
    window: '24h',
    errorBudget: 0.05,     // 5% of requests can exceed
  },
];
```

### 7.2 Error Budget Tracking

```typescript
class ErrorBudgetTracker {
  private slo: ServiceLevelObjective;

  calculateErrorBudget(
    slo: ServiceLevelObjective,
    actualPerformance: number
  ): {
    budgetRemaining: number;
    budgetUsed: number;
    budgetExhausted: boolean;
  } {
    const targetUptime = slo.target / 100;
    const actualUptime = actualPerformance / 100;

    const allowedDowntime = 1 - targetUptime;
    const actualDowntime = 1 - actualUptime;

    const budgetUsed = (actualDowntime / allowedDowntime) * 100;
    const budgetRemaining = Math.max(0, 100 - budgetUsed);

    return {
      budgetRemaining,
      budgetUsed,
      budgetExhausted: budgetUsed >= 100,
    };
  }

  shouldHaltOptimization(errorBudget: {
    budgetRemaining: number;
    budgetUsed: number;
  }): boolean {
    // Halt optimization if error budget is low
    return errorBudget.budgetRemaining < 10; // Less than 10% remaining
  }
}
```

---

## 8. Observability in Production

### 8.1 Production Readiness Checklist

- [ ] All critical paths instrumented with metrics
- [ ] Structured logging implemented
- [ ] Distributed tracing enabled
- [ ] Dashboards created and reviewed
- [ ] Alerts configured and tested
- [ ] SLIs/SLOs defined
- [ ] Error budget tracking enabled
- [ ] On-call runbooks created
- [ ] Log retention policies set
- [ ] Metrics retention policies set
- [ ] Cost monitoring enabled
- [ ] Performance baselines established

### 8.2 Incident Response Playbook

```typescript
interface Incident {
  id: string;
  severity: 'sev1' | 'sev2' | 'sev3';
  title: string;
  description: string;
  detectedAt: Date;
  status: 'investigating' | 'identified' | 'monitoring' | 'resolved';
}

class IncidentResponse {
  async handleIncident(incident: Incident): Promise<void> {
    // 1. Acknowledge
    await this.acknowledge(incident);

    // 2. Assess severity
    const severity = this.assessSeverity(incident);

    // 3. Gather context
    const context = await this.gatherContext(incident);

    // 4. Mitigate
    if (severity === 'sev1') {
      // Immediate rollback for critical issues
      await this.rollback();
    }

    // 5. Investigate
    const rootCause = await this.investigate(context);

    // 6. Resolve
    await this.resolve(incident, rootCause);

    // 7. Post-mortem
    await this.schedulePostMortem(incident);
  }

  private async gatherContext(incident: Incident): Promise<Record<string, any>> {
    // Query recent logs
    const logs = await this.queryLogs({
      timeRange: '15m',
      severity: ['error', 'critical'],
    });

    // Query metrics
    const metrics = await this.queryMetrics({
      timeRange: '1h',
      metrics: ['error_rate', 'latency', 'quality'],
    });

    // Query traces
    const traces = await this.queryTraces({
      timeRange: '15m',
      hasError: true,
    });

    return {
      logs,
      metrics,
      traces,
      recentChanges: await this.getRecentDeployments(),
      activeExperiments: await this.getActiveExperiments(),
    };
  }
}
```

---

## Summary

This observability framework provides:

1. **Comprehensive Metrics**: Business, system, optimization, and model metrics
2. **Structured Logging**: JSON-formatted logs with context
3. **Distributed Tracing**: Request flow visibility across services
4. **Dashboards**: Pre-configured views for common monitoring needs
5. **Alerting**: Proactive detection of issues
6. **SLI/SLO Framework**: Objective measurement of service quality
7. **Error Budget**: Data-driven decision making
8. **Incident Response**: Structured approach to handling issues

Key principles:
- **Instrument everything**: Every optimization decision, every request
- **Structure all data**: Use consistent schemas for logs and metrics
- **Aggregate intelligently**: Balance granularity with storage costs
- **Alert thoughtfully**: Minimize false positives, prioritize actionable alerts
- **Visualize effectively**: Make data accessible to all stakeholders
- **Learn continuously**: Use observability data to improve the system
