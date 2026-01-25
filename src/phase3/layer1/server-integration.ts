/**
 * Phase 3 Layer 1 Server Integration
 *
 * Integrates Phase 3 Layer 1 bootstrap and guards into the main server.
 * This module provides middleware and hooks for the HTTP server.
 *
 * @module phase3/layer1/server-integration
 */

import { IncomingMessage, ServerResponse } from 'http';
import {
  AGENT_PHASE,
  AGENT_LAYER,
  Phase3Layer1Config,
  bootstrapPhase3Layer1,
  createExecutionGuard,
  ExecutionGuard,
  ExecutionGuardViolation,
  checkRuVectorAvailability,
  FAILURE_MODES,
} from './index';

// ============================================================================
// Server State
// ============================================================================

let _phase3Config: Phase3Layer1Config | null = null;
let _bootstrapComplete = false;
let _bootstrapError: Error | null = null;

// ============================================================================
// Bootstrap Integration
// ============================================================================

/**
 * Initialize Phase 3 Layer 1 for the server.
 * MUST be called before server starts accepting requests.
 *
 * @throws Error if bootstrap fails (triggers crashloop)
 */
export async function initializePhase3Layer1(): Promise<Phase3Layer1Config> {
  if (_bootstrapComplete && _phase3Config) {
    return _phase3Config;
  }

  if (_bootstrapError) {
    throw _bootstrapError;
  }

  try {
    const result = await bootstrapPhase3Layer1();
    if (!result.success || !result.config) {
      const error = new Error(result.error || 'Bootstrap failed');
      _bootstrapError = error;
      throw error;
    }
    _phase3Config = result.config;
    _bootstrapComplete = true;
    return _phase3Config;
  } catch (error) {
    _bootstrapError = error as Error;
    throw error;
  }
}

/**
 * Check if Phase 3 Layer 1 is initialized.
 */
export function isPhase3Initialized(): boolean {
  return _bootstrapComplete && _phase3Config !== null;
}

/**
 * Get the Phase 3 Layer 1 configuration.
 * @throws Error if not initialized
 */
export function getPhase3Config(): Phase3Layer1Config {
  if (!_phase3Config) {
    throw new Error('Phase 3 Layer 1 not initialized. Call initializePhase3Layer1() first.');
  }
  return _phase3Config;
}

// ============================================================================
// Request Context
// ============================================================================

/**
 * Context attached to each request for Phase 3 Layer 1.
 */
export interface Phase3RequestContext {
  /** Execution guard for this request */
  guard: ExecutionGuard;
  /** Request ID */
  requestId: string;
  /** Start timestamp */
  startTime: number;
  /** Phase identifier */
  phase: typeof AGENT_PHASE;
  /** Layer identifier */
  layer: typeof AGENT_LAYER;
}

// WeakMap to store context per request
const requestContexts = new WeakMap<IncomingMessage, Phase3RequestContext>();

/**
 * Create Phase 3 request context for an incoming request.
 */
export function createRequestContext(req: IncomingMessage): Phase3RequestContext {
  const config = getPhase3Config();
  const requestId = generateRequestId();
  const guard = createExecutionGuard(config);

  const context: Phase3RequestContext = {
    guard,
    requestId,
    startTime: Date.now(),
    phase: AGENT_PHASE,
    layer: AGENT_LAYER,
  };

  requestContexts.set(req, context);
  return context;
}

/**
 * Get Phase 3 request context.
 */
export function getRequestContext(req: IncomingMessage): Phase3RequestContext | undefined {
  return requestContexts.get(req);
}

/**
 * Finalize request context and clean up.
 */
export function finalizeRequestContext(req: IncomingMessage): void {
  const context = requestContexts.get(req);
  if (context) {
    try {
      context.guard.finalize();
    } catch (error) {
      // Log but don't throw on finalization errors
      console.error(JSON.stringify({
        level: 'error',
        message: 'request_context_finalization_error',
        requestId: context.requestId,
        error: error instanceof Error ? error.message : 'Unknown error',
        timestamp: new Date().toISOString(),
      }));
    }
    requestContexts.delete(req);
  }
}

// ============================================================================
// Middleware
// ============================================================================

/**
 * Phase 3 Layer 1 middleware for request handling.
 * Wraps the request with execution guards.
 *
 * @param handler - The actual request handler
 * @returns Wrapped handler with Phase 3 guards
 */
export function phase3Middleware(
  handler: (req: IncomingMessage, res: ServerResponse) => Promise<void>
): (req: IncomingMessage, res: ServerResponse) => Promise<void> {
  return async (req: IncomingMessage, res: ServerResponse) => {
    // Skip Phase 3 guards for health/ready endpoints
    const path = req.url?.split('?')[0] || '/';
    if (path === '/health' || path === '/ready') {
      return handler(req, res);
    }

    // Create request context
    const context = createRequestContext(req);

    // Add Phase 3 headers to response
    res.setHeader('X-Phase', AGENT_PHASE);
    res.setHeader('X-Layer', AGENT_LAYER);
    res.setHeader('X-Request-Id', context.requestId);

    try {
      // Check RuVector availability before processing
      await checkRuVectorAvailability(getPhase3Config(), context.guard);

      // Execute the actual handler
      await handler(req, res);
    } catch (error) {
      if (error instanceof ExecutionGuardViolation) {
        // Send guard violation response
        sendGuardViolationResponse(res, error, context);
      } else {
        // Re-throw other errors
        throw error;
      }
    } finally {
      finalizeRequestContext(req);
    }
  };
}

/**
 * Send a guard violation error response.
 */
function sendGuardViolationResponse(
  res: ServerResponse,
  error: ExecutionGuardViolation,
  context: Phase3RequestContext
): void {
  const status = error.failureMode === FAILURE_MODES.RUVECTOR_UNAVAILABLE ? 503 : 422;

  res.writeHead(status, {
    'Content-Type': 'application/json',
    'X-Phase': AGENT_PHASE,
    'X-Layer': AGENT_LAYER,
    'X-Request-Id': context.requestId,
    'X-Failure-Mode': error.failureMode,
  });

  res.end(JSON.stringify({
    error: {
      code: error.failureMode,
      message: error.message,
      violation: error.violation,
    },
    phase: AGENT_PHASE,
    layer: AGENT_LAYER,
    request_id: context.requestId,
    timestamp: new Date().toISOString(),
  }));
}

// ============================================================================
// Health Check Enhancements
// ============================================================================

/**
 * Get Phase 3 Layer 1 health status.
 */
export function getPhase3HealthStatus(): {
  phase3_initialized: boolean;
  phase: string;
  layer: string;
  budgets: Record<string, number> | null;
  ruvector_configured: boolean;
} {
  return {
    phase3_initialized: _bootstrapComplete,
    phase: AGENT_PHASE,
    layer: AGENT_LAYER,
    budgets: _phase3Config ? _phase3Config.budgets : null,
    ruvector_configured: _phase3Config ? !!_phase3Config.ruvector.apiKey : false,
  };
}

// ============================================================================
// Utilities
// ============================================================================

function generateRequestId(): string {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).substring(2, 10);
  return `p3l1-${timestamp}-${random}`;
}
