# Control Loop Specification

## Overview

This document provides detailed specifications for the LLM Auto-Optimizer control loop, including timing, triggers, algorithms, and operational procedures.

---

## 1. Loop Timing Specifications

### 1.1 Base Loop Frequency

```typescript
const LOOP_TIMING = {
  // Primary optimization loop
  MAIN_LOOP: {
    interval: 900000, // 15 minutes
    description: 'Main optimization evaluation cycle',
    enabled: true
  },

  // Fast feedback loop for critical metrics
  FAST_LOOP: {
    interval: 60000, // 1 minute
    description: 'Quick anomaly detection and emergency response',
    enabled: true
  },

  // Slow loop for deep analysis
  DEEP_ANALYSIS: {
    interval: 3600000, // 1 hour
    description: 'Comprehensive trend analysis and ML model updates',
    enabled: true
  },

  // Daily optimization review
  DAILY_REVIEW: {
    schedule: '0 2 * * *', // 2 AM daily
    description: 'Daily performance review and baseline updates',
    enabled: true
  }
};
```

### 1.2 Phase Duration Budgets

Each loop phase has a maximum duration budget to ensure timely completion:

```typescript
const PHASE_BUDGETS = {
  OBSERVE: {
    maxDuration: 30000, // 30 seconds
    timeout: 45000, // hard timeout
    criticalMetrics: ['latency', 'errors', 'cost']
  },

  ORIENT: {
    maxDuration: 60000, // 1 minute
    timeout: 90000,
    analysis: ['trends', 'anomalies', 'baselines']
  },

  DECIDE: {
    maxDuration: 45000, // 45 seconds
    timeout: 60000,
    strategies: ['rules', 'ml', 'constraints']
  },

  ACT: {
    maxDuration: 120000, // 2 minutes
    timeout: 180000,
    actions: ['validate', 'deploy', 'monitor']
  }
};
```

---

## 2. Trigger Specifications

### 2.1 Priority-Based Trigger System

Triggers are evaluated in priority order:

```typescript
enum TriggerPriority {
  CRITICAL = 1,    // Immediate action required
  HIGH = 2,        // Action within 1 minute
  MEDIUM = 3,      // Action within 5 minutes
  LOW = 4          // Action within 15 minutes
}

interface TriggerDefinition {
  id: string;
  priority: TriggerPriority;
  condition: TriggerCondition;
  action: TriggerAction;
  cooldown: number;
  enabled: boolean;
}
```

### 2.2 Critical Triggers (Priority 1)

**Immediate Emergency Response**:

```typescript
const CRITICAL_TRIGGERS = [
  {
    id: 'critical_error_spike',
    priority: TriggerPriority.CRITICAL,
    condition: {
      metric: 'errors.rate',
      operator: '>',
      threshold: 0.15, // 15% error rate
      window: 60000 // 1 minute
    },
    action: {
      type: 'rollback',
      immediate: true,
      notify: ['oncall', 'slack']
    },
    cooldown: 0, // No cooldown for critical
    enabled: true
  },

  {
    id: 'critical_latency_spike',
    priority: TriggerPriority.CRITICAL,
    condition: {
      metric: 'latency.p95',
      operator: '>',
      threshold: 5000, // 5 seconds
      window: 120000 // 2 minutes
    },
    action: {
      type: 'rollback',
      immediate: true,
      notify: ['oncall']
    },
    cooldown: 0,
    enabled: true
  },

  {
    id: 'critical_provider_outage',
    priority: TriggerPriority.CRITICAL,
    condition: {
      eventType: 'provider.unavailable',
      provider: 'primary'
    },
    action: {
      type: 'failover',
      targetProvider: 'secondary',
      notify: ['oncall', 'slack']
    },
    cooldown: 0,
    enabled: true
  }
];
```

### 2.3 High Priority Triggers (Priority 2)

**Fast Response to Degradation**:

```typescript
const HIGH_PRIORITY_TRIGGERS = [
  {
    id: 'quality_degradation',
    priority: TriggerPriority.HIGH,
    condition: {
      metric: 'quality.overall',
      operator: '<',
      threshold: 80, // Below 80 quality score
      duration: 300000 // Sustained for 5 minutes
    },
    action: {
      type: 'evaluate_rollback',
      analyzeRoot: true,
      notify: ['team']
    },
    cooldown: 600000, // 10 minutes
    enabled: true
  },

  {
    id: 'cost_spike',
    priority: TriggerPriority.HIGH,
    condition: {
      metric: 'cost.hourly',
      operator: '>',
      threshold: 200, // $200/hour
      window: 300000
    },
    action: {
      type: 'optimize_cost',
      aggressive: false,
      notify: ['team', 'finance']
    },
    cooldown: 1800000, // 30 minutes
    enabled: true
  },

  {
    id: 'traffic_spike',
    priority: TriggerPriority.HIGH,
    condition: {
      metric: 'requests.rate',
      operator: '>',
      threshold: 2.0, // 2x normal
      baselineComparison: true,
      window: 180000 // 3 minutes
    },
    action: {
      type: 'scale_up',
      evaluateConfig: true,
      notify: ['team']
    },
    cooldown: 900000, // 15 minutes
    enabled: true
  }
];
```

### 2.4 Medium Priority Triggers (Priority 3)

**Proactive Optimization**:

```typescript
const MEDIUM_PRIORITY_TRIGGERS = [
  {
    id: 'optimization_opportunity',
    priority: TriggerPriority.MEDIUM,
    condition: {
      type: 'composite',
      operator: 'AND',
      conditions: [
        {
          metric: 'quality.overall',
          operator: '>=',
          threshold: 90,
          duration: 1800000 // 30 minutes stable
        },
        {
          metric: 'cost.perRequest',
          operator: '>',
          threshold: 0.05,
          baselineComparison: true
        }
      ]
    },
    action: {
      type: 'evaluate_cheaper_model',
      testMode: 'ab_test',
      notify: ['team']
    },
    cooldown: 3600000, // 1 hour
    enabled: true
  },

  {
    id: 'latency_optimization',
    priority: TriggerPriority.MEDIUM,
    condition: {
      metric: 'latency.p95',
      operator: '>',
      threshold: 1500, // 1.5 seconds
      percentIncrease: 20, // 20% worse than baseline
      duration: 1800000
    },
    action: {
      type: 'optimize_latency',
      strategies: ['faster_model', 'streaming', 'caching'],
      notify: ['team']
    },
    cooldown: 7200000, // 2 hours
    enabled: true
  }
];
```

### 2.5 Low Priority Triggers (Priority 4)

**Scheduled and Periodic Optimization**:

```typescript
const LOW_PRIORITY_TRIGGERS = [
  {
    id: 'periodic_evaluation',
    priority: TriggerPriority.LOW,
    condition: {
      type: 'schedule',
      cron: '0 */6 * * *' // Every 6 hours
    },
    action: {
      type: 'full_evaluation',
      includeMlOptimizer: true,
      notify: []
    },
    cooldown: 0, // Schedule handles timing
    enabled: true
  },

  {
    id: 'baseline_update',
    priority: TriggerPriority.LOW,
    condition: {
      type: 'schedule',
      cron: '0 0 * * 0' // Weekly on Sunday
    },
    action: {
      type: 'update_baseline',
      snapshotMetrics: true,
      notify: ['team']
    },
    cooldown: 0,
    enabled: true
  },

  {
    id: 'model_availability_check',
    priority: TriggerPriority.LOW,
    condition: {
      type: 'schedule',
      cron: '0 0 * * *' // Daily
    },
    action: {
      type: 'check_new_models',
      evaluateForMigration: true,
      notify: []
    },
    cooldown: 0,
    enabled: true
  }
];
```

---

## 3. Decision Algorithm Specifications

### 3.1 Decision Tree Flow

```
START
  │
  ├─ Collect all triggered events
  │
  ├─ Filter by enabled status
  │
  ├─ Check cooldown periods
  │
  ├─ Sort by priority (CRITICAL → LOW)
  │
  ├─ For each trigger (highest priority first):
  │   │
  │   ├─ Evaluate condition
  │   │
  │   ├─ If condition met:
  │   │   │
  │   │   ├─ Check constraints
  │   │   │
  │   │   ├─ If constraints satisfied:
  │   │   │   │
  │   │   │   ├─ Execute action
  │   │   │   │
  │   │   │   ├─ Record execution
  │   │   │   │
  │   │   │   ├─ Set cooldown
  │   │   │   │
  │   │   │   └─ If CRITICAL: Skip remaining triggers
  │   │   │
  │   │   └─ Else: Log constraint violation
  │   │
  │   └─ Continue to next trigger
  │
  └─ END
```

### 3.2 Constraint Evaluation Order

Constraints are checked in this order:

```typescript
const CONSTRAINT_EVALUATION_ORDER = [
  // 1. Safety constraints (cannot be violated)
  'SAFETY',
  {
    maxErrorRate: 0.05,
    maxLatencyP95: 3000,
    minQuality: 75,
    minAvailability: 0.995
  },

  // 2. Budget constraints (hard limits)
  'BUDGET',
  {
    dailyBudget: 1000,
    hourlyBudget: 50,
    perRequestMax: 0.50
  },

  // 3. Operational constraints (business rules)
  'OPERATIONAL',
  {
    maxChangesPerDay: 5,
    minTimeBetweenChanges: 1800000, // 30 minutes
    requiresApprovalOver: 100 // $100/day impact
  },

  // 4. Soft constraints (preferences)
  'SOFT',
  {
    preferredProviders: ['anthropic', 'openai'],
    preferredModels: ['claude-sonnet-4'],
    preferStreaming: true
  }
];
```

### 3.3 Multi-Objective Optimization Algorithm

**Weighted Sum Method**:

```typescript
function optimizeMultiObjective(
  candidates: LLMConfig[],
  objectives: Objective[]
): LLMConfig {
  let bestConfig: LLMConfig | null = null;
  let bestScore = -Infinity;

  for (const config of candidates) {
    // Predict metrics for this config
    const predicted = await predictMetrics(config);

    // Calculate weighted score
    let score = 0;
    let feasible = true;

    for (const obj of objectives) {
      const value = predicted[obj.metric];

      // Check constraints
      if (obj.constraint) {
        if (obj.constraint.min !== undefined && value < obj.constraint.min) {
          feasible = false;
          break;
        }
        if (obj.constraint.max !== undefined && value > obj.constraint.max) {
          feasible = false;
          break;
        }
      }

      // Normalize and weight
      const normalized = normalize(value, obj.metric);
      const contribution = obj.goal === 'minimize'
        ? (1 - normalized) * obj.weight
        : normalized * obj.weight;

      score += contribution;
    }

    if (feasible && score > bestScore) {
      bestScore = score;
      bestConfig = config;
    }
  }

  return bestConfig;
}
```

**Pareto-Based Selection**:

```typescript
function selectFromParetoFrontier(
  frontier: LLMConfig[],
  preferences: ObjectiveWeights
): LLMConfig {
  // Calculate preference score for each Pareto-optimal config
  const scored = frontier.map(config => ({
    config,
    score: calculatePreferenceScore(config, preferences)
  }));

  // Sort by score
  scored.sort((a, b) => b.score - a.score);

  // Return highest-scoring config
  return scored[0].config;
}

function calculatePreferenceScore(
  config: LLMConfig,
  preferences: ObjectiveWeights
): number {
  let score = 0;

  for (const [objective, weight] of Object.entries(preferences)) {
    const value = predictedMetrics[config.id][objective];
    const normalized = normalize(value, objective);
    score += normalized * weight;
  }

  return score;
}
```

---

## 4. State Machine Transitions

### 4.1 State Definitions

```typescript
enum ControlLoopState {
  IDLE = 'IDLE',
  COLLECTING = 'COLLECTING',
  ANALYZING = 'ANALYZING',
  DECIDING = 'DECIDING',
  VALIDATING = 'VALIDATING',
  DEPLOYING = 'DEPLOYING',
  MONITORING = 'MONITORING',
  ROLLING_BACK = 'ROLLING_BACK',
  ERROR = 'ERROR'
}

interface StateTransition {
  from: ControlLoopState;
  to: ControlLoopState;
  condition: () => boolean;
  action?: () => Promise<void>;
}
```

### 4.2 Transition Table

```typescript
const STATE_TRANSITIONS: StateTransition[] = [
  // IDLE → COLLECTING
  {
    from: ControlLoopState.IDLE,
    to: ControlLoopState.COLLECTING,
    condition: () => shouldStartLoop(),
    action: async () => await initializeCollection()
  },

  // COLLECTING → ANALYZING
  {
    from: ControlLoopState.COLLECTING,
    to: ControlLoopState.ANALYZING,
    condition: () => hasEnoughData(),
    action: async () => await startAnalysis()
  },

  // COLLECTING → IDLE (insufficient data)
  {
    from: ControlLoopState.COLLECTING,
    to: ControlLoopState.IDLE,
    condition: () => !hasEnoughData() && timeout(),
    action: async () => await cleanup()
  },

  // ANALYZING → DECIDING
  {
    from: ControlLoopState.ANALYZING,
    to: ControlLoopState.DECIDING,
    condition: () => analysisComplete(),
    action: async () => await prepareDecision()
  },

  // ANALYZING → ERROR
  {
    from: ControlLoopState.ANALYZING,
    to: ControlLoopState.ERROR,
    condition: () => analysisFailed(),
    action: async () => await handleError()
  },

  // DECIDING → IDLE (no action needed)
  {
    from: ControlLoopState.DECIDING,
    to: ControlLoopState.IDLE,
    condition: () => noActionNeeded(),
    action: async () => await recordNoAction()
  },

  // DECIDING → VALIDATING
  {
    from: ControlLoopState.DECIDING,
    to: ControlLoopState.VALIDATING,
    condition: () => hasOptimizationPlan(),
    action: async () => await validatePlan()
  },

  // VALIDATING → DEPLOYING
  {
    from: ControlLoopState.VALIDATING,
    to: ControlLoopState.DEPLOYING,
    condition: () => planIsValid(),
    action: async () => await startDeployment()
  },

  // VALIDATING → IDLE (invalid plan)
  {
    from: ControlLoopState.VALIDATING,
    to: ControlLoopState.IDLE,
    condition: () => !planIsValid(),
    action: async () => await rejectPlan()
  },

  // DEPLOYING → MONITORING
  {
    from: ControlLoopState.DEPLOYING,
    to: ControlLoopState.MONITORING,
    condition: () => deploymentSuccessful(),
    action: async () => await startMonitoring()
  },

  // DEPLOYING → ROLLING_BACK
  {
    from: ControlLoopState.DEPLOYING,
    to: ControlLoopState.ROLLING_BACK,
    condition: () => deploymentFailed(),
    action: async () => await initiateRollback()
  },

  // MONITORING → IDLE (stable)
  {
    from: ControlLoopState.MONITORING,
    to: ControlLoopState.IDLE,
    condition: () => isStable(),
    action: async () => await confirmSuccess()
  },

  // MONITORING → ROLLING_BACK
  {
    from: ControlLoopState.MONITORING,
    to: ControlLoopState.ROLLING_BACK,
    condition: () => healthCheckFailed(),
    action: async () => await initiateRollback()
  },

  // ROLLING_BACK → IDLE
  {
    from: ControlLoopState.ROLLING_BACK,
    to: ControlLoopState.IDLE,
    condition: () => rollbackComplete(),
    action: async () => await recordRollback()
  },

  // ERROR → IDLE (recovered)
  {
    from: ControlLoopState.ERROR,
    to: ControlLoopState.IDLE,
    condition: () => errorRecovered(),
    action: async () => await resetError()
  }
];
```

### 4.3 State Machine Implementation

```typescript
class ControlLoopStateMachine {
  private currentState: ControlLoopState = ControlLoopState.IDLE;
  private transitions: StateTransition[];

  constructor() {
    this.transitions = STATE_TRANSITIONS;
  }

  async tick(): Promise<void> {
    // Find valid transitions from current state
    const validTransitions = this.transitions.filter(
      t => t.from === this.currentState && t.condition()
    );

    if (validTransitions.length === 0) {
      // No valid transition, stay in current state
      return;
    }

    // Take first valid transition (highest priority)
    const transition = validTransitions[0];

    // Execute transition action
    if (transition.action) {
      try {
        await transition.action();
      } catch (error) {
        this.currentState = ControlLoopState.ERROR;
        throw error;
      }
    }

    // Update state
    const previousState = this.currentState;
    this.currentState = transition.to;

    // Log transition
    logger.info('State transition', {
      from: previousState,
      to: this.currentState
    });

    // Emit event
    this.emitStateChange(previousState, this.currentState);
  }

  getState(): ControlLoopState {
    return this.currentState;
  }
}
```

---

## 5. Optimization Strategies

### 5.1 Conservative Strategy

**When to Use**: Production environments, risk-averse scenarios

```typescript
const CONSERVATIVE_STRATEGY = {
  name: 'conservative',

  // Stricter thresholds
  thresholds: {
    minImprovement: 0.15, // 15% improvement required
    minConfidence: 0.9,   // 90% confidence required
    maxRisk: 0.05         // 5% risk tolerance
  },

  // Slower rollout
  deployment: {
    strategy: 'canary',
    steps: [
      { percentage: 1, duration: 600000 },   // 1% for 10 min
      { percentage: 5, duration: 900000 },   // 5% for 15 min
      { percentage: 25, duration: 1800000 }, // 25% for 30 min
      { percentage: 50, duration: 3600000 }, // 50% for 1 hour
      { percentage: 100, duration: 0 }
    ]
  },

  // Aggressive rollback
  rollback: {
    automatic: true,
    triggers: {
      errorRateIncrease: 0.02,  // 2% increase
      latencyIncrease: 0.1,     // 10% increase
      qualityDecrease: 0.05     // 5% decrease
    }
  },

  // Limited exploration
  exploration: {
    enabled: false,
    explorationRate: 0.0
  }
};
```

### 5.2 Balanced Strategy

**When to Use**: Default strategy for most scenarios

```typescript
const BALANCED_STRATEGY = {
  name: 'balanced',

  thresholds: {
    minImprovement: 0.1,  // 10% improvement
    minConfidence: 0.8,   // 80% confidence
    maxRisk: 0.1          // 10% risk tolerance
  },

  deployment: {
    strategy: 'canary',
    steps: [
      { percentage: 5, duration: 300000 },   // 5% for 5 min
      { percentage: 25, duration: 600000 },  // 25% for 10 min
      { percentage: 50, duration: 900000 },  // 50% for 15 min
      { percentage: 100, duration: 0 }
    ]
  },

  rollback: {
    automatic: true,
    triggers: {
      errorRateIncrease: 0.05,
      latencyIncrease: 0.2,
      qualityDecrease: 0.1
    }
  },

  exploration: {
    enabled: true,
    explorationRate: 0.05  // 5% exploration
  }
};
```

### 5.3 Aggressive Strategy

**When to Use**: Development environments, cost optimization focus

```typescript
const AGGRESSIVE_STRATEGY = {
  name: 'aggressive',

  thresholds: {
    minImprovement: 0.05, // 5% improvement
    minConfidence: 0.7,   // 70% confidence
    maxRisk: 0.2          // 20% risk tolerance
  },

  deployment: {
    strategy: 'gradual',
    steps: [
      { percentage: 10, duration: 180000 },  // 10% for 3 min
      { percentage: 50, duration: 300000 },  // 50% for 5 min
      { percentage: 100, duration: 0 }
    ]
  },

  rollback: {
    automatic: true,
    triggers: {
      errorRateIncrease: 0.1,
      latencyIncrease: 0.5,
      qualityDecrease: 0.15
    }
  },

  exploration: {
    enabled: true,
    explorationRate: 0.15  // 15% exploration
  }
};
```

---

## 6. Performance Benchmarks

### 6.1 Loop Performance Targets

```typescript
const PERFORMANCE_TARGETS = {
  // Throughput
  metricsIngestionRate: 10000, // events/second
  analysisCapacity: 1000000,   // data points/cycle

  // Latency
  observePhaseLatency: 30000,  // ms (p95)
  analyzePhaseLatency: 60000,  // ms (p95)
  decidePhaseLatency: 45000,   // ms (p95)
  actPhaseLatency: 120000,     // ms (p95)

  // Total cycle time
  totalCycleTime: 300000,      // ms (p95) - 5 minutes

  // Resource usage
  maxMemoryUsage: 2048,        // MB
  maxCpuUsage: 0.8,            // 80%

  // Data freshness
  metricsFreshness: 60000,     // ms - metrics no older than 1 min
  configPropagation: 5000      // ms - config changes propagate within 5s
};
```

### 6.2 Scalability Targets

```typescript
const SCALABILITY_TARGETS = {
  // Horizontal scaling
  maxServices: 1000,           // monitored services
  maxEndpoints: 10000,         // monitored endpoints
  maxModels: 100,              // supported models

  // Data volume
  metricsRetention: {
    raw: 7 * 24 * 3600 * 1000,      // 7 days
    aggregated: 365 * 24 * 3600 * 1000  // 1 year
  },

  // Concurrent operations
  maxConcurrentOptimizations: 10,
  maxConcurrentDeployments: 5,
  maxConcurrentRollbacks: 3
};
```

---

## 7. Operational Procedures

### 7.1 Startup Procedure

```typescript
async function startupProcedure(): Promise<void> {
  // 1. Initialize databases
  await initializeDatabases();

  // 2. Load configuration
  const config = await loadConfiguration();

  // 3. Restore state
  const state = await restoreState();

  // 4. Verify connectivity
  await verifyConnections();

  // 5. Start metrics collection
  await startMetricsCollection();

  // 6. Initialize analyzers
  await initializeAnalyzers();

  // 7. Load ML models
  await loadMLModels();

  // 8. Start control loop
  await startControlLoop();

  // 9. Enable API endpoints
  await enableAPI();

  logger.info('Optimizer started successfully');
}
```

### 7.2 Shutdown Procedure

```typescript
async function shutdownProcedure(): Promise<void> {
  // 1. Stop accepting new requests
  await disableAPI();

  // 2. Stop control loop
  await stopControlLoop();

  // 3. Complete in-flight operations
  await waitForInflightOperations();

  // 4. Flush metrics buffers
  await flushMetrics();

  // 5. Save state
  await saveState();

  // 6. Close connections
  await closeConnections();

  logger.info('Optimizer shutdown complete');
}
```

### 7.3 Emergency Stop Procedure

```typescript
async function emergencyStop(reason: string): Promise<void> {
  logger.error('Emergency stop initiated', { reason });

  // 1. Pause all optimizations
  await pauseOptimizations();

  // 2. Halt deployments
  await haltDeployments();

  // 3. Notify operators
  await notifyOperators({
    severity: 'critical',
    message: `Emergency stop: ${reason}`
  });

  // 4. Enter safe mode
  await enterSafeMode();

  // Safe mode: Only observe, do not act
  this.safeMode = true;
}
```

---

## 8. Testing & Validation

### 8.1 Control Loop Tests

```typescript
describe('Control Loop', () => {
  test('completes full cycle within time budget', async () => {
    const start = Date.now();
    await controlLoop.run();
    const duration = Date.now() - start;

    expect(duration).toBeLessThan(PERFORMANCE_TARGETS.totalCycleTime);
  });

  test('handles missing data gracefully', async () => {
    mockMetricsSource.setData(null);
    const result = await controlLoop.run();

    expect(result.decision).toBe('skip');
    expect(result.reason).toContain('insufficient data');
  });

  test('respects cooldown periods', async () => {
    await controlLoop.run(); // First run
    const result = await controlLoop.run(); // Immediate second run

    expect(result.decision).toBe('skip');
    expect(result.reason).toContain('cooldown');
  });

  test('transitions states correctly', async () => {
    const states = [];
    controlLoop.on('stateChange', (state) => states.push(state));

    await controlLoop.run();

    expect(states).toEqual([
      'IDLE',
      'COLLECTING',
      'ANALYZING',
      'DECIDING',
      'IDLE'
    ]);
  });
});
```

### 8.2 Decision Algorithm Tests

```typescript
describe('Decision Engine', () => {
  test('selects correct optimization based on objectives', async () => {
    const objectives = [
      { metric: 'cost', goal: 'minimize', weight: 0.7 },
      { metric: 'quality', goal: 'maximize', weight: 0.3 }
    ];

    const decision = await decisionEngine.decide(objectives);

    expect(decision.plan).toBeDefined();
    expect(decision.plan.expectedImprovement.cost).toBeLessThan(0);
  });

  test('respects hard constraints', async () => {
    const constraints = [
      { metric: 'quality', min: 90 }
    ];

    const decision = await decisionEngine.decide([], constraints);

    for (const config of decision.candidates) {
      expect(config.expectedQuality).toBeGreaterThanOrEqual(90);
    }
  });

  test('handles conflicting objectives', async () => {
    const objectives = [
      { metric: 'cost', goal: 'minimize', weight: 1.0 },
      { metric: 'latency', goal: 'minimize', weight: 1.0 }
    ];

    const decision = await decisionEngine.decide(objectives);

    // Should find Pareto-optimal solution
    expect(decision.plan).toBeDefined();
    expect(decision.alternatives.length).toBeGreaterThan(0);
  });
});
```

---

**End of Control Loop Specification**
