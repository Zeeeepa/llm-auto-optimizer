/**
 * Token Optimization Agent - Type Definitions
 *
 * Token-specific schemas for analyzing and optimizing token usage patterns
 * across LLM operations without degrading output quality.
 *
 * @module agents/token-optimization-agent/types
 */

// ============================================================================
// Token Analysis Types
// ============================================================================

/**
 * Token usage pattern identified in telemetry data.
 */
export interface TokenUsagePattern {
  /** Pattern identifier */
  pattern_id: string;
  /** Pattern category */
  category: TokenPatternCategory;
  /** Description of the pattern */
  description: string;
  /** Frequency of occurrence (0.0 - 1.0) */
  frequency: number;
  /** Token waste associated with this pattern */
  token_waste: TokenWaste;
  /** Affected prompts/requests */
  affected_count: number;
  /** Example instances (anonymized) */
  examples?: TokenPatternExample[];
}

export type TokenPatternCategory =
  | 'redundant_context'      // Same context repeated across requests
  | 'verbose_instructions'   // Instructions that can be condensed
  | 'unnecessary_formatting' // Excess whitespace, markdown, etc.
  | 'duplicate_examples'     // Same examples repeated
  | 'over_specification'     // More detail than needed for task
  | 'inefficient_structure'  // Poor prompt structure increasing tokens
  | 'template_bloat'         // Template overhead in structured prompts
  | 'response_padding'       // Model generating unnecessary padding
  | 'repetitive_output'      // Model repeating content unnecessarily
  | 'suboptimal_encoding';   // Inefficient token encoding patterns

export interface TokenWaste {
  /** Average wasted input tokens per occurrence */
  avg_input_tokens: number;
  /** Average wasted output tokens per occurrence */
  avg_output_tokens: number;
  /** Total wasted tokens in analysis window */
  total_tokens: number;
  /** Estimated cost of waste in USD */
  cost_usd: number;
  /** Waste as percentage of total usage */
  waste_pct: number;
}

export interface TokenPatternExample {
  /** Anonymized snippet showing the pattern */
  snippet: string;
  /** Token count for this example */
  token_count: number;
  /** Optimized version (if applicable) */
  optimized_snippet?: string;
  /** Token count after optimization */
  optimized_token_count?: number;
}

// ============================================================================
// Token Efficiency Metrics
// ============================================================================

/**
 * Comprehensive token efficiency metrics for a service/model.
 */
export interface TokenEfficiencyMetrics {
  /** Service or model identifier */
  entity_id: string;
  /** Entity type */
  entity_type: 'service' | 'model' | 'endpoint' | 'use_case';
  /** Time window for metrics */
  time_window: {
    start: string;
    end: string;
  };
  /** Input token metrics */
  input_metrics: TokenDirectionMetrics;
  /** Output token metrics */
  output_metrics: TokenDirectionMetrics;
  /** Efficiency scores */
  efficiency: TokenEfficiencyScores;
  /** Comparison with benchmarks */
  benchmark_comparison: TokenBenchmarkComparison;
}

export interface TokenDirectionMetrics {
  /** Total tokens consumed/generated */
  total_tokens: number;
  /** Average tokens per request */
  avg_per_request: number;
  /** P50 tokens per request */
  p50_per_request: number;
  /** P95 tokens per request */
  p95_per_request: number;
  /** P99 tokens per request */
  p99_per_request: number;
  /** Max tokens observed */
  max_observed: number;
  /** Token distribution by bucket */
  distribution: TokenDistribution;
}

export interface TokenDistribution {
  /** Percentage of requests in each bucket */
  buckets: {
    range: string;  // e.g., "0-100", "101-500"
    count: number;
    percentage: number;
  }[];
}

export interface TokenEfficiencyScores {
  /** Overall efficiency score (0.0 - 1.0) */
  overall: number;
  /** Input efficiency score */
  input_efficiency: number;
  /** Output efficiency score */
  output_efficiency: number;
  /** Context utilization (how well context is used) */
  context_utilization: number;
  /** Compression potential (how much can be optimized) */
  compression_potential: number;
}

export interface TokenBenchmarkComparison {
  /** Benchmark source */
  source: string;
  /** Input tokens vs benchmark */
  input_vs_benchmark_pct: number;
  /** Output tokens vs benchmark */
  output_vs_benchmark_pct: number;
  /** Overall vs benchmark */
  overall_vs_benchmark_pct: number;
  /** Industry percentile */
  industry_percentile: number;
}

// ============================================================================
// Token Optimization Opportunity Types
// ============================================================================

/**
 * Identified opportunity for token optimization.
 */
export interface TokenOptimizationOpportunity {
  /** Opportunity identifier */
  opportunity_id: string;
  /** Opportunity type */
  type: TokenOptimizationType;
  /** Priority score (0.0 - 1.0) */
  priority: number;
  /** Estimated token savings per request */
  savings_per_request: TokenSavings;
  /** Total estimated savings */
  total_savings: TokenSavings;
  /** Quality impact assessment */
  quality_impact: QualityImpactAssessment;
  /** Implementation complexity */
  complexity: 'trivial' | 'low' | 'medium' | 'high';
  /** Specific actions to implement */
  actions: TokenOptimizationAction[];
  /** Supporting evidence */
  evidence: TokenOptimizationEvidence[];
}

export type TokenOptimizationType =
  | 'prompt_compression'      // Compress/condense prompts
  | 'context_caching'         // Cache repeated context
  | 'template_optimization'   // Optimize prompt templates
  | 'response_trimming'       // Reduce response verbosity
  | 'batching'               // Batch similar requests
  | 'model_downgrade'        // Use smaller model for task
  | 'encoding_optimization'  // Optimize token encoding
  | 'instruction_tuning'     // Tune system instructions
  | 'example_reduction'      // Reduce few-shot examples
  | 'structured_output';     // Use structured output format

export interface TokenSavings {
  /** Input tokens saved */
  input_tokens: number;
  /** Output tokens saved */
  output_tokens: number;
  /** Total tokens saved */
  total_tokens: number;
  /** Cost savings in USD */
  cost_usd: number;
  /** Savings as percentage */
  savings_pct: number;
}

export interface QualityImpactAssessment {
  /** Expected quality change (-1.0 to 1.0) */
  expected_change: number;
  /** Confidence in assessment (0.0 - 1.0) */
  confidence: number;
  /** Risk of quality degradation */
  risk_level: 'none' | 'low' | 'medium' | 'high';
  /** Specific quality metrics affected */
  affected_metrics: string[];
  /** Mitigation strategies */
  mitigations?: string[];
}

export interface TokenOptimizationAction {
  /** Action identifier */
  action_id: string;
  /** Action type */
  type: 'modify' | 'add' | 'remove' | 'replace';
  /** Target component */
  target: 'prompt_template' | 'system_instruction' | 'examples' | 'response_format' | 'model_config';
  /** Description of action */
  description: string;
  /** Current value/state */
  current?: string | object;
  /** Proposed value/state */
  proposed: string | object;
  /** Expected token impact */
  token_impact: {
    input_delta: number;
    output_delta: number;
  };
}

export interface TokenOptimizationEvidence {
  /** Evidence type */
  type: 'statistical' | 'comparative' | 'experimental' | 'benchmark';
  /** Source of evidence */
  source: string;
  /** Evidence description */
  description: string;
  /** Confidence score (0.0 - 1.0) */
  confidence: number;
  /** Supporting data points */
  data: Record<string, unknown>;
}

// ============================================================================
// Token Optimization Input/Output Extensions
// ============================================================================

/**
 * Extended input for token optimization analysis.
 * Extends SelfOptimizingAgentInput with token-specific fields.
 */
export interface TokenOptimizationInput {
  /** Unique execution reference for tracing */
  execution_ref: string;
  /** Time window for analysis */
  time_window: {
    start: string;
    end: string;
    granularity: 'minute' | 'hour' | 'day';
  };
  /** Services to analyze */
  target_services: string[];
  /** Constraints for optimization */
  constraints: TokenOptimizationConstraint[];
  /** Token usage telemetry */
  token_telemetry: TokenTelemetryData;
  /** Analysis depth */
  analysis_depth: 'shallow' | 'standard' | 'deep';
  /** Focus areas for optimization */
  focus_areas?: TokenPatternCategory[];
  /** Quality preservation threshold (0.0 - 1.0) */
  quality_threshold: number;
  /** Maximum acceptable latency increase (ms) */
  max_latency_increase_ms?: number;
}

export interface TokenOptimizationConstraint {
  /** Constraint type */
  type: TokenConstraintType;
  /** Constraint value */
  value: string | number | boolean;
  /** Hard constraint (blocking) vs soft (preference) */
  hard: boolean;
}

export type TokenConstraintType =
  | 'min_quality_score'
  | 'max_latency_increase_pct'
  | 'min_savings_threshold_pct'
  | 'allowed_optimization_types'
  | 'excluded_services'
  | 'max_prompt_change_pct'
  | 'preserve_examples'
  | 'preserve_formatting';

/**
 * Token telemetry data from LLM-Observatory and Latency-Lens.
 */
export interface TokenTelemetryData {
  /** Token usage by model */
  by_model: Record<string, ModelTokenMetrics>;
  /** Token usage by service */
  by_service: Record<string, ServiceTokenMetrics>;
  /** Token usage by use case/endpoint */
  by_use_case?: Record<string, UseCaseTokenMetrics>;
  /** Aggregate metrics */
  aggregate: AggregateTokenMetrics;
  /** Detected patterns (from previous analysis) */
  detected_patterns?: TokenUsagePattern[];
  /** Historical comparison */
  historical?: TokenHistoricalData;
}

export interface ModelTokenMetrics {
  /** Model identifier */
  model_id: string;
  /** Total requests */
  request_count: number;
  /** Total input tokens */
  total_input_tokens: number;
  /** Total output tokens */
  total_output_tokens: number;
  /** Average input tokens */
  avg_input_tokens: number;
  /** Average output tokens */
  avg_output_tokens: number;
  /** Input/output ratio */
  io_ratio: number;
  /** Total cost */
  total_cost_usd: number;
  /** Cost per 1K tokens (input) */
  cost_per_1k_input: number;
  /** Cost per 1K tokens (output) */
  cost_per_1k_output: number;
}

export interface ServiceTokenMetrics {
  /** Service identifier */
  service_id: string;
  /** Total requests */
  request_count: number;
  /** Total tokens (input + output) */
  total_tokens: number;
  /** Average tokens per request */
  avg_tokens_per_request: number;
  /** Token efficiency score */
  efficiency_score: number;
  /** Primary model used */
  primary_model: string;
  /** Models used */
  models_used: string[];
}

export interface UseCaseTokenMetrics {
  /** Use case identifier */
  use_case_id: string;
  /** Description */
  description?: string;
  /** Total requests */
  request_count: number;
  /** Average input tokens */
  avg_input_tokens: number;
  /** Average output tokens */
  avg_output_tokens: number;
  /** Quality score */
  quality_score: number;
  /** Optimization potential */
  optimization_potential: number;
}

export interface AggregateTokenMetrics {
  /** Total requests */
  total_requests: number;
  /** Total input tokens */
  total_input_tokens: number;
  /** Total output tokens */
  total_output_tokens: number;
  /** Total cost */
  total_cost_usd: number;
  /** Average input tokens per request */
  avg_input_per_request: number;
  /** Average output tokens per request */
  avg_output_per_request: number;
  /** Overall efficiency score */
  efficiency_score: number;
  /** Daily token trend */
  daily_trend: {
    date: string;
    input_tokens: number;
    output_tokens: number;
    cost_usd: number;
  }[];
}

export interface TokenHistoricalData {
  /** Comparison period */
  comparison_period: 'previous_day' | 'previous_week' | 'previous_month';
  /** Token change percentage */
  token_change_pct: number;
  /** Cost change percentage */
  cost_change_pct: number;
  /** Efficiency change */
  efficiency_change: number;
}

// ============================================================================
// Token Optimization Output
// ============================================================================

/**
 * Output from Token Optimization Agent.
 */
export interface TokenOptimizationOutput {
  /** Unique output identifier */
  output_id: string;
  /** Execution reference (matches input) */
  execution_ref: string;
  /** Agent version */
  agent_version: string;
  /** Output timestamp */
  timestamp: string;
  /** Analysis summary */
  analysis: TokenAnalysisSummary;
  /** Identified patterns */
  patterns: TokenUsagePattern[];
  /** Optimization opportunities (ranked) */
  opportunities: TokenOptimizationOpportunity[];
  /** Constraints evaluated */
  constraints_evaluated: EvaluatedTokenConstraint[];
  /** Metadata */
  metadata: TokenOptimizationMetadata;
}

export interface TokenAnalysisSummary {
  /** Current token efficiency state */
  current_state: {
    total_tokens_analyzed: number;
    total_cost_analyzed_usd: number;
    overall_efficiency: number;
    input_efficiency: number;
    output_efficiency: number;
  };
  /** Patterns identified count */
  patterns_identified: number;
  /** Opportunities identified count */
  opportunities_identified: number;
  /** Total potential savings */
  total_potential_savings: TokenSavings;
  /** Risk assessment */
  risk_level: 'low' | 'medium' | 'high';
  /** Key findings */
  key_findings: string[];
}

export interface EvaluatedTokenConstraint {
  /** Original constraint */
  constraint: TokenOptimizationConstraint;
  /** Whether satisfied */
  satisfied: boolean;
  /** Actual value observed */
  actual_value?: string | number | boolean;
  /** Impact on opportunities */
  impact_description?: string;
}

export interface TokenOptimizationMetadata {
  /** Processing duration in ms */
  processing_duration_ms: number;
  /** Number of data points analyzed */
  data_points_analyzed: number;
  /** Analysis strategies used */
  strategies_used: string[];
  /** Warnings generated */
  warnings: string[];
  /** Analysis coverage */
  coverage: {
    services_analyzed: number;
    models_analyzed: number;
    requests_sampled: number;
    time_range_hours: number;
  };
}

// ============================================================================
// Token Model Reference Data
// ============================================================================

/**
 * Reference data for token pricing and characteristics.
 */
export interface TokenModelReference {
  /** Model identifier */
  model_id: string;
  /** Provider */
  provider: string;
  /** Input cost per 1K tokens */
  input_cost_per_1k: number;
  /** Output cost per 1K tokens */
  output_cost_per_1k: number;
  /** Context window size */
  context_window: number;
  /** Max output tokens */
  max_output_tokens: number;
  /** Typical tokens per word */
  tokens_per_word: number;
  /** Quality benchmark (0.0 - 1.0) */
  quality_benchmark: number;
  /** Latency per 1K output tokens (ms) */
  latency_per_1k_output_ms: number;
}

/**
 * Token model reference data for common models.
 */
export const TOKEN_MODEL_REFERENCE: Record<string, TokenModelReference> = {
  'claude-3-opus': {
    model_id: 'claude-3-opus',
    provider: 'anthropic',
    input_cost_per_1k: 0.015,
    output_cost_per_1k: 0.075,
    context_window: 200000,
    max_output_tokens: 4096,
    tokens_per_word: 1.3,
    quality_benchmark: 0.95,
    latency_per_1k_output_ms: 1500,
  },
  'claude-3-sonnet': {
    model_id: 'claude-3-sonnet',
    provider: 'anthropic',
    input_cost_per_1k: 0.003,
    output_cost_per_1k: 0.015,
    context_window: 200000,
    max_output_tokens: 4096,
    tokens_per_word: 1.3,
    quality_benchmark: 0.90,
    latency_per_1k_output_ms: 800,
  },
  'claude-3-haiku': {
    model_id: 'claude-3-haiku',
    provider: 'anthropic',
    input_cost_per_1k: 0.00025,
    output_cost_per_1k: 0.00125,
    context_window: 200000,
    max_output_tokens: 4096,
    tokens_per_word: 1.3,
    quality_benchmark: 0.82,
    latency_per_1k_output_ms: 300,
  },
  'gpt-4': {
    model_id: 'gpt-4',
    provider: 'openai',
    input_cost_per_1k: 0.03,
    output_cost_per_1k: 0.06,
    context_window: 8192,
    max_output_tokens: 4096,
    tokens_per_word: 1.3,
    quality_benchmark: 0.93,
    latency_per_1k_output_ms: 1200,
  },
  'gpt-4-turbo': {
    model_id: 'gpt-4-turbo',
    provider: 'openai',
    input_cost_per_1k: 0.01,
    output_cost_per_1k: 0.03,
    context_window: 128000,
    max_output_tokens: 4096,
    tokens_per_word: 1.3,
    quality_benchmark: 0.91,
    latency_per_1k_output_ms: 600,
  },
  'gpt-3.5-turbo': {
    model_id: 'gpt-3.5-turbo',
    provider: 'openai',
    input_cost_per_1k: 0.0005,
    output_cost_per_1k: 0.0015,
    context_window: 16384,
    max_output_tokens: 4096,
    tokens_per_word: 1.3,
    quality_benchmark: 0.75,
    latency_per_1k_output_ms: 200,
  },
};
