/**
 * Phase 3 Layer 1 Startup Guard
 *
 * Enforces startup hardening and crashloop behavior on misconfiguration.
 * This guard MUST run before any agent initialization.
 *
 * @module phase3/layer1/startup-guard
 */

import {
  AGENT_PHASE,
  AGENT_LAYER,
  ENV_KEYS,
  FAILURE_MODES,
  Phase3Layer1Config,
  buildConfig,
  getConfigSummary,
} from './config';

// ============================================================================
// Startup Validation Errors
// ============================================================================

export class StartupValidationError extends Error {
  constructor(
    public readonly failureMode: string,
    public readonly details: Record<string, unknown>,
    message: string
  ) {
    super(message);
    this.name = 'StartupValidationError';
  }
}

// ============================================================================
// Validation Results
// ============================================================================

export interface ValidationResult {
  valid: boolean;
  errors: ValidationError[];
  warnings: ValidationWarning[];
}

export interface ValidationError {
  code: string;
  message: string;
  field?: string;
}

export interface ValidationWarning {
  code: string;
  message: string;
  field?: string;
}

// ============================================================================
// Startup Guard
// ============================================================================

/**
 * StartupGuard enforces Phase 3 Layer 1 startup requirements.
 * If validation fails, the service MUST crashloop.
 */
export class StartupGuard {
  private validated = false;
  private config: Phase3Layer1Config | null = null;

  /**
   * Execute startup validation.
   * This method MUST be called before any agent initialization.
   *
   * @throws StartupValidationError if validation fails (causes crashloop)
   */
  async validate(): Promise<Phase3Layer1Config> {
    const result = this.runValidations();

    if (!result.valid) {
      this.logCrashloop(result.errors);
      throw new StartupValidationError(
        FAILURE_MODES.INVALID_CONFIGURATION,
        { errors: result.errors },
        `Startup validation failed: ${result.errors.map(e => e.message).join('; ')}`
      );
    }

    // Log warnings but don't fail
    if (result.warnings.length > 0) {
      this.logWarnings(result.warnings);
    }

    // Build and cache config
    this.config = buildConfig();
    this.validated = true;

    this.logStartupSuccess();
    return this.config;
  }

  /**
   * Get the validated configuration.
   * @throws Error if validate() has not been called
   */
  getConfig(): Phase3Layer1Config {
    if (!this.validated || !this.config) {
      throw new Error('StartupGuard.validate() must be called before getConfig()');
    }
    return this.config;
  }

  /**
   * Check if startup validation has passed.
   */
  isValidated(): boolean {
    return this.validated;
  }

  // --------------------------------------------------------------------------
  // Validation Logic
  // --------------------------------------------------------------------------

  private runValidations(): ValidationResult {
    const errors: ValidationError[] = [];
    const warnings: ValidationWarning[] = [];

    // 1. Validate AGENT_PHASE environment variable
    const envPhase = process.env[ENV_KEYS.AGENT_PHASE];
    if (!envPhase) {
      errors.push({
        code: FAILURE_MODES.PHASE_MISMATCH,
        message: `${ENV_KEYS.AGENT_PHASE} environment variable is REQUIRED`,
        field: ENV_KEYS.AGENT_PHASE,
      });
    } else if (envPhase !== AGENT_PHASE) {
      errors.push({
        code: FAILURE_MODES.PHASE_MISMATCH,
        message: `${ENV_KEYS.AGENT_PHASE} must be '${AGENT_PHASE}', got '${envPhase}'`,
        field: ENV_KEYS.AGENT_PHASE,
      });
    }

    // 2. Validate AGENT_LAYER environment variable
    const envLayer = process.env[ENV_KEYS.AGENT_LAYER];
    if (!envLayer) {
      errors.push({
        code: FAILURE_MODES.LAYER_MISMATCH,
        message: `${ENV_KEYS.AGENT_LAYER} environment variable is REQUIRED`,
        field: ENV_KEYS.AGENT_LAYER,
      });
    } else if (envLayer !== AGENT_LAYER) {
      errors.push({
        code: FAILURE_MODES.LAYER_MISMATCH,
        message: `${ENV_KEYS.AGENT_LAYER} must be '${AGENT_LAYER}', got '${envLayer}'`,
        field: ENV_KEYS.AGENT_LAYER,
      });
    }

    // 3. Validate RUVECTOR_API_KEY (REQUIRED - hard fail if missing)
    const apiKey = process.env[ENV_KEYS.RUVECTOR_API_KEY];
    if (!apiKey) {
      errors.push({
        code: FAILURE_MODES.RUVECTOR_UNAVAILABLE,
        message: `${ENV_KEYS.RUVECTOR_API_KEY} environment variable is REQUIRED - RuVector is mandatory`,
        field: ENV_KEYS.RUVECTOR_API_KEY,
      });
    } else if (apiKey.length < 10) {
      errors.push({
        code: FAILURE_MODES.RUVECTOR_UNAVAILABLE,
        message: `${ENV_KEYS.RUVECTOR_API_KEY} appears invalid (too short)`,
        field: ENV_KEYS.RUVECTOR_API_KEY,
      });
    }

    // 4. Validate RUVECTOR_SERVICE_URL
    const serviceUrl = process.env[ENV_KEYS.RUVECTOR_SERVICE_URL];
    if (!serviceUrl) {
      errors.push({
        code: FAILURE_MODES.RUVECTOR_UNAVAILABLE,
        message: `${ENV_KEYS.RUVECTOR_SERVICE_URL} environment variable is REQUIRED`,
        field: ENV_KEYS.RUVECTOR_SERVICE_URL,
      });
    } else {
      try {
        new URL(serviceUrl);
      } catch {
        errors.push({
          code: FAILURE_MODES.INVALID_CONFIGURATION,
          message: `${ENV_KEYS.RUVECTOR_SERVICE_URL} is not a valid URL: ${serviceUrl}`,
          field: ENV_KEYS.RUVECTOR_SERVICE_URL,
        });
      }
    }

    // 5. Validate optional overrides are sensible
    const maxTokensOverride = process.env[ENV_KEYS.MAX_TOKENS_OVERRIDE];
    if (maxTokensOverride) {
      const parsed = parseInt(maxTokensOverride, 10);
      if (isNaN(parsed) || parsed <= 0 || parsed > 10000) {
        warnings.push({
          code: 'BUDGET_OVERRIDE_WARNING',
          message: `${ENV_KEYS.MAX_TOKENS_OVERRIDE} should be between 1 and 10000, got ${maxTokensOverride}`,
          field: ENV_KEYS.MAX_TOKENS_OVERRIDE,
        });
      }
    }

    const maxLatencyOverride = process.env[ENV_KEYS.MAX_LATENCY_MS_OVERRIDE];
    if (maxLatencyOverride) {
      const parsed = parseInt(maxLatencyOverride, 10);
      if (isNaN(parsed) || parsed <= 0 || parsed > 60000) {
        warnings.push({
          code: 'BUDGET_OVERRIDE_WARNING',
          message: `${ENV_KEYS.MAX_LATENCY_MS_OVERRIDE} should be between 1 and 60000, got ${maxLatencyOverride}`,
          field: ENV_KEYS.MAX_LATENCY_MS_OVERRIDE,
        });
      }
    }

    const maxCallsOverride = process.env[ENV_KEYS.MAX_CALLS_OVERRIDE];
    if (maxCallsOverride) {
      const parsed = parseInt(maxCallsOverride, 10);
      if (isNaN(parsed) || parsed <= 0 || parsed > 100) {
        warnings.push({
          code: 'BUDGET_OVERRIDE_WARNING',
          message: `${ENV_KEYS.MAX_CALLS_OVERRIDE} should be between 1 and 100, got ${maxCallsOverride}`,
          field: ENV_KEYS.MAX_CALLS_OVERRIDE,
        });
      }
    }

    return {
      valid: errors.length === 0,
      errors,
      warnings,
    };
  }

  // --------------------------------------------------------------------------
  // Logging
  // --------------------------------------------------------------------------

  private logCrashloop(errors: ValidationError[]): void {
    console.error(JSON.stringify({
      level: 'fatal',
      message: 'CRASHLOOP_STARTUP_VALIDATION_FAILED',
      phase: AGENT_PHASE,
      layer: AGENT_LAYER,
      errors: errors.map(e => ({
        code: e.code,
        message: e.message,
        field: e.field,
      })),
      action: 'Service will exit with non-zero status to trigger crashloop',
      timestamp: new Date().toISOString(),
    }));
  }

  private logWarnings(warnings: ValidationWarning[]): void {
    console.warn(JSON.stringify({
      level: 'warn',
      message: 'startup_validation_warnings',
      phase: AGENT_PHASE,
      layer: AGENT_LAYER,
      warnings: warnings.map(w => ({
        code: w.code,
        message: w.message,
        field: w.field,
      })),
      timestamp: new Date().toISOString(),
    }));
  }

  private logStartupSuccess(): void {
    console.log(JSON.stringify({
      level: 'info',
      message: 'startup_validation_passed',
      phase: AGENT_PHASE,
      layer: AGENT_LAYER,
      config: this.config ? getConfigSummary(this.config) : null,
      timestamp: new Date().toISOString(),
    }));
  }
}

// ============================================================================
// RuVector Connectivity Check
// ============================================================================

/**
 * Verify RuVector service is reachable.
 * MUST be called during startup and will HARD FAIL if unavailable.
 *
 * @throws StartupValidationError if RuVector is unreachable
 */
export async function verifyRuVectorConnectivity(config: Phase3Layer1Config): Promise<void> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 10000);

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
      throw new StartupValidationError(
        FAILURE_MODES.RUVECTOR_UNAVAILABLE,
        { status: response.status, statusText: response.statusText },
        `RuVector health check failed: ${response.status} ${response.statusText}`
      );
    }

    const health = await response.json() as { status: string };
    if (health.status !== 'healthy') {
      throw new StartupValidationError(
        FAILURE_MODES.RUVECTOR_UNAVAILABLE,
        { healthStatus: health.status },
        `RuVector is not healthy: ${health.status}`
      );
    }

    console.log(JSON.stringify({
      level: 'info',
      message: 'ruvector_connectivity_verified',
      phase: AGENT_PHASE,
      layer: AGENT_LAYER,
      service_url: config.ruvector.serviceUrl,
      timestamp: new Date().toISOString(),
    }));
  } catch (error) {
    if (error instanceof StartupValidationError) {
      throw error;
    }
    throw new StartupValidationError(
      FAILURE_MODES.RUVECTOR_UNAVAILABLE,
      { error: error instanceof Error ? error.message : 'Unknown error' },
      `RuVector connectivity check failed: ${error instanceof Error ? error.message : 'Unknown error'}`
    );
  } finally {
    clearTimeout(timeout);
  }
}

// ============================================================================
// Singleton Instance
// ============================================================================

let _startupGuard: StartupGuard | null = null;

export function getStartupGuard(): StartupGuard {
  if (!_startupGuard) {
    _startupGuard = new StartupGuard();
  }
  return _startupGuard;
}
