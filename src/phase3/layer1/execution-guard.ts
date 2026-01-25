/**
 * Phase 3 Layer 1 Execution Guard
 *
 * Enforces runtime execution constraints including:
 * - Performance budgets (tokens, latency, API calls)
 * - Execution role clarity (coordinate, route, optimize, escalate only)
 * - Forbidden output detection
 *
 * Violations trigger HARD FAIL.
 *
 * @module phase3/layer1/execution-guard
 */

import {
  Phase3Layer1Config,
  FAILURE_MODES,
  FailureMode,
  ALLOWED_OPERATIONS,
  AllowedOperation,
  FORBIDDEN_OUTPUTS,
  ForbiddenOutput,
  AGENT_PHASE,
  AGENT_LAYER,
} from './config';

// ============================================================================
// Execution Guard Error
// ============================================================================

export class ExecutionGuardViolation extends Error {
  constructor(
    public readonly failureMode: FailureMode,
    public readonly violation: ViolationDetails,
    message: string
  ) {
    super(message);
    this.name = 'ExecutionGuardViolation';
  }
}

export interface ViolationDetails {
  /** Violation type */
  type: 'budget_exceeded' | 'forbidden_output' | 'invalid_operation' | 'ruvector_unavailable';
  /** What was violated */
  constraint: string;
  /** Actual value (if applicable) */
  actual?: number | string;
  /** Limit value (if applicable) */
  limit?: number | string;
  /** Additional context */
  context?: Record<string, unknown>;
}

// ============================================================================
// Execution Metrics
// ============================================================================

export interface ExecutionMetrics {
  /** Tokens used in this execution */
  tokensUsed: number;
  /** Latency in milliseconds */
  latencyMs: number;
  /** Number of external API calls made */
  apiCallsMade: number;
  /** Operations performed */
  operations: AllowedOperation[];
  /** Timestamp of execution start */
  startTime: number;
}

// ============================================================================
// Execution Guard
// ============================================================================

/**
 * ExecutionGuard enforces runtime constraints for Phase 3 Layer 1.
 * Create a new instance for each agent invocation.
 */
export class ExecutionGuard {
  private metrics: ExecutionMetrics;
  private config: Phase3Layer1Config;
  private violations: ViolationDetails[] = [];
  private finalized = false;

  constructor(config: Phase3Layer1Config) {
    this.config = config;
    this.metrics = {
      tokensUsed: 0,
      latencyMs: 0,
      apiCallsMade: 0,
      operations: [],
      startTime: Date.now(),
    };
  }

  // --------------------------------------------------------------------------
  // Budget Tracking
  // --------------------------------------------------------------------------

  /**
   * Record token usage.
   * @throws ExecutionGuardViolation if budget exceeded
   */
  recordTokens(tokens: number): void {
    this.checkNotFinalized();
    this.metrics.tokensUsed += tokens;

    if (this.metrics.tokensUsed > this.config.budgets.maxTokens) {
      const violation: ViolationDetails = {
        type: 'budget_exceeded',
        constraint: 'MAX_TOKENS',
        actual: this.metrics.tokensUsed,
        limit: this.config.budgets.maxTokens,
      };
      this.violations.push(violation);
      this.throwViolation(FAILURE_MODES.BUDGET_EXCEEDED, violation,
        `Token budget exceeded: ${this.metrics.tokensUsed} > ${this.config.budgets.maxTokens}`);
    }
  }

  /**
   * Record an external API call.
   * @throws ExecutionGuardViolation if call limit exceeded
   */
  recordApiCall(target: string): void {
    this.checkNotFinalized();
    this.metrics.apiCallsMade += 1;

    if (this.metrics.apiCallsMade > this.config.budgets.maxCallsPerRun) {
      const violation: ViolationDetails = {
        type: 'budget_exceeded',
        constraint: 'MAX_CALLS_PER_RUN',
        actual: this.metrics.apiCallsMade,
        limit: this.config.budgets.maxCallsPerRun,
        context: { lastCall: target },
      };
      this.violations.push(violation);
      this.throwViolation(FAILURE_MODES.BUDGET_EXCEEDED, violation,
        `API call budget exceeded: ${this.metrics.apiCallsMade} > ${this.config.budgets.maxCallsPerRun}`);
    }
  }

  /**
   * Record an operation being performed.
   * @throws ExecutionGuardViolation if operation not allowed
   */
  recordOperation(operation: string): void {
    this.checkNotFinalized();

    if (!ALLOWED_OPERATIONS.includes(operation as AllowedOperation)) {
      const violation: ViolationDetails = {
        type: 'invalid_operation',
        constraint: 'ALLOWED_OPERATIONS',
        actual: operation,
        limit: ALLOWED_OPERATIONS.join(', '),
      };
      this.violations.push(violation);
      this.throwViolation(FAILURE_MODES.EXECUTION_GUARD_VIOLATED, violation,
        `Operation '${operation}' is not allowed in Phase 3 Layer 1. Allowed: ${ALLOWED_OPERATIONS.join(', ')}`);
    }

    this.metrics.operations.push(operation as AllowedOperation);
  }

  // --------------------------------------------------------------------------
  // Output Validation
  // --------------------------------------------------------------------------

  /**
   * Check output for forbidden content.
   * @throws ExecutionGuardViolation if forbidden output detected
   */
  validateOutput(output: unknown): void {
    this.checkNotFinalized();

    if (!output || typeof output !== 'object') {
      return;
    }

    const outputStr = JSON.stringify(output).toLowerCase();

    // Check for forbidden output patterns
    const forbiddenPatterns: Array<{ pattern: RegExp; type: ForbiddenOutput }> = [
      { pattern: /final[_\s-]?decision/i, type: 'final_decision' },
      { pattern: /executive[_\s-]?conclusion/i, type: 'executive_conclusion' },
      { pattern: /binding[_\s-]?action/i, type: 'binding_action' },
      { pattern: /unilateral[_\s-]?change/i, type: 'unilateral_change' },
      { pattern: /"type"\s*:\s*"decision"/i, type: 'final_decision' },
      { pattern: /"is_final"\s*:\s*true/i, type: 'final_decision' },
      { pattern: /"execute_immediately"\s*:\s*true/i, type: 'binding_action' },
    ];

    for (const { pattern, type } of forbiddenPatterns) {
      if (pattern.test(outputStr)) {
        const violation: ViolationDetails = {
          type: 'forbidden_output',
          constraint: 'FORBIDDEN_OUTPUTS',
          actual: type,
          limit: 'Signal only (no final decisions)',
          context: { matchedPattern: pattern.source },
        };
        this.violations.push(violation);
        this.throwViolation(FAILURE_MODES.FORBIDDEN_OUTPUT_DETECTED, violation,
          `Forbidden output detected: '${type}'. Phase 3 Layer 1 agents MUST NOT emit final decisions.`);
      }
    }
  }

  /**
   * Validate that output explicitly indicates it's a signal/recommendation.
   */
  validateOutputIsSignal(output: unknown): void {
    this.checkNotFinalized();

    if (!output || typeof output !== 'object') {
      const violation: ViolationDetails = {
        type: 'forbidden_output',
        constraint: 'OUTPUT_MUST_BE_SIGNAL',
        actual: typeof output,
        limit: 'object with signal indicators',
      };
      this.violations.push(violation);
      this.throwViolation(FAILURE_MODES.FORBIDDEN_OUTPUT_DETECTED, violation,
        'Output must be an object containing signal indicators');
    }

    const obj = output as Record<string, unknown>;

    // Output should NOT contain decision indicators
    if (obj.is_decision === true || obj.is_final === true) {
      const violation: ViolationDetails = {
        type: 'forbidden_output',
        constraint: 'OUTPUT_MUST_BE_SIGNAL',
        actual: 'is_decision=true or is_final=true',
        limit: 'Signals only, not decisions',
      };
      this.violations.push(violation);
      this.throwViolation(FAILURE_MODES.FORBIDDEN_OUTPUT_DETECTED, violation,
        'Output must be a signal, not a decision (is_decision/is_final should not be true)');
    }
  }

  // --------------------------------------------------------------------------
  // Finalization
  // --------------------------------------------------------------------------

  /**
   * Finalize the execution guard and check latency budget.
   * @throws ExecutionGuardViolation if latency budget exceeded
   */
  finalize(): ExecutionMetrics {
    if (this.finalized) {
      return this.metrics;
    }

    this.metrics.latencyMs = Date.now() - this.metrics.startTime;

    if (this.metrics.latencyMs > this.config.budgets.maxLatencyMs) {
      const violation: ViolationDetails = {
        type: 'budget_exceeded',
        constraint: 'MAX_LATENCY_MS',
        actual: this.metrics.latencyMs,
        limit: this.config.budgets.maxLatencyMs,
      };
      this.violations.push(violation);
      this.throwViolation(FAILURE_MODES.BUDGET_EXCEEDED, violation,
        `Latency budget exceeded: ${this.metrics.latencyMs}ms > ${this.config.budgets.maxLatencyMs}ms`);
    }

    this.finalized = true;
    this.logMetrics();
    return this.metrics;
  }

  /**
   * Get current metrics without finalizing.
   */
  getMetrics(): ExecutionMetrics {
    return {
      ...this.metrics,
      latencyMs: Date.now() - this.metrics.startTime,
    };
  }

  /**
   * Get any recorded violations (for testing/debugging).
   */
  getViolations(): ViolationDetails[] {
    return [...this.violations];
  }

  // --------------------------------------------------------------------------
  // Internal
  // --------------------------------------------------------------------------

  private checkNotFinalized(): void {
    if (this.finalized) {
      throw new Error('ExecutionGuard has been finalized');
    }
  }

  private throwViolation(mode: FailureMode, details: ViolationDetails, message: string): never {
    this.logViolation(details);
    throw new ExecutionGuardViolation(mode, details, message);
  }

  private logViolation(violation: ViolationDetails): void {
    console.error(JSON.stringify({
      level: 'error',
      message: 'execution_guard_violation',
      phase: AGENT_PHASE,
      layer: AGENT_LAYER,
      violation,
      metrics: this.getMetrics(),
      timestamp: new Date().toISOString(),
    }));
  }

  private logMetrics(): void {
    console.log(JSON.stringify({
      level: 'info',
      message: 'execution_metrics_finalized',
      phase: AGENT_PHASE,
      layer: AGENT_LAYER,
      metrics: this.metrics,
      budgets: this.config.budgets,
      budgetUtilization: {
        tokens: (this.metrics.tokensUsed / this.config.budgets.maxTokens * 100).toFixed(1) + '%',
        latency: (this.metrics.latencyMs / this.config.budgets.maxLatencyMs * 100).toFixed(1) + '%',
        apiCalls: (this.metrics.apiCallsMade / this.config.budgets.maxCallsPerRun * 100).toFixed(1) + '%',
      },
      timestamp: new Date().toISOString(),
    }));
  }
}

// ============================================================================
// Factory Function
// ============================================================================

/**
 * Create an execution guard for a new agent invocation.
 */
export function createExecutionGuard(config: Phase3Layer1Config): ExecutionGuard {
  return new ExecutionGuard(config);
}

// ============================================================================
// RuVector Availability Check
// ============================================================================

/**
 * Check RuVector availability at runtime.
 * MUST be called before any persistence operation.
 *
 * @throws ExecutionGuardViolation if RuVector is unavailable
 */
export async function checkRuVectorAvailability(
  config: Phase3Layer1Config,
  guard: ExecutionGuard
): Promise<void> {
  guard.recordApiCall('ruvector-health-check');

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 5000);

  try {
    const response = await fetch(`${config.ruvector.serviceUrl}/api/v1/health`, {
      method: 'GET',
      headers: {
        'Authorization': `Bearer ${config.ruvector.apiKey}`,
        'X-Client': 'llm-auto-optimizer',
        'X-Phase': AGENT_PHASE,
        'X-Layer': AGENT_LAYER,
      },
      signal: controller.signal,
    });

    if (!response.ok) {
      throw new ExecutionGuardViolation(
        FAILURE_MODES.RUVECTOR_UNAVAILABLE,
        {
          type: 'ruvector_unavailable',
          constraint: 'RUVECTOR_REQUIRED',
          actual: `HTTP ${response.status}`,
          limit: 'HTTP 200 OK',
        },
        `RuVector is unavailable: ${response.status} ${response.statusText}`
      );
    }
  } catch (error) {
    if (error instanceof ExecutionGuardViolation) {
      throw error;
    }
    throw new ExecutionGuardViolation(
      FAILURE_MODES.RUVECTOR_UNAVAILABLE,
      {
        type: 'ruvector_unavailable',
        constraint: 'RUVECTOR_REQUIRED',
        actual: error instanceof Error ? error.message : 'Unknown error',
        limit: 'Healthy connection',
      },
      `RuVector connectivity failed: ${error instanceof Error ? error.message : 'Unknown error'}`
    );
  } finally {
    clearTimeout(timeout);
  }
}
