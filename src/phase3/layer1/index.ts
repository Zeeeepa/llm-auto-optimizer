/**
 * Phase 3 Layer 1 - Automation & Resilience
 *
 * This module provides the complete Phase 3 Layer 1 implementation including:
 * - Startup hardening with crashloop behavior
 * - Execution guards with performance budgets
 * - Signal types for DecisionEvents
 * - Role clarity enforcement (coordinate, route, optimize, escalate only)
 *
 * @module phase3/layer1
 */

// ============================================================================
// Configuration Exports
// ============================================================================

export {
  // Constants
  AGENT_PHASE,
  AGENT_LAYER,
  PHASE_NAME,
  MAX_TOKENS,
  MAX_LATENCY_MS,
  MAX_CALLS_PER_RUN,
  ALLOWED_OPERATIONS,
  FORBIDDEN_OUTPUTS,
  PHASE3_SIGNAL_TYPES,
  ENV_KEYS,
  FAILURE_MODES,

  // Types
  type AllowedOperation,
  type ForbiddenOutput,
  type Phase3SignalType,
  type FailureMode,
  type Phase3Layer1Config,

  // Functions
  buildConfig,
  getConfigSummary,
} from './config';

// ============================================================================
// Startup Guard Exports
// ============================================================================

export {
  StartupGuard,
  StartupValidationError,
  getStartupGuard,
  verifyRuVectorConnectivity,

  // Types
  type ValidationResult,
  type ValidationError,
  type ValidationWarning,
} from './startup-guard';

// ============================================================================
// Execution Guard Exports
// ============================================================================

export {
  ExecutionGuard,
  ExecutionGuardViolation,
  createExecutionGuard,
  checkRuVectorAvailability,

  // Types
  type ViolationDetails,
  type ExecutionMetrics,
} from './execution-guard';

// ============================================================================
// Signal Exports
// ============================================================================

export {
  buildPhase3Signal,
  isExecutionStrategySignal,
  isOptimizationSignal,
  isIncidentSignal,

  // Types
  type Phase3Signal,
  type SignalPayload,
  type ExecutionStrategyPayload,
  type OptimizationPayload,
  type IncidentPayload,
  type SignalReference,
  type BuildSignalParams,
} from './signals';

// ============================================================================
// Bootstrap Function
// ============================================================================

import { StartupGuard, getStartupGuard, verifyRuVectorConnectivity, StartupValidationError } from './startup-guard';
import type { Phase3Layer1Config } from './config';
import { AGENT_PHASE, AGENT_LAYER } from './config';

export interface BootstrapResult {
  success: boolean;
  config: Phase3Layer1Config | null;
  error?: string;
  errorDetails?: Record<string, unknown>;
}

/**
 * Bootstrap Phase 3 Layer 1.
 * This function MUST be called at service startup.
 * It will cause a crashloop if configuration is invalid.
 *
 * @returns BootstrapResult with config on success
 * @throws StartupValidationError to trigger crashloop on failure
 */
export async function bootstrapPhase3Layer1(): Promise<BootstrapResult> {
  console.log(JSON.stringify({
    level: 'info',
    message: 'phase3_layer1_bootstrap_starting',
    phase: AGENT_PHASE,
    layer: AGENT_LAYER,
    timestamp: new Date().toISOString(),
  }));

  try {
    // Step 1: Validate startup configuration
    const guard = getStartupGuard();
    const config = await guard.validate();

    // Step 2: Verify RuVector connectivity (MANDATORY)
    await verifyRuVectorConnectivity(config);

    console.log(JSON.stringify({
      level: 'info',
      message: 'phase3_layer1_bootstrap_complete',
      phase: AGENT_PHASE,
      layer: AGENT_LAYER,
      timestamp: new Date().toISOString(),
    }));

    return {
      success: true,
      config,
    };
  } catch (error) {
    // Log fatal error before re-throwing to trigger crashloop
    console.error(JSON.stringify({
      level: 'fatal',
      message: 'phase3_layer1_bootstrap_failed',
      phase: AGENT_PHASE,
      layer: AGENT_LAYER,
      error: error instanceof Error ? error.message : 'Unknown error',
      errorType: error instanceof StartupValidationError ? 'StartupValidationError' : 'UnknownError',
      timestamp: new Date().toISOString(),
    }));

    // Re-throw to cause crashloop
    throw error;
  }
}
