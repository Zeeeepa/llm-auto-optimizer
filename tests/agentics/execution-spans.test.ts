/**
 * Agentics Execution Span Tests
 *
 * Verifies all invariants from the Agentics execution contract:
 *
 * 1. ExecutionContext extraction and validation
 * 2. Repo-level span creation on entry
 * 3. Agent-level span creation per agent
 * 4. Artifact and evidence attachment
 * 5. Enforcement: no successful result without agent spans
 * 6. Enforcement: failed spans still return all spans
 * 7. Output contract: hierarchical JSON-serializable spans
 */

import {
  extractExecutionContext,
  ExecutionContextError,
  SpanManager,
  REPO_NAME,
} from '../../src/agentics';

// ============================================================================
// ExecutionContext Extraction
// ============================================================================

describe('extractExecutionContext', () => {
  it('extracts context from headers', () => {
    const ctx = extractExecutionContext({
      'x-execution-id': 'exec-123',
      'x-parent-span-id': 'span-abc',
    });

    expect(ctx.execution_id).toBe('exec-123');
    expect(ctx.parent_span_id).toBe('span-abc');
  });

  it('extracts context from request body', () => {
    const ctx = extractExecutionContext({}, {
      execution_context: {
        execution_id: 'exec-body',
        parent_span_id: 'span-body',
      },
    });

    expect(ctx.execution_id).toBe('exec-body');
    expect(ctx.parent_span_id).toBe('span-body');
  });

  it('headers take precedence over body', () => {
    const ctx = extractExecutionContext(
      {
        'x-execution-id': 'exec-header',
        'x-parent-span-id': 'span-header',
      },
      {
        execution_context: {
          execution_id: 'exec-body',
          parent_span_id: 'span-body',
        },
      }
    );

    expect(ctx.execution_id).toBe('exec-header');
    expect(ctx.parent_span_id).toBe('span-header');
  });

  it('throws ExecutionContextError when parent_span_id is missing', () => {
    expect(() => extractExecutionContext({})).toThrow(ExecutionContextError);
    expect(() => extractExecutionContext({})).toThrow(/parent_span_id is required/);
  });

  it('throws ExecutionContextError with correct code', () => {
    try {
      extractExecutionContext({});
      fail('Expected ExecutionContextError');
    } catch (error) {
      expect(error).toBeInstanceOf(ExecutionContextError);
      expect((error as ExecutionContextError).code).toBe('MISSING_PARENT_SPAN');
    }
  });

  it('generates execution_id if not provided', () => {
    const ctx = extractExecutionContext({
      'x-parent-span-id': 'span-123',
    });

    expect(ctx.execution_id).toBeDefined();
    expect(ctx.execution_id.length).toBeGreaterThan(0);
    expect(ctx.parent_span_id).toBe('span-123');
  });
});

// ============================================================================
// SpanManager - Repo Span
// ============================================================================

describe('SpanManager', () => {
  const mockContext = {
    execution_id: 'exec-test-001',
    parent_span_id: 'core-span-001',
  };

  describe('repo span creation', () => {
    it('creates repo-level span on construction', () => {
      const manager = new SpanManager(mockContext);
      const result = manager.finalize();

      expect(result.repo_span).toBeDefined();
      expect(result.repo_span.type).toBe('repo');
      expect(result.repo_span.repo_name).toBe(REPO_NAME);
      expect(result.repo_span.parent_span_id).toBe('core-span-001');
      expect(result.repo_span.span_id).toBeDefined();
      expect(result.repo_span.start_time).toBeDefined();
    });

    it('repo span references core parent_span_id', () => {
      const manager = new SpanManager(mockContext);
      const result = manager.finalize();

      expect(result.repo_span.parent_span_id).toBe(mockContext.parent_span_id);
    });
  });

  // --------------------------------------------------------------------------
  // Agent Spans
  // --------------------------------------------------------------------------

  describe('agent span lifecycle', () => {
    it('creates agent-level span with correct parent', () => {
      const manager = new SpanManager(mockContext);
      const agentSpanId = manager.startAgentSpan('self-optimizing-agent');

      expect(agentSpanId).toBeDefined();

      const result = manager.finalize();
      expect(result.agent_spans).toHaveLength(1);
      expect(result.agent_spans[0].type).toBe('agent');
      expect(result.agent_spans[0].agent_name).toBe('self-optimizing-agent');
      expect(result.agent_spans[0].repo_name).toBe(REPO_NAME);
      expect(result.agent_spans[0].parent_span_id).toBe(result.repo_span.span_id);
    });

    it('each agent gets its own span (no sharing)', () => {
      const manager = new SpanManager(mockContext);
      const span1 = manager.startAgentSpan('self-optimizing-agent');
      const span2 = manager.startAgentSpan('token-optimization-agent');
      const span3 = manager.startAgentSpan('model-selection-agent');

      expect(span1).not.toBe(span2);
      expect(span2).not.toBe(span3);

      manager.completeAgentSpan(span1);
      manager.completeAgentSpan(span2);
      manager.completeAgentSpan(span3);

      const result = manager.finalize();
      expect(result.agent_spans).toHaveLength(3);

      const spanIds = result.agent_spans.map(s => s.span_id);
      expect(new Set(spanIds).size).toBe(3); // All unique
    });

    it('completes agent span with timing', () => {
      const manager = new SpanManager(mockContext);
      const spanId = manager.startAgentSpan('test-agent');
      manager.completeAgentSpan(spanId);

      const result = manager.finalize();
      const agentSpan = result.agent_spans[0];

      expect(agentSpan.status).toBe('completed');
      expect(agentSpan.end_time).toBeDefined();
      expect(agentSpan.duration_ms).toBeDefined();
      expect(agentSpan.duration_ms).toBeGreaterThanOrEqual(0);
    });

    it('fails agent span with reasons', () => {
      const manager = new SpanManager(mockContext);
      const spanId = manager.startAgentSpan('test-agent');
      manager.failAgentSpan(spanId, ['Connection timeout', 'Service unavailable']);

      const result = manager.finalize();
      const agentSpan = result.agent_spans[0];

      expect(agentSpan.status).toBe('failed');
      expect(agentSpan.failure_reasons).toContain('Connection timeout');
      expect(agentSpan.failure_reasons).toContain('Service unavailable');
    });
  });

  // --------------------------------------------------------------------------
  // Artifacts & Evidence
  // --------------------------------------------------------------------------

  describe('artifact attachment', () => {
    it('attaches artifacts to agent spans', () => {
      const manager = new SpanManager(mockContext);
      const spanId = manager.startAgentSpan('test-agent');

      const artifactId = manager.attachArtifact(spanId, {
        type: 'decision_event',
        reference: 'decision-001',
        metadata: { action: 'recommend' },
      });

      expect(artifactId).toBeDefined();
      expect(artifactId).toMatch(/^artifact-/);

      manager.completeAgentSpan(spanId);
      const result = manager.finalize();

      expect(result.agent_spans[0].artifacts).toHaveLength(1);
      expect(result.agent_spans[0].artifacts[0].type).toBe('decision_event');
      expect(result.agent_spans[0].artifacts[0].reference).toBe('decision-001');
      expect(result.agent_spans[0].artifacts[0].created_at).toBeDefined();
    });

    it('attaches artifacts to repo span', () => {
      const manager = new SpanManager(mockContext);
      const spanId = manager.startAgentSpan('test-agent');
      manager.completeAgentSpan(spanId);

      manager.attachRepoArtifact({
        type: 'telemetry',
        reference: 'repo-telemetry-001',
      });

      const result = manager.finalize();
      expect(result.repo_span.artifacts).toHaveLength(1);
      expect(result.repo_span.artifacts[0].type).toBe('telemetry');
    });
  });

  describe('evidence attachment', () => {
    it('attaches machine-verifiable evidence to agent spans', () => {
      const manager = new SpanManager(mockContext);
      const spanId = manager.startAgentSpan('test-agent');

      const evidenceId = manager.attachEvidence(spanId, {
        type: 'hash',
        value: 'abc123def456',
        description: 'SHA-256 hash of response body',
      });

      expect(evidenceId).toBeDefined();
      expect(evidenceId).toMatch(/^evidence-/);

      manager.completeAgentSpan(spanId);
      const result = manager.finalize();

      expect(result.agent_spans[0].evidence).toHaveLength(1);
      expect(result.agent_spans[0].evidence[0].type).toBe('hash');
      expect(result.agent_spans[0].evidence[0].value).toBe('abc123def456');
    });
  });

  // --------------------------------------------------------------------------
  // ENFORCEMENT: Invariants
  // --------------------------------------------------------------------------

  describe('enforcement invariants', () => {
    it('FAILS if no agent spans were emitted', () => {
      const manager = new SpanManager(mockContext);
      const result = manager.finalize();

      expect(result.success).toBe(false);
      expect(result.failure_reasons).toContain(
        'No agent-level spans were emitted. Execution is INVALID per Agentics contract.'
      );
      expect(result.repo_span.status).toBe('failed');
    });

    it('FAILS if agent span was not completed', () => {
      const manager = new SpanManager(mockContext);
      manager.startAgentSpan('abandoned-agent'); // Started but never completed

      const result = manager.finalize();

      expect(result.success).toBe(false);
      expect(result.agent_spans[0].status).toBe('failed');
      expect(result.agent_spans[0].failure_reasons).toContain(
        'Agent span was not properly completed before finalization'
      );
    });

    it('returns all spans even on failure', () => {
      const manager = new SpanManager(mockContext);
      const span1 = manager.startAgentSpan('agent-a');
      const span2 = manager.startAgentSpan('agent-b');

      manager.completeAgentSpan(span1);
      manager.failAgentSpan(span2, ['Something broke']);

      const result = manager.finalize();

      // Repo span present
      expect(result.repo_span).toBeDefined();
      // Both agent spans present
      expect(result.agent_spans).toHaveLength(2);
      // Result reports failure
      expect(result.success).toBe(false);
      // All spans are returned
      expect(result.agent_spans.find(s => s.agent_name === 'agent-a')?.status).toBe('completed');
      expect(result.agent_spans.find(s => s.agent_name === 'agent-b')?.status).toBe('failed');
    });

    it('succeeds when all agent spans complete', () => {
      const manager = new SpanManager(mockContext);
      const span1 = manager.startAgentSpan('agent-a');
      const span2 = manager.startAgentSpan('agent-b');

      manager.completeAgentSpan(span1);
      manager.completeAgentSpan(span2);

      const result = manager.finalize();

      expect(result.success).toBe(true);
      expect(result.failure_reasons).toBeUndefined();
      expect(result.repo_span.status).toBe('completed');
    });
  });

  // --------------------------------------------------------------------------
  // Output Contract
  // --------------------------------------------------------------------------

  describe('output contract', () => {
    it('produces correct hierarchical structure', () => {
      const manager = new SpanManager(mockContext);
      const spanId = manager.startAgentSpan('self-optimizing-agent');

      manager.attachArtifact(spanId, {
        type: 'decision_event',
        reference: 'out-self-optimizing-agent-12345',
      });

      manager.attachEvidence(spanId, {
        type: 'hash',
        value: 'sha256-abc',
        description: 'Response hash',
      });

      manager.completeAgentSpan(spanId);
      const result = manager.finalize();

      // Verify hierarchy: Core -> Repo -> Agent
      expect(result.repo_span.parent_span_id).toBe(mockContext.parent_span_id);
      expect(result.agent_spans[0].parent_span_id).toBe(result.repo_span.span_id);
    });

    it('is JSON-serializable without loss', () => {
      const manager = new SpanManager(mockContext);
      const spanId = manager.startAgentSpan('test-agent');

      manager.attachArtifact(spanId, {
        type: 'metrics',
        reference: 'metrics-001',
        metadata: { key: 'value', nested: { deep: true } },
      });

      manager.completeAgentSpan(spanId);
      const result = manager.finalize();

      const serialized = JSON.stringify(result);
      const deserialized = JSON.parse(serialized);

      // Verify round-trip integrity
      expect(deserialized.repo_span.span_id).toBe(result.repo_span.span_id);
      expect(deserialized.agent_spans[0].span_id).toBe(result.agent_spans[0].span_id);
      expect(deserialized.agent_spans[0].artifacts[0].reference).toBe('metrics-001');
      expect(deserialized.execution_id).toBe(mockContext.execution_id);
    });

    it('spans are causally ordered via parent_span_id', () => {
      const manager = new SpanManager(mockContext);
      const s1 = manager.startAgentSpan('agent-1');
      const s2 = manager.startAgentSpan('agent-2');

      manager.completeAgentSpan(s1);
      manager.completeAgentSpan(s2);
      const result = manager.finalize();

      // All agent spans reference the repo span as parent
      for (const agentSpan of result.agent_spans) {
        expect(agentSpan.parent_span_id).toBe(result.repo_span.span_id);
      }

      // Repo span references the Core span as parent
      expect(result.repo_span.parent_span_id).toBe(mockContext.parent_span_id);
    });

    it('includes execution_id for correlation', () => {
      const manager = new SpanManager(mockContext);
      const spanId = manager.startAgentSpan('test-agent');
      manager.completeAgentSpan(spanId);
      const result = manager.finalize();

      expect(result.execution_id).toBe(mockContext.execution_id);
    });
  });
});
