/**
 * Model Selection Agent
 *
 * A RECOMMENDATION agent that recommends optimal model selection based on
 * historical performance signals, task context, and constraints.
 *
 * Classification: RECOMMENDATION
 * Decision Type: model_selection_recommendation
 *
 * This agent:
 * - Analyzes cost, latency, and quality metrics by model
 * - Proposes alternative model candidates with scoring
 * - Emits advisory-only selection guidance
 *
 * This agent MUST NOT:
 * - Route traffic directly
 * - Enforce model selection
 * - Modify execution paths
 * - Execute optimizations directly
 * - Modify runtime behavior autonomously
 * - Intercept execution traffic
 * - Trigger retries
 * - Emit alerts (that is Sentinel)
 * - Enforce policies (that is Shield)
 * - Perform orchestration (that is LLM-Orchestrator)
 *
 * @module agents/model-selection-agent
 * @version 1.0.0
 */

import {
  // Input/Output schemas
  ModelSelectionAgentInput,
  ModelSelectionAgentOutput,
  // DecisionEvent
  DecisionEvent,
  DecisionOutputs,
  AppliedConstraint,
  // Validation
  validateModelSelectionInput,
  validateModelSelectionOutput,
  hashInputs,
  // Types
  ModelRecommendation,
  ModelSelectionAnalysis,
  ModelSelectionConstraintEvaluation,
  ModelSelectionMetadata,
  ScoreBreakdown,
  ExpectedPerformance,
  ModelSelectionEvidence,
  ModelConfig,
  TaskContext,
  ModelPerformanceHistory,
  ModelHistoricalMetrics,
  ModelSelectionConstraint,
} from '../../contracts';

import { RuVectorClient, createRuVectorClient } from '../../services/ruvector-client';

// ============================================================================
// Constants
// ============================================================================

export const AGENT_ID = 'model-selection-agent';
export const AGENT_VERSION = '1.0.0';
export const DECISION_TYPE = 'model_selection_recommendation';

// Model cost data (from provider pricing as of 2024)
const MODEL_COSTS: Record<string, { input: number; output: number }> = {
  'claude-3-opus': { input: 0.015, output: 0.075 },
  'claude-3-5-sonnet': { input: 0.003, output: 0.015 },
  'claude-3-sonnet': { input: 0.003, output: 0.015 },
  'claude-3-haiku': { input: 0.00025, output: 0.00125 },
  'gpt-4o': { input: 0.005, output: 0.015 },
  'gpt-4-turbo': { input: 0.01, output: 0.03 },
  'gpt-4': { input: 0.03, output: 0.06 },
  'gpt-3.5-turbo': { input: 0.0005, output: 0.0015 },
  'gemini-1.5-pro': { input: 0.00125, output: 0.005 },
  'gemini-1.5-flash': { input: 0.000075, output: 0.0003 },
};

// Quality benchmarks (0-1 scale based on standard benchmarks)
const QUALITY_BENCHMARKS: Record<string, number> = {
  'claude-3-opus': 0.95,
  'claude-3-5-sonnet': 0.93,
  'claude-3-sonnet': 0.90,
  'claude-3-haiku': 0.82,
  'gpt-4o': 0.92,
  'gpt-4-turbo': 0.91,
  'gpt-4': 0.93,
  'gpt-3.5-turbo': 0.75,
  'gemini-1.5-pro': 0.91,
  'gemini-1.5-flash': 0.80,
};

// Average latency benchmarks (P95 in ms)
const LATENCY_BENCHMARKS: Record<string, number> = {
  'claude-3-opus': 3000,
  'claude-3-5-sonnet': 1500,
  'claude-3-sonnet': 1200,
  'claude-3-haiku': 400,
  'gpt-4o': 1000,
  'gpt-4-turbo': 2000,
  'gpt-4': 3500,
  'gpt-3.5-turbo': 500,
  'gemini-1.5-pro': 1800,
  'gemini-1.5-flash': 350,
};

// Task type to model capability mapping (suitability scores)
const TASK_MODEL_SUITABILITY: Record<string, Record<string, number>> = {
  'code_generation': {
    'claude-3-opus': 0.98,
    'claude-3-5-sonnet': 0.96,
    'claude-3-sonnet': 0.90,
    'claude-3-haiku': 0.75,
    'gpt-4o': 0.94,
    'gpt-4-turbo': 0.92,
    'gpt-4': 0.93,
    'gpt-3.5-turbo': 0.70,
    'gemini-1.5-pro': 0.88,
    'gemini-1.5-flash': 0.72,
  },
  'code_review': {
    'claude-3-opus': 0.97,
    'claude-3-5-sonnet': 0.95,
    'claude-3-sonnet': 0.88,
    'claude-3-haiku': 0.70,
    'gpt-4o': 0.92,
    'gpt-4-turbo': 0.90,
    'gpt-4': 0.91,
    'gpt-3.5-turbo': 0.65,
    'gemini-1.5-pro': 0.86,
    'gemini-1.5-flash': 0.68,
  },
  'reasoning': {
    'claude-3-opus': 0.98,
    'claude-3-5-sonnet': 0.94,
    'claude-3-sonnet': 0.85,
    'claude-3-haiku': 0.65,
    'gpt-4o': 0.92,
    'gpt-4-turbo': 0.88,
    'gpt-4': 0.93,
    'gpt-3.5-turbo': 0.60,
    'gemini-1.5-pro': 0.90,
    'gemini-1.5-flash': 0.62,
  },
  'creative_writing': {
    'claude-3-opus': 0.96,
    'claude-3-5-sonnet': 0.94,
    'claude-3-sonnet': 0.90,
    'claude-3-haiku': 0.72,
    'gpt-4o': 0.90,
    'gpt-4-turbo': 0.88,
    'gpt-4': 0.91,
    'gpt-3.5-turbo': 0.70,
    'gemini-1.5-pro': 0.85,
    'gemini-1.5-flash': 0.68,
  },
  'text_summarization': {
    'claude-3-opus': 0.92,
    'claude-3-5-sonnet': 0.90,
    'claude-3-sonnet': 0.88,
    'claude-3-haiku': 0.80,
    'gpt-4o': 0.88,
    'gpt-4-turbo': 0.85,
    'gpt-4': 0.87,
    'gpt-3.5-turbo': 0.75,
    'gemini-1.5-pro': 0.86,
    'gemini-1.5-flash': 0.78,
  },
  'question_answering': {
    'claude-3-opus': 0.95,
    'claude-3-5-sonnet': 0.93,
    'claude-3-sonnet': 0.88,
    'claude-3-haiku': 0.78,
    'gpt-4o': 0.91,
    'gpt-4-turbo': 0.88,
    'gpt-4': 0.90,
    'gpt-3.5-turbo': 0.72,
    'gemini-1.5-pro': 0.87,
    'gemini-1.5-flash': 0.75,
  },
  'data_extraction': {
    'claude-3-opus': 0.92,
    'claude-3-5-sonnet': 0.90,
    'claude-3-sonnet': 0.86,
    'claude-3-haiku': 0.82,
    'gpt-4o': 0.88,
    'gpt-4-turbo': 0.86,
    'gpt-4': 0.87,
    'gpt-3.5-turbo': 0.78,
    'gemini-1.5-pro': 0.85,
    'gemini-1.5-flash': 0.80,
  },
  'classification': {
    'claude-3-opus': 0.88,
    'claude-3-5-sonnet': 0.86,
    'claude-3-sonnet': 0.84,
    'claude-3-haiku': 0.82,
    'gpt-4o': 0.86,
    'gpt-4-turbo': 0.84,
    'gpt-4': 0.85,
    'gpt-3.5-turbo': 0.80,
    'gemini-1.5-pro': 0.83,
    'gemini-1.5-flash': 0.80,
  },
  'general': {
    'claude-3-opus': 0.94,
    'claude-3-5-sonnet': 0.92,
    'claude-3-sonnet': 0.87,
    'claude-3-haiku': 0.75,
    'gpt-4o': 0.90,
    'gpt-4-turbo': 0.87,
    'gpt-4': 0.89,
    'gpt-3.5-turbo': 0.72,
    'gemini-1.5-pro': 0.86,
    'gemini-1.5-flash': 0.73,
  },
};

// Model provider mapping
const MODEL_PROVIDERS: Record<string, string> = {
  'claude-3-opus': 'anthropic',
  'claude-3-5-sonnet': 'anthropic',
  'claude-3-sonnet': 'anthropic',
  'claude-3-haiku': 'anthropic',
  'gpt-4o': 'openai',
  'gpt-4-turbo': 'openai',
  'gpt-4': 'openai',
  'gpt-3.5-turbo': 'openai',
  'gemini-1.5-pro': 'google',
  'gemini-1.5-flash': 'google',
};

// ============================================================================
// Agent Configuration
// ============================================================================

export interface ModelSelectionAgentConfig {
  /** RuVector client for persistence */
  ruVectorClient?: RuVectorClient;
  /** Enable telemetry emission */
  enableTelemetry?: boolean;
  /** Telemetry endpoint */
  telemetryEndpoint?: string;
  /** Maximum alternative recommendations */
  maxAlternatives?: number;
  /** Minimum confidence threshold */
  minConfidenceThreshold?: number;
}

const DEFAULT_CONFIG: Required<ModelSelectionAgentConfig> = {
  ruVectorClient: undefined as unknown as RuVectorClient,
  enableTelemetry: true,
  telemetryEndpoint: process.env.TELEMETRY_ENDPOINT || '',
  maxAlternatives: 5,
  minConfidenceThreshold: 0.5,
};

// ============================================================================
// Model Selection Agent Class
// ============================================================================

/**
 * Model Selection Agent - Recommends optimal model selection.
 *
 * This is a stateless, deterministic agent designed to run as a
 * Google Cloud Edge Function.
 */
export class ModelSelectionAgent {
  private config: Required<ModelSelectionAgentConfig>;
  private ruVectorClient: RuVectorClient;

  constructor(config: ModelSelectionAgentConfig = {}) {
    this.config = {
      ...DEFAULT_CONFIG,
      ...config,
    };

    // Initialize RuVector client
    this.ruVectorClient = this.config.ruVectorClient || createRuVectorClient();
  }

  // --------------------------------------------------------------------------
  // Public API - CLI Invocable Endpoints
  // --------------------------------------------------------------------------

  /**
   * ANALYZE - Perform analysis without generating recommendations.
   * Use this for understanding current model landscape and capabilities.
   */
  async analyze(input: ModelSelectionAgentInput): Promise<ModelSelectionAgentOutput> {
    return this.execute(input, 'analyze');
  }

  /**
   * RECOMMEND - Generate model selection recommendations.
   * Primary entry point for the agent.
   */
  async recommend(input: ModelSelectionAgentInput): Promise<ModelSelectionAgentOutput> {
    return this.execute(input, 'recommend');
  }

  /**
   * SIMULATE - Simulate model selection without persisting.
   * Use for what-if analysis with different task contexts.
   */
  async simulate(input: ModelSelectionAgentInput): Promise<ModelSelectionAgentOutput> {
    return this.execute(input, 'simulate');
  }

  // --------------------------------------------------------------------------
  // Core Execution Logic
  // --------------------------------------------------------------------------

  private async execute(
    input: ModelSelectionAgentInput,
    mode: 'analyze' | 'recommend' | 'simulate'
  ): Promise<ModelSelectionAgentOutput> {
    const startTime = Date.now();

    // Step 1: Validate input against schema
    validateModelSelectionInput(input);

    // Step 2: Compute inputs hash for reproducibility
    const inputsHash = hashInputs(input);

    // Step 3: Get all available models
    const availableModels = this.getAvailableModels(input.performance_history);

    // Step 4: Filter models by constraints
    const constraintResults = this.evaluateConstraints(
      availableModels,
      input.constraints,
      input.task_context
    );

    const eligibleModels = constraintResults.eligibleModels;

    // Step 5: Score eligible models
    const scoredModels = this.scoreModels(
      eligibleModels,
      input.task_context,
      input.performance_history
    );

    // Step 6: Build recommendations
    const recommendations = mode === 'analyze'
      ? []
      : this.buildRecommendations(scoredModels, input);

    // Step 7: Build analysis summary
    const analysisSummary = this.buildAnalysisSummary(
      availableModels,
      eligibleModels,
      scoredModels,
      input.task_context
    );

    // Step 8: Build output
    const output = this.buildOutput(
      input,
      recommendations,
      analysisSummary,
      constraintResults.evaluations,
      availableModels,
      startTime
    );

    // Step 9: Validate output
    validateModelSelectionOutput(output);

    // Step 10: Build and persist DecisionEvent (skip for simulate mode)
    if (mode !== 'simulate') {
      const decisionEvent = this.buildDecisionEvent(
        input,
        output,
        inputsHash,
        constraintResults.evaluations
      );

      // Persist to ruvector-service (MANDATORY)
      await this.ruVectorClient.persistDecisionEvent(decisionEvent);

      // Step 11: Emit telemetry (if enabled)
      if (this.config.enableTelemetry) {
        await this.emitTelemetry(decisionEvent, output, Date.now() - startTime);
      }
    }

    return output;
  }

  // --------------------------------------------------------------------------
  // Model Discovery
  // --------------------------------------------------------------------------

  private getAvailableModels(history: ModelPerformanceHistory): string[] {
    // Start with known models from benchmarks
    const models = new Set(Object.keys(MODEL_COSTS));

    // Add models from historical data
    for (const modelId of Object.keys(history.by_model)) {
      models.add(modelId);
    }

    return Array.from(models);
  }

  // --------------------------------------------------------------------------
  // Constraint Evaluation
  // --------------------------------------------------------------------------

  private evaluateConstraints(
    models: string[],
    constraints: ModelSelectionConstraint[],
    context: TaskContext
  ): {
    eligibleModels: string[];
    evaluations: ModelSelectionConstraintEvaluation[];
  } {
    let eligibleModels = [...models];
    const evaluations: ModelSelectionConstraintEvaluation[] = [];

    for (const constraint of constraints) {
      const beforeCount = eligibleModels.length;
      const passingModels: string[] = [];

      for (const model of eligibleModels) {
        if (this.modelPassesConstraint(model, constraint, context)) {
          passingModels.push(model);
        }
      }

      const blockedCount = beforeCount - passingModels.length;

      evaluations.push({
        constraint,
        satisfied: constraint.hard ? passingModels.length > 0 : true,
        models_passing: passingModels.length,
        models_blocked: blockedCount,
        impact_description: blockedCount > 0
          ? `Blocked ${blockedCount} models: ${constraint.type} = ${JSON.stringify(constraint.value)}`
          : 'No models blocked',
      });

      // Apply hard constraints immediately
      if (constraint.hard) {
        eligibleModels = passingModels;
      }
    }

    return { eligibleModels, evaluations };
  }

  private modelPassesConstraint(
    model: string,
    constraint: ModelSelectionConstraint,
    context: TaskContext
  ): boolean {
    switch (constraint.type) {
      case 'allowed_models': {
        const allowed = Array.isArray(constraint.value)
          ? constraint.value
          : String(constraint.value).split(',');
        return allowed.includes(model);
      }

      case 'blocked_models': {
        const blocked = Array.isArray(constraint.value)
          ? constraint.value
          : String(constraint.value).split(',');
        return !blocked.includes(model);
      }

      case 'allowed_providers': {
        const providers = Array.isArray(constraint.value)
          ? constraint.value
          : String(constraint.value).split(',');
        return providers.includes(MODEL_PROVIDERS[model] || '');
      }

      case 'blocked_providers': {
        const providers = Array.isArray(constraint.value)
          ? constraint.value
          : String(constraint.value).split(',');
        return !providers.includes(MODEL_PROVIDERS[model] || '');
      }

      case 'max_cost_per_request': {
        const maxCost = Number(constraint.value);
        const cost = this.estimateCostPerRequest(model, context);
        return cost <= maxCost;
      }

      case 'min_quality_threshold': {
        const minQuality = Number(constraint.value);
        const quality = QUALITY_BENCHMARKS[model] || 0.7;
        return quality >= minQuality;
      }

      case 'max_latency_ms': {
        const maxLatency = Number(constraint.value);
        const latency = LATENCY_BENCHMARKS[model] || 2000;
        return latency <= maxLatency;
      }

      default:
        return true;
    }
  }

  private estimateCostPerRequest(model: string, context: TaskContext): number {
    const costs = MODEL_COSTS[model];
    if (!costs) return 0.1; // Default fallback

    const avgInputTokens = context.expected_input_tokens.avg ||
      (context.expected_input_tokens.min + context.expected_input_tokens.max) / 2;
    const avgOutputTokens = context.expected_output_tokens.avg ||
      (context.expected_output_tokens.min + context.expected_output_tokens.max) / 2;

    return (avgInputTokens / 1000) * costs.input + (avgOutputTokens / 1000) * costs.output;
  }

  // --------------------------------------------------------------------------
  // Model Scoring
  // --------------------------------------------------------------------------

  private scoreModels(
    models: string[],
    context: TaskContext,
    history: ModelPerformanceHistory
  ): ScoredModel[] {
    const scoredModels: ScoredModel[] = [];

    // Determine weights based on task context
    const weights = this.determineWeights(context);

    for (const model of models) {
      const scores = this.calculateModelScores(model, context, history);
      const totalScore =
        scores.quality_score * weights.quality +
        scores.latency_score * weights.latency +
        scores.cost_score * weights.cost +
        scores.task_fit_score * weights.task_fit +
        scores.reliability_score * weights.reliability;

      scoredModels.push({
        model,
        totalScore,
        scores: {
          ...scores,
          weights,
        },
      });
    }

    // Sort by total score descending
    return scoredModels.sort((a, b) => b.totalScore - a.totalScore);
  }

  private determineWeights(context: TaskContext): {
    quality: number;
    latency: number;
    cost: number;
    task_fit: number;
    reliability: number;
  } {
    // Base weights
    let quality = 0.25;
    let latency = 0.20;
    let cost = 0.20;
    let task_fit = 0.25;
    let reliability = 0.10;

    // Adjust based on quality requirements
    if (context.quality_requirements.strict) {
      quality = 0.35;
      task_fit = 0.30;
      cost = 0.15;
    }

    // Adjust based on latency requirements
    if (context.latency_requirements.strict) {
      latency = 0.30;
      quality = 0.20;
    }

    // Adjust based on cost optimization priority
    if (context.budget_constraints.cost_optimization_priority === 'high') {
      cost = 0.35;
      quality = 0.20;
      task_fit = 0.20;
    }

    // Adjust based on complexity
    if (context.complexity === 'expert') {
      quality = Math.min(quality + 0.10, 0.40);
      task_fit = Math.min(task_fit + 0.05, 0.35);
      cost = Math.max(cost - 0.10, 0.10);
    }

    // Normalize to sum to 1.0
    const total = quality + latency + cost + task_fit + reliability;
    return {
      quality: quality / total,
      latency: latency / total,
      cost: cost / total,
      task_fit: task_fit / total,
      reliability: reliability / total,
    };
  }

  private calculateModelScores(
    model: string,
    context: TaskContext,
    history: ModelPerformanceHistory
  ): Omit<ScoreBreakdown, 'weights'> {
    // Quality score (based on benchmarks and history)
    const benchmarkQuality = QUALITY_BENCHMARKS[model] || 0.7;
    const historicalQuality = history.by_model[model]?.quality.avg_score || benchmarkQuality;
    const quality_score = (benchmarkQuality + historicalQuality) / 2;

    // Latency score (inverted - lower latency = higher score)
    const benchmarkLatency = LATENCY_BENCHMARKS[model] || 2000;
    const historicalLatency = history.by_model[model]?.latency.p95_ms || benchmarkLatency;
    const avgLatency = (benchmarkLatency + historicalLatency) / 2;
    const targetLatency = context.latency_requirements.target_latency_ms ||
                          context.latency_requirements.max_latency_ms;
    const latency_score = Math.min(1.0, Math.max(0.0, 1 - (avgLatency - targetLatency * 0.5) / targetLatency));

    // Cost score (inverted - lower cost = higher score)
    const estimatedCost = this.estimateCostPerRequest(model, context);
    const maxCost = context.budget_constraints.max_cost_per_request_usd || 0.5;
    const cost_score = Math.min(1.0, Math.max(0.0, 1 - estimatedCost / maxCost));

    // Task fit score
    const taskSuitability = TASK_MODEL_SUITABILITY[context.task_type] ||
                           TASK_MODEL_SUITABILITY['general'];
    const task_fit_score = taskSuitability[model] || 0.7;

    // Reliability score (based on availability and error rate)
    const availability = history.by_model[model]?.availability.uptime_pct || 99.5;
    const errorRate = history.by_model[model]?.quality.error_rate || 0.02;
    const reliability_score = (availability / 100) * (1 - errorRate);

    return {
      quality_score,
      latency_score,
      cost_score,
      task_fit_score,
      reliability_score,
    };
  }

  // --------------------------------------------------------------------------
  // Recommendation Building
  // --------------------------------------------------------------------------

  private buildRecommendations(
    scoredModels: ScoredModel[],
    input: ModelSelectionAgentInput
  ): ModelRecommendation[] {
    const recommendations: ModelRecommendation[] = [];

    for (let i = 0; i < Math.min(scoredModels.length, this.config.maxAlternatives + 1); i++) {
      const scored = scoredModels[i];

      if (scored.totalScore < this.config.minConfidenceThreshold) {
        continue;
      }

      const recommendation = this.buildSingleRecommendation(
        scored,
        i + 1,
        input.task_context,
        input.performance_history
      );

      recommendations.push(recommendation);
    }

    return recommendations;
  }

  private buildSingleRecommendation(
    scored: ScoredModel,
    rank: number,
    context: TaskContext,
    history: ModelPerformanceHistory
  ): ModelRecommendation {
    const model = scored.model;
    const provider = MODEL_PROVIDERS[model] || 'unknown';

    // Build reasoning
    const topScores = this.getTopScoringFactors(scored.scores);
    const reasoning = this.buildReasoning(model, topScores, context);

    // Build evidence
    const evidence = this.buildEvidence(model, history, context);

    // Build expected performance
    const expectedPerformance = this.buildExpectedPerformance(model, context, history);

    // Build suggested config
    const suggestedConfig = this.buildSuggestedConfig(model, context);

    return {
      model_id: model,
      provider,
      rank,
      suitability_score: scored.totalScore,
      confidence: this.calculateConfidence(scored, history),
      reasoning,
      score_breakdown: scored.scores,
      expected_performance: expectedPerformance,
      evidence,
      suggested_config: suggestedConfig,
    };
  }

  private getTopScoringFactors(scores: ScoreBreakdown): string[] {
    const factors: Array<{ name: string; score: number; weight: number }> = [
      { name: 'quality', score: scores.quality_score, weight: scores.weights.quality },
      { name: 'latency', score: scores.latency_score, weight: scores.weights.latency },
      { name: 'cost', score: scores.cost_score, weight: scores.weights.cost },
      { name: 'task_fit', score: scores.task_fit_score, weight: scores.weights.task_fit },
      { name: 'reliability', score: scores.reliability_score, weight: scores.weights.reliability },
    ];

    return factors
      .sort((a, b) => (b.score * b.weight) - (a.score * a.weight))
      .slice(0, 3)
      .map(f => f.name);
  }

  private buildReasoning(model: string, topFactors: string[], context: TaskContext): string {
    const parts: string[] = [];

    parts.push(`${model} is recommended for ${context.task_type} tasks.`);

    if (topFactors.includes('quality')) {
      parts.push(`Strong quality performance (benchmark: ${(QUALITY_BENCHMARKS[model] || 0.8) * 100}%).`);
    }
    if (topFactors.includes('latency')) {
      parts.push(`Meets latency requirements (P95: ${LATENCY_BENCHMARKS[model] || 2000}ms).`);
    }
    if (topFactors.includes('cost')) {
      const costs = MODEL_COSTS[model];
      if (costs) {
        parts.push(`Cost-effective pricing ($${costs.input}/1K input, $${costs.output}/1K output).`);
      }
    }
    if (topFactors.includes('task_fit')) {
      parts.push(`Well-suited for ${context.task_type} based on capability analysis.`);
    }

    return parts.join(' ');
  }

  private buildEvidence(
    model: string,
    history: ModelPerformanceHistory,
    context: TaskContext
  ): ModelSelectionEvidence[] {
    const evidence: ModelSelectionEvidence[] = [];

    // Historical performance evidence
    const modelHistory = history.by_model[model];
    if (modelHistory && modelHistory.request_count > 0) {
      evidence.push({
        type: 'historical_performance',
        source: 'llm-observatory',
        data: {
          request_count: modelHistory.request_count,
          avg_quality: modelHistory.quality.avg_score,
          p95_latency: modelHistory.latency.p95_ms,
          success_rate: modelHistory.quality.success_rate,
        },
        strength: Math.min(1.0, modelHistory.request_count / 1000),
      });
    }

    // Benchmark evidence
    evidence.push({
      type: 'benchmark',
      source: 'model_benchmarks',
      data: {
        quality_benchmark: QUALITY_BENCHMARKS[model] || 0.8,
        latency_benchmark: LATENCY_BENCHMARKS[model] || 2000,
        task_suitability: TASK_MODEL_SUITABILITY[context.task_type]?.[model] || 0.7,
      },
      strength: 0.8,
    });

    // Task similarity evidence
    const taskPerf = history.by_task_type[context.task_type];
    if (taskPerf) {
      const modelRank = taskPerf.top_models.findIndex(m => m.model_id === model);
      if (modelRank >= 0) {
        evidence.push({
          type: 'task_similarity',
          source: 'task_performance_history',
          data: {
            task_type: context.task_type,
            rank_for_task: modelRank + 1,
            score_for_task: taskPerf.top_models[modelRank].score,
          },
          strength: 0.9,
        });
      }
    }

    // Cost analysis evidence
    const costs = MODEL_COSTS[model];
    if (costs) {
      const estimatedCost = this.estimateCostPerRequest(model, context);
      evidence.push({
        type: 'cost_analysis',
        source: 'pricing_data',
        data: {
          input_cost_per_1k: costs.input,
          output_cost_per_1k: costs.output,
          estimated_cost_per_request: estimatedCost,
        },
        strength: 1.0,
      });
    }

    return evidence;
  }

  private buildExpectedPerformance(
    model: string,
    context: TaskContext,
    history: ModelPerformanceHistory
  ): ExpectedPerformance {
    const modelHistory = history.by_model[model];
    const benchmarkLatency = LATENCY_BENCHMARKS[model] || 2000;
    const historicalLatency = modelHistory?.latency.p95_ms || benchmarkLatency;

    return {
      latency_ms: {
        min: Math.round(historicalLatency * 0.5),
        max: Math.round(historicalLatency * 1.2),
        avg: Math.round(historicalLatency * 0.85),
      },
      quality_score: QUALITY_BENCHMARKS[model] || 0.8,
      cost_per_request_usd: this.estimateCostPerRequest(model, context),
      success_rate: modelHistory?.quality.success_rate || 0.98,
      projection_confidence: modelHistory ? 0.85 : 0.7,
    };
  }

  private buildSuggestedConfig(model: string, context: TaskContext): ModelConfig {
    const config: ModelConfig = {};

    // Temperature based on task type
    if (context.task_type === 'creative_writing') {
      config.temperature = 0.8;
    } else if (context.task_type === 'code_generation' || context.task_type === 'data_extraction') {
      config.temperature = 0.2;
    } else {
      config.temperature = 0.5;
    }

    // Max tokens based on expected output
    config.max_tokens = Math.min(4096, context.expected_output_tokens.max);

    // Add guidance notes
    if (context.complexity === 'expert') {
      config.system_prompt_notes = 'Consider using chain-of-thought prompting for complex reasoning.';
    }

    return config;
  }

  private calculateConfidence(
    scored: ScoredModel,
    history: ModelPerformanceHistory
  ): number {
    const modelHistory = history.by_model[scored.model];

    // Base confidence from score
    let confidence = scored.totalScore;

    // Boost confidence if we have historical data
    if (modelHistory && modelHistory.request_count > 100) {
      confidence = Math.min(1.0, confidence * 1.1);
    }

    // Reduce confidence if scores are close
    // (this would require comparing to other models)

    return Math.round(confidence * 100) / 100;
  }

  // --------------------------------------------------------------------------
  // Analysis Summary Building
  // --------------------------------------------------------------------------

  private buildAnalysisSummary(
    allModels: string[],
    eligibleModels: string[],
    scoredModels: ScoredModel[],
    context: TaskContext
  ): ModelSelectionAnalysis {
    const topModel = scoredModels[0]?.model || 'none';
    const scoreMargin = scoredModels.length > 1
      ? scoredModels[0].totalScore - scoredModels[1].totalScore
      : 0;

    const keyFactors: string[] = [];
    if (context.quality_requirements.strict) {
      keyFactors.push('Strict quality requirements applied');
    }
    if (context.latency_requirements.strict) {
      keyFactors.push('Strict latency requirements applied');
    }
    if (context.budget_constraints.cost_optimization_priority === 'high') {
      keyFactors.push('High cost optimization priority');
    }
    keyFactors.push(`Task type: ${context.task_type}`);
    keyFactors.push(`Complexity: ${context.complexity}`);

    const warnings: string[] = [];
    if (eligibleModels.length < 3) {
      warnings.push('Limited model options after constraint filtering');
    }
    if (scoreMargin < 0.05) {
      warnings.push('Multiple models scored similarly - consider A/B testing');
    }
    if (scoredModels[0]?.totalScore < 0.7) {
      warnings.push('Top recommendation has lower confidence - verify requirements');
    }

    const taskCompatibility = scoredModels[0]?.totalScore >= 0.85 ? 'excellent' :
                             scoredModels[0]?.totalScore >= 0.75 ? 'good' :
                             scoredModels[0]?.totalScore >= 0.65 ? 'adequate' : 'marginal';

    const confidenceLevel = scoredModels[0]?.totalScore >= 0.8 ? 'high' :
                           scoredModels[0]?.totalScore >= 0.6 ? 'medium' : 'low';

    return {
      models_evaluated: allModels.length,
      models_passing_constraints: eligibleModels.length,
      top_model: topModel,
      score_margin: Math.round(scoreMargin * 100) / 100,
      key_factors: keyFactors,
      warnings,
      task_compatibility: taskCompatibility,
      confidence_level: confidenceLevel,
    };
  }

  // --------------------------------------------------------------------------
  // Output Building
  // --------------------------------------------------------------------------

  private buildOutput(
    input: ModelSelectionAgentInput,
    recommendations: ModelRecommendation[],
    analysisSummary: ModelSelectionAnalysis,
    constraintEvaluations: ModelSelectionConstraintEvaluation[],
    modelsConsidered: string[],
    startTime: number
  ): ModelSelectionAgentOutput {
    const timestamp = new Date().toISOString();
    const outputId = `out-${AGENT_ID}-${Date.now()}`;

    // Primary is first recommendation (or a placeholder if none)
    const primaryRecommendation = recommendations[0] || this.buildNoRecommendationPlaceholder();

    // Alternatives are the rest
    const alternativeRecommendations = recommendations.slice(1);

    const metadata: ModelSelectionMetadata = {
      processing_duration_ms: Date.now() - startTime,
      data_points_analyzed:
        Object.keys(input.performance_history.by_model).length * 5 +
        Object.keys(input.performance_history.by_task_type).length,
      strategies_used: ['task_fit_analysis', 'cost_analysis', 'latency_analysis', 'quality_analysis'],
      warnings: analysisSummary.warnings,
      models_considered: modelsConsidered,
    };

    return {
      output_id: outputId,
      execution_ref: input.execution_ref,
      agent_version: AGENT_VERSION,
      timestamp,
      primary_recommendation: primaryRecommendation,
      alternative_recommendations: alternativeRecommendations,
      analysis_summary: analysisSummary,
      constraints_evaluated: constraintEvaluations,
      metadata,
    };
  }

  private buildNoRecommendationPlaceholder(): ModelRecommendation {
    return {
      model_id: 'none',
      provider: 'none',
      rank: 1,
      suitability_score: 0,
      confidence: 0,
      reasoning: 'No models met all constraints. Consider relaxing constraints.',
      score_breakdown: {
        quality_score: 0,
        latency_score: 0,
        cost_score: 0,
        task_fit_score: 0,
        reliability_score: 0,
        weights: {
          quality: 0.25,
          latency: 0.20,
          cost: 0.20,
          task_fit: 0.25,
          reliability: 0.10,
        },
      },
      expected_performance: {
        latency_ms: { min: 0, max: 0 },
        quality_score: 0,
        cost_per_request_usd: 0,
        success_rate: 0,
        projection_confidence: 0,
      },
      evidence: [],
    };
  }

  // --------------------------------------------------------------------------
  // DecisionEvent Building
  // --------------------------------------------------------------------------

  private buildDecisionEvent(
    input: ModelSelectionAgentInput,
    output: ModelSelectionAgentOutput,
    inputsHash: string,
    constraintEvaluations: ModelSelectionConstraintEvaluation[]
  ): DecisionEvent {
    const outputs: DecisionOutputs = {
      recommendations: [{
        recommendation_id: output.output_id,
        rank: 1,
        category: 'model_substitution',
        title: `Select ${output.primary_recommendation.model_id} for ${input.task_context.task_type}`,
        description: output.primary_recommendation.reasoning,
        proposed_changes: [{
          type: 'update',
          parameter: 'model_selection',
          current_value: input.current_model_config?.default_model,
          proposed_value: output.primary_recommendation.model_id,
          affected_services: input.target_services,
        }],
        expected_impact: {
          cost_change_pct: 0,
          latency_change_pct: 0,
          quality_change_pct: 0,
          confidence: output.primary_recommendation.confidence,
        },
        complexity: 'low',
        risk: 'low',
        confidence: output.primary_recommendation.confidence,
        evidence: output.primary_recommendation.evidence.map(e => ({
          type: e.type as 'historical_data' | 'benchmark' | 'a_b_test' | 'simulation' | 'industry_standard',
          source: e.source,
          data_points: e.data,
          strength: e.strength,
        })),
        rollout_strategy: {
          type: 'canary',
          canary_percentage: 10,
          rollback_criteria: {
            max_error_rate_increase_pct: 5,
            max_latency_increase_pct: 20,
            min_quality_threshold: 0.8,
            monitoring_duration_minutes: 60,
          },
        },
      }],
    };

    const constraintsApplied: AppliedConstraint[] = constraintEvaluations.map((ce, idx) => ({
      constraint_id: `constraint-${idx}`,
      type: ce.constraint.type as any,
      value: ce.constraint.value as any,
      hard: ce.constraint.hard,
      satisfied: ce.satisfied,
    }));

    return {
      agent_id: AGENT_ID,
      agent_version: AGENT_VERSION,
      decision_type: DECISION_TYPE,
      inputs_hash: inputsHash,
      outputs,
      confidence: output.primary_recommendation.confidence,
      constraints_applied: constraintsApplied,
      execution_ref: input.execution_ref,
      timestamp: output.timestamp,
    };
  }

  // --------------------------------------------------------------------------
  // Telemetry Emission
  // --------------------------------------------------------------------------

  private async emitTelemetry(
    decisionEvent: DecisionEvent,
    output: ModelSelectionAgentOutput,
    durationMs: number
  ): Promise<void> {
    if (!this.config.telemetryEndpoint) return;

    const telemetry = {
      agent_id: AGENT_ID,
      agent_version: AGENT_VERSION,
      execution_ref: decisionEvent.execution_ref,
      metrics: {
        processing_duration_ms: durationMs,
        models_evaluated: output.metadata.models_considered.length,
        recommendations_generated: output.alternative_recommendations.length + 1,
        primary_confidence: output.primary_recommendation.confidence,
        constraints_evaluated: decisionEvent.constraints_applied.length,
        constraints_satisfied: decisionEvent.constraints_applied.filter(c => c.satisfied).length,
      },
      timestamp: new Date().toISOString(),
    };

    try {
      await fetch(this.config.telemetryEndpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-Agent-Id': AGENT_ID,
        },
        body: JSON.stringify(telemetry),
      });
    } catch (error) {
      // Telemetry failure should not fail the agent
      console.error('[ModelSelectionAgent] Telemetry emission failed:', error);
    }
  }
}

// ============================================================================
// Internal Types
// ============================================================================

interface ScoredModel {
  model: string;
  totalScore: number;
  scores: ScoreBreakdown;
}

// ============================================================================
// Factory Function
// ============================================================================

/**
 * Create a Model Selection Agent instance.
 */
export function createModelSelectionAgent(
  config?: ModelSelectionAgentConfig
): ModelSelectionAgent {
  return new ModelSelectionAgent(config);
}
