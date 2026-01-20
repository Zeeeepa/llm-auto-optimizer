/**
 * Self-Optimizing Agent
 *
 * A RECOMMENDATION agent that generates holistic optimization recommendations
 * across cost, latency, and quality dimensions.
 *
 * Classification: RECOMMENDATION
 * Decision Type: system_optimization_recommendation
 *
 * This agent:
 * - Analyzes multi-signal historical data
 * - Proposes bounded configuration improvements
 * - Emits ranked optimization recommendations
 *
 * This agent MUST NOT:
 * - Execute optimizations directly
 * - Modify runtime behavior autonomously
 * - Intercept execution traffic
 * - Trigger retries
 * - Emit alerts (that is Sentinel)
 * - Enforce policies (that is Shield)
 * - Perform orchestration (that is LLM-Orchestrator)
 *
 * @module agents/self-optimizing-agent
 * @version 1.0.0
 */

import {
  // Input/Output schemas
  SelfOptimizingAgentInput,
  SelfOptimizingAgentOutput,
  // DecisionEvent
  DecisionEvent,
  DecisionOutputs,
  AppliedConstraint,
  // Validation
  validateInput,
  validateOutput,
  hashInputs,
  // Types
  OptimizationRecommendation,
  RecommendationCategory,
  ProposedChange,
  ExpectedImpact,
  Evidence,
  RolloutStrategy,
  AnalysisSummary,
  StateAssessment,
  ImprovementPotential,
  EvaluatedConstraint,
  OutputMetadata,
  TelemetryData,
  OptimizationTargets,
  InputConstraint,
} from '../../contracts';

import { RuVectorClient, createRuVectorClient } from '../../services/ruvector-client';

// ============================================================================
// Constants
// ============================================================================

export const AGENT_ID = 'self-optimizing-agent';
export const AGENT_VERSION = '1.0.0';
export const DECISION_TYPE = 'system_optimization_recommendation';

// Model cost data (from Anthropic pricing)
const MODEL_COSTS: Record<string, { input: number; output: number }> = {
  'claude-3-opus': { input: 0.015, output: 0.075 },
  'claude-3-sonnet': { input: 0.003, output: 0.015 },
  'claude-3-haiku': { input: 0.00025, output: 0.00125 },
  'gpt-4': { input: 0.03, output: 0.06 },
  'gpt-4-turbo': { input: 0.01, output: 0.03 },
  'gpt-3.5-turbo': { input: 0.0005, output: 0.0015 },
};

// Quality benchmarks (0-1 scale)
const QUALITY_BENCHMARKS: Record<string, number> = {
  'claude-3-opus': 0.95,
  'claude-3-sonnet': 0.90,
  'claude-3-haiku': 0.82,
  'gpt-4': 0.93,
  'gpt-4-turbo': 0.91,
  'gpt-3.5-turbo': 0.75,
};

// ============================================================================
// Agent Configuration
// ============================================================================

export interface SelfOptimizingAgentConfig {
  /** RuVector client for persistence */
  ruVectorClient?: RuVectorClient;
  /** Enable telemetry emission */
  enableTelemetry?: boolean;
  /** Telemetry endpoint */
  telemetryEndpoint?: string;
  /** Maximum recommendations to generate */
  maxRecommendations?: number;
  /** Minimum confidence threshold */
  minConfidenceThreshold?: number;
}

const DEFAULT_CONFIG: Required<SelfOptimizingAgentConfig> = {
  ruVectorClient: undefined as unknown as RuVectorClient,
  enableTelemetry: true,
  telemetryEndpoint: process.env.TELEMETRY_ENDPOINT || '',
  maxRecommendations: 10,
  minConfidenceThreshold: 0.6,
};

// ============================================================================
// Self-Optimizing Agent Class
// ============================================================================

/**
 * Self-Optimizing Agent - Generates holistic optimization recommendations.
 *
 * This is a stateless, deterministic agent designed to run as a
 * Google Cloud Edge Function.
 */
export class SelfOptimizingAgent {
  private config: Required<SelfOptimizingAgentConfig>;
  private ruVectorClient: RuVectorClient;

  constructor(config: SelfOptimizingAgentConfig = {}) {
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
   * Use this for understanding current state and identifying opportunities.
   */
  async analyze(input: SelfOptimizingAgentInput): Promise<SelfOptimizingAgentOutput> {
    return this.execute(input, 'analyze');
  }

  /**
   * RECOMMEND - Generate ranked optimization recommendations.
   * Primary entry point for the agent.
   */
  async recommend(input: SelfOptimizingAgentInput): Promise<SelfOptimizingAgentOutput> {
    return this.execute(input, 'recommend');
  }

  /**
   * SIMULATE - Simulate proposed changes without persisting.
   * Use for what-if analysis.
   */
  async simulate(input: SelfOptimizingAgentInput): Promise<SelfOptimizingAgentOutput> {
    return this.execute(input, 'simulate');
  }

  // --------------------------------------------------------------------------
  // Core Execution Logic
  // --------------------------------------------------------------------------

  private async execute(
    input: SelfOptimizingAgentInput,
    mode: 'analyze' | 'recommend' | 'simulate'
  ): Promise<SelfOptimizingAgentOutput> {
    const startTime = Date.now();

    // Step 1: Validate input against schema
    validateInput(input);

    // Step 2: Compute inputs hash for reproducibility
    const inputsHash = hashInputs(input);

    // Step 3: Perform analysis
    const stateAssessment = this.assessCurrentState(input.telemetry);
    const opportunities = this.identifyOpportunities(input.telemetry, input.optimization_targets);

    // Step 4: Generate recommendations (if not analyze-only)
    const recommendations =
      mode === 'analyze'
        ? []
        : this.generateRecommendations(
            input.telemetry,
            input.optimization_targets,
            input.constraints,
            opportunities
          );

    // Step 5: Evaluate constraints
    const evaluatedConstraints = this.evaluateConstraints(
      input.constraints,
      recommendations
    );

    // Step 6: Build output
    const output = this.buildOutput(
      input,
      stateAssessment,
      recommendations,
      evaluatedConstraints,
      startTime
    );

    // Step 7: Validate output
    validateOutput(output);

    // Step 8: Build and persist DecisionEvent
    const decisionEvent = this.buildDecisionEvent(
      input,
      output,
      inputsHash,
      evaluatedConstraints
    );

    // Persist to ruvector-service (MANDATORY)
    await this.ruVectorClient.persistDecisionEvent(decisionEvent);

    // Step 9: Emit telemetry (if enabled)
    if (this.config.enableTelemetry) {
      await this.emitTelemetry(decisionEvent, output, Date.now() - startTime);
    }

    return output;
  }

  // --------------------------------------------------------------------------
  // Analysis Logic
  // --------------------------------------------------------------------------

  private assessCurrentState(telemetry: TelemetryData): StateAssessment {
    // Calculate cost efficiency
    const costEfficiency = this.calculateCostEfficiency(telemetry.cost_metrics);

    // Calculate latency efficiency
    const latencyEfficiency = this.calculateLatencyEfficiency(telemetry.latency_metrics);

    // Quality score from telemetry
    const qualityScore = telemetry.quality_metrics.overall_score;

    // Overall health is weighted average
    const overallHealth =
      (costEfficiency * 0.3 + latencyEfficiency * 0.3 + qualityScore * 0.4);

    return {
      cost_efficiency: Math.round(costEfficiency * 100) / 100,
      latency_efficiency: Math.round(latencyEfficiency * 100) / 100,
      quality_score: Math.round(qualityScore * 100) / 100,
      overall_health: Math.round(overallHealth * 100) / 100,
    };
  }

  private calculateCostEfficiency(costMetrics: TelemetryData['cost_metrics']): number {
    // Compare actual cost per request vs optimal possible cost
    const models = Object.keys(costMetrics.by_model);
    if (models.length === 0) return 0.5;

    let totalActualCost = 0;
    let totalOptimalCost = 0;

    for (const model of models) {
      const modelCost = costMetrics.by_model[model];
      totalActualCost += modelCost.cost_usd;

      // Optimal would be using cheapest model (haiku) with same request count
      const haikuCost = MODEL_COSTS['claude-3-haiku'];
      const estimatedTokens = modelCost.input_tokens + modelCost.output_tokens;
      totalOptimalCost +=
        (modelCost.input_tokens / 1000) * haikuCost.input +
        (modelCost.output_tokens / 1000) * haikuCost.output;
    }

    if (totalActualCost === 0) return 1.0;

    // Efficiency is how close actual is to optimal (inverted ratio, capped at 1)
    return Math.min(1.0, totalOptimalCost / totalActualCost);
  }

  private calculateLatencyEfficiency(latencyMetrics: TelemetryData['latency_metrics']): number {
    // Compare P95 against target (500ms is good, 2000ms is poor)
    const targetP95 = 500;
    const maxAcceptable = 2000;

    if (latencyMetrics.p95_ms <= targetP95) {
      return 1.0;
    } else if (latencyMetrics.p95_ms >= maxAcceptable) {
      return 0.0;
    } else {
      // Linear interpolation
      return 1.0 - (latencyMetrics.p95_ms - targetP95) / (maxAcceptable - targetP95);
    }
  }

  private identifyOpportunities(
    telemetry: TelemetryData,
    targets: OptimizationTargets
  ): OptimizationOpportunity[] {
    const opportunities: OptimizationOpportunity[] = [];

    // Opportunity 1: Model substitution for cost savings
    for (const [model, metrics] of Object.entries(telemetry.cost_metrics.by_model)) {
      const cheaperModels = this.findCheaperModels(model);
      for (const cheaperModel of cheaperModels) {
        const qualityDrop = this.estimateQualityDrop(model, cheaperModel);
        if (qualityDrop < 0.15) { // Allow up to 15% quality drop
          opportunities.push({
            type: 'model_substitution',
            source_model: model,
            target_model: cheaperModel,
            cost_savings_pct: this.estimateCostSavings(model, cheaperModel),
            quality_impact_pct: -qualityDrop * 100,
            affected_requests: metrics.request_count,
          });
        }
      }
    }

    // Opportunity 2: Latency optimization via model routing
    const slowModels = this.identifySlowModels(telemetry.latency_metrics);
    for (const slowModel of slowModels) {
      opportunities.push({
        type: 'latency_routing',
        source_model: slowModel.model,
        target_model: slowModel.alternative,
        latency_improvement_pct: slowModel.improvement_pct,
        cost_impact_pct: slowModel.cost_delta_pct,
        affected_requests: telemetry.latency_metrics.by_model[slowModel.model]?.request_count || 0,
      });
    }

    // Opportunity 3: Anomaly-based adjustments
    for (const anomaly of telemetry.anomaly_signals) {
      if (anomaly.type === 'cost_spike' && anomaly.severity !== 'low') {
        opportunities.push({
          type: 'anomaly_response',
          anomaly_id: anomaly.anomaly_id,
          anomaly_type: anomaly.type,
          affected_entities: anomaly.affected_entities,
          urgency: anomaly.severity,
        });
      }
    }

    return opportunities;
  }

  private findCheaperModels(currentModel: string): string[] {
    const currentCost = MODEL_COSTS[currentModel];
    if (!currentCost) return [];

    return Object.entries(MODEL_COSTS)
      .filter(([model, cost]) =>
        model !== currentModel &&
        (cost.input + cost.output) < (currentCost.input + currentCost.output)
      )
      .sort((a, b) => (a[1].input + a[1].output) - (b[1].input + b[1].output))
      .map(([model]) => model);
  }

  private estimateQualityDrop(fromModel: string, toModel: string): number {
    const fromQuality = QUALITY_BENCHMARKS[fromModel] || 0.85;
    const toQuality = QUALITY_BENCHMARKS[toModel] || 0.75;
    return Math.max(0, fromQuality - toQuality);
  }

  private estimateCostSavings(fromModel: string, toModel: string): number {
    const fromCost = MODEL_COSTS[fromModel];
    const toCost = MODEL_COSTS[toModel];
    if (!fromCost || !toCost) return 0;

    const fromAvg = (fromCost.input + fromCost.output) / 2;
    const toAvg = (toCost.input + toCost.output) / 2;

    return ((fromAvg - toAvg) / fromAvg) * 100;
  }

  private identifySlowModels(latencyMetrics: TelemetryData['latency_metrics']): Array<{
    model: string;
    alternative: string;
    improvement_pct: number;
    cost_delta_pct: number;
  }> {
    const results: Array<{
      model: string;
      alternative: string;
      improvement_pct: number;
      cost_delta_pct: number;
    }> = [];

    const avgLatency = latencyMetrics.avg_ms;

    for (const [model, metrics] of Object.entries(latencyMetrics.by_model)) {
      if (metrics.p95_ms > avgLatency * 1.5) { // 50% slower than average
        // Suggest faster model (typically haiku is fastest)
        const alternative = model.includes('opus') ? 'claude-3-sonnet' :
                          model.includes('gpt-4') ? 'gpt-3.5-turbo' : 'claude-3-haiku';

        results.push({
          model,
          alternative,
          improvement_pct: 30, // Estimated
          cost_delta_pct: this.estimateCostSavings(model, alternative),
        });
      }
    }

    return results;
  }

  // --------------------------------------------------------------------------
  // Recommendation Generation
  // --------------------------------------------------------------------------

  private generateRecommendations(
    telemetry: TelemetryData,
    targets: OptimizationTargets,
    constraints: InputConstraint[],
    opportunities: OptimizationOpportunity[]
  ): OptimizationRecommendation[] {
    const recommendations: OptimizationRecommendation[] = [];
    let rank = 1;

    // Sort opportunities by weighted score
    const scoredOpportunities = opportunities.map(opp => ({
      opportunity: opp,
      score: this.scoreOpportunity(opp, targets),
    })).sort((a, b) => b.score - a.score);

    for (const { opportunity, score } of scoredOpportunities) {
      if (rank > this.config.maxRecommendations) break;
      if (score < this.config.minConfidenceThreshold) continue;

      // Check if recommendation violates hard constraints
      if (this.violatesHardConstraints(opportunity, constraints)) continue;

      const recommendation = this.buildRecommendation(opportunity, rank, score, telemetry);
      recommendations.push(recommendation);
      rank++;
    }

    return recommendations;
  }

  private scoreOpportunity(opportunity: OptimizationOpportunity, targets: OptimizationTargets): number {
    let score = 0;

    switch (opportunity.type) {
      case 'model_substitution':
        score += (opportunity.cost_savings_pct || 0) * targets.cost_weight * 0.01;
        score -= Math.abs(opportunity.quality_impact_pct || 0) * targets.quality_weight * 0.01;
        break;

      case 'latency_routing':
        score += (opportunity.latency_improvement_pct || 0) * targets.latency_weight * 0.01;
        score += (opportunity.cost_impact_pct || 0) * targets.cost_weight * 0.005;
        break;

      case 'anomaly_response':
        // Anomaly responses get high priority if severe
        score = opportunity.urgency === 'critical' ? 0.95 :
                opportunity.urgency === 'high' ? 0.85 :
                opportunity.urgency === 'medium' ? 0.7 : 0.5;
        break;
    }

    return Math.min(1.0, Math.max(0.0, score));
  }

  private violatesHardConstraints(
    opportunity: OptimizationOpportunity,
    constraints: InputConstraint[]
  ): boolean {
    for (const constraint of constraints) {
      if (!constraint.hard) continue;

      switch (constraint.type) {
        case 'max_cost_increase_pct':
          if (opportunity.cost_impact_pct && opportunity.cost_impact_pct > Number(constraint.value)) {
            return true;
          }
          break;

        case 'min_quality_threshold':
          if (opportunity.quality_impact_pct &&
              (100 + opportunity.quality_impact_pct) / 100 < Number(constraint.value)) {
            return true;
          }
          break;

        case 'allowed_models':
          const allowedModels = String(constraint.value).split(',');
          if (opportunity.target_model && !allowedModels.includes(opportunity.target_model)) {
            return true;
          }
          break;
      }
    }

    return false;
  }

  private buildRecommendation(
    opportunity: OptimizationOpportunity,
    rank: number,
    score: number,
    telemetry: TelemetryData
  ): OptimizationRecommendation {
    const recommendationId = `rec-${Date.now()}-${rank}`;

    let category: RecommendationCategory;
    let title: string;
    let description: string;
    let proposedChanges: ProposedChange[] = [];

    switch (opportunity.type) {
      case 'model_substitution':
        category = 'model_substitution';
        title = `Switch from ${opportunity.source_model} to ${opportunity.target_model}`;
        description =
          `Replace ${opportunity.source_model} with ${opportunity.target_model} for ` +
          `${opportunity.affected_requests} requests. ` +
          `Expected ${opportunity.cost_savings_pct?.toFixed(1)}% cost savings ` +
          `with ${Math.abs(opportunity.quality_impact_pct || 0).toFixed(1)}% quality impact.`;

        proposedChanges = [{
          type: 'update',
          parameter: 'model_routing.default_model',
          current_value: opportunity.source_model,
          proposed_value: opportunity.target_model,
          affected_services: ['llm-gateway'],
        }];
        break;

      case 'latency_routing':
        category = 'traffic_routing';
        title = `Optimize latency routing for ${opportunity.source_model}`;
        description =
          `Route latency-sensitive requests from ${opportunity.source_model} to ${opportunity.target_model}. ` +
          `Expected ${opportunity.latency_improvement_pct}% latency improvement.`;

        proposedChanges = [{
          type: 'add',
          parameter: 'routing_rules.latency_sensitive',
          proposed_value: {
            condition: 'latency_budget_ms < 500',
            preferred_model: opportunity.target_model,
          },
          affected_services: ['router-l2'],
        }];
        break;

      case 'anomaly_response':
        category = 'cost_optimization';
        title = `Address ${opportunity.anomaly_type} anomaly`;
        description =
          `Detected ${opportunity.urgency} severity ${opportunity.anomaly_type} ` +
          `affecting ${opportunity.affected_entities?.join(', ')}. ` +
          `Recommend investigating and applying mitigation.`;

        proposedChanges = [{
          type: 'add',
          parameter: 'anomaly_mitigations',
          proposed_value: {
            anomaly_id: opportunity.anomaly_id,
            action: 'investigate_and_mitigate',
          },
          affected_services: opportunity.affected_entities || [],
        }];
        break;

      default:
        category = 'cost_optimization';
        title = 'General optimization';
        description = 'See details for optimization opportunity.';
    }

    const expectedImpact: ExpectedImpact = {
      cost_change_pct: -(opportunity.cost_savings_pct || 0),
      latency_change_pct: -(opportunity.latency_improvement_pct || 0),
      quality_change_pct: opportunity.quality_impact_pct || 0,
      confidence: score,
    };

    // Estimate monthly savings
    if (opportunity.cost_savings_pct && telemetry.cost_metrics.total_cost_usd) {
      expectedImpact.monthly_savings_usd =
        telemetry.cost_metrics.total_cost_usd * (opportunity.cost_savings_pct / 100) * 30;
    }

    const evidence: Evidence[] = [{
      type: 'historical_data',
      source: 'llm-observatory',
      data_points: {
        time_window: 'input.time_window',
        request_count: opportunity.affected_requests,
      },
      strength: 0.8,
    }];

    const rolloutStrategy: RolloutStrategy = {
      type: score > 0.85 ? 'gradual' : 'canary',
      canary_percentage: 10,
      phases: score > 0.85 ? [
        { name: 'pilot', percentage: 10, duration_hours: 24 },
        { name: 'expand', percentage: 50, duration_hours: 48 },
        { name: 'full', percentage: 100, duration_hours: 0 },
      ] : undefined,
      rollback_criteria: {
        max_error_rate_increase_pct: 5,
        max_latency_increase_pct: 20,
        min_quality_threshold: 0.8,
        monitoring_duration_minutes: 60,
      },
    };

    return {
      recommendation_id: recommendationId,
      rank,
      category,
      title,
      description,
      proposed_changes: proposedChanges,
      expected_impact: expectedImpact,
      complexity: proposedChanges.length > 1 ? 'medium' : 'low',
      risk: score > 0.8 ? 'low' : score > 0.6 ? 'medium' : 'high',
      confidence: score,
      evidence,
      rollout_strategy: rolloutStrategy,
    };
  }

  // --------------------------------------------------------------------------
  // Constraint Evaluation
  // --------------------------------------------------------------------------

  private evaluateConstraints(
    constraints: InputConstraint[],
    recommendations: OptimizationRecommendation[]
  ): EvaluatedConstraint[] {
    return constraints.map(constraint => {
      const satisfied = !recommendations.some(rec =>
        this.recommendationViolatesConstraint(rec, constraint)
      );

      return {
        constraint,
        satisfied,
        impact_description: satisfied
          ? 'All recommendations comply with this constraint'
          : 'Some recommendations were filtered due to this constraint',
      };
    });
  }

  private recommendationViolatesConstraint(
    rec: OptimizationRecommendation,
    constraint: InputConstraint
  ): boolean {
    switch (constraint.type) {
      case 'max_cost_increase_pct':
        return rec.expected_impact.cost_change_pct > Number(constraint.value);

      case 'min_quality_threshold':
        return (100 + rec.expected_impact.quality_change_pct) / 100 < Number(constraint.value);

      case 'max_latency_increase_ms':
        return rec.expected_impact.latency_change_pct > Number(constraint.value);

      default:
        return false;
    }
  }

  // --------------------------------------------------------------------------
  // Output Building
  // --------------------------------------------------------------------------

  private buildOutput(
    input: SelfOptimizingAgentInput,
    stateAssessment: StateAssessment,
    recommendations: OptimizationRecommendation[],
    evaluatedConstraints: EvaluatedConstraint[],
    startTime: number
  ): SelfOptimizingAgentOutput {
    const timestamp = new Date().toISOString();
    const outputId = `out-${AGENT_ID}-${Date.now()}`;

    const totalImprovement: ImprovementPotential = recommendations.reduce(
      (acc, rec) => ({
        cost_reduction_pct: acc.cost_reduction_pct + Math.abs(Math.min(0, rec.expected_impact.cost_change_pct)),
        latency_improvement_pct: acc.latency_improvement_pct + Math.abs(Math.min(0, rec.expected_impact.latency_change_pct)),
        quality_improvement_pct: acc.quality_improvement_pct + Math.max(0, rec.expected_impact.quality_change_pct),
      }),
      { cost_reduction_pct: 0, latency_improvement_pct: 0, quality_improvement_pct: 0 }
    );

    const summary: AnalysisSummary = {
      current_state: stateAssessment,
      opportunities_identified: recommendations.length,
      total_improvement_potential: totalImprovement,
      risk_level: recommendations.some(r => r.risk === 'high') ? 'high' :
                  recommendations.some(r => r.risk === 'medium') ? 'medium' : 'low',
    };

    const metadata: OutputMetadata = {
      processing_duration_ms: Date.now() - startTime,
      data_points_analyzed:
        Object.keys(input.telemetry.cost_metrics.by_model).length +
        Object.keys(input.telemetry.latency_metrics.by_model).length +
        input.telemetry.anomaly_signals.length +
        input.telemetry.routing_history.length,
      strategies_used: ['cost_optimization', 'latency_routing', 'anomaly_response'],
      warnings: [],
    };

    return {
      output_id: outputId,
      execution_ref: input.execution_ref,
      agent_version: AGENT_VERSION,
      timestamp,
      summary,
      recommendations,
      constraints_evaluated: evaluatedConstraints,
      metadata,
    };
  }

  // --------------------------------------------------------------------------
  // DecisionEvent Building
  // --------------------------------------------------------------------------

  private buildDecisionEvent(
    input: SelfOptimizingAgentInput,
    output: SelfOptimizingAgentOutput,
    inputsHash: string,
    evaluatedConstraints: EvaluatedConstraint[]
  ): DecisionEvent {
    const outputs: DecisionOutputs = {
      recommendations: output.recommendations,
    };

    const constraintsApplied: AppliedConstraint[] = evaluatedConstraints.map((ec, idx) => ({
      constraint_id: `constraint-${idx}`,
      type: ec.constraint.type,
      value: ec.constraint.value,
      hard: ec.constraint.hard,
      satisfied: ec.satisfied,
    }));

    // Calculate overall confidence (average of recommendation confidences)
    const overallConfidence = output.recommendations.length > 0
      ? output.recommendations.reduce((sum, r) => sum + r.confidence, 0) / output.recommendations.length
      : 0.5;

    return {
      agent_id: AGENT_ID,
      agent_version: AGENT_VERSION,
      decision_type: DECISION_TYPE,
      inputs_hash: inputsHash,
      outputs,
      confidence: Math.round(overallConfidence * 100) / 100,
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
    output: SelfOptimizingAgentOutput,
    durationMs: number
  ): Promise<void> {
    if (!this.config.telemetryEndpoint) return;

    const telemetry = {
      agent_id: AGENT_ID,
      agent_version: AGENT_VERSION,
      execution_ref: decisionEvent.execution_ref,
      metrics: {
        processing_duration_ms: durationMs,
        recommendations_generated: output.recommendations.length,
        overall_confidence: decisionEvent.confidence,
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
      console.error('[SelfOptimizingAgent] Telemetry emission failed:', error);
    }
  }
}

// ============================================================================
// Internal Types
// ============================================================================

interface OptimizationOpportunity {
  type: 'model_substitution' | 'latency_routing' | 'anomaly_response';
  source_model?: string;
  target_model?: string;
  cost_savings_pct?: number;
  cost_impact_pct?: number;
  quality_impact_pct?: number;
  latency_improvement_pct?: number;
  affected_requests?: number;
  anomaly_id?: string;
  anomaly_type?: string;
  affected_entities?: string[];
  urgency?: 'low' | 'medium' | 'high' | 'critical';
}

// ============================================================================
// Factory Function
// ============================================================================

/**
 * Create a Self-Optimizing Agent instance.
 */
export function createSelfOptimizingAgent(
  config?: SelfOptimizingAgentConfig
): SelfOptimizingAgent {
  return new SelfOptimizingAgent(config);
}
