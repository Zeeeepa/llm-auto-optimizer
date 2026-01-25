/**
 * Phase 3 Layer 1 Agent Wrapper
 *
 * Wraps existing agents with Phase 3 Layer 1 execution guards and signal emission.
 * Ensures all agent invocations comply with Phase 3 requirements.
 *
 * @module phase3/layer1/agent-wrapper
 */

import {
  AGENT_PHASE,
  AGENT_LAYER,
  Phase3Layer1Config,
  ExecutionGuard,
  createExecutionGuard,
  ExecutionGuardViolation,
  buildPhase3Signal,
  SignalPayload,
  SignalReference,
  Phase3Signal,
  checkRuVectorAvailability,
} from './index';
import { DecisionEvent, AppliedConstraint, hashInputs } from '../../contracts';
import { createRuVectorClient, RuVectorClient } from '../../services/ruvector-client';

// ============================================================================
// Agent Wrapper Types
// ============================================================================

export interface WrappedAgentInput {
  /** Original agent input */
  input: unknown;
  /** Operation being performed */
  operation: 'coordinate' | 'route' | 'optimize' | 'escalate';
  /** Execution reference */
  execution_ref: string;
  /** Optional references for signal */
  references?: SignalReference[];
}

export interface WrappedAgentOutput<T> {
  /** Original agent output */
  output: T;
  /** Phase 3 signal emitted */
  signal: Phase3Signal;
  /** Execution metrics */
  metrics: {
    tokensUsed: number;
    latencyMs: number;
    apiCallsMade: number;
  };
  /** Whether signal was persisted to RuVector */
  persisted: boolean;
  /** RuVector record ID (if persisted) */
  record_id?: string;
}

export type AgentHandler<TInput, TOutput> = (
  input: TInput,
  guard: ExecutionGuard
) => Promise<TOutput>;

// ============================================================================
// Agent Wrapper
// ============================================================================

/**
 * Wraps an agent handler with Phase 3 Layer 1 guards and signal emission.
 *
 * @param config - Phase 3 Layer 1 configuration
 * @param agentId - Agent identifier
 * @param agentVersion - Agent version
 * @param handler - Original agent handler
 * @returns Wrapped handler with Phase 3 compliance
 */
export function wrapAgent<TInput, TOutput>(
  config: Phase3Layer1Config,
  agentId: string,
  agentVersion: string,
  handler: AgentHandler<TInput, TOutput>
): (input: WrappedAgentInput) => Promise<WrappedAgentOutput<TOutput>> {
  const ruVectorClient = createRuVectorClient({
    baseUrl: config.ruvector.serviceUrl,
    apiKey: config.ruvector.apiKey,
  });

  return async (wrappedInput: WrappedAgentInput): Promise<WrappedAgentOutput<TOutput>> => {
    const guard = createExecutionGuard(config);
    const startTime = Date.now();

    try {
      // Record the operation type
      guard.recordOperation(wrappedInput.operation);

      // Check RuVector availability (MANDATORY)
      await checkRuVectorAvailability(config, guard);

      // Execute the original handler
      const output = await handler(wrappedInput.input as TInput, guard);

      // Validate output doesn't contain forbidden content
      guard.validateOutput(output);
      guard.validateOutputIsSignal(output);

      // Build the Phase 3 signal
      const signalPayload = buildSignalPayload(wrappedInput, output);
      const signal = buildPhase3Signal({
        agentId,
        agentVersion,
        executionRef: wrappedInput.execution_ref,
        inputsHash: computeInputsHash(wrappedInput.input),
        signalType: mapOperationToSignalType(wrappedInput.operation),
        payload: signalPayload,
        confidence: extractConfidence(output),
        constraintsApplied: extractConstraints(output),
        references: wrappedInput.references || generateDefaultReferences(wrappedInput),
      });

      // Persist signal to RuVector (MANDATORY)
      guard.recordApiCall('ruvector-persist');
      const persistResult = await ruVectorClient.persistDecisionEvent(signal);

      // Finalize execution guard
      const metrics = guard.finalize();

      return {
        output,
        signal,
        metrics: {
          tokensUsed: metrics.tokensUsed,
          latencyMs: metrics.latencyMs,
          apiCallsMade: metrics.apiCallsMade,
        },
        persisted: persistResult.success,
        record_id: persistResult.record_id,
      };
    } catch (error) {
      // Ensure guard is finalized even on error
      try {
        guard.finalize();
      } catch {
        // Ignore finalization errors
      }
      throw error;
    }
  };
}

// ============================================================================
// Coordination Agent Factory
// ============================================================================

/**
 * Create a Phase 3 Layer 1 coordination agent.
 * This agent can ONLY coordinate, route, optimize, and escalate.
 */
export function createCoordinationAgent(
  config: Phase3Layer1Config,
  agentId: string,
  agentVersion: string
): {
  coordinate: (input: unknown, execution_ref: string, references?: SignalReference[]) => Promise<WrappedAgentOutput<CoordinationResult>>;
  route: (input: unknown, execution_ref: string, references?: SignalReference[]) => Promise<WrappedAgentOutput<RoutingResult>>;
  optimize: (input: unknown, execution_ref: string, references?: SignalReference[]) => Promise<WrappedAgentOutput<OptimizationResult>>;
  escalate: (input: unknown, execution_ref: string, references?: SignalReference[]) => Promise<WrappedAgentOutput<EscalationResult>>;
} {
  const ruVectorClient = createRuVectorClient({
    baseUrl: config.ruvector.serviceUrl,
    apiKey: config.ruvector.apiKey,
  });

  const createHandler = <T>(
    operation: 'coordinate' | 'route' | 'optimize' | 'escalate',
    processor: (input: unknown, guard: ExecutionGuard) => Promise<T>
  ) => {
    return async (
      input: unknown,
      execution_ref: string,
      references?: SignalReference[]
    ): Promise<WrappedAgentOutput<T>> => {
      const guard = createExecutionGuard(config);

      try {
        guard.recordOperation(operation);
        await checkRuVectorAvailability(config, guard);

        const output = await processor(input, guard);

        guard.validateOutput(output);

        const signalPayload = buildSignalPayload({ input, operation, execution_ref }, output);
        const signal = buildPhase3Signal({
          agentId,
          agentVersion,
          executionRef: execution_ref,
          inputsHash: computeInputsHash(input),
          signalType: mapOperationToSignalType(operation),
          payload: signalPayload,
          confidence: extractConfidence(output),
          constraintsApplied: extractConstraints(output),
          references: references || generateDefaultReferences({ input, operation, execution_ref }),
        });

        guard.recordApiCall('ruvector-persist');
        const persistResult = await ruVectorClient.persistDecisionEvent(signal);

        const metrics = guard.finalize();

        return {
          output,
          signal,
          metrics: {
            tokensUsed: metrics.tokensUsed,
            latencyMs: metrics.latencyMs,
            apiCallsMade: metrics.apiCallsMade,
          },
          persisted: persistResult.success,
          record_id: persistResult.record_id,
        };
      } catch (error) {
        try { guard.finalize(); } catch {}
        throw error;
      }
    };
  };

  return {
    coordinate: createHandler('coordinate', coordinateProcessor),
    route: createHandler('route', routeProcessor),
    optimize: createHandler('optimize', optimizeProcessor),
    escalate: createHandler('escalate', escalateProcessor),
  };
}

// ============================================================================
// Result Types
// ============================================================================

export interface CoordinationResult {
  strategy: {
    id: string;
    name: string;
    description: string;
    target_agents: string[];
    parameters: Record<string, unknown>;
  };
  estimated_tokens: number;
  estimated_latency_ms: number;
  confidence: number;
}

export interface RoutingResult {
  from_agent: string;
  to_agents: string[];
  reason: string;
  parameters: Record<string, unknown>;
  confidence: number;
}

export interface OptimizationResult {
  opportunity: {
    id: string;
    category: string;
    current_value: number;
    proposed_value: number;
    improvement_pct: number;
  };
  risk_level: string;
  reversible: boolean;
  confidence: number;
}

export interface EscalationResult {
  incident: {
    id: string;
    type: string;
    severity: string;
    affected_components: string[];
  };
  escalation_target: string;
  urgency: string;
  recommended_actions: string[];
  confidence: number;
}

// ============================================================================
// Processors (Phase 3 Layer 1 Logic)
// ============================================================================

async function coordinateProcessor(input: unknown, guard: ExecutionGuard): Promise<CoordinationResult> {
  guard.recordTokens(100); // Estimate token usage

  const inputObj = input as Record<string, unknown>;
  return {
    strategy: {
      id: `coord-${Date.now()}`,
      name: inputObj.strategy_name as string || 'default-coordination',
      description: 'Coordination strategy signal (not a final decision)',
      target_agents: inputObj.target_agents as string[] || [],
      parameters: inputObj.parameters as Record<string, unknown> || {},
    },
    estimated_tokens: 500,
    estimated_latency_ms: 1000,
    confidence: 0.85,
  };
}

async function routeProcessor(input: unknown, guard: ExecutionGuard): Promise<RoutingResult> {
  guard.recordTokens(80);

  const inputObj = input as Record<string, unknown>;
  return {
    from_agent: inputObj.from_agent as string || 'unknown',
    to_agents: inputObj.to_agents as string[] || [],
    reason: inputObj.reason as string || 'Routing signal (not a final decision)',
    parameters: inputObj.parameters as Record<string, unknown> || {},
    confidence: 0.90,
  };
}

async function optimizeProcessor(input: unknown, guard: ExecutionGuard): Promise<OptimizationResult> {
  guard.recordTokens(150);

  const inputObj = input as Record<string, unknown>;
  return {
    opportunity: {
      id: `opt-${Date.now()}`,
      category: inputObj.category as string || 'general',
      current_value: inputObj.current_value as number || 0,
      proposed_value: inputObj.proposed_value as number || 0,
      improvement_pct: inputObj.improvement_pct as number || 0,
    },
    risk_level: inputObj.risk_level as string || 'medium',
    reversible: true,
    confidence: 0.80,
  };
}

async function escalateProcessor(input: unknown, guard: ExecutionGuard): Promise<EscalationResult> {
  guard.recordTokens(120);

  const inputObj = input as Record<string, unknown>;
  return {
    incident: {
      id: `inc-${Date.now()}`,
      type: inputObj.incident_type as string || 'unknown',
      severity: inputObj.severity as string || 'medium',
      affected_components: inputObj.affected_components as string[] || [],
    },
    escalation_target: inputObj.escalation_target as string || 'ops-team',
    urgency: inputObj.urgency as string || 'medium',
    recommended_actions: ['Review incident', 'Assess impact', 'Coordinate response'],
    confidence: 0.75,
  };
}

// ============================================================================
// Helper Functions
// ============================================================================

function computeInputsHash(input: unknown): string {
  try {
    return hashInputs(input as any);
  } catch {
    const crypto = require('crypto');
    return crypto.createHash('sha256').update(JSON.stringify(input)).digest('hex');
  }
}

function mapOperationToSignalType(operation: string): 'execution_strategy_signal' | 'optimization_signal' | 'incident_signal' {
  switch (operation) {
    case 'coordinate':
    case 'route':
      return 'execution_strategy_signal';
    case 'optimize':
      return 'optimization_signal';
    case 'escalate':
      return 'incident_signal';
    default:
      return 'execution_strategy_signal';
  }
}

function buildSignalPayload(wrappedInput: WrappedAgentInput, output: unknown): SignalPayload {
  const operation = wrappedInput.operation;

  switch (operation) {
    case 'coordinate':
    case 'route': {
      const routingResult = output as RoutingResult;
      return {
        signal_type: 'execution_strategy_signal',
        strategy: {
          id: `strategy-${Date.now()}`,
          name: operation === 'coordinate' ? 'coordination-strategy' : 'routing-strategy',
          description: 'Strategy signal for downstream processing',
          target: routingResult.to_agents?.[0] || 'unknown',
          parameters: routingResult.parameters || {},
        },
        routing: {
          from_agent: routingResult.from_agent || 'self',
          to_agents: routingResult.to_agents || [],
          reason: routingResult.reason || 'Routing decision',
        },
        strategy_confidence: routingResult.confidence || 0.8,
        expected_outcome: 'Signal emitted for downstream processing',
        estimated_tokens: 500,
        estimated_latency_ms: 1000,
      };
    }
    case 'optimize': {
      const optResult = output as OptimizationResult;
      return {
        signal_type: 'optimization_signal',
        opportunity: {
          id: optResult.opportunity?.id || `opt-${Date.now()}`,
          category: (optResult.opportunity?.category as any) || 'general',
          current_value: optResult.opportunity?.current_value || 0,
          proposed_value: optResult.opportunity?.proposed_value || 0,
          improvement_pct: optResult.opportunity?.improvement_pct || 0,
          complexity: 'medium',
        },
        impact: {
          affected_services: [],
          risk_level: (optResult.risk_level as any) || 'medium',
          reversible: optResult.reversible !== false,
        },
        optimization_confidence: optResult.confidence || 0.8,
        metrics: {},
      };
    }
    case 'escalate': {
      const escResult = output as EscalationResult;
      return {
        signal_type: 'incident_signal',
        incident: {
          id: escResult.incident?.id || `inc-${Date.now()}`,
          type: (escResult.incident?.type as any) || 'unknown',
          severity: (escResult.incident?.severity as any) || 'medium',
          affected_components: escResult.incident?.affected_components || [],
          first_detected: new Date().toISOString(),
          status: 'escalated',
        },
        escalation: {
          recommended: true,
          target: escResult.escalation_target,
          reason: 'Automated escalation based on incident severity',
          urgency: (escResult.urgency as any) || 'medium',
        },
        detection_confidence: escResult.confidence || 0.75,
        recommended_actions: escResult.recommended_actions || [],
      };
    }
    default:
      throw new Error(`Unknown operation: ${operation}`);
  }
}

function extractConfidence(output: unknown): number {
  if (output && typeof output === 'object' && 'confidence' in output) {
    return (output as { confidence: number }).confidence;
  }
  return 0.8; // Default confidence
}

function extractConstraints(output: unknown): AppliedConstraint[] {
  if (output && typeof output === 'object' && 'constraints_applied' in output) {
    return (output as { constraints_applied: AppliedConstraint[] }).constraints_applied;
  }
  return [];
}

function generateDefaultReferences(wrappedInput: WrappedAgentInput): SignalReference[] {
  return [{
    type: 'telemetry',
    id: `telemetry-${Date.now()}`,
    source: 'phase3-layer1-agent',
    timestamp: new Date().toISOString(),
    relevance: 0.9,
  }];
}
