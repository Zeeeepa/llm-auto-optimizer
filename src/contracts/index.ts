/**
 * Agentics Contracts - Schema Definitions for LLM-Auto-Optimizer Agents
 *
 * This module defines ALL schemas and optimization envelopes for the Auto-Optimizer
 * agent ecosystem. All agents MUST import schemas from this module exclusively.
 *
 * @module contracts
 */

// ============================================================================
// DecisionEvent Schema (MANDATORY for all agents)
// ============================================================================

/**
 * DecisionEvent - Core event schema that MUST be emitted by every agent invocation.
 * This is persisted to ruvector-service for governance and audit.
 */
export interface DecisionEvent {
  /** Unique agent identifier (e.g., "self-optimizing-agent") */
  agent_id: string;
  /** Semantic version of the agent (e.g., "1.0.0") */
  agent_version: string;
  /** Type of decision made by the agent */
  decision_type: DecisionType;
  /** SHA-256 hash of canonicalized inputs for reproducibility */
  inputs_hash: string;
  /** Agent outputs (varies by agent type) */
  outputs: DecisionOutputs;
  /** Confidence score for expected impact (0.0 - 1.0) */
  confidence: number;
  /** Constraints and guardrails applied during decision */
  constraints_applied: AppliedConstraint[];
  /** Reference to the execution context (trace/request ID) */
  execution_ref: string;
  /** UTC timestamp of decision emission */
  timestamp: string;
}

export type DecisionType =
  | 'system_optimization_recommendation'  // Self-Optimizing Agent
  | 'token_optimization_recommendation'   // Token Optimization Agent
  | 'token_optimization_analysis'         // Token Optimization Agent (analysis mode)
  | 'model_selection_recommendation'      // Model Selection Agent
  | 'model_selection_analysis'            // Model Selection Agent (analysis mode)
  | 'cost_optimization_analysis'
  | 'latency_optimization_analysis'
  | 'quality_optimization_analysis'
  | 'bounded_parameter_adjustment'
  | 'configuration_simulation';

export interface DecisionOutputs {
  /** Ranked recommendations (for recommendation agents) */
  recommendations?: OptimizationRecommendation[];
  /** Analysis results (for analysis agents) */
  analysis?: AnalysisResult;
  /** Simulation results (for simulation mode) */
  simulation?: SimulationResult;
}

export interface AppliedConstraint {
  /** Constraint identifier */
  constraint_id: string;
  /** Constraint type */
  type: ConstraintType;
  /** Constraint value */
  value: string | number | boolean;
  /** Whether this was a hard (blocking) constraint */
  hard: boolean;
  /** Whether the constraint was satisfied */
  satisfied: boolean;
}

export type ConstraintType =
  | 'max_cost_increase_pct'
  | 'min_quality_threshold'
  | 'max_latency_increase_ms'
  | 'allowed_models'
  | 'allowed_providers'
  | 'budget_cap_daily'
  | 'sla_requirement'
  | 'compliance_policy';

// ============================================================================
// Self-Optimizing Agent Input Schema
// ============================================================================

/**
 * Input schema for the Self-Optimizing Agent.
 * All inputs MUST be validated against this schema before processing.
 */
export interface SelfOptimizingAgentInput {
  /** Unique execution reference for tracing */
  execution_ref: string;
  /** Time window for analysis (ISO 8601 duration or timestamps) */
  time_window: TimeWindow;
  /** Optimization target weights */
  optimization_targets: OptimizationTargets;
  /** Services to include in analysis */
  target_services: string[];
  /** Hard constraints that must be respected */
  constraints: InputConstraint[];
  /** Historical telemetry data */
  telemetry: TelemetryData;
  /** Optional: Previous recommendations for comparison */
  previous_recommendations?: string[];
}

export interface TimeWindow {
  /** Start of analysis window (ISO 8601) */
  start: string;
  /** End of analysis window (ISO 8601) */
  end: string;
  /** Granularity for aggregation */
  granularity: 'minute' | 'hour' | 'day';
}

export interface OptimizationTargets {
  /** Weight for cost optimization (0.0 - 1.0) */
  cost_weight: number;
  /** Weight for latency optimization (0.0 - 1.0) */
  latency_weight: number;
  /** Weight for quality optimization (0.0 - 1.0) */
  quality_weight: number;
}

export interface InputConstraint {
  /** Constraint type */
  type: ConstraintType;
  /** Constraint value */
  value: string | number | boolean;
  /** Hard constraint (blocking) vs soft (preference) */
  hard: boolean;
}

export interface TelemetryData {
  /** Cost metrics from LLM-Cost-Ops */
  cost_metrics: CostMetrics;
  /** Latency metrics from Latency-Lens */
  latency_metrics: LatencyMetrics;
  /** Quality metrics from Observatory */
  quality_metrics: QualityMetrics;
  /** Anomaly signals from Sentinel */
  anomaly_signals: AnomalySignal[];
  /** Historical routing decisions from Router-L2 */
  routing_history: RoutingHistoryEntry[];
}

export interface CostMetrics {
  /** Total cost in the window */
  total_cost_usd: number;
  /** Cost breakdown by model */
  by_model: Record<string, ModelCostMetrics>;
  /** Cost breakdown by provider */
  by_provider: Record<string, number>;
  /** Daily cost trend */
  daily_trend: number[];
}

export interface ModelCostMetrics {
  /** Total cost for this model */
  cost_usd: number;
  /** Number of requests */
  request_count: number;
  /** Average cost per request */
  avg_cost_per_request: number;
  /** Input tokens consumed */
  input_tokens: number;
  /** Output tokens generated */
  output_tokens: number;
}

export interface LatencyMetrics {
  /** P50 latency in milliseconds */
  p50_ms: number;
  /** P95 latency in milliseconds */
  p95_ms: number;
  /** P99 latency in milliseconds */
  p99_ms: number;
  /** Average latency in milliseconds */
  avg_ms: number;
  /** Latency by model */
  by_model: Record<string, ModelLatencyMetrics>;
}

export interface ModelLatencyMetrics {
  /** P50 latency */
  p50_ms: number;
  /** P95 latency */
  p95_ms: number;
  /** P99 latency */
  p99_ms: number;
  /** Request count */
  request_count: number;
  /** Timeout rate */
  timeout_rate: number;
}

export interface QualityMetrics {
  /** Overall quality score (0.0 - 1.0) */
  overall_score: number;
  /** Quality by model */
  by_model: Record<string, ModelQualityMetrics>;
  /** User satisfaction metrics */
  user_satisfaction: UserSatisfactionMetrics;
}

export interface ModelQualityMetrics {
  /** Quality score (0.0 - 1.0) */
  score: number;
  /** Number of quality samples */
  sample_count: number;
  /** Error rate */
  error_rate: number;
  /** Hallucination rate (if tracked) */
  hallucination_rate?: number;
}

export interface UserSatisfactionMetrics {
  /** Average rating (1-5) */
  avg_rating: number;
  /** Number of ratings */
  rating_count: number;
  /** Thumbs up/down ratio */
  positive_ratio: number;
}

export interface AnomalySignal {
  /** Anomaly identifier */
  anomaly_id: string;
  /** Detection timestamp */
  timestamp: string;
  /** Anomaly type */
  type: 'latency_spike' | 'cost_spike' | 'quality_drop' | 'error_spike' | 'drift';
  /** Severity level */
  severity: 'low' | 'medium' | 'high' | 'critical';
  /** Affected models/services */
  affected_entities: string[];
  /** Anomaly details */
  details: Record<string, unknown>;
}

export interface RoutingHistoryEntry {
  /** Routing decision ID */
  decision_id: string;
  /** Timestamp */
  timestamp: string;
  /** Selected model */
  model: string;
  /** Selection reason */
  reason: string;
  /** Outcome metrics */
  outcome: {
    latency_ms: number;
    cost_usd: number;
    quality_score?: number;
  };
}

// ============================================================================
// Self-Optimizing Agent Output Schema
// ============================================================================

/**
 * Output schema for the Self-Optimizing Agent.
 * All outputs MUST conform to this schema.
 */
export interface SelfOptimizingAgentOutput {
  /** Unique output identifier */
  output_id: string;
  /** Execution reference (matches input) */
  execution_ref: string;
  /** Agent version */
  agent_version: string;
  /** Output timestamp */
  timestamp: string;
  /** Analysis summary */
  summary: AnalysisSummary;
  /** Ranked optimization recommendations */
  recommendations: OptimizationRecommendation[];
  /** Constraints that were applied/evaluated */
  constraints_evaluated: EvaluatedConstraint[];
  /** Metadata for debugging/audit */
  metadata: OutputMetadata;
}

export interface AnalysisSummary {
  /** Current state assessment */
  current_state: StateAssessment;
  /** Identified optimization opportunities */
  opportunities_identified: number;
  /** Estimated total improvement potential */
  total_improvement_potential: ImprovementPotential;
  /** Risk assessment */
  risk_level: 'low' | 'medium' | 'high';
}

export interface StateAssessment {
  /** Cost efficiency score (0.0 - 1.0) */
  cost_efficiency: number;
  /** Latency efficiency score (0.0 - 1.0) */
  latency_efficiency: number;
  /** Quality score (0.0 - 1.0) */
  quality_score: number;
  /** Overall health score (0.0 - 1.0) */
  overall_health: number;
}

export interface ImprovementPotential {
  /** Potential cost reduction percentage */
  cost_reduction_pct: number;
  /** Potential latency improvement percentage */
  latency_improvement_pct: number;
  /** Potential quality improvement */
  quality_improvement_pct: number;
}

export interface OptimizationRecommendation {
  /** Unique recommendation identifier */
  recommendation_id: string;
  /** Recommendation rank (1 = highest priority) */
  rank: number;
  /** Recommendation category */
  category: RecommendationCategory;
  /** Human-readable title */
  title: string;
  /** Detailed description */
  description: string;
  /** Proposed configuration changes */
  proposed_changes: ProposedChange[];
  /** Expected impact */
  expected_impact: ExpectedImpact;
  /** Implementation complexity */
  complexity: 'low' | 'medium' | 'high';
  /** Risk level */
  risk: 'low' | 'medium' | 'high';
  /** Confidence in this recommendation (0.0 - 1.0) */
  confidence: number;
  /** Supporting evidence */
  evidence: Evidence[];
  /** Suggested rollout strategy */
  rollout_strategy: RolloutStrategy;
}

export type RecommendationCategory =
  | 'model_substitution'
  | 'parameter_tuning'
  | 'load_balancing'
  | 'caching_strategy'
  | 'request_batching'
  | 'prompt_optimization'
  | 'token_reduction'
  | 'context_optimization'
  | 'response_optimization'
  | 'traffic_routing'
  | 'cost_optimization'
  | 'quality_improvement';

export interface ProposedChange {
  /** Change type */
  type: 'replace' | 'add' | 'remove' | 'update';
  /** Parameter being changed */
  parameter: string;
  /** Current value (if applicable) */
  current_value?: unknown;
  /** Proposed new value */
  proposed_value: unknown;
  /** Affected services */
  affected_services: string[];
}

export interface ExpectedImpact {
  /** Expected cost change (percentage, negative = reduction) */
  cost_change_pct: number;
  /** Expected latency change (percentage, negative = improvement) */
  latency_change_pct: number;
  /** Expected quality change (percentage, positive = improvement) */
  quality_change_pct: number;
  /** Monthly cost savings in USD */
  monthly_savings_usd?: number;
  /** Impact confidence (0.0 - 1.0) */
  confidence: number;
}

export interface Evidence {
  /** Evidence type */
  type: 'historical_data' | 'benchmark' | 'a_b_test' | 'simulation' | 'industry_standard';
  /** Evidence source */
  source: string;
  /** Supporting data points */
  data_points: Record<string, unknown>;
  /** Evidence strength (0.0 - 1.0) */
  strength: number;
}

export interface RolloutStrategy {
  /** Recommended strategy type */
  type: 'immediate' | 'canary' | 'gradual' | 'scheduled';
  /** Canary percentage (if canary) */
  canary_percentage?: number;
  /** Gradual rollout phases */
  phases?: RolloutPhase[];
  /** Recommended schedule (if scheduled) */
  scheduled_time?: string;
  /** Rollback criteria */
  rollback_criteria: RollbackCriteria;
}

export interface RolloutPhase {
  /** Phase name */
  name: string;
  /** Traffic percentage */
  percentage: number;
  /** Duration before next phase */
  duration_hours: number;
}

export interface RollbackCriteria {
  /** Maximum error rate increase before rollback */
  max_error_rate_increase_pct: number;
  /** Maximum latency increase before rollback */
  max_latency_increase_pct: number;
  /** Minimum quality threshold */
  min_quality_threshold: number;
  /** Monitoring duration in minutes */
  monitoring_duration_minutes: number;
}

export interface EvaluatedConstraint {
  /** Constraint from input */
  constraint: InputConstraint;
  /** Whether it was satisfied */
  satisfied: boolean;
  /** Actual value observed */
  actual_value?: string | number | boolean;
  /** Impact on recommendations */
  impact_description?: string;
}

export interface OutputMetadata {
  /** Processing duration in milliseconds */
  processing_duration_ms: number;
  /** Number of data points analyzed */
  data_points_analyzed: number;
  /** Algorithms/strategies used */
  strategies_used: string[];
  /** Any warnings generated */
  warnings: string[];
}

// ============================================================================
// Analysis Result Schema (for analysis mode)
// ============================================================================

export interface AnalysisResult {
  /** Analysis type */
  analysis_type: 'cost' | 'latency' | 'quality' | 'holistic';
  /** Analysis depth */
  depth: 'shallow' | 'medium' | 'deep';
  /** Key findings */
  findings: AnalysisFinding[];
  /** Trend analysis */
  trends: TrendAnalysis;
  /** Comparison with benchmarks */
  benchmark_comparison?: BenchmarkComparison;
}

export interface AnalysisFinding {
  /** Finding ID */
  finding_id: string;
  /** Finding category */
  category: string;
  /** Severity */
  severity: 'info' | 'warning' | 'critical';
  /** Description */
  description: string;
  /** Supporting metrics */
  metrics: Record<string, number>;
}

export interface TrendAnalysis {
  /** Cost trend direction */
  cost_trend: 'increasing' | 'stable' | 'decreasing';
  /** Latency trend direction */
  latency_trend: 'increasing' | 'stable' | 'decreasing';
  /** Quality trend direction */
  quality_trend: 'improving' | 'stable' | 'degrading';
  /** Trend confidence */
  confidence: number;
}

export interface BenchmarkComparison {
  /** Industry benchmark source */
  benchmark_source: string;
  /** Cost vs benchmark (percentage difference) */
  cost_vs_benchmark_pct: number;
  /** Latency vs benchmark */
  latency_vs_benchmark_pct: number;
  /** Quality vs benchmark */
  quality_vs_benchmark_pct: number;
}

// ============================================================================
// Simulation Result Schema (for simulate mode)
// ============================================================================

export interface SimulationResult {
  /** Simulation ID */
  simulation_id: string;
  /** Simulated scenarios */
  scenarios: SimulatedScenario[];
  /** Optimal scenario based on targets */
  optimal_scenario_id: string;
  /** Comparison matrix */
  comparison_matrix: ScenarioComparison[];
}

export interface SimulatedScenario {
  /** Scenario ID */
  scenario_id: string;
  /** Scenario name */
  name: string;
  /** Applied changes */
  changes: ProposedChange[];
  /** Projected metrics */
  projected_metrics: ProjectedMetrics;
  /** Risk assessment */
  risk_score: number;
}

export interface ProjectedMetrics {
  /** Projected cost */
  projected_cost_usd: number;
  /** Projected latency P95 */
  projected_latency_p95_ms: number;
  /** Projected quality score */
  projected_quality_score: number;
  /** Projection confidence */
  confidence: number;
}

export interface ScenarioComparison {
  /** Scenario A ID */
  scenario_a: string;
  /** Scenario B ID */
  scenario_b: string;
  /** Cost difference */
  cost_diff_pct: number;
  /** Latency difference */
  latency_diff_pct: number;
  /** Quality difference */
  quality_diff_pct: number;
  /** Recommended choice */
  recommendation: 'scenario_a' | 'scenario_b' | 'either';
}

// ============================================================================
// CLI Invocation Schema
// ============================================================================

export interface CLIInvocation {
  /** Command to execute */
  command: 'analyze' | 'recommend' | 'simulate';
  /** Input parameters */
  params: CLIParams;
  /** Output format */
  output_format: 'json' | 'yaml' | 'table' | 'summary';
}

export interface CLIParams {
  /** Time window (e.g., "24h", "7d") */
  window?: string;
  /** Target services (comma-separated) */
  services?: string;
  /** Optimization focus */
  focus?: 'cost' | 'latency' | 'quality' | 'balanced';
  /** Constraint flags */
  max_cost_increase?: number;
  min_quality?: number;
  max_latency_increase?: number;
  /** Verbose output */
  verbose?: boolean;
  /** Dry run (for simulate) */
  dry_run?: boolean;
}

// ============================================================================
// Validation Functions
// ============================================================================

/**
 * Validates SelfOptimizingAgentInput against schema requirements.
 * @throws Error if validation fails
 */
export function validateInput(input: unknown): asserts input is SelfOptimizingAgentInput {
  if (!input || typeof input !== 'object') {
    throw new Error('Invalid input: must be an object');
  }

  const obj = input as Record<string, unknown>;

  // Required fields
  const requiredFields = [
    'execution_ref',
    'time_window',
    'optimization_targets',
    'target_services',
    'constraints',
    'telemetry'
  ];

  for (const field of requiredFields) {
    if (!(field in obj)) {
      throw new Error(`Invalid input: missing required field '${field}'`);
    }
  }

  // Validate optimization targets sum to ~1.0
  const targets = obj.optimization_targets as OptimizationTargets;
  const totalWeight = targets.cost_weight + targets.latency_weight + targets.quality_weight;
  if (Math.abs(totalWeight - 1.0) > 0.01) {
    throw new Error('Invalid input: optimization target weights must sum to 1.0');
  }

  // Validate time window
  const window = obj.time_window as TimeWindow;
  const start = new Date(window.start);
  const end = new Date(window.end);
  if (isNaN(start.getTime()) || isNaN(end.getTime())) {
    throw new Error('Invalid input: time_window dates must be valid ISO 8601');
  }
  if (end <= start) {
    throw new Error('Invalid input: time_window end must be after start');
  }
}

/**
 * Validates SelfOptimizingAgentOutput against schema requirements.
 * @throws Error if validation fails
 */
export function validateOutput(output: unknown): asserts output is SelfOptimizingAgentOutput {
  if (!output || typeof output !== 'object') {
    throw new Error('Invalid output: must be an object');
  }

  const obj = output as Record<string, unknown>;

  // Required fields
  const requiredFields = [
    'output_id',
    'execution_ref',
    'agent_version',
    'timestamp',
    'summary',
    'recommendations',
    'constraints_evaluated',
    'metadata'
  ];

  for (const field of requiredFields) {
    if (!(field in obj)) {
      throw new Error(`Invalid output: missing required field '${field}'`);
    }
  }

  // Validate recommendations are properly ranked
  const recommendations = obj.recommendations as OptimizationRecommendation[];
  const ranks = recommendations.map(r => r.rank);
  const uniqueRanks = new Set(ranks);
  if (ranks.length !== uniqueRanks.size) {
    throw new Error('Invalid output: recommendation ranks must be unique');
  }
}

/**
 * Creates a canonicalized hash of inputs for reproducibility tracking.
 */
export function hashInputs(input: SelfOptimizingAgentInput | ModelSelectionAgentInput): string {
  const canonicalized = JSON.stringify(input, Object.keys(input).sort());

  // Use sync hash for simplicity (Edge Functions have crypto)
  // In production, use async SubtleCrypto
  const hashBuffer = require('crypto').createHash('sha256').update(canonicalized).digest();
  return hashBuffer.toString('hex');
}

// ============================================================================
// Model Selection Agent Input Schema
// ============================================================================

/**
 * Input schema for the Model Selection Agent.
 * All inputs MUST be validated against this schema before processing.
 */
export interface ModelSelectionAgentInput {
  /** Unique execution reference for tracing */
  execution_ref: string;
  /** Time window for analysis (ISO 8601 duration or timestamps) */
  time_window: TimeWindow;
  /** Task context for model selection */
  task_context: TaskContext;
  /** Target services to include in analysis */
  target_services: string[];
  /** Constraints that must be respected */
  constraints: ModelSelectionConstraint[];
  /** Historical performance data for models */
  performance_history: ModelPerformanceHistory;
  /** Current model configuration (optional) */
  current_model_config?: CurrentModelConfig;
}

/**
 * Context about the task requiring model selection.
 */
export interface TaskContext {
  /** Task type classification */
  task_type: TaskType;
  /** Complexity level of the task */
  complexity: 'simple' | 'moderate' | 'complex' | 'expert';
  /** Expected input token range */
  expected_input_tokens: TokenRange;
  /** Expected output token range */
  expected_output_tokens: TokenRange;
  /** Latency requirements */
  latency_requirements: LatencyRequirements;
  /** Quality requirements */
  quality_requirements: QualityRequirements;
  /** Budget constraints */
  budget_constraints: BudgetConstraints;
  /** Domain/use case context */
  domain?: string;
  /** Additional context metadata */
  metadata?: Record<string, unknown>;
}

/**
 * Task type classification for model selection routing.
 */
export type TaskType =
  | 'code_generation'
  | 'code_review'
  | 'text_generation'
  | 'text_summarization'
  | 'question_answering'
  | 'data_extraction'
  | 'classification'
  | 'translation'
  | 'analysis'
  | 'creative_writing'
  | 'reasoning'
  | 'multi_turn_conversation'
  | 'general';

/**
 * Token range specification.
 */
export interface TokenRange {
  /** Minimum expected tokens */
  min: number;
  /** Maximum expected tokens */
  max: number;
  /** Average expected tokens */
  avg?: number;
}

/**
 * Latency requirements for task execution.
 */
export interface LatencyRequirements {
  /** Maximum acceptable latency in milliseconds */
  max_latency_ms: number;
  /** Target latency in milliseconds (preferred) */
  target_latency_ms?: number;
  /** Whether latency is a hard constraint */
  strict: boolean;
}

/**
 * Quality requirements for task execution.
 */
export interface QualityRequirements {
  /** Minimum acceptable quality score (0-1) */
  min_quality_score: number;
  /** Target quality score (preferred) */
  target_quality_score?: number;
  /** Whether quality is a hard constraint */
  strict: boolean;
  /** Specific quality aspects to prioritize */
  priorities?: ('accuracy' | 'coherence' | 'relevance' | 'completeness' | 'safety')[];
}

/**
 * Budget constraints for model selection.
 */
export interface BudgetConstraints {
  /** Maximum cost per request in USD */
  max_cost_per_request_usd?: number;
  /** Maximum daily budget in USD */
  max_daily_budget_usd?: number;
  /** Maximum monthly budget in USD */
  max_monthly_budget_usd?: number;
  /** Optimize for cost (may accept lower quality) */
  cost_optimization_priority: 'high' | 'medium' | 'low';
}

/**
 * Constraint for model selection decisions.
 */
export interface ModelSelectionConstraint {
  /** Constraint type */
  type: ModelSelectionConstraintType;
  /** Constraint value */
  value: string | number | boolean | string[];
  /** Hard constraint (blocking) vs soft (preference) */
  hard: boolean;
}

/**
 * Types of constraints for model selection.
 */
export type ModelSelectionConstraintType =
  | 'allowed_models'
  | 'blocked_models'
  | 'allowed_providers'
  | 'blocked_providers'
  | 'max_cost_per_request'
  | 'min_quality_threshold'
  | 'max_latency_ms'
  | 'required_capabilities'
  | 'compliance_policy'
  | 'region_restriction';

/**
 * Historical performance data for model selection analysis.
 */
export interface ModelPerformanceHistory {
  /** Performance data by model */
  by_model: Record<string, ModelHistoricalMetrics>;
  /** Performance data by task type */
  by_task_type: Record<string, TaskTypePerformance>;
  /** Total requests analyzed */
  total_requests: number;
  /** Time window for historical data */
  data_window: TimeWindow;
}

/**
 * Historical performance metrics for a specific model.
 */
export interface ModelHistoricalMetrics {
  /** Model identifier */
  model_id: string;
  /** Provider */
  provider: string;
  /** Total requests processed */
  request_count: number;
  /** Average latency metrics */
  latency: {
    p50_ms: number;
    p95_ms: number;
    p99_ms: number;
    avg_ms: number;
  };
  /** Cost metrics */
  cost: {
    total_cost_usd: number;
    avg_cost_per_request_usd: number;
    input_cost_per_1k_tokens: number;
    output_cost_per_1k_tokens: number;
  };
  /** Quality metrics */
  quality: {
    avg_score: number;
    error_rate: number;
    success_rate: number;
    user_satisfaction?: number;
  };
  /** Token usage */
  tokens: {
    total_input: number;
    total_output: number;
    avg_input: number;
    avg_output: number;
  };
  /** Availability */
  availability: {
    uptime_pct: number;
    timeout_rate: number;
    rate_limit_hit_rate: number;
  };
}

/**
 * Performance data broken down by task type.
 */
export interface TaskTypePerformance {
  /** Task type */
  task_type: TaskType;
  /** Best performing models for this task type */
  top_models: Array<{
    model_id: string;
    score: number;
    request_count: number;
  }>;
  /** Average quality score for this task type */
  avg_quality_score: number;
  /** Average latency for this task type */
  avg_latency_ms: number;
  /** Average cost for this task type */
  avg_cost_usd: number;
}

/**
 * Current model configuration.
 */
export interface CurrentModelConfig {
  /** Currently configured default model */
  default_model: string;
  /** Routing rules in place */
  routing_rules?: Array<{
    condition: string;
    model: string;
  }>;
  /** Fallback model */
  fallback_model?: string;
}

// ============================================================================
// Model Selection Agent Output Schema
// ============================================================================

/**
 * Output schema for the Model Selection Agent.
 * All outputs MUST conform to this schema.
 */
export interface ModelSelectionAgentOutput {
  /** Unique output identifier */
  output_id: string;
  /** Execution reference (matches input) */
  execution_ref: string;
  /** Agent version */
  agent_version: string;
  /** Output timestamp */
  timestamp: string;
  /** Primary recommended model */
  primary_recommendation: ModelRecommendation;
  /** Alternative model recommendations (ranked) */
  alternative_recommendations: ModelRecommendation[];
  /** Analysis summary */
  analysis_summary: ModelSelectionAnalysis;
  /** Constraints that were applied/evaluated */
  constraints_evaluated: ModelSelectionConstraintEvaluation[];
  /** Metadata for debugging/audit */
  metadata: ModelSelectionMetadata;
}

/**
 * A single model recommendation.
 */
export interface ModelRecommendation {
  /** Recommended model ID */
  model_id: string;
  /** Provider */
  provider: string;
  /** Recommendation rank (1 = primary) */
  rank: number;
  /** Overall suitability score (0-1) */
  suitability_score: number;
  /** Confidence in this recommendation (0-1) */
  confidence: number;
  /** Why this model was recommended */
  reasoning: string;
  /** Detailed scores breakdown */
  score_breakdown: ScoreBreakdown;
  /** Expected performance */
  expected_performance: ExpectedPerformance;
  /** Supporting evidence */
  evidence: ModelSelectionEvidence[];
  /** Suggested configuration */
  suggested_config?: ModelConfig;
}

/**
 * Score breakdown for model recommendation.
 */
export interface ScoreBreakdown {
  /** Quality fit score */
  quality_score: number;
  /** Latency fit score */
  latency_score: number;
  /** Cost fit score */
  cost_score: number;
  /** Task type fit score */
  task_fit_score: number;
  /** Reliability score */
  reliability_score: number;
  /** Weights applied */
  weights: {
    quality: number;
    latency: number;
    cost: number;
    task_fit: number;
    reliability: number;
  };
}

/**
 * Expected performance metrics for a model.
 */
export interface ExpectedPerformance {
  /** Expected latency range */
  latency_ms: TokenRange;
  /** Expected quality score */
  quality_score: number;
  /** Expected cost per request */
  cost_per_request_usd: number;
  /** Expected success rate */
  success_rate: number;
  /** Confidence in projections */
  projection_confidence: number;
}

/**
 * Evidence supporting a model recommendation.
 */
export interface ModelSelectionEvidence {
  /** Evidence type */
  type: 'historical_performance' | 'benchmark' | 'task_similarity' | 'capability_match' | 'cost_analysis';
  /** Evidence source */
  source: string;
  /** Supporting data */
  data: Record<string, unknown>;
  /** Evidence strength (0-1) */
  strength: number;
}

/**
 * Suggested model configuration.
 */
export interface ModelConfig {
  /** Temperature setting */
  temperature?: number;
  /** Max tokens */
  max_tokens?: number;
  /** Top P */
  top_p?: number;
  /** Stop sequences */
  stop_sequences?: string[];
  /** System prompt suggestion */
  system_prompt_notes?: string;
}

/**
 * Analysis summary for model selection.
 */
export interface ModelSelectionAnalysis {
  /** Number of models evaluated */
  models_evaluated: number;
  /** Number of models passing constraints */
  models_passing_constraints: number;
  /** Top scoring model */
  top_model: string;
  /** Score difference between top and runner-up */
  score_margin: number;
  /** Key factors in the decision */
  key_factors: string[];
  /** Any warnings or considerations */
  warnings: string[];
  /** Task-model compatibility assessment */
  task_compatibility: 'excellent' | 'good' | 'adequate' | 'marginal';
  /** Confidence level */
  confidence_level: 'high' | 'medium' | 'low';
}

/**
 * Constraint evaluation result for model selection.
 */
export interface ModelSelectionConstraintEvaluation {
  /** Original constraint */
  constraint: ModelSelectionConstraint;
  /** Whether satisfied */
  satisfied: boolean;
  /** Number of models passing this constraint */
  models_passing: number;
  /** Number of models blocked by this constraint */
  models_blocked: number;
  /** Impact description */
  impact_description?: string;
}

/**
 * Metadata for model selection output.
 */
export interface ModelSelectionMetadata {
  /** Processing duration in milliseconds */
  processing_duration_ms: number;
  /** Number of data points analyzed */
  data_points_analyzed: number;
  /** Selection strategies used */
  strategies_used: string[];
  /** Any warnings generated */
  warnings: string[];
  /** Models considered (full list) */
  models_considered: string[];
}

// ============================================================================
// Model Selection Validation Functions
// ============================================================================

/**
 * Validates ModelSelectionAgentInput against schema requirements.
 * @throws Error if validation fails
 */
export function validateModelSelectionInput(input: unknown): asserts input is ModelSelectionAgentInput {
  if (!input || typeof input !== 'object') {
    throw new Error('Invalid input: must be an object');
  }

  const obj = input as Record<string, unknown>;

  // Required fields
  const requiredFields = [
    'execution_ref',
    'time_window',
    'task_context',
    'target_services',
    'constraints',
    'performance_history'
  ];

  for (const field of requiredFields) {
    if (!(field in obj)) {
      throw new Error(`Invalid input: missing required field '${field}'`);
    }
  }

  // Validate task context
  const context = obj.task_context as TaskContext;
  if (!context.task_type || !context.complexity || !context.expected_input_tokens ||
      !context.expected_output_tokens || !context.latency_requirements ||
      !context.quality_requirements || !context.budget_constraints) {
    throw new Error('Invalid input: task_context missing required fields');
  }

  // Validate time window
  const window = obj.time_window as TimeWindow;
  const start = new Date(window.start);
  const end = new Date(window.end);
  if (isNaN(start.getTime()) || isNaN(end.getTime())) {
    throw new Error('Invalid input: time_window dates must be valid ISO 8601');
  }
  if (end <= start) {
    throw new Error('Invalid input: time_window end must be after start');
  }
}

/**
 * Validates ModelSelectionAgentOutput against schema requirements.
 * @throws Error if validation fails
 */
export function validateModelSelectionOutput(output: unknown): asserts output is ModelSelectionAgentOutput {
  if (!output || typeof output !== 'object') {
    throw new Error('Invalid output: must be an object');
  }

  const obj = output as Record<string, unknown>;

  // Required fields
  const requiredFields = [
    'output_id',
    'execution_ref',
    'agent_version',
    'timestamp',
    'primary_recommendation',
    'alternative_recommendations',
    'analysis_summary',
    'constraints_evaluated',
    'metadata'
  ];

  for (const field of requiredFields) {
    if (!(field in obj)) {
      throw new Error(`Invalid output: missing required field '${field}'`);
    }
  }

  // Validate primary recommendation
  const primary = obj.primary_recommendation as ModelRecommendation;
  if (!primary.model_id || primary.suitability_score === undefined ||
      primary.confidence === undefined) {
    throw new Error('Invalid output: primary_recommendation missing required fields');
  }
}
