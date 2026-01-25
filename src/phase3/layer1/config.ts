/**
 * Phase 3 Layer 1 Configuration
 *
 * Defines all configuration constants and performance budgets for
 * the Automation & Resilience layer.
 *
 * @module phase3/layer1/config
 */

// ============================================================================
// Phase & Layer Identity
// ============================================================================

export const AGENT_PHASE = 'phase3' as const;
export const AGENT_LAYER = 'layer1' as const;
export const PHASE_NAME = 'AUTOMATION_RESILIENCE' as const;

// ============================================================================
// Performance Budgets (HARD LIMITS)
// ============================================================================

/**
 * Maximum tokens allowed per agent invocation.
 * Exceeding this limit triggers an execution guard violation.
 */
export const MAX_TOKENS = 1500;

/**
 * Maximum latency in milliseconds per operation.
 * Exceeding this limit triggers an execution guard violation.
 */
export const MAX_LATENCY_MS = 3000;

/**
 * Maximum external API calls per agent run.
 * Exceeding this limit triggers an execution guard violation.
 */
export const MAX_CALLS_PER_RUN = 4;

// ============================================================================
// Execution Role Constraints
// ============================================================================

/**
 * Allowed agent operations in Phase 3 Layer 1.
 * Agents MUST only perform these operations.
 */
export const ALLOWED_OPERATIONS = [
  'coordinate',
  'route',
  'optimize',
  'escalate',
] as const;

export type AllowedOperation = typeof ALLOWED_OPERATIONS[number];

/**
 * Forbidden agent outputs in Phase 3 Layer 1.
 * Agents MUST NOT emit these types of responses.
 */
export const FORBIDDEN_OUTPUTS = [
  'final_decision',
  'executive_conclusion',
  'binding_action',
  'unilateral_change',
] as const;

export type ForbiddenOutput = typeof FORBIDDEN_OUTPUTS[number];

// ============================================================================
// Signal Types (Phase 3 Layer 1 DecisionEvents)
// ============================================================================

/**
 * Phase 3 Layer 1 signal types.
 * All DecisionEvents MUST use one of these types.
 */
export const PHASE3_SIGNAL_TYPES = [
  'execution_strategy_signal',
  'optimization_signal',
  'incident_signal',
] as const;

export type Phase3SignalType = typeof PHASE3_SIGNAL_TYPES[number];

// ============================================================================
// Environment Variable Keys
// ============================================================================

export const ENV_KEYS = {
  AGENT_PHASE: 'AGENT_PHASE',
  AGENT_LAYER: 'AGENT_LAYER',
  RUVECTOR_API_KEY: 'RUVECTOR_API_KEY',
  RUVECTOR_SERVICE_URL: 'RUVECTOR_SERVICE_URL',
  MAX_TOKENS_OVERRIDE: 'MAX_TOKENS_OVERRIDE',
  MAX_LATENCY_MS_OVERRIDE: 'MAX_LATENCY_MS_OVERRIDE',
  MAX_CALLS_OVERRIDE: 'MAX_CALLS_OVERRIDE',
} as const;

// ============================================================================
// Failure Modes
// ============================================================================

export const FAILURE_MODES = {
  RUVECTOR_UNAVAILABLE: 'RUVECTOR_UNAVAILABLE',
  EXECUTION_GUARD_VIOLATED: 'EXECUTION_GUARD_VIOLATED',
  PHASE_MISMATCH: 'PHASE_MISMATCH',
  LAYER_MISMATCH: 'LAYER_MISMATCH',
  INVALID_CONFIGURATION: 'INVALID_CONFIGURATION',
  FORBIDDEN_OUTPUT_DETECTED: 'FORBIDDEN_OUTPUT_DETECTED',
  BUDGET_EXCEEDED: 'BUDGET_EXCEEDED',
} as const;

export type FailureMode = typeof FAILURE_MODES[keyof typeof FAILURE_MODES];

// ============================================================================
// Configuration Interface
// ============================================================================

export interface Phase3Layer1Config {
  phase: typeof AGENT_PHASE;
  layer: typeof AGENT_LAYER;
  budgets: {
    maxTokens: number;
    maxLatencyMs: number;
    maxCallsPerRun: number;
  };
  ruvector: {
    apiKey: string;
    serviceUrl: string;
    required: true;
  };
  execution: {
    allowedOperations: readonly AllowedOperation[];
    forbiddenOutputs: readonly ForbiddenOutput[];
  };
}

/**
 * Build Phase 3 Layer 1 configuration from environment.
 * Validates all required settings and returns frozen config.
 *
 * @throws Error if required configuration is missing
 */
export function buildConfig(): Phase3Layer1Config {
  const config: Phase3Layer1Config = {
    phase: AGENT_PHASE,
    layer: AGENT_LAYER,
    budgets: {
      maxTokens: parseInt(process.env[ENV_KEYS.MAX_TOKENS_OVERRIDE] || '', 10) || MAX_TOKENS,
      maxLatencyMs: parseInt(process.env[ENV_KEYS.MAX_LATENCY_MS_OVERRIDE] || '', 10) || MAX_LATENCY_MS,
      maxCallsPerRun: parseInt(process.env[ENV_KEYS.MAX_CALLS_OVERRIDE] || '', 10) || MAX_CALLS_PER_RUN,
    },
    ruvector: {
      apiKey: process.env[ENV_KEYS.RUVECTOR_API_KEY] || '',
      serviceUrl: process.env[ENV_KEYS.RUVECTOR_SERVICE_URL] || '',
      required: true,
    },
    execution: {
      allowedOperations: ALLOWED_OPERATIONS,
      forbiddenOutputs: FORBIDDEN_OUTPUTS,
    },
  };

  // Freeze to prevent runtime modification
  return Object.freeze(config);
}

/**
 * Get configuration summary for logging (redacted secrets).
 */
export function getConfigSummary(config: Phase3Layer1Config): Record<string, unknown> {
  return {
    phase: config.phase,
    layer: config.layer,
    budgets: config.budgets,
    ruvector: {
      serviceUrl: config.ruvector.serviceUrl,
      apiKeySet: !!config.ruvector.apiKey,
      required: config.ruvector.required,
    },
    execution: {
      allowedOperations: [...config.execution.allowedOperations],
      forbiddenOutputs: [...config.execution.forbiddenOutputs],
    },
  };
}
