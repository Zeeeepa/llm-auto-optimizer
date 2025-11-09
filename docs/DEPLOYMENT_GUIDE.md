# Deployment Guide

## Overview

This guide provides practical instructions for deploying the LLM Auto-Optimizer in different architectures and environments.

---

## Table of Contents

1. [Prerequisites](#1-prerequisites)
2. [Sidecar Deployment](#2-sidecar-deployment)
3. [Standalone Service Deployment](#3-standalone-service-deployment)
4. [Daemon/Batch Deployment](#4-daemonbatch-deployment)
5. [Infrastructure Setup](#5-infrastructure-setup)
6. [Configuration](#6-configuration)
7. [Monitoring](#7-monitoring)
8. [Troubleshooting](#8-troubleshooting)

---

## 1. Prerequisites

### 1.1 System Requirements

**Minimum**:
- CPU: 2 cores
- Memory: 2 GB RAM
- Storage: 10 GB
- Network: Stable connectivity to LLM providers

**Recommended**:
- CPU: 4 cores
- Memory: 8 GB RAM
- Storage: 50 GB (for metrics retention)
- Network: Low-latency connection

### 1.2 Dependencies

**Required**:
- Node.js >= 18.x
- PostgreSQL >= 14.x
- Redis >= 6.x
- TimescaleDB >= 2.x (or InfluxDB >= 2.x)

**Optional**:
- Kubernetes >= 1.24 (for orchestration)
- Prometheus (for metrics)
- Grafana (for visualization)

### 1.3 Access Requirements

- LLM provider API keys (Anthropic, OpenAI, etc.)
- Database credentials
- Kubernetes cluster access (if using K8s)
- Monitoring system access

---

## 2. Sidecar Deployment

### 2.1 Overview

In sidecar mode, the optimizer runs as a companion container to your application.

### 2.2 Kubernetes Deployment

**File: `deployments/sidecar/kubernetes.yaml`**

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: llm-application
  namespace: default
spec:
  replicas: 3
  selector:
    matchLabels:
      app: llm-app
  template:
    metadata:
      labels:
        app: llm-app
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9090"
        prometheus.io/path: "/metrics"
    spec:
      containers:
      # Main application container
      - name: application
        image: your-registry/llm-app:latest
        ports:
        - name: http
          containerPort: 8080
        env:
        - name: OPTIMIZER_ENDPOINT
          value: "http://localhost:9090"
        - name: ANTHROPIC_API_KEY
          valueFrom:
            secretKeyRef:
              name: llm-secrets
              key: anthropic-api-key
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 5

      # Optimizer sidecar
      - name: optimizer
        image: your-registry/llm-auto-optimizer:latest
        ports:
        - name: metrics
          containerPort: 9090
        - name: api
          containerPort: 8081
        env:
        - name: MODE
          value: "sidecar"
        - name: LOG_LEVEL
          value: "info"

        # Database connections
        - name: POSTGRES_URL
          valueFrom:
            secretKeyRef:
              name: optimizer-secrets
              key: postgres-url
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: optimizer-secrets
              key: redis-url
        - name: TIMESCALEDB_URL
          valueFrom:
            secretKeyRef:
              name: optimizer-secrets
              key: timescaledb-url

        # Optimization configuration
        - name: OPTIMIZATION_STRATEGY
          value: "balanced"
        - name: ENABLE_AUTO_OPTIMIZATION
          value: "true"
        - name: OPTIMIZATION_INTERVAL
          value: "900000"  # 15 minutes

        # Constraints
        - name: MAX_DAILY_COST
          value: "100"
        - name: MIN_QUALITY_SCORE
          value: "85"
        - name: MAX_P95_LATENCY
          value: "2000"

        resources:
          requests:
            memory: "256Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"

        livenessProbe:
          httpGet:
            path: /health
            port: 8081
          initialDelaySeconds: 30
          periodSeconds: 10

        readinessProbe:
          httpGet:
            path: /ready
            port: 8081
          initialDelaySeconds: 10
          periodSeconds: 5

      # Shared volume for configuration
      volumes:
      - name: config
        configMap:
          name: optimizer-config
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: optimizer-config
  namespace: default
data:
  config.yaml: |
    optimizer:
      mode: sidecar
      strategy: balanced

      collection:
        bufferSize: 1000
        flushInterval: 30000

      analysis:
        windows:
          - name: "1min"
            duration: 60000
          - name: "5min"
            duration: 300000
          - name: "15min"
            duration: 900000

      decision:
        enableRules: true
        enableML: true
        minConfidence: 0.8

      deployment:
        strategy: canary
        canarySteps:
          - percentage: 5
            duration: 300000
          - percentage: 25
            duration: 600000
          - percentage: 50
            duration: 900000
          - percentage: 100
            duration: 0

      rollback:
        automatic: true
        errorRateThreshold: 0.05
        latencyThreshold: 2000
---
apiVersion: v1
kind: Secret
metadata:
  name: optimizer-secrets
  namespace: default
type: Opaque
stringData:
  postgres-url: "postgresql://user:password@postgres:5432/optimizer"
  redis-url: "redis://redis:6379"
  timescaledb-url: "postgresql://user:password@timescale:5432/metrics"
---
apiVersion: v1
kind: Secret
metadata:
  name: llm-secrets
  namespace: default
type: Opaque
stringData:
  anthropic-api-key: "your-api-key-here"
```

### 2.3 Application Integration

**File: `examples/sidecar-integration.ts`**

```typescript
import { OptimizerClient } from 'llm-auto-optimizer/client';

// Initialize optimizer client
const optimizer = new OptimizerClient({
  endpoint: process.env.OPTIMIZER_ENDPOINT || 'http://localhost:9090',
  timeout: 5000
});

// Example: Send metrics after each LLM request
async function callLLM(prompt: string): Promise<string> {
  const startTime = Date.now();

  try {
    // Get current optimized configuration
    const config = await optimizer.getCurrentConfig();

    // Make LLM request with optimized config
    const response = await anthropic.messages.create({
      model: config.model,
      max_tokens: config.maxTokens,
      temperature: config.temperature,
      messages: [{ role: 'user', content: prompt }]
    });

    const latency = Date.now() - startTime;

    // Report metrics
    await optimizer.reportMetric({
      timestamp: Date.now(),
      service: 'my-service',
      endpoint: '/api/chat',
      model: config.model,
      provider: 'anthropic',
      latencyMs: latency,
      tokensInput: response.usage.input_tokens,
      tokensOutput: response.usage.output_tokens,
      tokensTotal: response.usage.input_tokens + response.usage.output_tokens,
      costUsd: calculateCost(response.usage),
      statusCode: 200
    });

    return response.content[0].text;

  } catch (error) {
    const latency = Date.now() - startTime;

    // Report error metrics
    await optimizer.reportMetric({
      timestamp: Date.now(),
      service: 'my-service',
      endpoint: '/api/chat',
      model: config.model,
      provider: 'anthropic',
      latencyMs: latency,
      tokensInput: 0,
      tokensOutput: 0,
      tokensTotal: 0,
      costUsd: 0,
      statusCode: error.status || 500,
      errorType: error.type
    });

    throw error;
  }
}
```

### 2.4 Deployment Steps

1. **Create secrets**:
```bash
kubectl create secret generic optimizer-secrets \
  --from-literal=postgres-url="postgresql://..." \
  --from-literal=redis-url="redis://..." \
  --from-literal=timescaledb-url="postgresql://..."

kubectl create secret generic llm-secrets \
  --from-literal=anthropic-api-key="sk-..."
```

2. **Apply configuration**:
```bash
kubectl apply -f deployments/sidecar/kubernetes.yaml
```

3. **Verify deployment**:
```bash
kubectl get pods -l app=llm-app
kubectl logs -f <pod-name> -c optimizer
```

4. **Check metrics**:
```bash
kubectl port-forward <pod-name> 9090:9090
curl http://localhost:9090/metrics
```

---

## 3. Standalone Service Deployment

### 3.1 Overview

In standalone mode, the optimizer runs as a centralized service managing multiple applications.

### 3.2 Kubernetes Deployment

**File: `deployments/standalone/kubernetes.yaml`**

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: llm-optimizer
  namespace: llm-system
spec:
  replicas: 3  # High availability
  selector:
    matchLabels:
      app: llm-optimizer
  template:
    metadata:
      labels:
        app: llm-optimizer
    spec:
      # Leader election for coordination
      serviceAccountName: llm-optimizer

      containers:
      - name: optimizer
        image: your-registry/llm-auto-optimizer:latest
        ports:
        - name: http
          containerPort: 8080
        - name: metrics
          containerPort: 9090
        - name: grpc
          containerPort: 9091

        env:
        - name: MODE
          value: "standalone"
        - name: ENABLE_LEADER_ELECTION
          value: "true"
        - name: POD_NAME
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: POD_NAMESPACE
          valueFrom:
            fieldRef:
              fieldPath: metadata.namespace

        # Database connections
        - name: POSTGRES_URL
          valueFrom:
            secretKeyRef:
              name: optimizer-secrets
              key: postgres-url
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: optimizer-secrets
              key: redis-url
        - name: TIMESCALEDB_URL
          valueFrom:
            secretKeyRef:
              name: optimizer-secrets
              key: timescaledb-url

        # Configuration
        - name: OPTIMIZATION_STRATEGY
          value: "balanced"
        - name: OPTIMIZATION_INTERVAL
          value: "900000"
        - name: MAX_CONCURRENT_OPTIMIZATIONS
          value: "5"

        resources:
          requests:
            memory: "1Gi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "2000m"

        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10

        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 5

        volumeMounts:
        - name: config
          mountPath: /etc/optimizer
          readOnly: true

      volumes:
      - name: config
        configMap:
          name: optimizer-config
---
apiVersion: v1
kind: Service
metadata:
  name: llm-optimizer
  namespace: llm-system
spec:
  selector:
    app: llm-optimizer
  ports:
  - name: http
    port: 80
    targetPort: 8080
  - name: metrics
    port: 9090
    targetPort: 9090
  - name: grpc
    port: 9091
    targetPort: 9091
  type: ClusterIP
---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: llm-optimizer
  namespace: llm-system
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: llm-optimizer-leader-election
  namespace: llm-system
rules:
- apiGroups: [""]
  resources: ["configmaps"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
- apiGroups: ["coordination.k8s.io"]
  resources: ["leases"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: llm-optimizer-leader-election
  namespace: llm-system
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: Role
  name: llm-optimizer-leader-election
subjects:
- kind: ServiceAccount
  name: llm-optimizer
  namespace: llm-system
---
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: llm-optimizer
  namespace: llm-system
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: llm-optimizer
  minReplicas: 3
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
```

### 3.3 Application Integration

**File: `examples/standalone-integration.ts`**

```typescript
import { OptimizerClient } from 'llm-auto-optimizer/client';

// Initialize client pointing to standalone service
const optimizer = new OptimizerClient({
  endpoint: 'http://llm-optimizer.llm-system.svc.cluster.local',
  timeout: 5000
});

// Register application with optimizer
await optimizer.registerApplication({
  name: 'my-service',
  endpoints: ['/api/chat', '/api/summarize'],
  defaultConfig: {
    model: 'claude-sonnet-4',
    temperature: 1.0,
    maxTokens: 1024
  },
  constraints: {
    maxDailyCost: 50,
    minQualityScore: 85,
    maxP95Latency: 2000
  }
});

// Use optimizer in your application
async function handleRequest(req, res) {
  const config = await optimizer.getConfigForEndpoint('my-service', '/api/chat');

  // Use config...
  const response = await callLLM(req.body.prompt, config);

  // Report metrics
  await optimizer.reportMetrics({...});

  res.json(response);
}
```

---

## 4. Daemon/Batch Deployment

### 4.1 Overview

In daemon mode, the optimizer runs as a scheduled job (CronJob).

### 4.2 Kubernetes CronJob

**File: `deployments/daemon/kubernetes.yaml`**

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: llm-optimizer-daemon
  namespace: llm-system
spec:
  schedule: "*/15 * * * *"  # Every 15 minutes
  concurrencyPolicy: Forbid  # Prevent overlapping runs
  successfulJobsHistoryLimit: 3
  failedJobsHistoryLimit: 3
  jobTemplate:
    spec:
      template:
        metadata:
          labels:
            app: llm-optimizer-daemon
        spec:
          restartPolicy: OnFailure
          containers:
          - name: optimizer
            image: your-registry/llm-auto-optimizer:latest
            command: ["npm", "run", "optimize:batch"]

            env:
            - name: MODE
              value: "daemon"
            - name: LOG_LEVEL
              value: "info"

            # Database connections
            - name: POSTGRES_URL
              valueFrom:
                secretKeyRef:
                  name: optimizer-secrets
                  key: postgres-url
            - name: TIMESCALEDB_URL
              valueFrom:
                secretKeyRef:
                  name: optimizer-secrets
                  key: timescaledb-url

            # Time window for analysis
            - name: ANALYSIS_WINDOW
              value: "900000"  # Last 15 minutes

            resources:
              requests:
                memory: "512Mi"
                cpu: "500m"
              limits:
                memory: "1Gi"
                cpu: "1000m"

            volumeMounts:
            - name: config
              mountPath: /etc/optimizer
              readOnly: true

          volumes:
          - name: config
            configMap:
              name: optimizer-config
```

### 4.3 AWS Lambda Deployment

**File: `deployments/lambda/serverless.yml`**

```yaml
service: llm-optimizer

provider:
  name: aws
  runtime: nodejs18.x
  region: us-east-1
  memorySize: 1024
  timeout: 300  # 5 minutes

  environment:
    MODE: daemon
    POSTGRES_URL: ${env:POSTGRES_URL}
    TIMESCALEDB_URL: ${env:TIMESCALEDB_URL}
    REDIS_URL: ${env:REDIS_URL}

  iamRoleStatements:
    - Effect: Allow
      Action:
        - secretsmanager:GetSecretValue
      Resource: "arn:aws:secretsmanager:*:*:secret:llm-optimizer/*"

functions:
  optimize:
    handler: dist/lambda.handler
    description: LLM Auto-Optimizer batch job
    events:
      # Run every 15 minutes
      - schedule:
          rate: rate(15 minutes)
          enabled: true

    reservedConcurrency: 1  # Prevent concurrent executions

    layers:
      - arn:aws:lambda:us-east-1:123456789:layer:nodejs-deps:1

plugins:
  - serverless-plugin-typescript
  - serverless-offline
```

**Lambda Handler (`src/lambda.ts`)**:

```typescript
import { ScheduledHandler } from 'aws-lambda';
import { BatchOptimizer } from './batch-optimizer';

export const handler: ScheduledHandler = async (event, context) => {
  console.log('Starting optimization cycle', { event, context });

  const optimizer = new BatchOptimizer({
    metricsSource: process.env.TIMESCALEDB_URL!,
    configStore: process.env.POSTGRES_URL!,
    redisUrl: process.env.REDIS_URL!,
  });

  try {
    const result = await optimizer.optimize();

    console.log('Optimization complete', {
      decision: result.decision,
      changesApplied: result.changesApplied,
      duration: result.duration,
    });

    if (result.changesApplied) {
      // Send SNS notification
      await sendNotification({
        topic: 'llm-optimizer-changes',
        message: JSON.stringify(result),
      });
    }

    return {
      statusCode: 200,
      body: JSON.stringify(result),
    };
  } catch (error) {
    console.error('Optimization failed', error);

    // Send error alert
    await sendAlert({
      severity: 'error',
      message: `Optimization failed: ${error.message}`,
    });

    throw error;
  }
};
```

---

## 5. Infrastructure Setup

### 5.1 PostgreSQL Setup

```sql
-- Create database
CREATE DATABASE optimizer;

-- Create user
CREATE USER optimizer_user WITH PASSWORD 'secure_password';
GRANT ALL PRIVILEGES ON DATABASE optimizer TO optimizer_user;

-- Connect to database
\c optimizer

-- Create schema
CREATE SCHEMA optimizer;
GRANT ALL ON SCHEMA optimizer TO optimizer_user;

-- Run migrations
-- (See ARCHITECTURE.md for table schemas)
```

### 5.2 TimescaleDB Setup

```sql
-- Create database
CREATE DATABASE metrics;

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Create user
CREATE USER metrics_user WITH PASSWORD 'secure_password';
GRANT ALL PRIVILEGES ON DATABASE metrics TO metrics_user;

-- Connect to database
\c metrics

-- Create metrics table
CREATE TABLE metrics (
  time TIMESTAMPTZ NOT NULL,
  metric_name VARCHAR(255) NOT NULL,
  value DOUBLE PRECISION NOT NULL,
  service VARCHAR(100),
  endpoint VARCHAR(255),
  model VARCHAR(100),
  provider VARCHAR(50),
  region VARCHAR(50),
  tags JSONB
);

-- Create hypertable
SELECT create_hypertable('metrics', 'time');

-- Create indexes
CREATE INDEX idx_metrics_name ON metrics (metric_name, time DESC);
CREATE INDEX idx_metrics_service ON metrics (service, time DESC);
CREATE INDEX idx_metrics_tags ON metrics USING gin (tags);

-- Create continuous aggregates
CREATE MATERIALIZED VIEW metrics_1min
WITH (timescaledb.continuous) AS
SELECT
  time_bucket('1 minute', time) AS bucket,
  metric_name,
  service,
  model,
  AVG(value) as avg_value,
  MIN(value) as min_value,
  MAX(value) as max_value,
  PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY value) as p95_value,
  COUNT(*) as count
FROM metrics
GROUP BY bucket, metric_name, service, model
WITH NO DATA;

-- Refresh policy
SELECT add_continuous_aggregate_policy('metrics_1min',
  start_offset => INTERVAL '1 hour',
  end_offset => INTERVAL '1 minute',
  schedule_interval => INTERVAL '1 minute');

-- Data retention policy
SELECT add_retention_policy('metrics', INTERVAL '7 days');
```

### 5.3 Redis Setup

```bash
# Redis configuration
cat > redis.conf << EOF
# Network
bind 0.0.0.0
port 6379
protected-mode yes
requirepass secure_password

# Memory
maxmemory 2gb
maxmemory-policy allkeys-lru

# Persistence
save 900 1
save 300 10
save 60 10000

# Performance
appendonly yes
appendfsync everysec
EOF

# Start Redis
redis-server redis.conf
```

---

## 6. Configuration

### 6.1 Environment Variables

```bash
# Mode
MODE=sidecar|standalone|daemon

# Database URLs
POSTGRES_URL=postgresql://user:password@host:5432/optimizer
TIMESCALEDB_URL=postgresql://user:password@host:5432/metrics
REDIS_URL=redis://password@host:6379

# Optimization settings
OPTIMIZATION_STRATEGY=conservative|balanced|aggressive
OPTIMIZATION_INTERVAL=900000  # ms
ENABLE_AUTO_OPTIMIZATION=true

# Constraints
MAX_DAILY_COST=100
MIN_QUALITY_SCORE=85
MAX_P95_LATENCY=2000

# Feature flags
ENABLE_ML_OPTIMIZER=true
ENABLE_RULE_BASED=true
ENABLE_AUTO_ROLLBACK=true

# Logging
LOG_LEVEL=debug|info|warn|error
LOG_FORMAT=json|text
```

### 6.2 Configuration File

**File: `config/production.yaml`**

```yaml
optimizer:
  mode: standalone
  strategy: balanced

  collection:
    bufferSize: 1000
    flushInterval: 30000
    batchSize: 100

  analysis:
    windows:
      - name: "1min"
        duration: 60000
      - name: "5min"
        duration: 300000
      - name: "15min"
        duration: 900000
      - name: "1hour"
        duration: 3600000

    anomalyDetection:
      enabled: true
      sensitivity: 0.8
      algorithms:
        - zscore
        - iqr

  decision:
    enableRules: true
    enableML: true
    minConfidence: 0.8
    minImprovement: 0.1

  deployment:
    strategy: canary
    validateBefore: true
    validateAfter: true
    autoRollbackOnError: true

    canarySteps:
      - percentage: 5
        duration: 300000   # 5 min
      - percentage: 25
        duration: 600000   # 10 min
      - percentage: 50
        duration: 900000   # 15 min
      - percentage: 100
        duration: 0

  constraints:
    budget:
      dailyMax: 100
      hourlyMax: 10
      perRequestMax: 0.50

    performance:
      maxP95Latency: 2000
      maxErrorRate: 0.05
      minAvailability: 0.995

    quality:
      minOverallScore: 85
      minAccuracy: 90

    operational:
      maxChangesPerDay: 5
      minTimeBetweenChanges: 1800000  # 30 min

  rollback:
    automatic: true
    errorRateThreshold: 0.05
    latencyThreshold: 2000
    qualityThreshold: 80

  monitoring:
    enableMetricsExport: true
    enableTracing: true
    metricsPort: 9090
    healthCheckPort: 8081
```

---

## 7. Monitoring

### 7.1 Prometheus Configuration

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'llm-optimizer'
    kubernetes_sd_configs:
      - role: pod
    relabel_configs:
      - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_scrape]
        action: keep
        regex: true
      - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_path]
        action: replace
        target_label: __metrics_path__
        regex: (.+)
      - source_labels: [__address__, __meta_kubernetes_pod_annotation_prometheus_io_port]
        action: replace
        regex: ([^:]+)(?::\d+)?;(\d+)
        replacement: $1:$2
        target_label: __address__
```

### 7.2 Grafana Dashboard

See `monitoring/grafana-dashboard.json` for complete dashboard.

Key panels:
- Optimization cycle metrics
- Configuration changes over time
- Cost savings
- Quality trends
- Error rates
- Deployment health

---

## 8. Troubleshooting

### 8.1 Common Issues

**Issue: Optimizer not making decisions**

```bash
# Check logs
kubectl logs -f <pod-name> -c optimizer

# Check control loop state
curl http://optimizer:8080/api/status

# Check if cooldown is active
curl http://optimizer:8080/api/cooldown
```

**Issue: High memory usage**

```bash
# Check metrics buffer
curl http://optimizer:8080/api/metrics/buffer-size

# Reduce buffer size
kubectl set env deployment/llm-optimizer BUFFER_SIZE=500

# Check for memory leaks
kubectl top pod <pod-name>
```

**Issue: Database connection errors**

```bash
# Test database connectivity
kubectl exec -it <pod-name> -- psql $POSTGRES_URL -c "SELECT 1"

# Check secrets
kubectl get secret optimizer-secrets -o yaml

# Restart deployment
kubectl rollout restart deployment/llm-optimizer
```

---

**End of Deployment Guide**
