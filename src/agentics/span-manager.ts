/**
 * Agentics Span Manager
 *
 * Manages the lifecycle of execution spans for this Foundational Execution Unit.
 * Enforces all invariants from the Agentics execution contract:
 *
 * - Every repo entry creates a repo-level span
 * - Every agent execution creates an agent-level span
 * - Agents MUST NOT execute without emitting a span
 * - Agents MUST NOT share spans
 * - No successful result if no agent spans were emitted
 * - Failed spans still return all emitted spans
 *
 * @module agentics/span-manager
 */

import { randomUUID } from 'crypto';
import {
  ExecutionContext,
  ExecutionSpan,
  SpanArtifact,
  SpanEvidence,
  SpanStatus,
  RepoExecutionResult,
  ExecutionContextError,
  REPO_NAME,
} from './execution-context';

// ============================================================================
// Span Manager
// ============================================================================

export class SpanManager {
  private readonly executionContext: ExecutionContext;
  private readonly repoSpan: ExecutionSpan;
  private readonly agentSpans: Map<string, ExecutionSpan> = new Map();

  constructor(context: ExecutionContext) {
    this.executionContext = context;

    // Create repo-level span immediately on construction
    this.repoSpan = {
      span_id: randomUUID(),
      parent_span_id: context.parent_span_id,
      type: 'repo',
      repo_name: REPO_NAME,
      status: 'running',
      start_time: new Date().toISOString(),
      artifacts: [],
      evidence: [],
    };
  }

  // --------------------------------------------------------------------------
  // Repo Span ID (for child spans to reference)
  // --------------------------------------------------------------------------

  get repoSpanId(): string {
    return this.repoSpan.span_id;
  }

  get executionId(): string {
    return this.executionContext.execution_id;
  }

  // --------------------------------------------------------------------------
  // Agent Span Lifecycle
  // --------------------------------------------------------------------------

  /**
   * Start a new agent-level span. Each agent MUST have its own span.
   * Returns the span_id for the caller to reference.
   */
  startAgentSpan(agentName: string): string {
    const spanId = randomUUID();

    const span: ExecutionSpan = {
      span_id: spanId,
      parent_span_id: this.repoSpan.span_id,
      type: 'agent',
      repo_name: REPO_NAME,
      agent_name: agentName,
      status: 'running',
      start_time: new Date().toISOString(),
      artifacts: [],
      evidence: [],
    };

    this.agentSpans.set(spanId, span);
    return spanId;
  }

  /**
   * Complete an agent span successfully.
   */
  completeAgentSpan(spanId: string): void {
    const span = this.agentSpans.get(spanId);
    if (!span) {
      throw new Error(`Agent span not found: ${spanId}`);
    }

    const now = new Date();
    span.status = 'completed';
    span.end_time = now.toISOString();
    span.duration_ms = now.getTime() - new Date(span.start_time).getTime();
  }

  /**
   * Mark an agent span as failed with reason(s).
   */
  failAgentSpan(spanId: string, reasons: string[]): void {
    const span = this.agentSpans.get(spanId);
    if (!span) {
      throw new Error(`Agent span not found: ${spanId}`);
    }

    const now = new Date();
    span.status = 'failed';
    span.end_time = now.toISOString();
    span.duration_ms = now.getTime() - new Date(span.start_time).getTime();
    span.failure_reasons = reasons;
  }

  // --------------------------------------------------------------------------
  // Artifact & Evidence Attachment
  // --------------------------------------------------------------------------

  /**
   * Attach an artifact to an agent span.
   * Artifacts MUST be attached at agent or repo level, never Core.
   */
  attachArtifact(spanId: string, artifact: Omit<SpanArtifact, 'artifact_id' | 'created_at'>): string {
    const span = this.agentSpans.get(spanId);
    if (!span) {
      throw new Error(`Agent span not found: ${spanId}`);
    }

    const artifactId = `artifact-${randomUUID()}`;
    span.artifacts.push({
      artifact_id: artifactId,
      type: artifact.type,
      reference: artifact.reference,
      created_at: new Date().toISOString(),
      metadata: artifact.metadata,
    });

    return artifactId;
  }

  /**
   * Attach an artifact to the repo-level span.
   */
  attachRepoArtifact(artifact: Omit<SpanArtifact, 'artifact_id' | 'created_at'>): string {
    const artifactId = `artifact-${randomUUID()}`;
    this.repoSpan.artifacts.push({
      artifact_id: artifactId,
      type: artifact.type,
      reference: artifact.reference,
      created_at: new Date().toISOString(),
      metadata: artifact.metadata,
    });
    return artifactId;
  }

  /**
   * Attach machine-verifiable evidence to an agent span.
   */
  attachEvidence(spanId: string, evidence: Omit<SpanEvidence, 'evidence_id'>): string {
    const span = this.agentSpans.get(spanId);
    if (!span) {
      throw new Error(`Agent span not found: ${spanId}`);
    }

    const evidenceId = `evidence-${randomUUID()}`;
    span.evidence.push({
      evidence_id: evidenceId,
      ...evidence,
    });

    return evidenceId;
  }

  // --------------------------------------------------------------------------
  // Finalization & Enforcement
  // --------------------------------------------------------------------------

  /**
   * Finalize execution and produce the RepoExecutionResult.
   *
   * ENFORCEMENT:
   * - If no agent spans were emitted, execution is INVALID -> result.success = false
   * - If any agent span has no completion, it is marked failed
   * - All spans are returned even on failure
   */
  finalize(): RepoExecutionResult {
    const failureReasons: string[] = [];
    const agentSpanList = Array.from(this.agentSpans.values());

    // ENFORCEMENT: No agent spans = invalid execution
    if (agentSpanList.length === 0) {
      failureReasons.push('No agent-level spans were emitted. Execution is INVALID per Agentics contract.');
    }

    // Mark any still-running agent spans as failed
    for (const span of agentSpanList) {
      if (span.status === 'running') {
        const now = new Date();
        span.status = 'failed';
        span.end_time = now.toISOString();
        span.duration_ms = now.getTime() - new Date(span.start_time).getTime();
        span.failure_reasons = ['Agent span was not properly completed before finalization'];
        failureReasons.push(`Agent "${span.agent_name}" span was not completed`);
      }
    }

    // Check if any agent failed
    const anyAgentFailed = agentSpanList.some(s => s.status === 'failed');
    if (anyAgentFailed && failureReasons.length === 0) {
      failureReasons.push('One or more agent executions failed');
    }

    // Determine overall success
    const success = failureReasons.length === 0;

    // Finalize repo span
    const now = new Date();
    this.repoSpan.status = success ? 'completed' : 'failed';
    this.repoSpan.end_time = now.toISOString();
    this.repoSpan.duration_ms = now.getTime() - new Date(this.repoSpan.start_time).getTime();
    if (!success) {
      this.repoSpan.failure_reasons = failureReasons;
    }

    return {
      repo_span: this.repoSpan,
      agent_spans: agentSpanList,
      execution_id: this.executionContext.execution_id,
      success,
      failure_reasons: failureReasons.length > 0 ? failureReasons : undefined,
    };
  }
}
