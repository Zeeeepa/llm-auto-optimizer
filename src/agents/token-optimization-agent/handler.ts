/**
 * Token Optimization Agent - Google Cloud Edge Function Handler
 *
 * Stateless HTTP handler for the Token Optimization Agent.
 * Deploys as part of the LLM-Auto-Optimizer unified GCP service.
 *
 * Endpoints:
 * - POST /analyze  - Analyze token patterns without recommendations
 * - POST /recommend - Generate token optimization recommendations
 * - POST /simulate - Simulate token optimization changes
 * - GET  /health   - Health check
 *
 * @module agents/token-optimization-agent/handler
 */

import {
  TokenOptimizationAgent,
  createTokenOptimizationAgent,
  AGENT_ID,
  AGENT_VERSION,
} from './index';

import {
  TokenOptimizationInput,
  TokenOptimizationOutput,
  TokenTelemetryData,
  TokenOptimizationConstraint,
  TokenPatternCategory,
} from './types';

// ============================================================================
// Types
// ============================================================================

interface HttpRequest {
  method: string;
  path: string;
  headers: Record<string, string>;
  body?: string | object;
  query?: Record<string, string>;
}

interface HttpResponse {
  status: number;
  headers: Record<string, string>;
  body: string;
}

interface HealthCheckResponse {
  status: 'healthy' | 'degraded' | 'unhealthy';
  agent_id: string;
  agent_version: string;
  timestamp: string;
  checks: {
    ruvector_service: boolean;
    telemetry_endpoint: boolean;
  };
}

interface ErrorResponse {
  error: {
    code: string;
    message: string;
    details?: unknown;
  };
  agent_id: string;
  timestamp: string;
}

/**
 * CLI parameters for Token Optimization Agent.
 */
export interface TokenOptimizationCLIParams {
  /** Time window (e.g., "24h", "7d", "30d") */
  window?: string;
  /** Target services (comma-separated) */
  services?: string;
  /** Analysis depth */
  depth?: 'shallow' | 'standard' | 'deep';
  /** Focus areas (comma-separated pattern categories) */
  focus?: string;
  /** Quality preservation threshold (0-1) */
  quality_threshold?: number;
  /** Minimum savings threshold (USD) */
  min_savings?: number;
  /** Maximum latency increase (ms) */
  max_latency_increase?: number;
  /** Verbose output */
  verbose?: boolean;
  /** Output format */
  output_format?: 'json' | 'yaml' | 'table' | 'summary';
}

// ============================================================================
// Handler Class
// ============================================================================

/**
 * HTTP handler for the Token Optimization Agent Edge Function.
 */
export class TokenOptimizationAgentHandler {
  private agent: TokenOptimizationAgent;

  constructor() {
    this.agent = createTokenOptimizationAgent();
  }

  /**
   * Main entry point for Edge Function invocations.
   */
  async handle(request: HttpRequest): Promise<HttpResponse> {
    const path = this.normalizePath(request.path);

    try {
      switch (path) {
        case '/health':
          return this.handleHealth();

        case '/analyze':
          return this.handleAnalyze(request);

        case '/recommend':
          return this.handleRecommend(request);

        case '/simulate':
          return this.handleSimulate(request);

        default:
          return this.notFound(path);
      }
    } catch (error) {
      return this.handleError(error);
    }
  }

  // --------------------------------------------------------------------------
  // Endpoint Handlers
  // --------------------------------------------------------------------------

  private async handleHealth(): Promise<HttpResponse> {
    const healthCheck: HealthCheckResponse = {
      status: 'healthy',
      agent_id: AGENT_ID,
      agent_version: AGENT_VERSION,
      timestamp: new Date().toISOString(),
      checks: {
        ruvector_service: true, // Would actually check in production
        telemetry_endpoint: !!process.env.TELEMETRY_ENDPOINT,
      },
    };

    return this.json(200, healthCheck);
  }

  private async handleAnalyze(request: HttpRequest): Promise<HttpResponse> {
    if (request.method !== 'POST') {
      return this.methodNotAllowed(request.method, ['POST']);
    }

    const input = this.parseInput(request);
    const result = await this.agent.analyze(input);
    return this.json(200, result);
  }

  private async handleRecommend(request: HttpRequest): Promise<HttpResponse> {
    if (request.method !== 'POST') {
      return this.methodNotAllowed(request.method, ['POST']);
    }

    const input = this.parseInput(request);
    const result = await this.agent.recommend(input);
    return this.json(200, result);
  }

  private async handleSimulate(request: HttpRequest): Promise<HttpResponse> {
    if (request.method !== 'POST') {
      return this.methodNotAllowed(request.method, ['POST']);
    }

    const input = this.parseInput(request);
    const result = await this.agent.simulate(input);
    return this.json(200, result);
  }

  // --------------------------------------------------------------------------
  // Input Parsing
  // --------------------------------------------------------------------------

  private parseInput(request: HttpRequest): TokenOptimizationInput {
    let body: unknown;

    if (typeof request.body === 'string') {
      try {
        body = JSON.parse(request.body);
      } catch {
        throw new ValidationError('Invalid JSON in request body');
      }
    } else {
      body = request.body;
    }

    if (!body) {
      throw new ValidationError('Request body is required');
    }

    // Basic validation
    const input = body as Record<string, unknown>;
    if (!input.execution_ref) {
      throw new ValidationError('execution_ref is required');
    }
    if (!input.time_window) {
      throw new ValidationError('time_window is required');
    }
    if (!input.token_telemetry) {
      throw new ValidationError('token_telemetry is required');
    }

    return body as TokenOptimizationInput;
  }

  // --------------------------------------------------------------------------
  // Response Helpers
  // --------------------------------------------------------------------------

  private json(status: number, data: unknown): HttpResponse {
    return {
      status,
      headers: {
        'Content-Type': 'application/json',
        'X-Agent-Id': AGENT_ID,
        'X-Agent-Version': AGENT_VERSION,
      },
      body: JSON.stringify(data),
    };
  }

  private notFound(path: string): HttpResponse {
    const error: ErrorResponse = {
      error: {
        code: 'NOT_FOUND',
        message: `Endpoint not found: ${path}`,
        details: {
          available_endpoints: ['/health', '/analyze', '/recommend', '/simulate'],
        },
      },
      agent_id: AGENT_ID,
      timestamp: new Date().toISOString(),
    };

    return this.json(404, error);
  }

  private methodNotAllowed(method: string, allowed: string[]): HttpResponse {
    const error: ErrorResponse = {
      error: {
        code: 'METHOD_NOT_ALLOWED',
        message: `Method ${method} not allowed`,
        details: { allowed_methods: allowed },
      },
      agent_id: AGENT_ID,
      timestamp: new Date().toISOString(),
    };

    return {
      ...this.json(405, error),
      headers: {
        ...this.json(405, error).headers,
        Allow: allowed.join(', '),
      },
    };
  }

  private handleError(error: unknown): HttpResponse {
    console.error('[TokenOptimizationAgentHandler] Error:', error);

    if (error instanceof ValidationError) {
      const errorResponse: ErrorResponse = {
        error: {
          code: 'VALIDATION_ERROR',
          message: error.message,
        },
        agent_id: AGENT_ID,
        timestamp: new Date().toISOString(),
      };
      return this.json(400, errorResponse);
    }

    const errorResponse: ErrorResponse = {
      error: {
        code: 'INTERNAL_ERROR',
        message: error instanceof Error ? error.message : 'Unknown error',
      },
      agent_id: AGENT_ID,
      timestamp: new Date().toISOString(),
    };

    return this.json(500, errorResponse);
  }

  private normalizePath(path: string): string {
    // Remove query string
    const questionIndex = path.indexOf('?');
    if (questionIndex !== -1) {
      path = path.substring(0, questionIndex);
    }

    // Remove trailing slash
    if (path.endsWith('/') && path.length > 1) {
      path = path.slice(0, -1);
    }

    // Remove agent prefix if present
    const agentPrefix = `/agents/${AGENT_ID}`;
    if (path.startsWith(agentPrefix)) {
      path = path.substring(agentPrefix.length) || '/';
    }

    return path;
  }
}

// ============================================================================
// Custom Errors
// ============================================================================

class ValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ValidationError';
  }
}

// ============================================================================
// Google Cloud Functions Entry Point
// ============================================================================

const handler = new TokenOptimizationAgentHandler();

/**
 * Google Cloud Functions HTTP entry point.
 *
 * @example
 * // Deploy as Cloud Function
 * gcloud functions deploy token-optimization-agent \
 *   --runtime nodejs20 \
 *   --trigger-http \
 *   --entry-point tokenOptimizationAgent
 */
export async function tokenOptimizationAgent(
  req: {
    method: string;
    path: string;
    headers: Record<string, string>;
    body?: unknown;
    query?: Record<string, string>;
  },
  res: {
    status: (code: number) => { set: (headers: Record<string, string>) => { send: (body: string) => void } };
  }
): Promise<void> {
  const request: HttpRequest = {
    method: req.method,
    path: req.path || '/',
    headers: req.headers,
    body: req.body as string | object | undefined,
    query: req.query,
  };

  const response = await handler.handle(request);

  res.status(response.status).set(response.headers).send(response.body);
}

/**
 * Google Cloud Run / Edge Function entry point.
 */
export { handler };

// ============================================================================
// CLI Helper Functions
// ============================================================================

/**
 * Build TokenOptimizationInput from CLI parameters.
 * Used by agentics-cli to invoke the agent.
 *
 * @example
 * // CLI invocation
 * npx @llm-auto-optimizer/cli token-optimization recommend \
 *   --window 7d \
 *   --services llm-gateway,chat-service \
 *   --depth deep \
 *   --quality-threshold 0.9
 */
export function buildInputFromCLI(params: TokenOptimizationCLIParams): TokenOptimizationInput {
  const now = new Date();
  const windowDuration = parseWindowDuration(params.window || '24h');
  const start = new Date(now.getTime() - windowDuration);

  // Parse focus areas
  const focusAreas = params.focus
    ? params.focus.split(',').map(s => s.trim() as TokenPatternCategory)
    : undefined;

  // Build constraints from CLI flags
  const constraints: TokenOptimizationConstraint[] = [];

  if (params.quality_threshold !== undefined) {
    constraints.push({
      type: 'min_quality_score',
      value: params.quality_threshold,
      hard: true,
    });
  }

  if (params.min_savings !== undefined) {
    constraints.push({
      type: 'min_savings_threshold_pct',
      value: params.min_savings,
      hard: false,
    });
  }

  if (params.max_latency_increase !== undefined) {
    constraints.push({
      type: 'max_latency_increase_pct',
      value: params.max_latency_increase,
      hard: true,
    });
  }

  // NOTE: In production, telemetry data would be fetched from
  // Observatory, Latency-Lens, and Analytics-Hub
  const mockTelemetry = createMockTokenTelemetryData();

  return {
    execution_ref: `cli-token-opt-${Date.now()}`,
    time_window: {
      start: start.toISOString(),
      end: now.toISOString(),
      granularity: windowDuration > 86400000 ? 'day' : 'hour',
    },
    target_services: params.services?.split(',').map(s => s.trim()) || ['default'],
    constraints,
    token_telemetry: mockTelemetry,
    analysis_depth: params.depth || 'standard',
    focus_areas: focusAreas,
    quality_threshold: params.quality_threshold || 0.85,
    max_latency_increase_ms: params.max_latency_increase,
  };
}

/**
 * Parse window duration string to milliseconds.
 */
function parseWindowDuration(window: string): number {
  const match = window.match(/^(\d+)([hdwm])$/);
  if (!match) return 86400000; // Default 24h

  const value = parseInt(match[1], 10);
  const unit = match[2];

  switch (unit) {
    case 'h': return value * 3600000;
    case 'd': return value * 86400000;
    case 'w': return value * 604800000;
    case 'm': return value * 2592000000;
    default: return 86400000;
  }
}

/**
 * Create mock telemetry data for development/testing.
 * In production, this would be fetched from Observatory, Latency-Lens, etc.
 */
function createMockTokenTelemetryData(): TokenTelemetryData {
  return {
    by_model: {
      'claude-3-opus': {
        model_id: 'claude-3-opus',
        request_count: 5000,
        total_input_tokens: 10000000,
        total_output_tokens: 2500000,
        avg_input_tokens: 2000,
        avg_output_tokens: 500,
        io_ratio: 4.0,
        total_cost_usd: 225.0,
        cost_per_1k_input: 0.015,
        cost_per_1k_output: 0.075,
      },
      'claude-3-sonnet': {
        model_id: 'claude-3-sonnet',
        request_count: 15000,
        total_input_tokens: 15000000,
        total_output_tokens: 6000000,
        avg_input_tokens: 1000,
        avg_output_tokens: 400,
        io_ratio: 2.5,
        total_cost_usd: 135.0,
        cost_per_1k_input: 0.003,
        cost_per_1k_output: 0.015,
      },
      'claude-3-haiku': {
        model_id: 'claude-3-haiku',
        request_count: 80000,
        total_input_tokens: 40000000,
        total_output_tokens: 20000000,
        avg_input_tokens: 500,
        avg_output_tokens: 250,
        io_ratio: 2.0,
        total_cost_usd: 35.0,
        cost_per_1k_input: 0.00025,
        cost_per_1k_output: 0.00125,
      },
    },
    by_service: {
      'llm-gateway': {
        service_id: 'llm-gateway',
        request_count: 80000,
        total_tokens: 70000000,
        avg_tokens_per_request: 875,
        efficiency_score: 0.75,
        primary_model: 'claude-3-haiku',
        models_used: ['claude-3-opus', 'claude-3-sonnet', 'claude-3-haiku'],
      },
      'chat-service': {
        service_id: 'chat-service',
        request_count: 15000,
        total_tokens: 18500000,
        avg_tokens_per_request: 1233,
        efficiency_score: 0.68,
        primary_model: 'claude-3-sonnet',
        models_used: ['claude-3-sonnet', 'claude-3-opus'],
      },
      'code-assistant': {
        service_id: 'code-assistant',
        request_count: 5000,
        total_tokens: 5000000,
        avg_tokens_per_request: 1000,
        efficiency_score: 0.82,
        primary_model: 'claude-3-opus',
        models_used: ['claude-3-opus'],
      },
    },
    by_use_case: {
      'chat_completion': {
        use_case_id: 'chat_completion',
        description: 'General chat completions',
        request_count: 60000,
        avg_input_tokens: 800,
        avg_output_tokens: 300,
        quality_score: 0.85,
        optimization_potential: 0.25,
      },
      'code_generation': {
        use_case_id: 'code_generation',
        description: 'Code generation tasks',
        request_count: 20000,
        avg_input_tokens: 1500,
        avg_output_tokens: 600,
        quality_score: 0.90,
        optimization_potential: 0.15,
      },
      'document_analysis': {
        use_case_id: 'document_analysis',
        description: 'Document analysis and summarization',
        request_count: 20000,
        avg_input_tokens: 3000,
        avg_output_tokens: 500,
        quality_score: 0.88,
        optimization_potential: 0.30,
      },
    },
    aggregate: {
      total_requests: 100000,
      total_input_tokens: 65000000,
      total_output_tokens: 28500000,
      total_cost_usd: 395.0,
      avg_input_per_request: 650,
      avg_output_per_request: 285,
      efficiency_score: 0.72,
      daily_trend: [
        { date: '2024-01-08', input_tokens: 9200000, output_tokens: 4000000, cost_usd: 55.0 },
        { date: '2024-01-09', input_tokens: 9400000, output_tokens: 4100000, cost_usd: 56.5 },
        { date: '2024-01-10', input_tokens: 9100000, output_tokens: 4050000, cost_usd: 55.2 },
        { date: '2024-01-11', input_tokens: 9500000, output_tokens: 4200000, cost_usd: 57.8 },
        { date: '2024-01-12', input_tokens: 9300000, output_tokens: 4100000, cost_usd: 56.3 },
        { date: '2024-01-13', input_tokens: 9200000, output_tokens: 4000000, cost_usd: 55.5 },
        { date: '2024-01-14', input_tokens: 9300000, output_tokens: 4050000, cost_usd: 56.7 },
      ],
    },
    detected_patterns: [],
    historical: {
      comparison_period: 'previous_week',
      token_change_pct: 5.2,
      cost_change_pct: 4.8,
      efficiency_change: -0.02,
    },
  };
}

/**
 * Format output for CLI display based on output_format parameter.
 */
export function formatOutputForCLI(
  output: TokenOptimizationOutput,
  format: 'json' | 'yaml' | 'table' | 'summary'
): string {
  switch (format) {
    case 'json':
      return JSON.stringify(output, null, 2);

    case 'summary':
      return formatSummary(output);

    case 'table':
      return formatTable(output);

    case 'yaml':
      // Basic YAML-like output
      return formatYaml(output);

    default:
      return JSON.stringify(output, null, 2);
  }
}

function formatSummary(output: TokenOptimizationOutput): string {
  const lines: string[] = [
    '╔══════════════════════════════════════════════════════════════════╗',
    '║         TOKEN OPTIMIZATION AGENT - ANALYSIS SUMMARY             ║',
    '╚══════════════════════════════════════════════════════════════════╝',
    '',
    `Execution: ${output.execution_ref}`,
    `Timestamp: ${output.timestamp}`,
    '',
    '─── Current State ───────────────────────────────────────────────────',
    `  Tokens Analyzed:    ${output.analysis.current_state.total_tokens_analyzed.toLocaleString()}`,
    `  Cost Analyzed:      $${output.analysis.current_state.total_cost_analyzed_usd.toFixed(2)}`,
    `  Overall Efficiency: ${(output.analysis.current_state.overall_efficiency * 100).toFixed(1)}%`,
    `  Input Efficiency:   ${(output.analysis.current_state.input_efficiency * 100).toFixed(1)}%`,
    `  Output Efficiency:  ${(output.analysis.current_state.output_efficiency * 100).toFixed(1)}%`,
    '',
    '─── Findings ────────────────────────────────────────────────────────',
    `  Patterns Identified:     ${output.analysis.patterns_identified}`,
    `  Opportunities Found:     ${output.analysis.opportunities_identified}`,
    `  Risk Level:              ${output.analysis.risk_level.toUpperCase()}`,
    '',
  ];

  if (output.analysis.key_findings.length > 0) {
    lines.push('─── Key Findings ────────────────────────────────────────────────────');
    for (const finding of output.analysis.key_findings) {
      lines.push(`  • ${finding}`);
    }
    lines.push('');
  }

  if (output.analysis.total_potential_savings.cost_usd > 0) {
    lines.push('─── Potential Savings ───────────────────────────────────────────────');
    lines.push(`  Input Tokens:       ${output.analysis.total_potential_savings.input_tokens.toLocaleString()}`);
    lines.push(`  Output Tokens:      ${output.analysis.total_potential_savings.output_tokens.toLocaleString()}`);
    lines.push(`  Daily Savings:      $${output.analysis.total_potential_savings.cost_usd.toFixed(2)}`);
    lines.push(`  Monthly Savings:    $${(output.analysis.total_potential_savings.cost_usd * 30).toFixed(2)}`);
    lines.push(`  Savings Percentage: ${output.analysis.total_potential_savings.savings_pct.toFixed(1)}%`);
    lines.push('');
  }

  if (output.opportunities.length > 0) {
    lines.push('─── Top Opportunities ───────────────────────────────────────────────');
    for (const opp of output.opportunities.slice(0, 5)) {
      lines.push(`  [${opp.priority.toFixed(2)}] ${opp.type.replace(/_/g, ' ').toUpperCase()}`);
      lines.push(`        Savings: $${opp.total_savings.cost_usd.toFixed(2)}/day | Quality Impact: ${(opp.quality_impact.expected_change * 100).toFixed(1)}%`);
      lines.push(`        Complexity: ${opp.complexity} | Risk: ${opp.quality_impact.risk_level}`);
      lines.push('');
    }
  }

  lines.push('═══════════════════════════════════════════════════════════════════');

  return lines.join('\n');
}

function formatTable(output: TokenOptimizationOutput): string {
  const lines: string[] = [
    '┌─────────────────────────────────────────────────────────────────────────────────┐',
    '│ RANK │ TYPE                  │ SAVINGS     │ QUALITY │ COMPLEXITY │ RISK       │',
    '├─────────────────────────────────────────────────────────────────────────────────┤',
  ];

  for (const opp of output.opportunities) {
    const type = opp.type.replace(/_/g, ' ').padEnd(20).substring(0, 20);
    const savings = `$${opp.total_savings.cost_usd.toFixed(2)}`.padStart(10);
    const quality = `${(opp.quality_impact.expected_change * 100).toFixed(1)}%`.padStart(7);
    const complexity = opp.complexity.padEnd(10);
    const risk = opp.quality_impact.risk_level.padEnd(10);

    lines.push(`│ ${String(output.opportunities.indexOf(opp) + 1).padStart(4)} │ ${type} │ ${savings} │ ${quality} │ ${complexity} │ ${risk} │`);
  }

  lines.push('└─────────────────────────────────────────────────────────────────────────────────┘');

  return lines.join('\n');
}

function formatYaml(output: TokenOptimizationOutput): string {
  const lines: string[] = [
    `output_id: ${output.output_id}`,
    `execution_ref: ${output.execution_ref}`,
    `agent_version: ${output.agent_version}`,
    `timestamp: ${output.timestamp}`,
    '',
    'analysis:',
    '  current_state:',
    `    total_tokens_analyzed: ${output.analysis.current_state.total_tokens_analyzed}`,
    `    total_cost_analyzed_usd: ${output.analysis.current_state.total_cost_analyzed_usd}`,
    `    overall_efficiency: ${output.analysis.current_state.overall_efficiency}`,
    `    input_efficiency: ${output.analysis.current_state.input_efficiency}`,
    `    output_efficiency: ${output.analysis.current_state.output_efficiency}`,
    `  patterns_identified: ${output.analysis.patterns_identified}`,
    `  opportunities_identified: ${output.analysis.opportunities_identified}`,
    `  risk_level: ${output.analysis.risk_level}`,
    '',
    'total_potential_savings:',
    `  input_tokens: ${output.analysis.total_potential_savings.input_tokens}`,
    `  output_tokens: ${output.analysis.total_potential_savings.output_tokens}`,
    `  total_tokens: ${output.analysis.total_potential_savings.total_tokens}`,
    `  cost_usd: ${output.analysis.total_potential_savings.cost_usd}`,
    `  savings_pct: ${output.analysis.total_potential_savings.savings_pct}`,
    '',
    'opportunities:',
  ];

  for (const opp of output.opportunities) {
    lines.push(`  - opportunity_id: ${opp.opportunity_id}`);
    lines.push(`    type: ${opp.type}`);
    lines.push(`    priority: ${opp.priority}`);
    lines.push(`    total_savings_cost_usd: ${opp.total_savings.cost_usd}`);
    lines.push(`    quality_impact: ${opp.quality_impact.expected_change}`);
    lines.push(`    complexity: ${opp.complexity}`);
    lines.push(`    risk_level: ${opp.quality_impact.risk_level}`);
  }

  return lines.join('\n');
}
