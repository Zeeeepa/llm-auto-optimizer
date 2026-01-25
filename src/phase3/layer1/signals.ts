/**
 * Phase 3 Layer 1 Signal Definitions
 *
 * Defines the signal schemas for DecisionEvents emitted by Phase 3 Layer 1 agents.
 * All signals MUST include confidence scores and references.
 *
 * @module phase3/layer1/signals
 */

import { DecisionEvent, AppliedConstraint, RecommendationCategory } from '../../contracts';
import { Phase3SignalType, AGENT_PHASE, AGENT_LAYER } from './config';

// ============================================================================
// Base Signal Schema
// ============================================================================

/**
 * Base interface for all Phase 3 Layer 1 signals.
 * Extends DecisionEvent with Phase 3 specific fields.
 */
export interface Phase3Signal extends DecisionEvent {
  /** Phase identifier */
  phase: typeof AGENT_PHASE;
  /** Layer identifier */
  layer: typeof AGENT_LAYER;
  /** Signal-specific payload */
  signal_payload: SignalPayload;
  /** References to supporting data/events */
  references: SignalReference[];
}

// ============================================================================
// Signal Payloads
// ============================================================================

export type SignalPayload =
  | ExecutionStrategyPayload
  | OptimizationPayload
  | IncidentPayload;

/**
 * Payload for execution_strategy_signal.
 * Emitted when routing or coordinating execution across agents.
 */
export interface ExecutionStrategyPayload {
  signal_type: 'execution_strategy_signal';
  /** Strategy being recommended */
  strategy: {
    /** Strategy identifier */
    id: string;
    /** Strategy name */
    name: string;
    /** Strategy description */
    description: string;
    /** Target agent or service */
    target: string;
    /** Strategy parameters */
    parameters: Record<string, unknown>;
  };
  /** Routing information */
  routing: {
    /** Source agent */
    from_agent: string;
    /** Target agents */
    to_agents: string[];
    /** Routing reason */
    reason: string;
  };
  /** Strategy confidence */
  strategy_confidence: number;
  /** Expected outcome description */
  expected_outcome: string;
  /** Estimated tokens for execution */
  estimated_tokens: number;
  /** Estimated latency in milliseconds */
  estimated_latency_ms: number;
}

/**
 * Payload for optimization_signal.
 * Emitted when proposing optimization opportunities.
 */
export interface OptimizationPayload {
  signal_type: 'optimization_signal';
  /** Optimization opportunity */
  opportunity: {
    /** Opportunity identifier */
    id: string;
    /** Opportunity category */
    category: 'cost' | 'latency' | 'quality' | 'throughput' | 'reliability';
    /** Current value */
    current_value: number;
    /** Proposed value */
    proposed_value: number;
    /** Improvement percentage */
    improvement_pct: number;
    /** Implementation complexity */
    complexity: 'low' | 'medium' | 'high';
  };
  /** Impact analysis */
  impact: {
    /** Affected services */
    affected_services: string[];
    /** Risk level */
    risk_level: 'low' | 'medium' | 'high';
    /** Rollback capability */
    reversible: boolean;
  };
  /** Optimization confidence */
  optimization_confidence: number;
  /** Supporting metrics */
  metrics: Record<string, number>;
}

/**
 * Payload for incident_signal.
 * Emitted when detecting or escalating incidents.
 */
export interface IncidentPayload {
  signal_type: 'incident_signal';
  /** Incident details */
  incident: {
    /** Incident identifier */
    id: string;
    /** Incident type */
    type: 'anomaly' | 'degradation' | 'threshold_breach' | 'error_spike' | 'availability';
    /** Severity level */
    severity: 'low' | 'medium' | 'high' | 'critical';
    /** Affected components */
    affected_components: string[];
    /** First detected timestamp */
    first_detected: string;
    /** Current status */
    status: 'detected' | 'acknowledged' | 'escalated' | 'resolved';
  };
  /** Escalation information */
  escalation: {
    /** Escalation recommended */
    recommended: boolean;
    /** Escalation target */
    target?: string;
    /** Escalation reason */
    reason?: string;
    /** Urgency level */
    urgency: 'low' | 'medium' | 'high' | 'immediate';
  };
  /** Detection confidence */
  detection_confidence: number;
  /** Recommended actions (NOT final decisions) */
  recommended_actions: string[];
}

// ============================================================================
// Signal Reference
// ============================================================================

/**
 * Reference to supporting data or events.
 * Required for all Phase 3 signals.
 */
export interface SignalReference {
  /** Reference type */
  type: 'decision_event' | 'telemetry' | 'metric' | 'log' | 'external';
  /** Reference identifier */
  id: string;
  /** Reference source */
  source: string;
  /** Timestamp */
  timestamp: string;
  /** Relevance score (0-1) */
  relevance: number;
}

// ============================================================================
// Signal Builder Functions
// ============================================================================

export interface BuildSignalParams {
  agentId: string;
  agentVersion: string;
  executionRef: string;
  inputsHash: string;
  signalType: Phase3SignalType;
  payload: SignalPayload;
  confidence: number;
  constraintsApplied: AppliedConstraint[];
  references: SignalReference[];
}

/**
 * Build a Phase 3 Layer 1 signal.
 * Validates all required fields and enforces signal constraints.
 *
 * @throws Error if signal validation fails
 */
export function buildPhase3Signal(params: BuildSignalParams): Phase3Signal {
  // Validate confidence is within bounds
  if (params.confidence < 0 || params.confidence > 1) {
    throw new Error('Signal confidence must be between 0 and 1');
  }

  // Validate references are provided
  if (params.references.length === 0) {
    throw new Error('Phase 3 signals MUST include at least one reference');
  }

  // Validate payload signal_type matches signalType
  if (params.payload.signal_type !== params.signalType) {
    throw new Error(`Payload signal_type '${params.payload.signal_type}' does not match signalType '${params.signalType}'`);
  }

  const signal: Phase3Signal = {
    // DecisionEvent fields
    agent_id: params.agentId,
    agent_version: params.agentVersion,
    decision_type: params.signalType,
    inputs_hash: params.inputsHash,
    outputs: {
      // Signal outputs are recommendations, NOT decisions
      recommendations: [{
        recommendation_id: `${params.signalType}-${Date.now()}`,
        rank: 1,
        category: mapSignalToCategory(params.signalType),
        title: getSignalTitle(params.payload),
        description: getSignalDescription(params.payload),
        proposed_changes: [],
        expected_impact: {
          cost_change_pct: 0,
          latency_change_pct: 0,
          quality_change_pct: 0,
          confidence: params.confidence,
        },
        complexity: 'medium',
        risk: 'medium',
        confidence: params.confidence,
        evidence: params.references.map(ref => ({
          type: mapReferenceToEvidence(ref.type),
          source: ref.source,
          data_points: { reference_id: ref.id },
          strength: ref.relevance,
        })),
        rollout_strategy: {
          type: 'gradual',
          rollback_criteria: {
            max_error_rate_increase_pct: 5,
            max_latency_increase_pct: 10,
            min_quality_threshold: 0.8,
            monitoring_duration_minutes: 30,
          },
        },
      }],
    },
    confidence: params.confidence,
    constraints_applied: params.constraintsApplied,
    execution_ref: params.executionRef,
    timestamp: new Date().toISOString(),

    // Phase 3 specific fields
    phase: AGENT_PHASE,
    layer: AGENT_LAYER,
    signal_payload: params.payload,
    references: params.references,
  };

  return signal;
}

// ============================================================================
// Helper Functions
// ============================================================================

function mapSignalToCategory(signalType: Phase3SignalType): RecommendationCategory {
  switch (signalType) {
    case 'execution_strategy_signal':
      return 'traffic_routing';
    case 'optimization_signal':
      return 'cost_optimization';
    case 'incident_signal':
      return 'quality_improvement';
  }
}

function getSignalTitle(payload: SignalPayload): string {
  switch (payload.signal_type) {
    case 'execution_strategy_signal':
      return `Execution Strategy: ${payload.strategy.name}`;
    case 'optimization_signal':
      return `Optimization Opportunity: ${payload.opportunity.category}`;
    case 'incident_signal':
      return `Incident Detected: ${payload.incident.type}`;
  }
}

function getSignalDescription(payload: SignalPayload): string {
  switch (payload.signal_type) {
    case 'execution_strategy_signal':
      return payload.strategy.description;
    case 'optimization_signal':
      return `${payload.opportunity.improvement_pct.toFixed(1)}% improvement in ${payload.opportunity.category}`;
    case 'incident_signal':
      return `${payload.incident.severity} severity ${payload.incident.type} affecting ${payload.incident.affected_components.join(', ')}`;
  }
}

function mapReferenceToEvidence(refType: SignalReference['type']): 'historical_data' | 'benchmark' | 'a_b_test' | 'simulation' | 'industry_standard' {
  switch (refType) {
    case 'decision_event':
      return 'historical_data';
    case 'telemetry':
      return 'historical_data';
    case 'metric':
      return 'benchmark';
    case 'log':
      return 'historical_data';
    case 'external':
      return 'industry_standard';
  }
}

// ============================================================================
// Signal Type Guards
// ============================================================================

export function isExecutionStrategySignal(payload: SignalPayload): payload is ExecutionStrategyPayload {
  return payload.signal_type === 'execution_strategy_signal';
}

export function isOptimizationSignal(payload: SignalPayload): payload is OptimizationPayload {
  return payload.signal_type === 'optimization_signal';
}

export function isIncidentSignal(payload: SignalPayload): payload is IncidentPayload {
  return payload.signal_type === 'incident_signal';
}
