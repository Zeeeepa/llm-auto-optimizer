# Optimization Testing and Validation Framework

## Overview

This document defines comprehensive testing strategies, validation frameworks, and quality assurance processes for the LLM Auto-Optimizer system.

---

## 1. Testing Strategy Hierarchy

### 1.1 Testing Pyramid

```
                    /\
                   /  \
                  /    \     E2E Tests (5%)
                 /------\    - Full optimization cycles
                /        \   - Multi-strategy integration
               /----------\
              /            \  Integration Tests (25%)
             /              \ - Strategy combinations
            /                \ - Cross-component interactions
           /------------------\
          /                    \ Unit Tests (70%)
         /                      \ - Individual algorithms
        /                        \ - Utility functions
       /                          \ - Statistical methods
      /----------------------------\
```

### 1.2 Test Coverage Requirements

| Component | Unit Test Coverage | Integration Test Coverage | E2E Test Coverage |
|-----------|-------------------|--------------------------|-------------------|
| A/B Testing | 90%+ | 80%+ | 2+ scenarios |
| Reinforcement Learning | 85%+ | 75%+ | 2+ scenarios |
| Cost Optimization | 90%+ | 85%+ | 3+ scenarios |
| Anomaly Detection | 85%+ | 70%+ | 2+ scenarios |
| Parameter Tuning | 80%+ | 75%+ | 2+ scenarios |

---

## 2. Unit Testing Framework

### 2.1 A/B Testing Unit Tests

```typescript
import { describe, it, expect, beforeEach } from '@jest/globals';
import { ABTestManager } from './ab-test-manager';

describe('ABTestManager', () => {
  let testManager: ABTestManager;

  beforeEach(() => {
    testManager = new ABTestManager();
  });

  describe('selectVariant', () => {
    it('should consistently assign same user to same variant', () => {
      const experimentId = 'test-exp-1';
      const userId = 'user-123';

      const experiment = createTestExperiment(experimentId);
      testManager.createExperiment(experiment);

      // Same user should get same variant
      const variant1 = testManager.selectVariant(userId, experimentId);
      const variant2 = testManager.selectVariant(userId, experimentId);

      expect(variant1.id).toBe(variant2.id);
    });

    it('should distribute traffic according to allocation weights', () => {
      const experimentId = 'test-exp-2';
      const experiment = createTestExperiment(experimentId, {
        variants: [
          { id: 'v1', allocationWeight: 0.5 },
          { id: 'v2', allocationWeight: 0.5 },
        ],
      });

      testManager.createExperiment(experiment);

      // Simulate 1000 users
      const assignments = new Map<string, number>();
      for (let i = 0; i < 1000; i++) {
        const userId = `user-${i}`;
        const variant = testManager.selectVariant(userId, experimentId);
        assignments.set(variant.id, (assignments.get(variant.id) || 0) + 1);
      }

      // Check distribution is roughly 50/50 (within 10% margin)
      const v1Count = assignments.get('v1') || 0;
      const v2Count = assignments.get('v2') || 0;

      expect(v1Count).toBeGreaterThan(400);
      expect(v1Count).toBeLessThan(600);
      expect(v2Count).toBeGreaterThan(400);
      expect(v2Count).toBeLessThan(600);
    });

    it('should throw error for non-existent experiment', () => {
      expect(() => {
        testManager.selectVariant('user-1', 'non-existent');
      }).toThrow('Experiment non-existent not found');
    });
  });

  describe('evaluateExperiment', () => {
    it('should detect insufficient data', () => {
      const experimentId = 'test-exp-3';
      const experiment = createTestExperiment(experimentId, {
        minSampleSize: 100,
      });

      testManager.createExperiment(experiment);

      // Add only 50 samples
      for (let i = 0; i < 50; i++) {
        testManager.recordMetrics(experimentId, 'v1', {
          success: true,
          latency: 1000,
          cost: 0.01,
          qualityScore: 85,
        });
      }

      const evaluation = testManager.evaluateExperiment(experimentId);

      expect(evaluation.hasWinner).toBe(false);
      expect(evaluation.recommendation).toContain('insufficient data');
    });

    it('should detect statistically significant winner', () => {
      const experimentId = 'test-exp-4';
      const experiment = createTestExperiment(experimentId, {
        minSampleSize: 100,
        confidenceLevel: 0.95,
      });

      testManager.createExperiment(experiment);

      // Control: 70% success rate
      for (let i = 0; i < 100; i++) {
        testManager.recordMetrics(experimentId, 'control', {
          success: i < 70,
          latency: 1000,
          cost: 0.01,
          qualityScore: 75,
        });
      }

      // Variant: 85% success rate (clearly better)
      for (let i = 0; i < 100; i++) {
        testManager.recordMetrics(experimentId, 'variant-1', {
          success: i < 85,
          latency: 1000,
          cost: 0.01,
          qualityScore: 85,
        });
      }

      const evaluation = testManager.evaluateExperiment(experimentId);

      expect(evaluation.hasWinner).toBe(true);
      expect(evaluation.winnerId).toBe('variant-1');
      expect(evaluation.confidence).toBeGreaterThan(0.95);
    });

    it('should detect insignificant differences', () => {
      const experimentId = 'test-exp-5';
      const experiment = createTestExperiment(experimentId);

      testManager.createExperiment(experiment);

      // Both variants perform similarly (75% success rate)
      for (let i = 0; i < 100; i++) {
        testManager.recordMetrics(experimentId, 'control', {
          success: i < 75,
          latency: 1000,
          cost: 0.01,
          qualityScore: 80,
        });

        testManager.recordMetrics(experimentId, 'variant-1', {
          success: i < 76, // Only 1% difference
          latency: 1000,
          cost: 0.01,
          qualityScore: 81,
        });
      }

      const evaluation = testManager.evaluateExperiment(experimentId);

      expect(evaluation.hasWinner).toBe(false);
      expect(evaluation.recommendation).toContain('not statistically significant');
    });
  });

  describe('recordMetrics', () => {
    it('should accumulate metrics correctly', () => {
      const experimentId = 'test-exp-6';
      const experiment = createTestExperiment(experimentId);

      testManager.createExperiment(experiment);

      testManager.recordMetrics(experimentId, 'v1', {
        success: true,
        latency: 1000,
        cost: 0.01,
        qualityScore: 85,
      });

      testManager.recordMetrics(experimentId, 'v1', {
        success: false,
        latency: 2000,
        cost: 0.02,
        qualityScore: 70,
      });

      const stats = testManager.getVariantStatistics(experimentId, 'v1');

      expect(stats.sampleSize).toBe(2);
      expect(stats.successRate).toBe(0.5);
      expect(stats.avgLatency).toBe(1500);
      expect(stats.avgCost).toBe(0.015);
      expect(stats.avgQuality).toBe(77.5);
    });
  });
});

function createTestExperiment(id: string, overrides: any = {}): any {
  return {
    experimentId: id,
    variants: [
      { id: 'control', allocationWeight: 0.5 },
      { id: 'variant-1', allocationWeight: 0.5 },
    ],
    minSampleSize: 100,
    confidenceLevel: 0.95,
    startDate: new Date(),
    endDate: null,
    ...overrides,
  };
}
```

### 2.2 Bandit Algorithm Unit Tests

```typescript
import { describe, it, expect, beforeEach } from '@jest/globals';
import { ThompsonSamplingBandit } from './thompson-sampling';

describe('ThompsonSamplingBandit', () => {
  let bandit: ThompsonSamplingBandit;

  beforeEach(() => {
    const parameterSpace = [
      { id: 'temp-0.0', value: 0.0 },
      { id: 'temp-0.5', value: 0.5 },
      { id: 'temp-1.0', value: 1.0 },
    ];
    bandit = new ThompsonSamplingBandit(parameterSpace);
  });

  describe('selectArm', () => {
    it('should explore all arms initially', () => {
      const selectedArms = new Set<string>();

      // With Thompson Sampling, we should see all arms selected
      // within a reasonable number of iterations
      for (let i = 0; i < 100; i++) {
        const arm = bandit.selectArm();
        selectedArms.add(arm.parameterId);
      }

      expect(selectedArms.size).toBe(3); // All arms explored
    });

    it('should converge to best arm over time', () => {
      // Simulate rewards: temp-0.5 is best
      const trueRewards = {
        'temp-0.0': 0.3,
        'temp-0.5': 0.8, // Best
        'temp-1.0': 0.4,
      };

      // Run many iterations
      for (let i = 0; i < 1000; i++) {
        const arm = bandit.selectArm();

        // Simulate reward with some noise
        const reward = trueRewards[arm.parameterId] + (Math.random() - 0.5) * 0.2;
        bandit.updateArm(arm.parameterId, reward);
      }

      // Best arm should have highest average reward
      const bestArm = bandit.getBestArm();
      expect(bestArm.parameterId).toBe('temp-0.5');
      expect(bestArm.avgReward).toBeGreaterThan(0.6);
    });
  });

  describe('updateArm', () => {
    it('should update statistics correctly', () => {
      const arm = bandit.selectArm();
      const armId = arm.parameterId;

      bandit.updateArm(armId, 0.8);
      bandit.updateArm(armId, 0.6);

      const stats = bandit.getStatistics().find(s => s.parameterId === armId);

      expect(stats?.totalPulls).toBe(2);
      expect(stats?.avgReward).toBeCloseTo(0.7, 1);
    });

    it('should increase confidence with more samples', () => {
      const armId = 'temp-0.5';

      // Few samples = wide confidence interval
      bandit.updateArm(armId, 0.8);
      bandit.updateArm(armId, 0.7);

      const statsFew = bandit.getStatistics().find(s => s.parameterId === armId);
      const widthFew = statsFew!.confidenceInterval[1] - statsFew!.confidenceInterval[0];

      // Many samples = narrow confidence interval
      for (let i = 0; i < 100; i++) {
        bandit.updateArm(armId, 0.75 + (Math.random() - 0.5) * 0.1);
      }

      const statsMany = bandit.getStatistics().find(s => s.parameterId === armId);
      const widthMany = statsMany!.confidenceInterval[1] - statsMany!.confidenceInterval[0];

      expect(widthMany).toBeLessThan(widthFew);
    });

    it('should handle negative rewards', () => {
      const armId = 'temp-1.0';

      bandit.updateArm(armId, -0.5);
      bandit.updateArm(armId, -0.3);

      const stats = bandit.getStatistics().find(s => s.parameterId === armId);

      expect(stats?.avgReward).toBeLessThan(0);
      expect(stats?.avgReward).toBeCloseTo(-0.4, 1);
    });
  });

  describe('getBestArm', () => {
    it('should return arm with highest average reward', () => {
      bandit.updateArm('temp-0.0', 0.3);
      bandit.updateArm('temp-0.5', 0.8);
      bandit.updateArm('temp-1.0', 0.5);

      const best = bandit.getBestArm();

      expect(best.parameterId).toBe('temp-0.5');
      expect(best.avgReward).toBe(0.8);
    });
  });
});
```

### 2.3 Anomaly Detection Unit Tests

```typescript
import { describe, it, expect } from '@jest/globals';
import { AnomalyDetector } from './anomaly-detector';

describe('AnomalyDetector', () => {
  describe('Z-Score Detection', () => {
    it('should detect spike anomalies', () => {
      const detector = new AnomalyDetector({
        method: 'zscore',
        sensitivity: 3.0,
      });

      const timeSeries = [
        ...generateNormalData(100, 50, 5), // Mean=50, StdDev=5
        { timestamp: new Date(), value: 80 }, // Spike!
      ];

      const anomalies = detector.detect(timeSeries);

      expect(anomalies.length).toBeGreaterThan(0);
      expect(anomalies[0].value).toBe(80);
      expect(anomalies[0].type).toBe('spike');
    });

    it('should detect drop anomalies', () => {
      const detector = new AnomalyDetector({
        method: 'zscore',
        sensitivity: 3.0,
      });

      const timeSeries = [
        ...generateNormalData(100, 50, 5),
        { timestamp: new Date(), value: 20 }, // Drop!
      ];

      const anomalies = detector.detect(timeSeries);

      expect(anomalies.length).toBeGreaterThan(0);
      expect(anomalies[0].value).toBe(20);
      expect(anomalies[0].type).toBe('drop');
    });

    it('should not flag normal variation', () => {
      const detector = new AnomalyDetector({
        method: 'zscore',
        sensitivity: 3.0,
      });

      const timeSeries = generateNormalData(100, 50, 5);

      const anomalies = detector.detect(timeSeries);

      // With 3-sigma threshold, ~99.7% should be normal
      expect(anomalies.length).toBeLessThan(5);
    });
  });

  describe('IQR Detection', () => {
    it('should be robust to outliers in baseline', () => {
      const detector = new AnomalyDetector({
        method: 'iqr',
        sensitivity: 1.5,
      });

      const timeSeries = [
        ...generateNormalData(95, 50, 5),
        { timestamp: new Date(), value: 1000 }, // Outlier in baseline
        ...generateNormalData(4, 50, 5),
        { timestamp: new Date(), value: 120 }, // Should still detect this
      ];

      const anomalies = detector.detect(timeSeries);

      const anomalyValues = anomalies.map(a => a.value);
      expect(anomalyValues).toContain(1000);
      expect(anomalyValues).toContain(120);
    });
  });

  describe('MAD Detection', () => {
    it('should handle small sample sizes', () => {
      const detector = new AnomalyDetector({
        method: 'mad',
        sensitivity: 3.0,
      });

      const timeSeries = [
        { timestamp: new Date(), value: 10 },
        { timestamp: new Date(), value: 12 },
        { timestamp: new Date(), value: 11 },
        { timestamp: new Date(), value: 50 }, // Clear outlier
      ];

      const anomalies = detector.detect(timeSeries);

      expect(anomalies.length).toBe(1);
      expect(anomalies[0].value).toBe(50);
    });
  });

  describe('Sensitivity Tuning', () => {
    it('should detect more anomalies with lower sensitivity', () => {
      const timeSeries = generateNormalData(100, 50, 10);

      const detectorHigh = new AnomalyDetector({
        method: 'zscore',
        sensitivity: 3.0,
      });

      const detectorLow = new AnomalyDetector({
        method: 'zscore',
        sensitivity: 2.0,
      });

      const anomaliesHigh = detectorHigh.detect(timeSeries);
      const anomaliesLow = detectorLow.detect(timeSeries);

      expect(anomaliesLow.length).toBeGreaterThanOrEqual(anomaliesHigh.length);
    });
  });
});

function generateNormalData(
  count: number,
  mean: number,
  stdDev: number
): Array<{ timestamp: Date; value: number }> {
  const data = [];
  for (let i = 0; i < count; i++) {
    // Box-Muller transform for normal distribution
    const u1 = Math.random();
    const u2 = Math.random();
    const z0 = Math.sqrt(-2 * Math.log(u1)) * Math.cos(2 * Math.PI * u2);
    const value = mean + z0 * stdDev;

    data.push({
      timestamp: new Date(Date.now() + i * 1000),
      value,
    });
  }
  return data;
}
```

---

## 3. Integration Testing Framework

### 3.1 Multi-Strategy Integration Test

```typescript
import { describe, it, expect, beforeEach } from '@jest/globals';
import { LLMAutoOptimizer } from './optimizer';

describe('LLMAutoOptimizer Integration', () => {
  let optimizer: LLMAutoOptimizer;

  beforeEach(() => {
    optimizer = new LLMAutoOptimizer({
      dailyBudget: 100,
      enableABTesting: true,
      enableBanditOptimization: true,
      enableAnomalyDetection: true,
    });
  });

  it('should coordinate A/B testing with budget constraints', async () => {
    // Create A/B test with expensive model
    await optimizer.createABTest({
      variants: [
        { id: 'control', modelId: 'gpt-4' },
        { id: 'variant', modelId: 'gpt-3.5-turbo' },
      ],
    });

    // Process many requests until budget is low
    const results = [];
    for (let i = 0; i < 100; i++) {
      try {
        const result = await optimizer.processRequest({
          userId: `user-${i}`,
          prompt: 'Test prompt',
        });
        results.push(result);
      } catch (error) {
        // Budget exceeded
        break;
      }
    }

    // Verify cheaper variant was preferred as budget decreased
    const finalResults = results.slice(-10);
    const cheaperModelUsage = finalResults.filter(
      r => r.metadata.modelUsed === 'gpt-3.5-turbo'
    ).length;

    expect(cheaperModelUsage).toBeGreaterThan(5); // Majority should use cheaper model
  });

  it('should adapt parameters based on feedback', async () => {
    const userId = 'test-user';
    const results = [];

    // Simulate 50 requests with feedback
    for (let i = 0; i < 50; i++) {
      const result = await optimizer.processRequest({
        userId,
        prompt: 'Test prompt',
      });

      // Provide feedback (simulate good results with temperature=0.7)
      if (result.metadata.temperature === 0.7) {
        await optimizer.provideFeedback(result.requestId, 'positive');
      } else {
        await optimizer.provideFeedback(result.requestId, 'negative');
      }

      results.push(result);
    }

    // Check if optimizer converged to temperature=0.7
    const recentResults = results.slice(-10);
    const optimalTempUsage = recentResults.filter(
      r => Math.abs(r.metadata.temperature - 0.7) < 0.1
    ).length;

    expect(optimalTempUsage).toBeGreaterThan(7); // Should learn to prefer 0.7
  });

  it('should detect and recover from performance degradation', async () => {
    // Establish baseline performance
    for (let i = 0; i < 100; i++) {
      await optimizer.processRequest({
        userId: `user-${i}`,
        prompt: 'Test prompt',
      });
    }

    const baselineMetrics = await optimizer.getPerformanceMetrics();

    // Simulate degradation (inject poor-performing configuration)
    await optimizer.injectConfiguration({
      modelId: 'poor-model',
      temperature: 2.0, // Too high
    });

    // Process more requests
    for (let i = 0; i < 50; i++) {
      await optimizer.processRequest({
        userId: `user-degraded-${i}`,
        prompt: 'Test prompt',
      });
    }

    // Check if degradation was detected
    const alerts = await optimizer.getAlerts();
    const degradationAlert = alerts.find(
      a => a.type === 'performance_degradation'
    );

    expect(degradationAlert).toBeDefined();
    expect(degradationAlert?.severity).toBe('severe');

    // Verify rollback occurred
    const currentConfig = await optimizer.getCurrentConfiguration();
    expect(currentConfig.modelId).not.toBe('poor-model');
  });

  it('should handle concurrent optimization strategies', async () => {
    // Enable all strategies
    await optimizer.enableStrategy('ab-testing');
    await optimizer.enableStrategy('bandit-optimization');
    await optimizer.enableStrategy('cost-optimization');

    const results = [];

    // Process requests concurrently
    const promises = [];
    for (let i = 0; i < 20; i++) {
      promises.push(
        optimizer.processRequest({
          userId: `user-${i}`,
          prompt: 'Test prompt',
        })
      );
    }

    const responses = await Promise.all(promises);

    // Verify all strategies were active
    const abTestAssignments = new Set(
      responses.map(r => r.metadata.variantId)
    );
    const temperaturesUsed = new Set(
      responses.map(r => r.metadata.temperature)
    );
    const modelsUsed = new Set(
      responses.map(r => r.metadata.modelUsed)
    );

    expect(abTestAssignments.size).toBeGreaterThan(1); // Multiple variants
    expect(temperaturesUsed.size).toBeGreaterThan(1); // Multiple temps
    expect(modelsUsed.size).toBeGreaterThan(1); // Multiple models
  });
});
```

---

## 4. End-to-End Testing

### 4.1 Full Optimization Cycle Test

```typescript
import { describe, it, expect } from '@jest/globals';
import { runE2ETest } from './e2e-helpers';

describe('End-to-End Optimization Cycle', () => {
  it('should complete full optimization lifecycle', async () => {
    const testDuration = 3600000; // 1 hour simulation
    const requestRate = 10; // 10 requests per second

    const testConfig = {
      initialConfig: {
        modelId: 'gpt-4',
        temperature: 0.7,
        maxTokens: 1000,
      },
      budget: {
        daily: 1000,
        perRequest: 1.0,
      },
      optimizationGoals: {
        primaryMetric: 'quality',
        constraints: {
          maxCostPerRequest: 0.5,
          minQuality: 80,
          maxLatency: 3000,
        },
      },
    };

    const results = await runE2ETest({
      duration: testDuration,
      requestRate,
      config: testConfig,
    });

    // Verify optimization objectives
    expect(results.finalMetrics.avgCostPerRequest).toBeLessThanOrEqual(
      testConfig.optimizationGoals.constraints.maxCostPerRequest
    );
    expect(results.finalMetrics.avgQuality).toBeGreaterThanOrEqual(
      testConfig.optimizationGoals.constraints.minQuality
    );
    expect(results.finalMetrics.avgLatency).toBeLessThanOrEqual(
      testConfig.optimizationGoals.constraints.maxLatency
    );

    // Verify improvement over baseline
    expect(results.finalMetrics.avgQuality).toBeGreaterThanOrEqual(
      results.baselineMetrics.avgQuality
    );

    // Verify cost reduction
    expect(results.totalCost).toBeLessThan(
      results.baselineCost
    );

    // Verify no budget violations
    expect(results.budgetViolations).toBe(0);

    // Verify successful A/B tests
    expect(results.abTests.completed).toBeGreaterThan(0);
    expect(results.abTests.successRate).toBeGreaterThan(0.7);
  }, 3700000); // Timeout slightly longer than test duration
});
```

---

## 5. Performance Testing

### 5.1 Load Testing

```typescript
import { describe, it, expect } from '@jest/globals';
import { loadTest } from './load-test-helpers';

describe('Performance Under Load', () => {
  it('should maintain performance at high request rates', async () => {
    const results = await loadTest({
      requestsPerSecond: 100,
      duration: 60000, // 1 minute
      concurrency: 50,
    });

    // Verify throughput
    expect(results.actualRequestsPerSecond).toBeGreaterThan(95);

    // Verify latency percentiles
    expect(results.latency.p50).toBeLessThan(100);
    expect(results.latency.p95).toBeLessThan(500);
    expect(results.latency.p99).toBeLessThan(1000);

    // Verify error rate
    expect(results.errorRate).toBeLessThan(0.01); // < 1%

    // Verify optimization quality maintained
    expect(results.avgQuality).toBeGreaterThan(75);
  });

  it('should handle traffic spikes gracefully', async () => {
    const results = await loadTest({
      pattern: 'spike',
      baseline: 10, // 10 req/s
      spike: 200,   // Spike to 200 req/s
      spikeDuration: 5000, // 5 seconds
      totalDuration: 60000,
    });

    // Verify system recovered
    expect(results.errorsDuringSpike).toBeLessThan(results.totalSpike * 0.05);
    expect(results.errorRateAfterSpike).toBeLessThan(0.01);

    // Verify optimization continued
    expect(results.optimizationsPaused).toBe(false);
  });
});
```

### 5.2 Stress Testing

```typescript
describe('Stress Testing', () => {
  it('should degrade gracefully under extreme load', async () => {
    const results = await loadTest({
      requestsPerSecond: 1000, // Extreme load
      duration: 30000,
      concurrency: 500,
    });

    // System should implement backpressure
    expect(results.backpressureActivated).toBe(true);

    // Should not crash
    expect(results.crashed).toBe(false);

    // Should maintain core functionality
    expect(results.requestsCompleted).toBeGreaterThan(0);

    // Should recover after load decrease
    const recoveryResults = await loadTest({
      requestsPerSecond: 10,
      duration: 10000,
    });

    expect(recoveryResults.errorRate).toBeLessThan(0.01);
  });
});
```

---

## 6. Validation Framework

### 6.1 Statistical Validation

```typescript
class StatisticalValidator {
  // Validate A/B test results
  validateABTest(results: ABTestResults): ValidationReport {
    const issues: ValidationIssue[] = [];

    // Check sample size
    if (results.sampleSize < results.minimumDetectableEffect.requiredSampleSize) {
      issues.push({
        severity: 'error',
        message: 'Insufficient sample size for reliable results',
        details: {
          actual: results.sampleSize,
          required: results.minimumDetectableEffect.requiredSampleSize,
        },
      });
    }

    // Check for Simpson's paradox
    const segmentAnalysis = this.analyzeSegments(results);
    if (segmentAnalysis.simpsonsParadoxDetected) {
      issues.push({
        severity: 'warning',
        message: "Simpson's paradox detected - aggregate winner differs from segment winners",
        details: segmentAnalysis,
      });
    }

    // Check for novelty effect
    const temporalAnalysis = this.analyzeTemporalPattern(results);
    if (temporalAnalysis.noveltyEffectSuspected) {
      issues.push({
        severity: 'warning',
        message: 'Possible novelty effect - performance degrading over time',
        details: temporalAnalysis,
      });
    }

    // Verify statistical assumptions
    const assumptions = this.checkStatisticalAssumptions(results);
    if (!assumptions.normalityHolds) {
      issues.push({
        severity: 'info',
        message: 'Non-normal distribution detected - consider non-parametric test',
        details: assumptions,
      });
    }

    return {
      valid: issues.filter(i => i.severity === 'error').length === 0,
      issues,
      confidence: this.calculateOverallConfidence(results, issues),
    };
  }

  // Validate reinforcement learning convergence
  validateBanditConvergence(history: BanditHistory): ValidationReport {
    const issues: ValidationIssue[] = [];

    // Check if exploration rate is reasonable
    const recentExplorationRate = this.calculateExplorationRate(
      history.recentSelections
    );

    if (recentExplorationRate > 0.3) {
      issues.push({
        severity: 'warning',
        message: 'High exploration rate - bandit may not have converged',
        details: { rate: recentExplorationRate },
      });
    }

    // Check for reward stationarity
    const stationarity = this.testStationarity(history.rewards);
    if (!stationarity.isStationary) {
      issues.push({
        severity: 'warning',
        message: 'Non-stationary rewards detected - environment may be changing',
        details: stationarity,
      });
    }

    // Verify regret is decreasing
    const regretAnalysis = this.analyzeRegret(history);
    if (!regretAnalysis.isDecreasing) {
      issues.push({
        severity: 'error',
        message: 'Cumulative regret not decreasing - bandit not learning',
        details: regretAnalysis,
      });
    }

    return {
      valid: issues.filter(i => i.severity === 'error').length === 0,
      issues,
      confidence: this.calculateConvergenceConfidence(history, issues),
    };
  }

  // Validate Pareto frontier
  validateParetoFrontier(
    configurations: ConfigurationPoint[]
  ): ValidationReport {
    const issues: ValidationIssue[] = [];

    // Check for dominated points (should be none)
    const dominatedPoints = configurations.filter(c => c.isDominated);
    if (dominatedPoints.length > 0) {
      issues.push({
        severity: 'error',
        message: 'Dominated points found in Pareto frontier',
        details: { count: dominatedPoints.length },
      });
    }

    // Check for gaps in frontier
    const gaps = this.detectFrontierGaps(configurations);
    if (gaps.length > 0) {
      issues.push({
        severity: 'info',
        message: 'Gaps detected in Pareto frontier - may benefit from more configurations',
        details: { gaps },
      });
    }

    // Verify diversity of solutions
    const diversity = this.calculateFrontierDiversity(configurations);
    if (diversity < 0.3) {
      issues.push({
        severity: 'warning',
        message: 'Low diversity in Pareto frontier - configurations too similar',
        details: { diversity },
      });
    }

    return {
      valid: issues.filter(i => i.severity === 'error').length === 0,
      issues,
      confidence: 0.95, // Pareto validation is deterministic
    };
  }

  private calculateExplorationRate(selections: string[]): number {
    const uniqueArms = new Set(selections.slice(-100));
    return uniqueArms.size / Math.min(selections.length, 100);
  }

  private testStationarity(rewards: number[]): {
    isStationary: boolean;
    pValue: number;
  } {
    // Augmented Dickey-Fuller test (simplified)
    const firstHalf = rewards.slice(0, Math.floor(rewards.length / 2));
    const secondHalf = rewards.slice(Math.floor(rewards.length / 2));

    const mean1 = this.mean(firstHalf);
    const mean2 = this.mean(secondHalf);

    const pooledStd = Math.sqrt(
      (this.variance(firstHalf) + this.variance(secondHalf)) / 2
    );

    const tStat = Math.abs(mean1 - mean2) / (pooledStd / Math.sqrt(rewards.length / 2));

    // Simplified p-value calculation
    const pValue = 2 * (1 - this.normalCDF(tStat));

    return {
      isStationary: pValue > 0.05,
      pValue,
    };
  }

  private analyzeRegret(history: BanditHistory): {
    isDecreasing: boolean;
    cumulativeRegret: number[];
  } {
    const optimalReward = Math.max(...history.optimalRewards);
    const cumulativeRegret = [];
    let sum = 0;

    for (let i = 0; i < history.rewards.length; i++) {
      sum += optimalReward - history.rewards[i];
      cumulativeRegret.push(sum);
    }

    // Check if recent regret growth is slowing (sublinear)
    const recentSlope = this.calculateSlope(
      cumulativeRegret.slice(-100)
    );

    return {
      isDecreasing: recentSlope < 1.0, // Sublinear growth
      cumulativeRegret,
    };
  }

  private calculateSlope(values: number[]): number {
    const n = values.length;
    const xMean = (n - 1) / 2;
    const yMean = this.mean(values);

    let numerator = 0;
    let denominator = 0;

    for (let i = 0; i < n; i++) {
      numerator += (i - xMean) * (values[i] - yMean);
      denominator += (i - xMean) ** 2;
    }

    return numerator / denominator;
  }

  private mean(values: number[]): number {
    return values.reduce((a, b) => a + b, 0) / values.length;
  }

  private variance(values: number[]): number {
    const m = this.mean(values);
    return values.reduce((sum, val) => sum + (val - m) ** 2, 0) / values.length;
  }

  private normalCDF(x: number): number {
    // Approximation
    const t = 1 / (1 + 0.2316419 * Math.abs(x));
    const d = 0.3989423 * Math.exp(-x * x / 2);
    const probability = d * t * (0.3193815 + t * (-0.3565638 + t * (1.781478 + t * (-1.821256 + t * 1.330274))));
    return x > 0 ? 1 - probability : probability;
  }
}
```

---

## 7. Continuous Validation

### 7.1 Production Monitoring Tests

```typescript
describe('Production Health Checks', () => {
  it('should detect quality regression in production', async () => {
    const monitor = new ProductionMonitor();

    // Simulate production traffic
    const results = await simulateProductionTraffic({
      duration: 3600000, // 1 hour
      requestsPerSecond: 50,
    });

    // Check quality didn't regress
    const qualityReport = monitor.checkQualityRegression(results);

    expect(qualityReport.regressionDetected).toBe(false);

    if (qualityReport.regressionDetected) {
      expect(qualityReport.autoRollbackTriggered).toBe(true);
    }
  });

  it('should validate cost savings claims', async () => {
    const validator = new CostValidator();

    const beforeOptimization = await collectMetrics({
      period: 'before',
      duration: 86400000, // 24 hours
    });

    const afterOptimization = await collectMetrics({
      period: 'after',
      duration: 86400000,
    });

    const report = validator.validateSavings(
      beforeOptimization,
      afterOptimization
    );

    expect(report.claimedSavings).toBeCloseTo(report.actualSavings, 0.05);
    expect(report.statisticallySignificant).toBe(true);
  });
});
```

---

## 8. Test Data Generation

### 8.1 Synthetic Data Generators

```typescript
class TestDataGenerator {
  // Generate realistic request distribution
  generateRequests(config: {
    count: number;
    distribution: 'uniform' | 'normal' | 'poisson';
    taskTypes: string[];
  }): TestRequest[] {
    const requests: TestRequest[] = [];

    for (let i = 0; i < config.count; i++) {
      const taskType = this.selectRandom(config.taskTypes);

      requests.push({
        id: `req-${i}`,
        userId: `user-${Math.floor(Math.random() * 1000)}`,
        prompt: this.generatePrompt(taskType),
        taskType,
        timestamp: this.generateTimestamp(i, config.distribution),
      });
    }

    return requests;
  }

  // Generate ground truth for quality evaluation
  generateGroundTruth(requests: TestRequest[]): Map<string, GroundTruth> {
    const groundTruth = new Map();

    for (const request of requests) {
      groundTruth.set(request.id, {
        expectedQuality: this.calculateExpectedQuality(request.taskType),
        expectedLatency: this.calculateExpectedLatency(request.taskType),
        expectedCost: this.calculateExpectedCost(request.taskType),
        referenceAnswer: this.generateReferenceAnswer(request),
      });
    }

    return groundTruth;
  }

  private generatePrompt(taskType: string): string {
    const templates = {
      'summarization': 'Summarize the following article: {text}',
      'qa': 'Answer this question: {question}',
      'code': 'Write a function that: {specification}',
      'translation': 'Translate to {language}: {text}',
    };

    return templates[taskType] || 'Default prompt template';
  }

  private generateTimestamp(index: number, distribution: string): Date {
    switch (distribution) {
      case 'uniform':
        return new Date(Date.now() + index * 1000);

      case 'poisson':
        // Exponential inter-arrival times
        const lambda = 0.1; // Rate parameter
        const interArrival = -Math.log(Math.random()) / lambda;
        return new Date(Date.now() + interArrival * 1000);

      default:
        return new Date(Date.now() + index * 1000);
    }
  }

  private selectRandom<T>(array: T[]): T {
    return array[Math.floor(Math.random() * array.length)];
  }
}
```

---

## 9. Test Coverage Reporting

### 9.1 Coverage Requirements

```json
{
  "jest": {
    "coverageThreshold": {
      "global": {
        "branches": 80,
        "functions": 85,
        "lines": 85,
        "statements": 85
      },
      "./src/core/": {
        "branches": 90,
        "functions": 95,
        "lines": 95,
        "statements": 95
      }
    }
  }
}
```

### 9.2 Quality Gates

```typescript
// CI/CD pipeline quality gates
const QUALITY_GATES = {
  unitTests: {
    minCoverage: 0.85,
    maxFailures: 0,
    maxDuration: 300000, // 5 minutes
  },
  integrationTests: {
    minCoverage: 0.75,
    maxFailures: 0,
    maxDuration: 600000, // 10 minutes
  },
  e2eTests: {
    minPassRate: 0.95,
    maxFailures: 1,
    maxDuration: 3600000, // 1 hour
  },
  performance: {
    maxLatencyP95: 500, // ms
    minThroughput: 100, // req/s
    maxErrorRate: 0.01,
  },
};
```

---

## Summary

This testing and validation framework provides:

1. **Comprehensive Unit Tests**: Cover individual algorithms and components
2. **Integration Tests**: Validate multi-strategy coordination
3. **End-to-End Tests**: Verify complete optimization cycles
4. **Performance Tests**: Ensure scalability and reliability
5. **Statistical Validation**: Rigorously validate optimization claims
6. **Continuous Monitoring**: Detect regressions in production
7. **Test Data Generation**: Create realistic test scenarios
8. **Quality Gates**: Enforce standards in CI/CD pipeline

Engineers should:
- Maintain >85% test coverage for all optimization code
- Run full test suite before production deployments
- Monitor production metrics continuously
- Validate all optimization claims statistically
- Automate regression detection and rollback
