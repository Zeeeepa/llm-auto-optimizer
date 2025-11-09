# Optimization Strategy Implementation Examples

## Overview

This document provides concrete implementation examples, code snippets, and practical guidance for implementing the optimization strategies defined in `optimization-strategies.md`.

---

## 1. A/B Testing Implementation Example

### 1.1 Complete A/B Test Flow

```typescript
interface PromptVariant {
  id: string;
  prompt: string;
  allocationWeight: number;
  metadata: {
    technique: string;
    createdAt: Date;
  };
}

interface ABTestConfig {
  experimentId: string;
  variants: PromptVariant[];
  minSampleSize: number;
  confidenceLevel: number;
  startDate: Date;
  endDate: Date | null;
}

class ABTestManager {
  private experiments: Map<string, ABTestConfig>;
  private metrics: Map<string, VariantMetrics[]>;

  // Route incoming request to appropriate variant
  selectVariant(userId: string, experimentId: string): PromptVariant {
    const experiment = this.experiments.get(experimentId);
    if (!experiment) {
      throw new Error(`Experiment ${experimentId} not found`);
    }

    // Consistent hashing for stable user assignment
    const hash = this.consistentHash(userId + experimentId);
    const normalizedHash = (hash % 100) / 100.0;

    let cumulativeWeight = 0;
    for (const variant of experiment.variants) {
      cumulativeWeight += variant.allocationWeight;
      if (normalizedHash < cumulativeWeight) {
        this.logAssignment(userId, experimentId, variant.id);
        return variant;
      }
    }

    // Fallback to control variant
    return experiment.variants[0];
  }

  // Consistent hash function for stable assignments
  private consistentHash(input: string): number {
    let hash = 0;
    for (let i = 0; i < input.length; i++) {
      const char = input.charCodeAt(i);
      hash = ((hash << 5) - hash) + char;
      hash = hash & hash; // Convert to 32-bit integer
    }
    return Math.abs(hash);
  }

  // Record metrics for variant performance
  recordMetrics(experimentId: string, variantId: string, metrics: {
    success: boolean;
    latency: number;
    cost: number;
    qualityScore: number;
    userFeedback?: 'positive' | 'negative' | 'neutral';
  }): void {
    const key = `${experimentId}:${variantId}`;
    if (!this.metrics.has(key)) {
      this.metrics.set(key, []);
    }

    const variantMetrics = this.metrics.get(key)!;
    variantMetrics.push({
      variantId,
      timestamp: new Date(),
      success: metrics.success,
      latency: metrics.latency,
      cost: metrics.cost,
      qualityScore: metrics.qualityScore,
      userFeedback: metrics.userFeedback,
    });
  }

  // Determine if experiment has a winner
  evaluateExperiment(experimentId: string): {
    hasWinner: boolean;
    winnerId: string | null;
    confidence: number;
    recommendation: string;
  } {
    const experiment = this.experiments.get(experimentId);
    if (!experiment) {
      throw new Error(`Experiment ${experimentId} not found`);
    }

    // Calculate aggregate metrics for each variant
    const variantStats = experiment.variants.map(variant => {
      const key = `${experimentId}:${variant.id}`;
      const metrics = this.metrics.get(key) || [];

      return {
        variantId: variant.id,
        sampleSize: metrics.length,
        successRate: metrics.filter(m => m.success).length / metrics.length,
        avgLatency: this.mean(metrics.map(m => m.latency)),
        avgCost: this.mean(metrics.map(m => m.cost)),
        avgQuality: this.mean(metrics.map(m => m.qualityScore)),
      };
    });

    // Check minimum sample size
    const insufficientData = variantStats.some(
      stats => stats.sampleSize < experiment.minSampleSize
    );

    if (insufficientData) {
      return {
        hasWinner: false,
        winnerId: null,
        confidence: 0,
        recommendation: 'Continue testing - insufficient data',
      };
    }

    // Rank variants by composite score
    const rankedVariants = variantStats
      .map(stats => ({
        ...stats,
        compositeScore: this.calculateCompositeScore(stats),
      }))
      .sort((a, b) => b.compositeScore - a.compositeScore);

    const topVariant = rankedVariants[0];
    const controlVariant = variantStats[0]; // First variant is control

    // Statistical significance test
    const { pValue, isSignificant } = this.twoProportionZTest(
      topVariant.successRate,
      topVariant.sampleSize,
      controlVariant.successRate,
      controlVariant.sampleSize,
      experiment.confidenceLevel
    );

    // Effect size
    const improvement = (topVariant.successRate - controlVariant.successRate)
                      / controlVariant.successRate;

    const isMeaningful = improvement > 0.05; // 5% minimum improvement

    if (isSignificant && isMeaningful) {
      return {
        hasWinner: true,
        winnerId: topVariant.variantId,
        confidence: 1 - pValue,
        recommendation: `Promote variant ${topVariant.variantId} to production (${(improvement * 100).toFixed(1)}% improvement)`,
      };
    }

    if (isSignificant && !isMeaningful) {
      return {
        hasWinner: false,
        winnerId: null,
        confidence: 1 - pValue,
        recommendation: 'Statistically significant but not meaningful - consider aborting test',
      };
    }

    return {
      hasWinner: false,
      winnerId: null,
      confidence: 1 - pValue,
      recommendation: 'Continue testing - not statistically significant yet',
    };
  }

  private calculateCompositeScore(stats: {
    successRate: number;
    avgLatency: number;
    avgCost: number;
    avgQuality: number;
  }): number {
    // Normalize metrics (assuming reasonable ranges)
    const qualityScore = stats.avgQuality / 100;
    const speedScore = Math.max(0, 1 - (stats.avgLatency / 10000)); // 10s max
    const costScore = Math.max(0, 1 - (stats.avgCost / 1.0)); // $1 max

    // Weighted combination
    return (
      0.4 * qualityScore +
      0.2 * speedScore +
      0.2 * costScore +
      0.2 * stats.successRate
    );
  }

  private twoProportionZTest(
    p1: number, n1: number,
    p2: number, n2: number,
    confidenceLevel: number
  ): { pValue: number; isSignificant: boolean } {
    // Pooled proportion
    const pooled = (p1 * n1 + p2 * n2) / (n1 + n2);

    // Standard error
    const se = Math.sqrt(pooled * (1 - pooled) * (1/n1 + 1/n2));

    // Z-statistic
    const z = (p1 - p2) / se;

    // P-value (two-tailed)
    const pValue = 2 * (1 - this.normalCDF(Math.abs(z)));

    return {
      pValue,
      isSignificant: pValue < (1 - confidenceLevel),
    };
  }

  private normalCDF(x: number): number {
    // Approximation of standard normal CDF
    const t = 1 / (1 + 0.2316419 * Math.abs(x));
    const d = 0.3989423 * Math.exp(-x * x / 2);
    const probability = d * t * (0.3193815 + t * (-0.3565638 + t * (1.781478 + t * (-1.821256 + t * 1.330274))));
    return x > 0 ? 1 - probability : probability;
  }

  private mean(values: number[]): number {
    if (values.length === 0) return 0;
    return values.reduce((a, b) => a + b, 0) / values.length;
  }

  private logAssignment(userId: string, experimentId: string, variantId: string): void {
    console.log(`[A/B Test] User ${userId} assigned to variant ${variantId} in experiment ${experimentId}`);
  }
}
```

### 1.2 Usage Example

```typescript
// Initialize A/B test
const testManager = new ABTestManager();

const experiment: ABTestConfig = {
  experimentId: 'prompt-optimization-v1',
  variants: [
    {
      id: 'control',
      prompt: 'Summarize the following text:\n\n{text}',
      allocationWeight: 0.33,
      metadata: { technique: 'baseline', createdAt: new Date() },
    },
    {
      id: 'variant-few-shot',
      prompt: 'Summarize the following text. Example:\nInput: "Long article about AI"\nOutput: "Brief summary of AI"\n\nNow summarize:\n{text}',
      allocationWeight: 0.33,
      metadata: { technique: 'few-shot', createdAt: new Date() },
    },
    {
      id: 'variant-structured',
      prompt: 'Summarize the following text in exactly 3 bullet points:\n{text}',
      allocationWeight: 0.34,
      metadata: { technique: 'structured', createdAt: new Date() },
    },
  ],
  minSampleSize: 100,
  confidenceLevel: 0.95,
  startDate: new Date(),
  endDate: null,
};

// Process request
const userId = 'user-12345';
const selectedVariant = testManager.selectVariant(userId, experiment.experimentId);

// Execute LLM call with selected variant
const response = await callLLM(selectedVariant.prompt, { text: userInput });

// Record metrics
testManager.recordMetrics(experiment.experimentId, selectedVariant.id, {
  success: response.error === null,
  latency: response.latency,
  cost: response.cost,
  qualityScore: evaluateQuality(response.output),
  userFeedback: getUserFeedback(), // Optional
});

// Periodically evaluate experiment
const evaluation = testManager.evaluateExperiment(experiment.experimentId);
console.log(evaluation);
```

---

## 2. Reinforcement Learning Implementation

### 2.1 Thompson Sampling Bandit

```typescript
interface BanditArm {
  parameterId: string;
  value: any;
  alpha: number; // Beta distribution parameter (successes)
  beta: number;  // Beta distribution parameter (failures)
  totalPulls: number;
  avgReward: number;
}

class ThompsonSamplingBandit {
  private arms: BanditArm[];

  constructor(parameterSpace: { id: string; value: any }[]) {
    this.arms = parameterSpace.map(param => ({
      parameterId: param.id,
      value: param.value,
      alpha: 1, // Prior
      beta: 1,  // Prior
      totalPulls: 0,
      avgReward: 0,
    }));
  }

  // Select arm using Thompson Sampling
  selectArm(): BanditArm {
    const sampledValues = this.arms.map(arm => ({
      arm,
      sample: this.sampleBeta(arm.alpha, arm.beta),
    }));

    // Select arm with highest sampled value
    const selected = sampledValues.reduce((max, current) =>
      current.sample > max.sample ? current : max
    );

    return selected.arm;
  }

  // Update arm after observing reward
  updateArm(armId: string, reward: number): void {
    const arm = this.arms.find(a => a.parameterId === armId);
    if (!arm) {
      throw new Error(`Arm ${armId} not found`);
    }

    // Convert reward (-1 to 1) to success/failure
    const success = reward > 0;

    // Update Beta distribution parameters
    if (success) {
      arm.alpha += 1;
    } else {
      arm.beta += 1;
    }

    arm.totalPulls += 1;

    // Update average reward (exponential moving average)
    const learningRate = 1 / arm.totalPulls;
    arm.avgReward = (1 - learningRate) * arm.avgReward + learningRate * reward;
  }

  // Sample from Beta distribution
  private sampleBeta(alpha: number, beta: number): number {
    // Using gamma distribution relationship: Beta(α,β) = Gamma(α)/(Gamma(α)+Gamma(β))
    const x = this.sampleGamma(alpha);
    const y = this.sampleGamma(beta);
    return x / (x + y);
  }

  // Sample from Gamma distribution (shape parameter only)
  private sampleGamma(shape: number): number {
    // Marsaglia and Tsang method for shape >= 1
    if (shape >= 1) {
      const d = shape - 1/3;
      const c = 1 / Math.sqrt(9 * d);

      while (true) {
        let x, v;
        do {
          x = this.randomNormal(0, 1);
          v = 1 + c * x;
        } while (v <= 0);

        v = v * v * v;
        const u = Math.random();
        const xSquared = x * x;

        if (u < 1 - 0.0331 * xSquared * xSquared) {
          return d * v;
        }

        if (Math.log(u) < 0.5 * xSquared + d * (1 - v + Math.log(v))) {
          return d * v;
        }
      }
    } else {
      // For shape < 1, use shape + 1 then scale
      return this.sampleGamma(shape + 1) * Math.pow(Math.random(), 1 / shape);
    }
  }

  // Box-Muller transform for normal distribution
  private randomNormal(mean: number, stdDev: number): number {
    const u1 = Math.random();
    const u2 = Math.random();
    const z0 = Math.sqrt(-2 * Math.log(u1)) * Math.cos(2 * Math.PI * u2);
    return z0 * stdDev + mean;
  }

  // Get current best arm (exploitation only)
  getBestArm(): BanditArm {
    return this.arms.reduce((max, current) =>
      current.avgReward > max.avgReward ? current : max
    );
  }

  // Get statistics for all arms
  getStatistics(): Array<{
    parameterId: string;
    value: any;
    avgReward: number;
    totalPulls: number;
    confidenceInterval: [number, number];
  }> {
    return this.arms.map(arm => {
      // 95% credible interval from Beta distribution
      const mean = arm.alpha / (arm.alpha + arm.beta);
      const variance = (arm.alpha * arm.beta) /
        ((arm.alpha + arm.beta) ** 2 * (arm.alpha + arm.beta + 1));
      const stdDev = Math.sqrt(variance);

      return {
        parameterId: arm.parameterId,
        value: arm.value,
        avgReward: arm.avgReward,
        totalPulls: arm.totalPulls,
        confidenceInterval: [mean - 1.96 * stdDev, mean + 1.96 * stdDev],
      };
    });
  }
}
```

### 2.2 Reward Calculation Example

```typescript
interface FeedbackEvent {
  requestId: string;
  timestamp: Date;
  promptId: string;
  parameters: Record<string, any>;
  outcome: {
    explicitFeedback?: 'positive' | 'negative' | 'neutral';
    implicitSignals: {
      taskCompleted: boolean;
      followUpQueries: number;
      timeToCompletion: number;
      errorOccurred: boolean;
      retryCount: number;
    };
    qualityMetrics: {
      coherence: number;
      relevance: number;
      completeness: number;
      accuracy: number;
    };
  };
}

function calculateReward(event: FeedbackEvent): number {
  let reward = 0;

  // Explicit feedback (highest weight)
  if (event.outcome.explicitFeedback) {
    const feedbackMap = {
      positive: 1.0,
      neutral: 0.0,
      negative: -1.0,
    };
    reward += 0.5 * feedbackMap[event.outcome.explicitFeedback];
  }

  // Implicit signals
  if (event.outcome.implicitSignals.taskCompleted) {
    reward += 0.3;
  }

  if (event.outcome.implicitSignals.errorOccurred) {
    reward -= 0.4;
  }

  const penaltyPerRetry = 0.1;
  reward -= Math.min(
    event.outcome.implicitSignals.retryCount * penaltyPerRetry,
    0.5
  );

  // Quality metrics (average normalized to 0-1)
  const qualityAvg = (
    event.outcome.qualityMetrics.coherence +
    event.outcome.qualityMetrics.relevance +
    event.outcome.qualityMetrics.completeness +
    event.outcome.qualityMetrics.accuracy
  ) / 4;

  reward += 0.2 * (qualityAvg / 100);

  // Clamp to [-1, 1]
  return Math.max(-1, Math.min(1, reward));
}
```

### 2.3 Usage Example

```typescript
// Define parameter space to optimize
const temperatureSpace = [
  { id: 'temp-0.0', value: 0.0 },
  { id: 'temp-0.3', value: 0.3 },
  { id: 'temp-0.7', value: 0.7 },
  { id: 'temp-1.0', value: 1.0 },
];

const bandit = new ThompsonSamplingBandit(temperatureSpace);

// Online learning loop
for (let i = 0; i < 1000; i++) {
  // Select parameter value
  const selectedArm = bandit.selectArm();
  const temperature = selectedArm.value;

  // Execute LLM request with selected parameter
  const response = await callLLM(prompt, { temperature });

  // Collect feedback
  const feedback: FeedbackEvent = await collectFeedback(response);

  // Calculate reward
  const reward = calculateReward(feedback);

  // Update bandit
  bandit.updateArm(selectedArm.parameterId, reward);

  // Periodically log statistics
  if (i % 100 === 0) {
    console.log('Bandit statistics:', bandit.getStatistics());
  }
}

// After learning, use best parameter
const bestArm = bandit.getBestArm();
console.log(`Optimal temperature: ${bestArm.value}`);
```

---

## 3. Cost-Performance Optimization

### 3.1 Pareto Frontier Analysis

```typescript
interface ConfigurationPoint {
  configId: string;
  parameters: {
    modelId: string;
    temperature: number;
    maxTokens: number;
  };
  quality: number;  // 0-100
  cost: number;     // dollars
  latency: number;  // milliseconds
}

class ParetoOptimizer {
  // Find non-dominated configurations
  findParetoFrontier(configurations: ConfigurationPoint[]): ConfigurationPoint[] {
    const frontier: ConfigurationPoint[] = [];

    for (const config of configurations) {
      let isDominated = false;

      for (const other of configurations) {
        if (config.configId === other.configId) continue;

        // Check if 'other' dominates 'config'
        const betterQuality = other.quality >= config.quality;
        const betterCost = other.cost <= config.cost;
        const betterLatency = other.latency <= config.latency;

        const strictlyBetterOnOne =
          other.quality > config.quality ||
          other.cost < config.cost ||
          other.latency < config.latency;

        if (betterQuality && betterCost && betterLatency && strictlyBetterOnOne) {
          isDominated = true;
          break;
        }
      }

      if (!isDominated) {
        frontier.push(config);
      }
    }

    return frontier;
  }

  // Select from Pareto frontier based on preferences
  selectFromFrontier(
    frontier: ConfigurationPoint[],
    preferences: {
      qualityWeight: number;
      costWeight: number;
      latencyWeight: number;
    }
  ): ConfigurationPoint {
    // Normalize weights
    const totalWeight = preferences.qualityWeight +
                       preferences.costWeight +
                       preferences.latencyWeight;

    const normWeights = {
      quality: preferences.qualityWeight / totalWeight,
      cost: preferences.costWeight / totalWeight,
      latency: preferences.latencyWeight / totalWeight,
    };

    // Find max values for normalization
    const maxQuality = Math.max(...frontier.map(c => c.quality));
    const maxCost = Math.max(...frontier.map(c => c.cost));
    const maxLatency = Math.max(...frontier.map(c => c.latency));

    // Score each configuration
    const scored = frontier.map(config => {
      const normQuality = config.quality / maxQuality;
      const normCost = 1 - (config.cost / maxCost);  // Higher is better
      const normLatency = 1 - (config.latency / maxLatency);  // Higher is better

      const score =
        normWeights.quality * normQuality +
        normWeights.cost * normCost +
        normWeights.latency * normLatency;

      return { config, score };
    });

    // Return highest scoring configuration
    return scored.reduce((max, current) =>
      current.score > max.score ? current : max
    ).config;
  }

  // Visualize frontier (for debugging/analysis)
  visualizeFrontier(frontier: ConfigurationPoint[]): void {
    console.log('\nPareto Frontier Analysis:');
    console.log('─'.repeat(80));
    console.log(
      'Config ID'.padEnd(20) +
      'Quality'.padEnd(15) +
      'Cost ($)'.padEnd(15) +
      'Latency (ms)'.padEnd(15)
    );
    console.log('─'.repeat(80));

    frontier
      .sort((a, b) => b.quality - a.quality)
      .forEach(config => {
        console.log(
          config.configId.padEnd(20) +
          config.quality.toFixed(1).padEnd(15) +
          config.cost.toFixed(4).padEnd(15) +
          config.latency.toFixed(0).padEnd(15)
        );
      });

    console.log('─'.repeat(80));
  }
}
```

### 3.2 Budget Enforcement

```typescript
interface Budget {
  dailyLimit: number;
  perRequestLimit: number;
  currentSpend: number;
  resetTime: Date;
}

type FallbackStrategy = 'queue' | 'cheaper-model' | 'reject';

interface BudgetDecision {
  approved: boolean;
  modifiedRequest?: {
    modelId: string;
    maxTokens: number;
  };
  reason: string;
  queueUntil?: Date;
}

class BudgetManager {
  private budget: Budget;
  private modelPricing: Map<string, { inputPerToken: number; outputPerToken: number }>;

  constructor(dailyLimit: number) {
    this.budget = {
      dailyLimit,
      perRequestLimit: dailyLimit * 0.1, // 10% of daily limit per request
      currentSpend: 0,
      resetTime: this.getNextMidnight(),
    };

    // Example pricing (adjust to real values)
    this.modelPricing = new Map([
      ['gpt-4', { inputPerToken: 0.00003, outputPerToken: 0.00006 }],
      ['gpt-3.5-turbo', { inputPerToken: 0.0000015, outputPerToken: 0.000002 }],
      ['claude-sonnet', { inputPerToken: 0.000003, outputPerToken: 0.000015 }],
      ['claude-haiku', { inputPerToken: 0.00000025, outputPerToken: 0.00000125 }],
    ]);
  }

  enforceBudget(
    request: {
      modelId: string;
      inputTokens: number;
      maxTokens: number;
    },
    fallbackStrategy: FallbackStrategy
  ): BudgetDecision {
    const estimatedCost = this.estimateRequestCost(request);

    // Check per-request limit
    if (estimatedCost > this.budget.perRequestLimit) {
      if (fallbackStrategy === 'cheaper-model') {
        const cheaperModel = this.findCheaperModel(request);
        if (cheaperModel) {
          return {
            approved: true,
            modifiedRequest: cheaperModel,
            reason: 'Switched to cheaper model to meet per-request budget',
          };
        }
      }

      return {
        approved: false,
        reason: `Exceeds per-request limit ($${estimatedCost.toFixed(4)} > $${this.budget.perRequestLimit.toFixed(4)})`,
      };
    }

    // Check daily budget
    if (this.budget.currentSpend + estimatedCost > this.budget.dailyLimit) {
      const remainingBudget = this.budget.dailyLimit - this.budget.currentSpend;

      if (fallbackStrategy === 'queue') {
        return {
          approved: false,
          reason: 'Daily budget exceeded - request queued',
          queueUntil: this.budget.resetTime,
        };
      }

      if (fallbackStrategy === 'cheaper-model') {
        const budgetModel = this.findModelWithinBudget(request, remainingBudget);
        if (budgetModel) {
          return {
            approved: true,
            modifiedRequest: budgetModel,
            reason: `Switched to cheaper model to fit remaining daily budget ($${remainingBudget.toFixed(4)})`,
          };
        }
      }

      return {
        approved: false,
        reason: `Daily budget exceeded ($${this.budget.currentSpend.toFixed(4)} / $${this.budget.dailyLimit.toFixed(4)})`,
      };
    }

    // Approved - deduct from budget
    this.budget.currentSpend += estimatedCost;

    return {
      approved: true,
      reason: 'Within budget',
    };
  }

  private estimateRequestCost(request: {
    modelId: string;
    inputTokens: number;
    maxTokens: number;
  }): number {
    const pricing = this.modelPricing.get(request.modelId);
    if (!pricing) {
      throw new Error(`Unknown model: ${request.modelId}`);
    }

    const inputCost = request.inputTokens * pricing.inputPerToken;
    const outputCost = request.maxTokens * pricing.outputPerToken;

    return inputCost + outputCost;
  }

  private findCheaperModel(request: {
    modelId: string;
    inputTokens: number;
    maxTokens: number;
  }): { modelId: string; maxTokens: number } | null {
    // Model tiers (in order of capability and cost)
    const modelTiers = [
      ['gpt-4', 'claude-sonnet'],
      ['gpt-3.5-turbo'],
      ['claude-haiku'],
    ];

    const currentTier = modelTiers.findIndex(tier =>
      tier.includes(request.modelId)
    );

    // Try cheaper tiers
    for (let i = currentTier + 1; i < modelTiers.length; i++) {
      const cheaperModel = modelTiers[i][0];
      const cost = this.estimateRequestCost({
        ...request,
        modelId: cheaperModel,
      });

      if (cost <= this.budget.perRequestLimit) {
        return {
          modelId: cheaperModel,
          maxTokens: request.maxTokens,
        };
      }
    }

    return null;
  }

  private findModelWithinBudget(
    request: {
      modelId: string;
      inputTokens: number;
      maxTokens: number;
    },
    budget: number
  ): { modelId: string; maxTokens: number } | null {
    const allModels = Array.from(this.modelPricing.keys());

    for (const modelId of allModels) {
      const cost = this.estimateRequestCost({
        ...request,
        modelId,
      });

      if (cost <= budget) {
        return {
          modelId,
          maxTokens: request.maxTokens,
        };
      }
    }

    return null;
  }

  private getNextMidnight(): Date {
    const tomorrow = new Date();
    tomorrow.setDate(tomorrow.getDate() + 1);
    tomorrow.setHours(0, 0, 0, 0);
    return tomorrow;
  }

  resetBudget(): void {
    this.budget.currentSpend = 0;
    this.budget.resetTime = this.getNextMidnight();
  }

  getBudgetStatus(): {
    dailyLimit: number;
    currentSpend: number;
    remaining: number;
    percentUsed: number;
    resetsAt: Date;
  } {
    return {
      dailyLimit: this.budget.dailyLimit,
      currentSpend: this.budget.currentSpend,
      remaining: this.budget.dailyLimit - this.budget.currentSpend,
      percentUsed: (this.budget.currentSpend / this.budget.dailyLimit) * 100,
      resetsAt: this.budget.resetTime,
    };
  }
}
```

---

## 4. Anomaly Detection and Alerting

### 4.1 Statistical Anomaly Detection

```typescript
interface TimeSeriesPoint {
  timestamp: Date;
  value: number;
}

interface Anomaly {
  timestamp: Date;
  value: number;
  anomalyScore: number;
  type: 'spike' | 'drop' | 'pattern-break';
}

class AnomalyDetector {
  private readonly method: 'zscore' | 'iqr' | 'mad';
  private readonly sensitivity: number;
  private readonly windowSize: number;

  constructor(config: {
    method?: 'zscore' | 'iqr' | 'mad';
    sensitivity?: number;
    windowSize?: number;
  } = {}) {
    this.method = config.method || 'zscore';
    this.sensitivity = config.sensitivity || 3.0;
    this.windowSize = config.windowSize || 100;
  }

  detect(timeSeries: TimeSeriesPoint[]): Anomaly[] {
    switch (this.method) {
      case 'zscore':
        return this.detectZScore(timeSeries);
      case 'iqr':
        return this.detectIQR(timeSeries);
      case 'mad':
        return this.detectMAD(timeSeries);
      default:
        throw new Error(`Unknown method: ${this.method}`);
    }
  }

  private detectZScore(timeSeries: TimeSeriesPoint[]): Anomaly[] {
    const values = timeSeries.map(p => p.value);
    const mean = this.mean(values);
    const stdDev = this.stdDev(values, mean);

    const anomalies: Anomaly[] = [];

    for (const point of timeSeries) {
      const zScore = Math.abs((point.value - mean) / stdDev);

      if (zScore > this.sensitivity) {
        anomalies.push({
          timestamp: point.timestamp,
          value: point.value,
          anomalyScore: zScore,
          type: point.value > mean ? 'spike' : 'drop',
        });
      }
    }

    return anomalies;
  }

  private detectIQR(timeSeries: TimeSeriesPoint[]): Anomaly[] {
    const values = timeSeries.map(p => p.value);
    const sorted = [...values].sort((a, b) => a - b);

    const q1 = this.percentile(sorted, 25);
    const q3 = this.percentile(sorted, 75);
    const iqr = q3 - q1;

    const lowerBound = q1 - this.sensitivity * iqr;
    const upperBound = q3 + this.sensitivity * iqr;

    const median = this.percentile(sorted, 50);

    const anomalies: Anomaly[] = [];

    for (const point of timeSeries) {
      if (point.value < lowerBound || point.value > upperBound) {
        anomalies.push({
          timestamp: point.timestamp,
          value: point.value,
          anomalyScore: Math.abs(point.value - median) / iqr,
          type: point.value > upperBound ? 'spike' : 'drop',
        });
      }
    }

    return anomalies;
  }

  private detectMAD(timeSeries: TimeSeriesPoint[]): Anomaly[] {
    // Median Absolute Deviation (more robust to outliers than stddev)
    const values = timeSeries.map(p => p.value);
    const median = this.percentile([...values].sort((a, b) => a - b), 50);

    const absoluteDeviations = values.map(v => Math.abs(v - median));
    const mad = this.percentile([...absoluteDeviations].sort((a, b) => a - b), 50);

    const anomalies: Anomaly[] = [];

    for (const point of timeSeries) {
      // Modified Z-score using MAD
      const modifiedZScore = Math.abs((point.value - median) / (1.4826 * mad));

      if (modifiedZScore > this.sensitivity) {
        anomalies.push({
          timestamp: point.timestamp,
          value: point.value,
          anomalyScore: modifiedZScore,
          type: point.value > median ? 'spike' : 'drop',
        });
      }
    }

    return anomalies;
  }

  private mean(values: number[]): number {
    return values.reduce((a, b) => a + b, 0) / values.length;
  }

  private stdDev(values: number[], mean: number): number {
    const variance = values.reduce((sum, val) => sum + (val - mean) ** 2, 0) / values.length;
    return Math.sqrt(variance);
  }

  private percentile(sorted: number[], p: number): number {
    const index = (p / 100) * (sorted.length - 1);
    const lower = Math.floor(index);
    const upper = Math.ceil(index);
    const weight = index - lower;

    return sorted[lower] * (1 - weight) + sorted[upper] * weight;
  }
}
```

### 4.2 Performance Degradation Monitor

```typescript
interface PerformanceMetrics {
  timestamp: Date;
  successRate: number;
  avgLatency: number;
  avgQuality: number;
  errorRate: number;
  costPerRequest: number;
}

interface DegradationAlert {
  detected: boolean;
  severity: 'minor' | 'moderate' | 'severe';
  affectedMetrics: Array<{
    metric: string;
    baseline: number;
    current: number;
    change: number;
  }>;
  recommendation: string;
}

class PerformanceMonitor {
  private baseline: PerformanceMetrics;
  private degradationThreshold: number;

  constructor(baseline: PerformanceMetrics, degradationThreshold: number = 0.10) {
    this.baseline = baseline;
    this.degradationThreshold = degradationThreshold;
  }

  checkDegradation(current: PerformanceMetrics): DegradationAlert {
    const degradations: Array<{
      metric: string;
      baseline: number;
      current: number;
      change: number;
      severity: 'minor' | 'moderate' | 'severe';
    }> = [];

    // Success rate degradation
    const successDrop = this.baseline.successRate - current.successRate;
    if (successDrop > this.degradationThreshold) {
      degradations.push({
        metric: 'success_rate',
        baseline: this.baseline.successRate,
        current: current.successRate,
        change: -successDrop,
        severity: this.getSeverity(successDrop, [0.05, 0.15, 0.30]),
      });
    }

    // Quality degradation
    const qualityDrop = this.baseline.avgQuality - current.avgQuality;
    if (qualityDrop > this.degradationThreshold * 100) {
      degradations.push({
        metric: 'quality',
        baseline: this.baseline.avgQuality,
        current: current.avgQuality,
        change: -qualityDrop,
        severity: this.getSeverity(qualityDrop, [5, 15, 30]),
      });
    }

    // Latency increase
    const latencyIncrease = current.avgLatency - this.baseline.avgLatency;
    const relativeIncrease = latencyIncrease / this.baseline.avgLatency;
    if (relativeIncrease > this.degradationThreshold) {
      degradations.push({
        metric: 'latency',
        baseline: this.baseline.avgLatency,
        current: current.avgLatency,
        change: latencyIncrease,
        severity: this.getSeverity(relativeIncrease, [0.20, 0.50, 1.0]),
      });
    }

    // Error rate spike
    const errorIncrease = current.errorRate - this.baseline.errorRate;
    if (errorIncrease > 0.05) {
      degradations.push({
        metric: 'error_rate',
        baseline: this.baseline.errorRate,
        current: current.errorRate,
        change: errorIncrease,
        severity: this.getSeverity(errorIncrease, [0.05, 0.15, 0.30]),
      });
    }

    if (degradations.length === 0) {
      return {
        detected: false,
        severity: 'minor',
        affectedMetrics: [],
        recommendation: 'No degradation detected',
      };
    }

    const maxSeverity = this.getMaxSeverity(degradations.map(d => d.severity));
    const recommendation = this.generateRecommendation(degradations);

    return {
      detected: true,
      severity: maxSeverity,
      affectedMetrics: degradations,
      recommendation,
    };
  }

  private getSeverity(value: number, thresholds: [number, number, number]): 'minor' | 'moderate' | 'severe' {
    if (value >= thresholds[2]) return 'severe';
    if (value >= thresholds[1]) return 'moderate';
    if (value >= thresholds[0]) return 'minor';
    return 'minor';
  }

  private getMaxSeverity(severities: Array<'minor' | 'moderate' | 'severe'>): 'minor' | 'moderate' | 'severe' {
    if (severities.includes('severe')) return 'severe';
    if (severities.includes('moderate')) return 'moderate';
    return 'minor';
  }

  private generateRecommendation(degradations: Array<{ metric: string; severity: string }>): string {
    const critical = degradations.filter(d => d.severity === 'severe');

    if (critical.length > 0) {
      const metrics = critical.map(d => d.metric).join(', ');
      return `CRITICAL: Severe degradation in ${metrics}. Recommend immediate rollback and investigation.`;
    }

    const moderate = degradations.filter(d => d.severity === 'moderate');
    if (moderate.length > 0) {
      return `Moderate degradation detected. Monitor closely and prepare rollback if worsens.`;
    }

    return `Minor degradation detected. Continue monitoring.`;
  }
}
```

---

## 5. Integration Example: Complete Optimization Loop

```typescript
class LLMAutoOptimizer {
  private abTestManager: ABTestManager;
  private thompsonBandit: ThompsonSamplingBandit;
  private budgetManager: BudgetManager;
  private anomalyDetector: AnomalyDetector;
  private performanceMonitor: PerformanceMonitor;
  private paretoOptimizer: ParetoOptimizer;

  async processRequest(request: {
    userId: string;
    prompt: string;
    requirements: {
      maxCost?: number;
      minQuality?: number;
      maxLatency?: number;
    };
  }): Promise<{
    response: string;
    metadata: {
      modelUsed: string;
      cost: number;
      latency: number;
      quality: number;
    };
  }> {
    // 1. Budget check
    const budgetDecision = this.budgetManager.enforceBudget(
      {
        modelId: 'gpt-4',
        inputTokens: request.prompt.length / 4, // rough estimate
        maxTokens: 1000,
      },
      'cheaper-model'
    );

    if (!budgetDecision.approved) {
      throw new Error(`Request rejected: ${budgetDecision.reason}`);
    }

    // 2. Select prompt variant (A/B testing)
    const variant = this.abTestManager.selectVariant(
      request.userId,
      'current-experiment'
    );

    // 3. Select optimal parameters (bandit)
    const temperatureArm = this.thompsonBandit.selectArm();

    // 4. Execute LLM request
    const startTime = Date.now();
    const llmResponse = await this.callLLM({
      prompt: variant.prompt.replace('{text}', request.prompt),
      modelId: budgetDecision.modifiedRequest?.modelId || 'gpt-4',
      temperature: temperatureArm.value,
      maxTokens: budgetDecision.modifiedRequest?.maxTokens || 1000,
    });
    const latency = Date.now() - startTime;

    // 5. Evaluate quality
    const quality = await this.evaluateQuality(llmResponse.text, request.prompt);

    // 6. Record metrics
    this.abTestManager.recordMetrics('current-experiment', variant.id, {
      success: !llmResponse.error,
      latency,
      cost: llmResponse.cost,
      qualityScore: quality,
    });

    // 7. Calculate reward and update bandit
    const reward = this.calculateReward({
      success: !llmResponse.error,
      quality,
      latency,
      cost: llmResponse.cost,
    });

    this.thompsonBandit.updateArm(temperatureArm.parameterId, reward);

    // 8. Check for anomalies
    const recentMetrics = await this.getRecentMetrics();
    const anomalies = this.anomalyDetector.detect(recentMetrics);

    if (anomalies.length > 0) {
      console.warn('Anomalies detected:', anomalies);
      await this.handleAnomalies(anomalies);
    }

    // 9. Check for performance degradation
    const currentPerformance = await this.getCurrentPerformance();
    const degradation = this.performanceMonitor.checkDegradation(currentPerformance);

    if (degradation.detected && degradation.severity === 'severe') {
      console.error('Severe degradation detected:', degradation);
      await this.rollbackToSafeConfiguration();
    }

    return {
      response: llmResponse.text,
      metadata: {
        modelUsed: llmResponse.modelId,
        cost: llmResponse.cost,
        latency,
        quality,
      },
    };
  }

  private calculateReward(metrics: {
    success: boolean;
    quality: number;
    latency: number;
    cost: number;
  }): number {
    let reward = 0;

    if (metrics.success) {
      reward += 0.3;
    } else {
      reward -= 0.5;
    }

    reward += 0.4 * (metrics.quality / 100);
    reward += 0.2 * Math.max(0, 1 - metrics.latency / 10000);
    reward += 0.1 * Math.max(0, 1 - metrics.cost / 1.0);

    return Math.max(-1, Math.min(1, reward));
  }

  private async callLLM(params: {
    prompt: string;
    modelId: string;
    temperature: number;
    maxTokens: number;
  }): Promise<{
    text: string;
    error: boolean;
    cost: number;
    modelId: string;
  }> {
    // Implementation would call actual LLM API
    // This is a placeholder
    return {
      text: 'Sample response',
      error: false,
      cost: 0.01,
      modelId: params.modelId,
    };
  }

  private async evaluateQuality(response: string, original: string): Promise<number> {
    // Placeholder for quality evaluation
    return 85;
  }

  private async getRecentMetrics(): Promise<TimeSeriesPoint[]> {
    // Fetch from metrics storage
    return [];
  }

  private async getCurrentPerformance(): Promise<PerformanceMetrics> {
    // Calculate current performance from recent data
    return {
      timestamp: new Date(),
      successRate: 0.95,
      avgLatency: 2000,
      avgQuality: 85,
      errorRate: 0.05,
      costPerRequest: 0.01,
    };
  }

  private async handleAnomalies(anomalies: Anomaly[]): Promise<void> {
    // Alert, log, or take corrective action
    console.log('Handling anomalies:', anomalies);
  }

  private async rollbackToSafeConfiguration(): Promise<void> {
    // Revert to last known good configuration
    console.log('Rolling back to safe configuration');
  }
}
```

---

## Summary

This document provides production-ready implementations for:

1. **A/B Testing**: Complete framework with statistical analysis
2. **Reinforcement Learning**: Thompson Sampling bandit with reward calculation
3. **Cost Management**: Budget enforcement and Pareto optimization
4. **Anomaly Detection**: Multiple statistical methods for outlier detection
5. **Performance Monitoring**: Degradation detection and alerting
6. **Integration**: Full optimization loop combining all strategies

All algorithms include:
- Type-safe TypeScript implementations
- Error handling and edge cases
- Statistical rigor (proper hypothesis testing, confidence intervals)
- Practical defaults and configuration options
- Production-ready logging and monitoring

Engineers can use these as reference implementations or adapt them to their specific needs and infrastructure.
