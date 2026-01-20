/**
 * Token Optimization Agent
 *
 * A RECOMMENDATION agent that analyzes token usage patterns and generates
 * strategies to reduce token consumption without degrading output quality.
 *
 * Classification: ANALYSIS / RECOMMENDATION
 * Decision Type: token_optimization_recommendation | token_optimization_analysis
 *
 * This agent:
 * - Analyzes prompt and response token patterns
 * - Identifies token usage inefficiencies
 * - Recommends bounded prompt or configuration adjustments
 * - Provides quality impact assessments for each recommendation
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
 * @module agents/token-optimization-agent
 * @version 1.0.0
 */

import {
  // DecisionEvent
  DecisionEvent,
  DecisionType,
  DecisionOutputs,
  AppliedConstraint,
  // Validation
  hashInputs,
  // Types
  OptimizationRecommendation,
  RecommendationCategory,
  ProposedChange,
  ExpectedImpact,
  Evidence,
  RolloutStrategy,
  EvaluatedConstraint,
  SelfOptimizingAgentInput,
} from '../../contracts';

import { RuVectorClient, createRuVectorClient } from '../../services/ruvector-client';

import {
  // Input/Output types
  TokenOptimizationInput,
  TokenOptimizationOutput,
  TokenOptimizationConstraint,
  TokenConstraintType,
  // Analysis types
  TokenUsagePattern,
  TokenPatternCategory,
  TokenWaste,
  TokenEfficiencyMetrics,
  TokenEfficiencyScores,
  // Opportunity types
  TokenOptimizationOpportunity,
  TokenOptimizationType,
  TokenSavings,
  QualityImpactAssessment,
  TokenOptimizationAction,
  TokenOptimizationEvidence,
  // Telemetry types
  TokenTelemetryData,
  ModelTokenMetrics,
  AggregateTokenMetrics,
  // Summary types
  TokenAnalysisSummary,
  EvaluatedTokenConstraint,
  TokenOptimizationMetadata,
  // Reference data
  TOKEN_MODEL_REFERENCE,
} from './types';

// ============================================================================
// Constants
// ============================================================================

export const AGENT_ID = 'token-optimization-agent';
export const AGENT_VERSION = '1.0.0';
export const DECISION_TYPE_RECOMMEND: DecisionType = 'token_optimization_recommendation';
export const DECISION_TYPE_ANALYZE: DecisionType = 'token_optimization_analysis';

// Pattern detection thresholds
const PATTERN_THRESHOLDS = {
  redundant_context_min_frequency: 0.1,     // 10% of requests have redundant context
  verbose_instructions_ratio: 0.3,           // 30% more tokens than necessary
  duplicate_examples_threshold: 0.2,         // 20% duplicated
  response_padding_threshold: 0.15,          // 15% padding in responses
  waste_significance_threshold: 100,         // Min 100 tokens to be significant
};

// Quality preservation defaults
const QUALITY_DEFAULTS = {
  min_quality_threshold: 0.85,               // 85% quality preservation
  max_quality_drop_pct: 5,                   // Max 5% quality drop
  high_risk_quality_threshold: 0.9,          // High-value tasks need 90%
};

// ============================================================================
// Agent Configuration
// ============================================================================

export interface TokenOptimizationAgentConfig {
  /** RuVector client for persistence */
  ruVectorClient?: RuVectorClient;
  /** Enable telemetry emission */
  enableTelemetry?: boolean;
  /** Telemetry endpoint */
  telemetryEndpoint?: string;
  /** Maximum opportunities to generate */
  maxOpportunities?: number;
  /** Minimum confidence threshold */
  minConfidenceThreshold?: number;
  /** Minimum savings to report (USD) */
  minSavingsThreshold?: number;
}

const DEFAULT_CONFIG: Required<TokenOptimizationAgentConfig> = {
  ruVectorClient: undefined as unknown as RuVectorClient,
  enableTelemetry: true,
  telemetryEndpoint: process.env.TELEMETRY_ENDPOINT || '',
  maxOpportunities: 15,
  minConfidenceThreshold: 0.6,
  minSavingsThreshold: 1.0,  // $1 minimum savings to report
};

// ============================================================================
// Token Optimization Agent Class
// ============================================================================

/**
 * Token Optimization Agent - Analyzes and recommends token usage optimizations.
 *
 * This is a stateless, deterministic agent designed to run as a
 * Google Cloud Edge Function.
 */
export class TokenOptimizationAgent {
  private config: Required<TokenOptimizationAgentConfig>;
  private ruVectorClient: RuVectorClient;

  constructor(config: TokenOptimizationAgentConfig = {}) {
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
   * ANALYZE - Perform token usage analysis without generating recommendations.
   * Use this for understanding current token patterns and identifying inefficiencies.
   *
   * CLI: npx @llm-auto-optimizer/cli token-optimization analyze [options]
   */
  async analyze(input: TokenOptimizationInput): Promise<TokenOptimizationOutput> {
    return this.execute(input, 'analyze');
  }

  /**
   * RECOMMEND - Generate ranked token optimization recommendations.
   * Primary entry point for the agent.
   *
   * CLI: npx @llm-auto-optimizer/cli token-optimization recommend [options]
   */
  async recommend(input: TokenOptimizationInput): Promise<TokenOptimizationOutput> {
    return this.execute(input, 'recommend');
  }

  /**
   * SIMULATE - Simulate proposed token optimizations without persisting.
   * Use for what-if analysis of specific optimization strategies.
   *
   * CLI: npx @llm-auto-optimizer/cli token-optimization simulate [options]
   */
  async simulate(input: TokenOptimizationInput): Promise<TokenOptimizationOutput> {
    return this.execute(input, 'simulate');
  }

  // --------------------------------------------------------------------------
  // Core Execution Logic
  // --------------------------------------------------------------------------

  private async execute(
    input: TokenOptimizationInput,
    mode: 'analyze' | 'recommend' | 'simulate'
  ): Promise<TokenOptimizationOutput> {
    const startTime = Date.now();

    // Step 1: Validate input
    this.validateInput(input);

    // Step 2: Compute inputs hash for reproducibility
    const inputsHash = this.hashInputs(input);

    // Step 3: Detect token usage patterns
    const patterns = this.detectPatterns(input.token_telemetry, input.analysis_depth);

    // Step 4: Calculate efficiency metrics
    const efficiencyMetrics = this.calculateEfficiencyMetrics(input.token_telemetry);

    // Step 5: Identify optimization opportunities
    const opportunities = mode === 'analyze'
      ? []
      : this.identifyOpportunities(
          input.token_telemetry,
          patterns,
          input.constraints,
          input.quality_threshold,
          input.focus_areas
        );

    // Step 6: Evaluate constraints
    const evaluatedConstraints = this.evaluateConstraints(input.constraints, opportunities);

    // Step 7: Build output
    const output = this.buildOutput(
      input,
      patterns,
      opportunities,
      evaluatedConstraints,
      efficiencyMetrics,
      startTime
    );

    // Step 8: Build and persist DecisionEvent
    const decisionType = mode === 'analyze' ? DECISION_TYPE_ANALYZE : DECISION_TYPE_RECOMMEND;
    const decisionEvent = this.buildDecisionEvent(
      input,
      output,
      inputsHash,
      evaluatedConstraints,
      decisionType
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
  // Input Validation
  // --------------------------------------------------------------------------

  private validateInput(input: TokenOptimizationInput): void {
    if (!input || typeof input !== 'object') {
      throw new Error('Invalid input: must be an object');
    }

    // Required fields
    const requiredFields = [
      'execution_ref',
      'time_window',
      'target_services',
      'constraints',
      'token_telemetry',
      'analysis_depth',
      'quality_threshold',
    ];

    for (const field of requiredFields) {
      if (!(field in input)) {
        throw new Error(`Invalid input: missing required field '${field}'`);
      }
    }

    // Validate time window
    const start = new Date(input.time_window.start);
    const end = new Date(input.time_window.end);
    if (isNaN(start.getTime()) || isNaN(end.getTime())) {
      throw new Error('Invalid input: time_window dates must be valid ISO 8601');
    }
    if (end <= start) {
      throw new Error('Invalid input: time_window end must be after start');
    }

    // Validate quality threshold
    if (input.quality_threshold < 0 || input.quality_threshold > 1) {
      throw new Error('Invalid input: quality_threshold must be between 0 and 1');
    }

    // Validate telemetry has data
    if (!input.token_telemetry.aggregate) {
      throw new Error('Invalid input: token_telemetry.aggregate is required');
    }
  }

  private hashInputs(input: TokenOptimizationInput): string {
    const crypto = require('crypto');
    const canonicalized = JSON.stringify(input, Object.keys(input).sort());
    return crypto.createHash('sha256').update(canonicalized).digest('hex');
  }

  // --------------------------------------------------------------------------
  // Pattern Detection
  // --------------------------------------------------------------------------

  private detectPatterns(
    telemetry: TokenTelemetryData,
    depth: 'shallow' | 'standard' | 'deep'
  ): TokenUsagePattern[] {
    const patterns: TokenUsagePattern[] = [];

    // Pattern 1: Redundant context detection
    const redundantContextPattern = this.detectRedundantContext(telemetry);
    if (redundantContextPattern) patterns.push(redundantContextPattern);

    // Pattern 2: Verbose instructions detection
    const verbosePattern = this.detectVerboseInstructions(telemetry);
    if (verbosePattern) patterns.push(verbosePattern);

    // Pattern 3: Response padding detection
    const paddingPattern = this.detectResponsePadding(telemetry);
    if (paddingPattern) patterns.push(paddingPattern);

    // Deep analysis patterns
    if (depth === 'deep' || depth === 'standard') {
      // Pattern 4: Duplicate examples
      const duplicatePattern = this.detectDuplicateExamples(telemetry);
      if (duplicatePattern) patterns.push(duplicatePattern);

      // Pattern 5: Inefficient structure
      const structurePattern = this.detectInefficientStructure(telemetry);
      if (structurePattern) patterns.push(structurePattern);
    }

    // Deep-only patterns
    if (depth === 'deep') {
      // Pattern 6: Over-specification
      const overSpecPattern = this.detectOverSpecification(telemetry);
      if (overSpecPattern) patterns.push(overSpecPattern);

      // Pattern 7: Suboptimal encoding
      const encodingPattern = this.detectSuboptimalEncoding(telemetry);
      if (encodingPattern) patterns.push(encodingPattern);
    }

    return patterns;
  }

  private detectRedundantContext(telemetry: TokenTelemetryData): TokenUsagePattern | null {
    // Analyze input token distribution for repeated patterns
    const aggregate = telemetry.aggregate;
    const avgInput = aggregate.avg_input_per_request;

    // Heuristic: If avg input is significantly higher than typical, suspect redundancy
    const typicalAvgInput = 500; // baseline tokens
    const redundancyRatio = (avgInput - typicalAvgInput) / avgInput;

    if (redundancyRatio > PATTERN_THRESHOLDS.redundant_context_min_frequency) {
      const wastedTokens = Math.floor(redundancyRatio * aggregate.total_input_tokens);
      const costPerToken = this.getAverageCostPerToken(telemetry, 'input');

      return {
        pattern_id: `pattern-redundant-${Date.now()}`,
        category: 'redundant_context',
        description: 'Repeated context or system prompts detected across requests. Consider using context caching or prompt references.',
        frequency: redundancyRatio,
        token_waste: {
          avg_input_tokens: Math.floor(avgInput * redundancyRatio),
          avg_output_tokens: 0,
          total_tokens: wastedTokens,
          cost_usd: wastedTokens * costPerToken / 1000,
          waste_pct: redundancyRatio * 100,
        },
        affected_count: aggregate.total_requests,
      };
    }

    return null;
  }

  private detectVerboseInstructions(telemetry: TokenTelemetryData): TokenUsagePattern | null {
    // Analyze input/output ratio - verbose instructions often have high input/low output ratio
    const aggregate = telemetry.aggregate;
    const ioRatio = aggregate.avg_input_per_request / Math.max(aggregate.avg_output_per_request, 1);

    // High I/O ratio (>3:1) may indicate verbose instructions
    if (ioRatio > 3.0) {
      const verbosityFactor = (ioRatio - 2.0) / ioRatio; // Above 2:1 is considered verbose
      const wastedTokens = Math.floor(aggregate.total_input_tokens * verbosityFactor * 0.3);
      const costPerToken = this.getAverageCostPerToken(telemetry, 'input');

      return {
        pattern_id: `pattern-verbose-${Date.now()}`,
        category: 'verbose_instructions',
        description: 'System prompts or instructions appear verbose relative to output. Consider condensing prompts while preserving intent.',
        frequency: verbosityFactor,
        token_waste: {
          avg_input_tokens: Math.floor(aggregate.avg_input_per_request * verbosityFactor * 0.3),
          avg_output_tokens: 0,
          total_tokens: wastedTokens,
          cost_usd: wastedTokens * costPerToken / 1000,
          waste_pct: (wastedTokens / aggregate.total_input_tokens) * 100,
        },
        affected_count: aggregate.total_requests,
      };
    }

    return null;
  }

  private detectResponsePadding(telemetry: TokenTelemetryData): TokenUsagePattern | null {
    // Analyze output token patterns for padding
    const aggregate = telemetry.aggregate;

    // Check for services with high output variability (may indicate padding)
    let paddingServices = 0;
    let totalPaddingWaste = 0;

    for (const [_, metrics] of Object.entries(telemetry.by_service || {})) {
      // Services with high token count but low efficiency may have padding
      if (metrics.efficiency_score < 0.7 && metrics.avg_tokens_per_request > 1000) {
        paddingServices++;
        totalPaddingWaste += metrics.total_tokens * (1 - metrics.efficiency_score) * 0.5;
      }
    }

    if (paddingServices > 0 && totalPaddingWaste > PATTERN_THRESHOLDS.waste_significance_threshold) {
      const costPerToken = this.getAverageCostPerToken(telemetry, 'output');

      return {
        pattern_id: `pattern-padding-${Date.now()}`,
        category: 'response_padding',
        description: 'Model responses contain unnecessary padding, preambles, or verbose formatting. Consider using structured output or response trimming.',
        frequency: paddingServices / Object.keys(telemetry.by_service || {}).length,
        token_waste: {
          avg_input_tokens: 0,
          avg_output_tokens: Math.floor(totalPaddingWaste / aggregate.total_requests),
          total_tokens: Math.floor(totalPaddingWaste),
          cost_usd: totalPaddingWaste * costPerToken / 1000,
          waste_pct: (totalPaddingWaste / aggregate.total_output_tokens) * 100,
        },
        affected_count: paddingServices,
      };
    }

    return null;
  }

  private detectDuplicateExamples(telemetry: TokenTelemetryData): TokenUsagePattern | null {
    // Heuristic: High input tokens with many requests may indicate repeated examples
    const aggregate = telemetry.aggregate;

    if (aggregate.avg_input_per_request > 1500 && aggregate.total_requests > 100) {
      const duplicateFactor = 0.15; // Assume 15% duplication
      const wastedTokens = Math.floor(aggregate.total_input_tokens * duplicateFactor);
      const costPerToken = this.getAverageCostPerToken(telemetry, 'input');

      return {
        pattern_id: `pattern-duplicate-${Date.now()}`,
        category: 'duplicate_examples',
        description: 'Few-shot examples may be repeated unnecessarily. Consider example caching or dynamic example selection.',
        frequency: duplicateFactor,
        token_waste: {
          avg_input_tokens: Math.floor(aggregate.avg_input_per_request * duplicateFactor),
          avg_output_tokens: 0,
          total_tokens: wastedTokens,
          cost_usd: wastedTokens * costPerToken / 1000,
          waste_pct: duplicateFactor * 100,
        },
        affected_count: aggregate.total_requests,
      };
    }

    return null;
  }

  private detectInefficientStructure(telemetry: TokenTelemetryData): TokenUsagePattern | null {
    // Analyze model-level metrics for structural inefficiencies
    const inefficientModels: string[] = [];
    let totalWaste = 0;

    for (const [modelId, metrics] of Object.entries(telemetry.by_model || {})) {
      const ref = TOKEN_MODEL_REFERENCE[modelId];
      if (ref) {
        // Compare actual I/O ratio with expected
        const actualIoRatio = metrics.avg_input_tokens / Math.max(metrics.avg_output_tokens, 1);
        if (actualIoRatio > 4.0) { // Unusually high
          inefficientModels.push(modelId);
          totalWaste += metrics.total_input_tokens * 0.2; // Assume 20% structure inefficiency
        }
      }
    }

    if (inefficientModels.length > 0 && totalWaste > PATTERN_THRESHOLDS.waste_significance_threshold) {
      const costPerToken = this.getAverageCostPerToken(telemetry, 'input');

      return {
        pattern_id: `pattern-structure-${Date.now()}`,
        category: 'inefficient_structure',
        description: `Prompt structure appears inefficient for models: ${inefficientModels.join(', ')}. Consider restructuring prompts for better token efficiency.`,
        frequency: inefficientModels.length / Object.keys(telemetry.by_model || {}).length,
        token_waste: {
          avg_input_tokens: Math.floor(totalWaste / telemetry.aggregate.total_requests),
          avg_output_tokens: 0,
          total_tokens: Math.floor(totalWaste),
          cost_usd: totalWaste * costPerToken / 1000,
          waste_pct: (totalWaste / telemetry.aggregate.total_input_tokens) * 100,
        },
        affected_count: inefficientModels.length,
      };
    }

    return null;
  }

  private detectOverSpecification(telemetry: TokenTelemetryData): TokenUsagePattern | null {
    // Deep analysis: Check for over-specification in high-token requests
    const aggregate = telemetry.aggregate;

    // High average input with low quality variance suggests over-specification
    if (aggregate.avg_input_per_request > 2000) {
      const overSpecFactor = 0.1; // Conservative estimate
      const wastedTokens = Math.floor(aggregate.total_input_tokens * overSpecFactor);
      const costPerToken = this.getAverageCostPerToken(telemetry, 'input');

      return {
        pattern_id: `pattern-overspec-${Date.now()}`,
        category: 'over_specification',
        description: 'Prompts may contain more detail than necessary for the task. Consider simplifying requirements or using tiered prompting.',
        frequency: overSpecFactor,
        token_waste: {
          avg_input_tokens: Math.floor(aggregate.avg_input_per_request * overSpecFactor),
          avg_output_tokens: 0,
          total_tokens: wastedTokens,
          cost_usd: wastedTokens * costPerToken / 1000,
          waste_pct: overSpecFactor * 100,
        },
        affected_count: aggregate.total_requests,
      };
    }

    return null;
  }

  private detectSuboptimalEncoding(telemetry: TokenTelemetryData): TokenUsagePattern | null {
    // Deep analysis: Check for encoding inefficiencies
    // This would require actual token-level analysis in production

    // Placeholder heuristic based on tokens-per-character ratio
    const aggregate = telemetry.aggregate;

    // If we have character data (not available in current telemetry),
    // we could detect encoding issues. For now, return null.
    return null;
  }

  // --------------------------------------------------------------------------
  // Efficiency Metrics Calculation
  // --------------------------------------------------------------------------

  private calculateEfficiencyMetrics(telemetry: TokenTelemetryData): TokenEfficiencyScores {
    const aggregate = telemetry.aggregate;

    // Input efficiency: Compare to baseline (500 tokens avg)
    const baselineInput = 500;
    const inputEfficiency = Math.min(1.0, baselineInput / aggregate.avg_input_per_request);

    // Output efficiency: Based on existing efficiency score
    const outputEfficiency = aggregate.efficiency_score || 0.7;

    // Context utilization: Based on I/O ratio (ideal is 1:1 to 2:1)
    const ioRatio = aggregate.avg_input_per_request / Math.max(aggregate.avg_output_per_request, 1);
    const contextUtilization = ioRatio <= 2.0 ? 1.0 : Math.max(0.3, 1.0 - (ioRatio - 2.0) * 0.1);

    // Compression potential: Based on detected waste
    const totalWastePct = Math.min(0.5, 1 - aggregate.efficiency_score);
    const compressionPotential = totalWastePct;

    // Overall efficiency
    const overall = (inputEfficiency * 0.3 + outputEfficiency * 0.3 + contextUtilization * 0.4);

    return {
      overall: Math.round(overall * 100) / 100,
      input_efficiency: Math.round(inputEfficiency * 100) / 100,
      output_efficiency: Math.round(outputEfficiency * 100) / 100,
      context_utilization: Math.round(contextUtilization * 100) / 100,
      compression_potential: Math.round(compressionPotential * 100) / 100,
    };
  }

  // --------------------------------------------------------------------------
  // Opportunity Identification
  // --------------------------------------------------------------------------

  private identifyOpportunities(
    telemetry: TokenTelemetryData,
    patterns: TokenUsagePattern[],
    constraints: TokenOptimizationConstraint[],
    qualityThreshold: number,
    focusAreas?: TokenPatternCategory[]
  ): TokenOptimizationOpportunity[] {
    const opportunities: TokenOptimizationOpportunity[] = [];

    // Generate opportunities from detected patterns
    for (const pattern of patterns) {
      // Skip if pattern is not in focus areas (when specified)
      if (focusAreas && focusAreas.length > 0 && !focusAreas.includes(pattern.category)) {
        continue;
      }

      // Skip low-impact patterns
      if (pattern.token_waste.cost_usd < this.config.minSavingsThreshold) {
        continue;
      }

      const opportunity = this.patternToOpportunity(pattern, telemetry, qualityThreshold);
      if (opportunity && !this.violatesHardConstraints(opportunity, constraints)) {
        opportunities.push(opportunity);
      }
    }

    // Generate model-specific opportunities
    const modelOpportunities = this.identifyModelOpportunities(telemetry, qualityThreshold);
    for (const opp of modelOpportunities) {
      if (!this.violatesHardConstraints(opp, constraints)) {
        opportunities.push(opp);
      }
    }

    // Sort by priority and limit
    return opportunities
      .sort((a, b) => b.priority - a.priority)
      .slice(0, this.config.maxOpportunities);
  }

  private patternToOpportunity(
    pattern: TokenUsagePattern,
    telemetry: TokenTelemetryData,
    qualityThreshold: number
  ): TokenOptimizationOpportunity | null {
    const opportunityId = `opp-${pattern.pattern_id}`;

    let type: TokenOptimizationType;
    let actions: TokenOptimizationAction[] = [];
    let qualityImpact: QualityImpactAssessment;

    switch (pattern.category) {
      case 'redundant_context':
        type = 'context_caching';
        qualityImpact = {
          expected_change: 0,
          confidence: 0.95,
          risk_level: 'none',
          affected_metrics: [],
        };
        actions = [{
          action_id: `action-${opportunityId}-1`,
          type: 'add',
          target: 'model_config',
          description: 'Enable context caching for repeated system prompts',
          proposed: { context_cache: { enabled: true, ttl_seconds: 3600 } },
          token_impact: { input_delta: -pattern.token_waste.avg_input_tokens, output_delta: 0 },
        }];
        break;

      case 'verbose_instructions':
        type = 'prompt_compression';
        qualityImpact = {
          expected_change: -0.02,
          confidence: 0.8,
          risk_level: 'low',
          affected_metrics: ['instruction_following'],
          mitigations: ['A/B test compressed prompts', 'Monitor quality metrics'],
        };
        actions = [{
          action_id: `action-${opportunityId}-1`,
          type: 'modify',
          target: 'prompt_template',
          description: 'Compress verbose instructions while preserving intent',
          current: 'Verbose system prompt (see analysis)',
          proposed: 'Condensed system prompt with key directives only',
          token_impact: { input_delta: -pattern.token_waste.avg_input_tokens, output_delta: 0 },
        }];
        break;

      case 'response_padding':
        type = 'response_trimming';
        qualityImpact = {
          expected_change: 0.01, // May improve by removing fluff
          confidence: 0.85,
          risk_level: 'low',
          affected_metrics: ['response_completeness'],
        };
        actions = [{
          action_id: `action-${opportunityId}-1`,
          type: 'add',
          target: 'response_format',
          description: 'Add response format constraints to reduce padding',
          proposed: {
            response_format: 'concise',
            max_preamble_tokens: 20,
            skip_acknowledgments: true,
          },
          token_impact: { input_delta: 0, output_delta: -pattern.token_waste.avg_output_tokens },
        }];
        break;

      case 'duplicate_examples':
        type = 'example_reduction';
        qualityImpact = {
          expected_change: -0.03,
          confidence: 0.75,
          risk_level: 'low',
          affected_metrics: ['few_shot_accuracy'],
          mitigations: ['Implement dynamic example selection', 'Cache top-performing examples'],
        };
        actions = [{
          action_id: `action-${opportunityId}-1`,
          type: 'modify',
          target: 'examples',
          description: 'Reduce duplicate examples using dynamic selection',
          current: 'Static example set with duplicates',
          proposed: 'Dynamic example selection based on task similarity',
          token_impact: { input_delta: -pattern.token_waste.avg_input_tokens, output_delta: 0 },
        }];
        break;

      case 'inefficient_structure':
        type = 'template_optimization';
        qualityImpact = {
          expected_change: 0,
          confidence: 0.9,
          risk_level: 'none',
          affected_metrics: [],
        };
        actions = [{
          action_id: `action-${opportunityId}-1`,
          type: 'modify',
          target: 'prompt_template',
          description: 'Restructure prompts for better token efficiency',
          current: 'Current template structure',
          proposed: 'Optimized template with reduced overhead',
          token_impact: { input_delta: -pattern.token_waste.avg_input_tokens, output_delta: 0 },
        }];
        break;

      case 'over_specification':
        type = 'instruction_tuning';
        qualityImpact = {
          expected_change: -0.02,
          confidence: 0.7,
          risk_level: 'medium',
          affected_metrics: ['task_accuracy', 'edge_case_handling'],
          mitigations: ['Gradual reduction with quality monitoring', 'Preserve critical specifications'],
        };
        actions = [{
          action_id: `action-${opportunityId}-1`,
          type: 'modify',
          target: 'system_instruction',
          description: 'Simplify over-specified instructions',
          current: 'Detailed specification',
          proposed: 'Simplified specification with key requirements',
          token_impact: { input_delta: -pattern.token_waste.avg_input_tokens, output_delta: 0 },
        }];
        break;

      default:
        return null;
    }

    // Check quality threshold
    if (qualityThreshold > 0 && Math.abs(qualityImpact.expected_change) > (1 - qualityThreshold)) {
      qualityImpact.risk_level = 'high';
    }

    const savings: TokenSavings = {
      input_tokens: pattern.token_waste.avg_input_tokens * pattern.affected_count,
      output_tokens: pattern.token_waste.avg_output_tokens * pattern.affected_count,
      total_tokens: pattern.token_waste.total_tokens,
      cost_usd: pattern.token_waste.cost_usd,
      savings_pct: pattern.token_waste.waste_pct,
    };

    // Calculate priority based on savings, confidence, and quality impact
    const savingsScore = Math.min(1.0, savings.cost_usd / 100); // $100 = 1.0
    const qualityScore = 1.0 - Math.abs(qualityImpact.expected_change);
    const confidenceScore = qualityImpact.confidence;
    const priority = (savingsScore * 0.4 + qualityScore * 0.3 + confidenceScore * 0.3);

    return {
      opportunity_id: opportunityId,
      type,
      priority: Math.round(priority * 100) / 100,
      savings_per_request: {
        input_tokens: pattern.token_waste.avg_input_tokens,
        output_tokens: pattern.token_waste.avg_output_tokens,
        total_tokens: pattern.token_waste.avg_input_tokens + pattern.token_waste.avg_output_tokens,
        cost_usd: pattern.token_waste.cost_usd / pattern.affected_count,
        savings_pct: pattern.token_waste.waste_pct,
      },
      total_savings: savings,
      quality_impact: qualityImpact,
      complexity: this.determineComplexity(type, actions),
      actions,
      evidence: [{
        type: 'statistical',
        source: 'token-telemetry-analysis',
        description: `Pattern detected with ${(pattern.frequency * 100).toFixed(1)}% frequency affecting ${pattern.affected_count} requests`,
        confidence: this.config.minConfidenceThreshold,
        data: {
          pattern_category: pattern.category,
          affected_count: pattern.affected_count,
          waste_pct: pattern.token_waste.waste_pct,
        },
      }],
    };
  }

  private identifyModelOpportunities(
    telemetry: TokenTelemetryData,
    qualityThreshold: number
  ): TokenOptimizationOpportunity[] {
    const opportunities: TokenOptimizationOpportunity[] = [];

    for (const [modelId, metrics] of Object.entries(telemetry.by_model || {})) {
      const ref = TOKEN_MODEL_REFERENCE[modelId];
      if (!ref) continue;

      // Check if a cheaper model could handle the workload
      const cheaperModels = this.findCheaperModelsForTokens(modelId, metrics);

      for (const cheaper of cheaperModels) {
        if (cheaper.qualityDrop <= (1 - qualityThreshold)) {
          const savings = this.calculateModelSwitchSavings(modelId, cheaper.modelId, metrics);

          opportunities.push({
            opportunity_id: `opp-model-${modelId}-to-${cheaper.modelId}`,
            type: 'model_downgrade',
            priority: Math.min(0.9, savings.savings_pct / 100 + 0.3),
            savings_per_request: {
              input_tokens: 0, // Same tokens, different cost
              output_tokens: 0,
              total_tokens: 0,
              cost_usd: savings.cost_usd / metrics.request_count,
              savings_pct: savings.savings_pct,
            },
            total_savings: savings,
            quality_impact: {
              expected_change: -cheaper.qualityDrop,
              confidence: 0.85,
              risk_level: cheaper.qualityDrop > 0.05 ? 'medium' : 'low',
              affected_metrics: ['overall_quality'],
              mitigations: ['Canary deployment', 'Quality monitoring'],
            },
            complexity: 'low',
            actions: [{
              action_id: `action-model-switch-${modelId}`,
              type: 'replace',
              target: 'model_config',
              description: `Switch from ${modelId} to ${cheaper.modelId} for cost optimization`,
              current: modelId,
              proposed: cheaper.modelId,
              token_impact: { input_delta: 0, output_delta: 0 },
            }],
            evidence: [{
              type: 'benchmark',
              source: 'model-pricing-reference',
              description: `${cheaper.modelId} costs ${(100 - savings.savings_pct).toFixed(0)}% less than ${modelId}`,
              confidence: 0.95,
              data: {
                current_model: modelId,
                proposed_model: cheaper.modelId,
                cost_reduction_pct: savings.savings_pct,
                quality_benchmark_delta: cheaper.qualityDrop,
              },
            }],
          });
        }
      }
    }

    return opportunities;
  }

  private findCheaperModelsForTokens(
    currentModel: string,
    metrics: ModelTokenMetrics
  ): Array<{ modelId: string; qualityDrop: number }> {
    const currentRef = TOKEN_MODEL_REFERENCE[currentModel];
    if (!currentRef) return [];

    const results: Array<{ modelId: string; qualityDrop: number }> = [];

    for (const [modelId, ref] of Object.entries(TOKEN_MODEL_REFERENCE)) {
      if (modelId === currentModel) continue;

      // Calculate cost comparison
      const currentCostPer1k = (currentRef.input_cost_per_1k + currentRef.output_cost_per_1k) / 2;
      const newCostPer1k = (ref.input_cost_per_1k + ref.output_cost_per_1k) / 2;

      if (newCostPer1k < currentCostPer1k) {
        const qualityDrop = Math.max(0, currentRef.quality_benchmark - ref.quality_benchmark);
        results.push({ modelId, qualityDrop });
      }
    }

    return results.sort((a, b) => a.qualityDrop - b.qualityDrop);
  }

  private calculateModelSwitchSavings(
    fromModel: string,
    toModel: string,
    metrics: ModelTokenMetrics
  ): TokenSavings {
    const fromRef = TOKEN_MODEL_REFERENCE[fromModel];
    const toRef = TOKEN_MODEL_REFERENCE[toModel];

    if (!fromRef || !toRef) {
      return {
        input_tokens: 0,
        output_tokens: 0,
        total_tokens: 0,
        cost_usd: 0,
        savings_pct: 0,
      };
    }

    const currentCost = metrics.total_cost_usd;
    const newInputCost = (metrics.total_input_tokens / 1000) * toRef.input_cost_per_1k;
    const newOutputCost = (metrics.total_output_tokens / 1000) * toRef.output_cost_per_1k;
    const newTotalCost = newInputCost + newOutputCost;
    const savings = currentCost - newTotalCost;

    return {
      input_tokens: 0,
      output_tokens: 0,
      total_tokens: 0,
      cost_usd: Math.max(0, savings),
      savings_pct: Math.max(0, (savings / currentCost) * 100),
    };
  }

  private determineComplexity(
    type: TokenOptimizationType,
    actions: TokenOptimizationAction[]
  ): 'trivial' | 'low' | 'medium' | 'high' {
    // Trivial: No code changes needed
    if (type === 'context_caching' || type === 'model_downgrade') {
      return actions.length <= 1 ? 'trivial' : 'low';
    }

    // Low: Simple configuration changes
    if (type === 'response_trimming' || type === 'structured_output') {
      return 'low';
    }

    // Medium: Template modifications
    if (type === 'prompt_compression' || type === 'template_optimization' || type === 'example_reduction') {
      return 'medium';
    }

    // High: Requires significant prompt engineering
    if (type === 'instruction_tuning' || type === 'encoding_optimization') {
      return 'high';
    }

    return 'medium';
  }

  // --------------------------------------------------------------------------
  // Constraint Evaluation
  // --------------------------------------------------------------------------

  private violatesHardConstraints(
    opportunity: TokenOptimizationOpportunity,
    constraints: TokenOptimizationConstraint[]
  ): boolean {
    for (const constraint of constraints) {
      if (!constraint.hard) continue;

      switch (constraint.type) {
        case 'min_quality_score':
          if (1 + opportunity.quality_impact.expected_change < Number(constraint.value)) {
            return true;
          }
          break;

        case 'max_latency_increase_pct':
          // Model downgrades may affect latency
          if (opportunity.type === 'model_downgrade') {
            // Assume some latency impact for model changes
            return false; // Would need actual latency data
          }
          break;

        case 'min_savings_threshold_pct':
          if (opportunity.total_savings.savings_pct < Number(constraint.value)) {
            return true;
          }
          break;

        case 'allowed_optimization_types':
          const allowedTypes = String(constraint.value).split(',');
          if (!allowedTypes.includes(opportunity.type)) {
            return true;
          }
          break;
      }
    }

    return false;
  }

  private evaluateConstraints(
    constraints: TokenOptimizationConstraint[],
    opportunities: TokenOptimizationOpportunity[]
  ): EvaluatedTokenConstraint[] {
    return constraints.map(constraint => {
      const violated = opportunities.some(opp =>
        this.opportunityViolatesConstraint(opp, constraint)
      );

      return {
        constraint,
        satisfied: !violated,
        impact_description: !violated
          ? 'All opportunities comply with this constraint'
          : 'Some opportunities were filtered due to this constraint',
      };
    });
  }

  private opportunityViolatesConstraint(
    opportunity: TokenOptimizationOpportunity,
    constraint: TokenOptimizationConstraint
  ): boolean {
    switch (constraint.type) {
      case 'min_quality_score':
        return 1 + opportunity.quality_impact.expected_change < Number(constraint.value);

      case 'min_savings_threshold_pct':
        return opportunity.total_savings.savings_pct < Number(constraint.value);

      case 'allowed_optimization_types':
        const allowedTypes = String(constraint.value).split(',');
        return !allowedTypes.includes(opportunity.type);

      default:
        return false;
    }
  }

  // --------------------------------------------------------------------------
  // Output Building
  // --------------------------------------------------------------------------

  private buildOutput(
    input: TokenOptimizationInput,
    patterns: TokenUsagePattern[],
    opportunities: TokenOptimizationOpportunity[],
    evaluatedConstraints: EvaluatedTokenConstraint[],
    efficiency: TokenEfficiencyScores,
    startTime: number
  ): TokenOptimizationOutput {
    const timestamp = new Date().toISOString();
    const outputId = `out-${AGENT_ID}-${Date.now()}`;

    // Calculate total potential savings
    const totalSavings = opportunities.reduce(
      (acc, opp) => ({
        input_tokens: acc.input_tokens + opp.total_savings.input_tokens,
        output_tokens: acc.output_tokens + opp.total_savings.output_tokens,
        total_tokens: acc.total_tokens + opp.total_savings.total_tokens,
        cost_usd: acc.cost_usd + opp.total_savings.cost_usd,
        savings_pct: 0, // Calculated below
      }),
      { input_tokens: 0, output_tokens: 0, total_tokens: 0, cost_usd: 0, savings_pct: 0 }
    );

    // Calculate savings percentage
    if (input.token_telemetry.aggregate.total_cost_usd > 0) {
      totalSavings.savings_pct =
        (totalSavings.cost_usd / input.token_telemetry.aggregate.total_cost_usd) * 100;
    }

    // Generate key findings
    const keyFindings: string[] = [];
    if (patterns.length > 0) {
      keyFindings.push(`Identified ${patterns.length} token usage patterns with optimization potential`);
    }
    if (totalSavings.cost_usd > 10) {
      keyFindings.push(`Potential monthly savings of $${(totalSavings.cost_usd * 30).toFixed(2)}`);
    }
    if (efficiency.compression_potential > 0.2) {
      keyFindings.push(`${(efficiency.compression_potential * 100).toFixed(0)}% compression potential identified`);
    }

    // Determine risk level
    const highRiskOpps = opportunities.filter(o => o.quality_impact.risk_level === 'high');
    const riskLevel = highRiskOpps.length > opportunities.length / 2 ? 'high' :
                      highRiskOpps.length > 0 ? 'medium' : 'low';

    const analysis: TokenAnalysisSummary = {
      current_state: {
        total_tokens_analyzed: input.token_telemetry.aggregate.total_input_tokens +
                               input.token_telemetry.aggregate.total_output_tokens,
        total_cost_analyzed_usd: input.token_telemetry.aggregate.total_cost_usd,
        overall_efficiency: efficiency.overall,
        input_efficiency: efficiency.input_efficiency,
        output_efficiency: efficiency.output_efficiency,
      },
      patterns_identified: patterns.length,
      opportunities_identified: opportunities.length,
      total_potential_savings: totalSavings,
      risk_level: riskLevel,
      key_findings: keyFindings,
    };

    const metadata: TokenOptimizationMetadata = {
      processing_duration_ms: Date.now() - startTime,
      data_points_analyzed:
        Object.keys(input.token_telemetry.by_model || {}).length +
        Object.keys(input.token_telemetry.by_service || {}).length +
        (input.token_telemetry.aggregate.daily_trend?.length || 0),
      strategies_used: [
        'pattern_detection',
        'efficiency_analysis',
        'model_comparison',
        'cost_optimization',
      ],
      warnings: [],
      coverage: {
        services_analyzed: Object.keys(input.token_telemetry.by_service || {}).length,
        models_analyzed: Object.keys(input.token_telemetry.by_model || {}).length,
        requests_sampled: input.token_telemetry.aggregate.total_requests,
        time_range_hours: this.calculateTimeRangeHours(input.time_window),
      },
    };

    return {
      output_id: outputId,
      execution_ref: input.execution_ref,
      agent_version: AGENT_VERSION,
      timestamp,
      analysis,
      patterns,
      opportunities,
      constraints_evaluated: evaluatedConstraints,
      metadata,
    };
  }

  // --------------------------------------------------------------------------
  // DecisionEvent Building
  // --------------------------------------------------------------------------

  private buildDecisionEvent(
    input: TokenOptimizationInput,
    output: TokenOptimizationOutput,
    inputsHash: string,
    evaluatedConstraints: EvaluatedTokenConstraint[],
    decisionType: DecisionType
  ): DecisionEvent {
    // Convert token opportunities to standard recommendations for DecisionOutputs
    const recommendations: OptimizationRecommendation[] = output.opportunities.map((opp, idx) => ({
      recommendation_id: opp.opportunity_id,
      rank: idx + 1,
      category: this.mapOpportunityTypeToCategory(opp.type),
      title: `Token Optimization: ${opp.type.replace(/_/g, ' ')}`,
      description: opp.actions[0]?.description || '',
      proposed_changes: opp.actions.map(action => ({
        type: action.type as 'replace' | 'add' | 'remove' | 'update',
        parameter: action.target,
        current_value: action.current,
        proposed_value: action.proposed,
        affected_services: input.target_services,
      })),
      expected_impact: {
        cost_change_pct: -opp.total_savings.savings_pct,
        latency_change_pct: 0,
        quality_change_pct: opp.quality_impact.expected_change * 100,
        monthly_savings_usd: opp.total_savings.cost_usd * 30,
        confidence: opp.quality_impact.confidence,
      },
      complexity: opp.complexity === 'trivial' ? 'low' : opp.complexity,
      risk: opp.quality_impact.risk_level === 'none' ? 'low' : opp.quality_impact.risk_level,
      confidence: opp.priority,
      evidence: opp.evidence.map(e => ({
        type: e.type as Evidence['type'],
        source: e.source,
        data_points: e.data,
        strength: e.confidence,
      })),
      rollout_strategy: {
        type: opp.quality_impact.risk_level === 'high' ? 'canary' : 'gradual',
        canary_percentage: 10,
        rollback_criteria: {
          max_error_rate_increase_pct: 5,
          max_latency_increase_pct: 10,
          min_quality_threshold: 0.85,
          monitoring_duration_minutes: 60,
        },
      },
    }));

    const outputs: DecisionOutputs = {
      recommendations,
      analysis: output.opportunities.length === 0 ? {
        analysis_type: 'cost',
        depth: input.analysis_depth === 'deep' ? 'deep' : input.analysis_depth === 'standard' ? 'medium' : 'shallow',
        findings: output.patterns.map(p => ({
          finding_id: p.pattern_id,
          category: p.category,
          severity: p.token_waste.cost_usd > 50 ? 'warning' : 'info',
          description: p.description,
          metrics: {
            waste_tokens: p.token_waste.total_tokens,
            waste_cost_usd: p.token_waste.cost_usd,
            waste_pct: p.token_waste.waste_pct,
          },
        })),
        trends: {
          cost_trend: 'stable',
          latency_trend: 'stable',
          quality_trend: 'stable',
          confidence: 0.8,
        },
      } : undefined,
    };

    const constraintsApplied: AppliedConstraint[] = evaluatedConstraints.map((ec, idx) => ({
      constraint_id: `constraint-${idx}`,
      type: this.mapTokenConstraintType(ec.constraint.type),
      value: ec.constraint.value,
      hard: ec.constraint.hard,
      satisfied: ec.satisfied,
    }));

    // Calculate overall confidence
    const overallConfidence = output.opportunities.length > 0
      ? output.opportunities.reduce((sum, o) => sum + o.priority, 0) / output.opportunities.length
      : 0.5;

    return {
      agent_id: AGENT_ID,
      agent_version: AGENT_VERSION,
      decision_type: decisionType,
      inputs_hash: inputsHash,
      outputs,
      confidence: Math.round(overallConfidence * 100) / 100,
      constraints_applied: constraintsApplied,
      execution_ref: input.execution_ref,
      timestamp: output.timestamp,
    };
  }

  private mapOpportunityTypeToCategory(type: TokenOptimizationType): RecommendationCategory {
    switch (type) {
      case 'prompt_compression':
      case 'template_optimization':
      case 'instruction_tuning':
        return 'prompt_optimization';
      case 'context_caching':
        return 'caching_strategy';
      case 'model_downgrade':
        return 'model_substitution';
      case 'response_trimming':
        return 'response_optimization';
      case 'batching':
        return 'request_batching';
      case 'example_reduction':
      case 'encoding_optimization':
        return 'token_reduction';
      case 'structured_output':
        return 'context_optimization';
      default:
        return 'cost_optimization';
    }
  }

  private mapTokenConstraintType(type: TokenConstraintType): AppliedConstraint['type'] {
    switch (type) {
      case 'min_quality_score':
        return 'min_quality_threshold';
      case 'max_latency_increase_pct':
        return 'max_latency_increase_ms';
      default:
        return 'compliance_policy';
    }
  }

  // --------------------------------------------------------------------------
  // Telemetry Emission
  // --------------------------------------------------------------------------

  private async emitTelemetry(
    decisionEvent: DecisionEvent,
    output: TokenOptimizationOutput,
    durationMs: number
  ): Promise<void> {
    if (!this.config.telemetryEndpoint) return;

    const telemetry = {
      agent_id: AGENT_ID,
      agent_version: AGENT_VERSION,
      execution_ref: decisionEvent.execution_ref,
      metrics: {
        processing_duration_ms: durationMs,
        patterns_detected: output.patterns.length,
        opportunities_generated: output.opportunities.length,
        total_potential_savings_usd: output.analysis.total_potential_savings.cost_usd,
        overall_efficiency: output.analysis.current_state.overall_efficiency,
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
      console.error('[TokenOptimizationAgent] Telemetry emission failed:', error);
    }
  }

  // --------------------------------------------------------------------------
  // Utility Methods
  // --------------------------------------------------------------------------

  private getAverageCostPerToken(telemetry: TokenTelemetryData, direction: 'input' | 'output'): number {
    let totalCost = 0;
    let totalTokens = 0;

    for (const [modelId, metrics] of Object.entries(telemetry.by_model || {})) {
      const ref = TOKEN_MODEL_REFERENCE[modelId];
      if (ref) {
        if (direction === 'input') {
          totalCost += metrics.total_input_tokens * ref.input_cost_per_1k / 1000;
          totalTokens += metrics.total_input_tokens;
        } else {
          totalCost += metrics.total_output_tokens * ref.output_cost_per_1k / 1000;
          totalTokens += metrics.total_output_tokens;
        }
      }
    }

    return totalTokens > 0 ? totalCost / totalTokens * 1000 : 0.001;
  }

  private calculateTimeRangeHours(timeWindow: { start: string; end: string }): number {
    const start = new Date(timeWindow.start);
    const end = new Date(timeWindow.end);
    return (end.getTime() - start.getTime()) / (1000 * 60 * 60);
  }
}

// ============================================================================
// Factory Function
// ============================================================================

/**
 * Create a Token Optimization Agent instance.
 */
export function createTokenOptimizationAgent(
  config?: TokenOptimizationAgentConfig
): TokenOptimizationAgent {
  return new TokenOptimizationAgent(config);
}
