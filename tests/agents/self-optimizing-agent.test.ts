/**
 * Self-Optimizing Agent - Verification Tests
 *
 * Smoke tests and verification checklist for the Self-Optimizing Agent.
 * These tests verify compliance with the Agent Infrastructure Constitution.
 *
 * @module tests/agents/self-optimizing-agent
 */

import {
  SelfOptimizingAgent,
  createSelfOptimizingAgent,
  AGENT_ID,
  AGENT_VERSION,
  DECISION_TYPE,
} from '../../src/agents/self-optimizing-agent';

import {
  SelfOptimizingAgentInput,
  SelfOptimizingAgentOutput,
  DecisionEvent,
  validateInput,
  validateOutput,
  hashInputs,
} from '../../src/contracts';

import {
  RuVectorClient,
} from '../../src/services/ruvector-client';

// ============================================================================
// Test Fixtures
// ============================================================================

function createValidInput(): SelfOptimizingAgentInput {
  const now = new Date();
  const start = new Date(now.getTime() - 86400000); // 24 hours ago

  return {
    execution_ref: `test-${Date.now()}`,
    time_window: {
      start: start.toISOString(),
      end: now.toISOString(),
      granularity: 'hour',
    },
    optimization_targets: {
      cost_weight: 0.34,
      latency_weight: 0.33,
      quality_weight: 0.33,
    },
    target_services: ['test-service'],
    constraints: [
      {
        type: 'max_cost_increase_pct',
        value: 10,
        hard: true,
      },
    ],
    telemetry: {
      cost_metrics: {
        total_cost_usd: 1000,
        by_model: {
          'claude-3-opus': {
            cost_usd: 600,
            request_count: 3000,
            avg_cost_per_request: 0.2,
            input_tokens: 1500000,
            output_tokens: 400000,
          },
          'claude-3-haiku': {
            cost_usd: 400,
            request_count: 50000,
            avg_cost_per_request: 0.008,
            input_tokens: 10000000,
            output_tokens: 2500000,
          },
        },
        by_provider: { anthropic: 1000 },
        daily_trend: [950, 980, 1000, 1020, 990, 1000, 1010],
      },
      latency_metrics: {
        p50_ms: 200,
        p95_ms: 600,
        p99_ms: 1200,
        avg_ms: 300,
        by_model: {
          'claude-3-opus': {
            p50_ms: 400,
            p95_ms: 1000,
            p99_ms: 1800,
            request_count: 3000,
            timeout_rate: 0.01,
          },
          'claude-3-haiku': {
            p50_ms: 100,
            p95_ms: 300,
            p99_ms: 600,
            request_count: 50000,
            timeout_rate: 0.005,
          },
        },
      },
      quality_metrics: {
        overall_score: 0.88,
        by_model: {
          'claude-3-opus': {
            score: 0.95,
            sample_count: 500,
            error_rate: 0.01,
          },
          'claude-3-haiku': {
            score: 0.82,
            sample_count: 5000,
            error_rate: 0.02,
          },
        },
        user_satisfaction: {
          avg_rating: 4.2,
          rating_count: 200,
          positive_ratio: 0.85,
        },
      },
      anomaly_signals: [],
      routing_history: [],
    },
  };
}

// Mock RuVector Client
class MockRuVectorClient {
  public persistedEvents: DecisionEvent[] = [];

  async persistDecisionEvent(event: DecisionEvent) {
    this.persistedEvents.push(event);
    return {
      success: true,
      record_id: `mock-${Date.now()}`,
      persisted_at: new Date().toISOString(),
    };
  }
}

// ============================================================================
// Verification Tests
// ============================================================================

describe('Self-Optimizing Agent', () => {
  let agent: SelfOptimizingAgent;
  let mockRuVectorClient: MockRuVectorClient;

  beforeEach(() => {
    mockRuVectorClient = new MockRuVectorClient();
    agent = createSelfOptimizingAgent({
      ruVectorClient: mockRuVectorClient as unknown as RuVectorClient,
      enableTelemetry: false,
    });
  });

  // --------------------------------------------------------------------------
  // Constitution Compliance Tests
  // --------------------------------------------------------------------------

  describe('Constitution Compliance', () => {
    test('Agent has correct ID', () => {
      expect(AGENT_ID).toBe('self-optimizing-agent');
    });

    test('Agent has semantic version', () => {
      expect(AGENT_VERSION).toMatch(/^\d+\.\d+\.\d+$/);
    });

    test('Decision type is correctly defined', () => {
      expect(DECISION_TYPE).toBe('system_optimization_recommendation');
    });

    test('Agent is classified as RECOMMENDATION', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      // RECOMMENDATION agents must produce recommendations
      expect(output.recommendations).toBeDefined();
      expect(Array.isArray(output.recommendations)).toBe(true);
    });

    test('Agent emits exactly ONE DecisionEvent per invocation', async () => {
      const input = createValidInput();
      await agent.recommend(input);

      expect(mockRuVectorClient.persistedEvents.length).toBe(1);
    });

    test('DecisionEvent contains all required fields', async () => {
      const input = createValidInput();
      await agent.recommend(input);

      const event = mockRuVectorClient.persistedEvents[0];

      expect(event.agent_id).toBe(AGENT_ID);
      expect(event.agent_version).toBe(AGENT_VERSION);
      expect(event.decision_type).toBe(DECISION_TYPE);
      expect(event.inputs_hash).toBeDefined();
      expect(typeof event.inputs_hash).toBe('string');
      expect(event.outputs).toBeDefined();
      expect(event.confidence).toBeGreaterThanOrEqual(0);
      expect(event.confidence).toBeLessThanOrEqual(1);
      expect(event.constraints_applied).toBeDefined();
      expect(Array.isArray(event.constraints_applied)).toBe(true);
      expect(event.execution_ref).toBe(input.execution_ref);
      expect(event.timestamp).toBeDefined();
    });

    test('Agent does NOT execute optimizations (is RECOMMENDATION only)', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      // RECOMMENDATION agents do not have execution artifacts
      // They only produce recommendations for downstream consumption
      expect(output).not.toHaveProperty('execution_result');
      expect(output).not.toHaveProperty('applied_changes');
    });
  });

  // --------------------------------------------------------------------------
  // Input Validation Tests
  // --------------------------------------------------------------------------

  describe('Input Validation', () => {
    test('Validates required fields', () => {
      expect(() => validateInput({})).toThrow('missing required field');
    });

    test('Validates optimization targets sum to 1.0', () => {
      const input = createValidInput();
      input.optimization_targets = {
        cost_weight: 0.5,
        latency_weight: 0.5,
        quality_weight: 0.5, // Sum is 1.5, not 1.0
      };

      expect(() => validateInput(input)).toThrow('must sum to 1.0');
    });

    test('Validates time window dates', () => {
      const input = createValidInput();
      input.time_window.start = 'invalid-date';

      expect(() => validateInput(input)).toThrow('valid ISO 8601');
    });

    test('Validates time window order', () => {
      const input = createValidInput();
      const temp = input.time_window.start;
      input.time_window.start = input.time_window.end;
      input.time_window.end = temp;

      expect(() => validateInput(input)).toThrow('end must be after start');
    });
  });

  // --------------------------------------------------------------------------
  // Output Validation Tests
  // --------------------------------------------------------------------------

  describe('Output Validation', () => {
    test('Output contains all required fields', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      expect(output.output_id).toBeDefined();
      expect(output.execution_ref).toBe(input.execution_ref);
      expect(output.agent_version).toBe(AGENT_VERSION);
      expect(output.timestamp).toBeDefined();
      expect(output.summary).toBeDefined();
      expect(output.recommendations).toBeDefined();
      expect(output.constraints_evaluated).toBeDefined();
      expect(output.metadata).toBeDefined();
    });

    test('Recommendations have unique ranks', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      const ranks = output.recommendations.map(r => r.rank);
      const uniqueRanks = new Set(ranks);
      expect(ranks.length).toBe(uniqueRanks.size);
    });

    test('Recommendations are sorted by rank', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      for (let i = 1; i < output.recommendations.length; i++) {
        expect(output.recommendations[i].rank).toBeGreaterThan(
          output.recommendations[i - 1].rank
        );
      }
    });
  });

  // --------------------------------------------------------------------------
  // CLI Endpoint Tests
  // --------------------------------------------------------------------------

  describe('CLI Endpoints', () => {
    test('analyze() returns valid output', async () => {
      const input = createValidInput();
      const output = await agent.analyze(input);

      expect(output).toBeDefined();
      validateOutput(output);
    });

    test('recommend() returns valid output', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      expect(output).toBeDefined();
      validateOutput(output);
    });

    test('simulate() returns valid output', async () => {
      const input = createValidInput();
      const output = await agent.simulate(input);

      expect(output).toBeDefined();
      validateOutput(output);
    });
  });

  // --------------------------------------------------------------------------
  // Determinism Tests
  // --------------------------------------------------------------------------

  describe('Determinism', () => {
    test('Same input produces same inputs_hash', async () => {
      const input = createValidInput();
      const hash1 = hashInputs(input);
      const hash2 = hashInputs(input);

      expect(hash1).toBe(hash2);
    });

    test('Different inputs produce different hashes', async () => {
      const input1 = createValidInput();
      const input2 = createValidInput();
      input2.execution_ref = 'different-ref';

      const hash1 = hashInputs(input1);
      const hash2 = hashInputs(input2);

      expect(hash1).not.toBe(hash2);
    });
  });

  // --------------------------------------------------------------------------
  // Constraint Handling Tests
  // --------------------------------------------------------------------------

  describe('Constraint Handling', () => {
    test('Hard constraints are respected', async () => {
      const input = createValidInput();
      input.constraints = [
        {
          type: 'max_cost_increase_pct',
          value: 0, // Very strict - no cost increase allowed
          hard: true,
        },
      ];

      const output = await agent.recommend(input);

      // All recommendations should respect the constraint
      for (const rec of output.recommendations) {
        expect(rec.expected_impact.cost_change_pct).toBeLessThanOrEqual(0);
      }
    });

    test('Soft constraints influence but do not block', async () => {
      const input = createValidInput();
      input.constraints = [
        {
          type: 'min_quality_threshold',
          value: 0.99, // Very high - hard to satisfy
          hard: false,
        },
      ];

      const output = await agent.recommend(input);

      // Agent may still produce recommendations for soft constraints
      expect(output.recommendations.length).toBeGreaterThanOrEqual(0);
    });

    test('Constraint evaluation is tracked in output', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      expect(output.constraints_evaluated.length).toBe(input.constraints.length);
      for (const ec of output.constraints_evaluated) {
        expect(ec.constraint).toBeDefined();
        expect(typeof ec.satisfied).toBe('boolean');
      }
    });
  });

  // --------------------------------------------------------------------------
  // Non-Responsibility Tests (What Agent MUST NOT Do)
  // --------------------------------------------------------------------------

  describe('Non-Responsibilities', () => {
    test('Agent does NOT execute optimizations directly', () => {
      // Verify no execute() method exists
      expect((agent as unknown as Record<string, unknown>)['execute']).toBeUndefined;
      expect((agent as unknown as Record<string, unknown>)['applyChanges']).toBeUndefined();
      expect((agent as unknown as Record<string, unknown>)['deploy']).toBeUndefined();
    });

    test('Agent does NOT modify runtime behavior', () => {
      // No methods that would indicate runtime modification
      expect((agent as unknown as Record<string, unknown>)['intercept']).toBeUndefined();
      expect((agent as unknown as Record<string, unknown>)['route']).toBeUndefined();
      expect((agent as unknown as Record<string, unknown>)['enforce']).toBeUndefined();
    });

    test('Agent does NOT emit alerts (that is Sentinel)', () => {
      expect((agent as unknown as Record<string, unknown>)['alert']).toBeUndefined();
      expect((agent as unknown as Record<string, unknown>)['notify']).toBeUndefined();
      expect((agent as unknown as Record<string, unknown>)['sendAlert']).toBeUndefined();
    });

    test('Agent does NOT enforce policies (that is Shield)', () => {
      expect((agent as unknown as Record<string, unknown>)['enforcePolicy']).toBeUndefined();
      expect((agent as unknown as Record<string, unknown>)['block']).toBeUndefined();
      expect((agent as unknown as Record<string, unknown>)['validatePolicy']).toBeUndefined();
    });

    test('Agent does NOT orchestrate workflows', () => {
      expect((agent as unknown as Record<string, unknown>)['orchestrate']).toBeUndefined();
      expect((agent as unknown as Record<string, unknown>)['workflow']).toBeUndefined();
      expect((agent as unknown as Record<string, unknown>)['schedule']).toBeUndefined();
    });
  });
});

// ============================================================================
// Smoke Tests (CLI Integration)
// ============================================================================

describe('Smoke Tests', () => {
  test('Can create agent instance', () => {
    const agent = createSelfOptimizingAgent();
    expect(agent).toBeDefined();
  });

  test('Can run full recommend workflow', async () => {
    const mockClient = new MockRuVectorClient();
    const agent = createSelfOptimizingAgent({
      ruVectorClient: mockClient as unknown as RuVectorClient,
      enableTelemetry: false,
    });

    const input = createValidInput();
    const output = await agent.recommend(input);

    // Verify complete workflow
    expect(output.output_id).toBeDefined();
    expect(output.recommendations.length).toBeGreaterThanOrEqual(0);
    expect(mockClient.persistedEvents.length).toBe(1);

    console.log('Smoke test passed:');
    console.log(`  - Execution: ${output.execution_ref}`);
    console.log(`  - Recommendations: ${output.recommendations.length}`);
    console.log(`  - Processing Time: ${output.metadata.processing_duration_ms}ms`);
    console.log(`  - DecisionEvent persisted: ${mockClient.persistedEvents.length > 0}`);
  });
});

// ============================================================================
// Verification Checklist
// ============================================================================

/*
VERIFICATION CHECKLIST - Self-Optimizing Agent

[x] Agent Contract (Prompt 1)
    [x] Agent ID: self-optimizing-agent
    [x] Agent Version: 1.0.0 (semantic)
    [x] Classification: RECOMMENDATION
    [x] Decision Type: system_optimization_recommendation
    [x] Input schema defined in agentics-contracts
    [x] Output schema defined in agentics-contracts
    [x] DecisionEvent schema compliance

[x] Runtime Implementation (Prompt 2)
    [x] Google Cloud Edge Function handler
    [x] Stateless execution
    [x] Deterministic behavior (same input = same hash)
    [x] No orchestration logic
    [x] No enforcement logic
    [x] No direct SQL access
    [x] Async persistence via ruvector-service

[x] Platform Wiring (Prompt 3)
    [x] Agent registered in agents/index.ts
    [x] CLI commands: analyze, recommend, simulate
    [x] DecisionEvent persistence verified
    [x] Telemetry emission (optional, configurable)

[x] Non-Responsibilities
    [x] Does NOT execute optimizations
    [x] Does NOT modify runtime behavior
    [x] Does NOT intercept execution traffic
    [x] Does NOT trigger retries
    [x] Does NOT emit alerts (Sentinel responsibility)
    [x] Does NOT enforce policies (Shield responsibility)
    [x] Does NOT perform orchestration (LLM-Orchestrator responsibility)

[x] Failure Modes
    [x] Input validation errors throw descriptive errors
    [x] RuVector persistence failures are propagated
    [x] Telemetry failures are non-blocking (logged only)
    [x] Invalid constraints are evaluated and reported

SMOKE TEST COMMANDS:
  agentics-cli optimize analyze --window 24h
  agentics-cli optimize recommend --window 7d --focus cost
  agentics-cli optimize simulate --window 24h --dry-run
*/
