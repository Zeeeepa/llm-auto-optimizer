/**
 * Agentics Execution Context & Span Types
 *
 * Defines the execution span hierarchy for this Foundational Execution Unit.
 * All externally-invoked operations MUST carry an ExecutionContext and produce
 * spans conforming to the Agentics ExecutionGraph contract.
 *
 * Invariant:
 *   Core
 *     -> Repo (this repo: llm-auto-optimizer)
 *         -> Agent (one or more)
 *
 * @module agentics/execution-context
 */

import { randomUUID } from 'crypto';

// ============================================================================
// Constants
// ============================================================================

export const REPO_NAME = 'llm-auto-optimizer';

// ============================================================================
// Execution Context (provided by caller / Core)
// ============================================================================

/**
 * Execution context that MUST be provided by the calling Core.
 * Requests missing parent_span_id MUST be rejected.
 */
export interface ExecutionContext {
  /** Globally unique execution identifier assigned by the Core */
  execution_id: string;
  /** Span ID of the Core-level span that initiated this repo execution */
  parent_span_id: string;
}

// ============================================================================
// Span Types
// ============================================================================

export type SpanType = 'repo' | 'agent';

export type SpanStatus = 'running' | 'completed' | 'failed';

/**
 * A single execution span in the hierarchical ExecutionGraph.
 * Spans are append-only and causally ordered via parent_span_id.
 */
export interface ExecutionSpan {
  /** Unique span identifier (UUID) */
  span_id: string;
  /** Parent span ID (Core span for repo, repo span for agent) */
  parent_span_id: string;
  /** Span type discriminator */
  type: SpanType;
  /** Repository that owns this span */
  repo_name: string;
  /** Agent name (only for type === 'agent') */
  agent_name?: string;
  /** Execution status */
  status: SpanStatus;
  /** ISO 8601 start timestamp */
  start_time: string;
  /** ISO 8601 end timestamp (set on completion/failure) */
  end_time?: string;
  /** Duration in milliseconds (set on completion/failure) */
  duration_ms?: number;
  /** Artifacts produced by this span */
  artifacts: SpanArtifact[];
  /** Evidence attached to this span */
  evidence: SpanEvidence[];
  /** Failure reasons (populated when status === 'failed') */
  failure_reasons?: string[];
}

/**
 * An artifact produced during execution.
 * Artifacts MUST be attached to agent-level or repo-level spans, never Core.
 */
export interface SpanArtifact {
  /** Stable artifact identifier */
  artifact_id: string;
  /** Artifact type classification */
  type: 'report' | 'metrics' | 'config' | 'alert' | 'mapping' | 'export' | 'dataset' | 'telemetry' | 'decision_event';
  /** Stable reference (ID, URI, hash, or filename) */
  reference: string;
  /** ISO 8601 timestamp of artifact creation */
  created_at: string;
  /** Optional artifact metadata */
  metadata?: Record<string, unknown>;
}

/**
 * Machine-verifiable evidence attached to a span.
 * Evidence MUST NOT be inferred or synthesized.
 */
export interface SpanEvidence {
  /** Evidence identifier */
  evidence_id: string;
  /** Evidence type */
  type: 'hash' | 'signature' | 'timestamp' | 'count' | 'reference';
  /** Machine-verifiable value */
  value: string;
  /** What this evidence proves */
  description: string;
}

// ============================================================================
// Repo Execution Result (Output Contract)
// ============================================================================

/**
 * The complete output of a repo-level execution.
 * This is what gets returned to the Core for inclusion in the ExecutionGraph.
 *
 * MUST include:
 * - Exactly one repo-level span
 * - One or more agent-level spans nested under the repo span
 * - Artifacts and evidence attached at correct levels
 */
export interface RepoExecutionResult {
  /** The repo-level span */
  repo_span: ExecutionSpan;
  /** All agent-level spans (nested under repo_span) */
  agent_spans: ExecutionSpan[];
  /** Execution ID for correlation */
  execution_id: string;
  /** Whether execution succeeded */
  success: boolean;
  /** Top-level failure reasons (if any) */
  failure_reasons?: string[];
}

// ============================================================================
// Validation Errors
// ============================================================================

/**
 * Thrown when execution context is missing or invalid.
 */
export class ExecutionContextError extends Error {
  public readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = 'ExecutionContextError';
    this.code = code;
  }
}

// ============================================================================
// Context Extraction
// ============================================================================

/**
 * Extract ExecutionContext from HTTP headers or request body.
 * Headers take precedence: X-Execution-Id, X-Parent-Span-Id.
 * Body fallback: execution_context.execution_id, execution_context.parent_span_id.
 *
 * @throws ExecutionContextError if parent_span_id is missing
 */
export function extractExecutionContext(
  headers: Record<string, string>,
  body?: unknown
): ExecutionContext {
  // Try headers first
  let execution_id = headers['x-execution-id'];
  let parent_span_id = headers['x-parent-span-id'];

  // Fallback to body
  if (!parent_span_id && body && typeof body === 'object') {
    const ctx = (body as Record<string, unknown>).execution_context;
    if (ctx && typeof ctx === 'object') {
      const ctxObj = ctx as Record<string, unknown>;
      if (!execution_id && typeof ctxObj.execution_id === 'string') {
        execution_id = ctxObj.execution_id;
      }
      if (typeof ctxObj.parent_span_id === 'string') {
        parent_span_id = ctxObj.parent_span_id;
      }
    }
  }

  if (!parent_span_id) {
    throw new ExecutionContextError(
      'MISSING_PARENT_SPAN',
      'parent_span_id is required. This repo MUST be invoked by a Core with a valid execution context.'
    );
  }

  return {
    execution_id: execution_id || randomUUID(),
    parent_span_id,
  };
}
