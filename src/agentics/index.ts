/**
 * Agentics Execution Instrumentation
 *
 * This module provides the complete execution span infrastructure for the
 * llm-auto-optimizer Foundational Execution Unit.
 *
 * This repository MUST emit agent-level execution spans and MUST integrate
 * into the hierarchical ExecutionGraph produced by a Core.
 *
 * @module agentics
 */

export {
  // Types
  type ExecutionContext,
  type ExecutionSpan,
  type SpanType,
  type SpanStatus,
  type SpanArtifact,
  type SpanEvidence,
  type RepoExecutionResult,

  // Constants
  REPO_NAME,

  // Errors
  ExecutionContextError,

  // Context extraction
  extractExecutionContext,
} from './execution-context';

export {
  SpanManager,
} from './span-manager';
