/**
 * Model Selection Agent - Verification Tests
 *
 * Smoke tests and verification checklist for the Model Selection Agent.
 * These tests verify compliance with the Agent Infrastructure Constitution.
 *
 * @module tests/agents/model-selection-agent
 */

import {
  ModelSelectionAgent,
  createModelSelectionAgent,
  AGENT_ID,
  AGENT_VERSION,
  DECISION_TYPE,
} from '../../src/agents/model-selection-agent';

import {
  ModelSelectionAgentInput,
  ModelSelectionAgentOutput,
  DecisionEvent,
  validateModelSelectionInput,
  validateModelSelectionOutput,
  hashInputs,
} from '../../src/contracts';

import {
  RuVectorClient,
} from '../../src/services/ruvector-client';

// ============================================================================
// Test Fixtures
// ============================================================================

function createValidInput(): ModelSelectionAgentInput {
  const now = new Date();
  const start = new Date(now.getTime() - 86400000); // 24 hours ago

  return {
    execution_ref: `test-${Date.now()}`,
    time_window: {
      start: start.toISOString(),
      end: now.toISOString(),
      granularity: 'hour',
    },
    task_context: {
      task_type: 'code_generation',
      complexity: 'complex',
      expected_input_tokens: {
        min: 500,
        max: 2000,
        avg: 1000,
      },
      expected_output_tokens: {
        min: 250,
        max: 1000,
        avg: 500,
      },
      latency_requirements: {
        max_latency_ms: 2000,
        target_latency_ms: 1000,
        strict: false,
      },
      quality_requirements: {
        min_quality_score: 0.85,
        target_quality_score: 0.95,
        strict: false,
        priorities: ['accuracy', 'completeness'],
      },
      budget_constraints: {
        max_cost_per_request_usd: 0.50,
        cost_optimization_priority: 'medium',
      },
    },
    target_services: ['test-service'],
    constraints: [
      {
        type: 'allowed_providers',
        value: ['anthropic', 'openai'],
        hard: true,
      },
    ],
    performance_history: {
      by_model: {
        'claude-3-opus': {
          model_id: 'claude-3-opus',
          provider: 'anthropic',
          request_count: 2500,
          latency: {
            p50_ms: 2200,
            p95_ms: 3500,
            p99_ms: 5000,
            avg_ms: 2500,
          },
          cost: {
            total_cost_usd: 1200.00,
            avg_cost_per_request_usd: 0.48,
            input_cost_per_1k_tokens: 0.015,
            output_cost_per_1k_tokens: 0.075,
          },
          quality: {
            avg_score: 0.95,
            error_rate: 0.01,
            success_rate: 0.99,
            user_satisfaction: 4.8,
          },
          tokens: {
            total_input: 5000000,
            total_output: 1250000,
            avg_input: 2000,
            avg_output: 500,
          },
          availability: {
            uptime_pct: 99.9,
            timeout_rate: 0.02,
            rate_limit_hit_rate: 0.01,
          },
        },
        'claude-3-5-sonnet': {
          model_id: 'claude-3-5-sonnet',
          provider: 'anthropic',
          request_count: 15000,
          latency: {
            p50_ms: 1000,
            p95_ms: 1800,
            p99_ms: 2500,
            avg_ms: 1200,
          },
          cost: {
            total_cost_usd: 2000.00,
            avg_cost_per_request_usd: 0.133,
            input_cost_per_1k_tokens: 0.003,
            output_cost_per_1k_tokens: 0.015,
          },
          quality: {
            avg_score: 0.93,
            error_rate: 0.015,
            success_rate: 0.985,
            user_satisfaction: 4.6,
          },
          tokens: {
            total_input: 30000000,
            total_output: 7500000,
            avg_input: 2000,
            avg_output: 500,
          },
          availability: {
            uptime_pct: 99.8,
            timeout_rate: 0.01,
            rate_limit_hit_rate: 0.02,
          },
        },
        'claude-3-haiku': {
          model_id: 'claude-3-haiku',
          provider: 'anthropic',
          request_count: 80000,
          latency: {
            p50_ms: 250,
            p95_ms: 450,
            p99_ms: 700,
            avg_ms: 300,
          },
          cost: {
            total_cost_usd: 400.00,
            avg_cost_per_request_usd: 0.005,
            input_cost_per_1k_tokens: 0.00025,
            output_cost_per_1k_tokens: 0.00125,
          },
          quality: {
            avg_score: 0.82,
            error_rate: 0.025,
            success_rate: 0.975,
            user_satisfaction: 4.1,
          },
          tokens: {
            total_input: 160000000,
            total_output: 40000000,
            avg_input: 2000,
            avg_output: 500,
          },
          availability: {
            uptime_pct: 99.95,
            timeout_rate: 0.005,
            rate_limit_hit_rate: 0.005,
          },
        },
      },
      by_task_type: {
        'code_generation': {
          task_type: 'code_generation',
          top_models: [
            { model_id: 'claude-3-opus', score: 0.98, request_count: 500 },
            { model_id: 'claude-3-5-sonnet', score: 0.96, request_count: 3000 },
          ],
          avg_quality_score: 0.94,
          avg_latency_ms: 1500,
          avg_cost_usd: 0.15,
        },
      },
      total_requests: 97500,
      data_window: {
        start: start.toISOString(),
        end: now.toISOString(),
        granularity: 'hour',
      },
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

describe('Model Selection Agent', () => {
  let agent: ModelSelectionAgent;
  let mockRuVectorClient: MockRuVectorClient;

  beforeEach(() => {
    mockRuVectorClient = new MockRuVectorClient();
    agent = createModelSelectionAgent({
      ruVectorClient: mockRuVectorClient as unknown as RuVectorClient,
      enableTelemetry: false,
    });
  });

  // ==========================================================================
  // Constitution Compliance Tests
  // ==========================================================================

  describe('Constitution Compliance', () => {
    test('MUST have correct agent identifier', () => {
      expect(AGENT_ID).toBe('model-selection-agent');
    });

    test('MUST have semantic version', () => {
      expect(AGENT_VERSION).toMatch(/^\d+\.\d+\.\d+$/);
    });

    test('MUST declare RECOMMENDATION classification', () => {
      expect(DECISION_TYPE).toBe('model_selection_recommendation');
    });

    test('MUST expose analyze/recommend/simulate methods', () => {
      expect(typeof agent.analyze).toBe('function');
      expect(typeof agent.recommend).toBe('function');
      expect(typeof agent.simulate).toBe('function');
    });
  });

  // ==========================================================================
  // Input Validation Tests
  // ==========================================================================

  describe('Input Validation', () => {
    test('MUST validate valid input', () => {
      const input = createValidInput();
      expect(() => validateModelSelectionInput(input)).not.toThrow();
    });

    test('MUST reject input missing required fields', () => {
      const input = { execution_ref: 'test' } as any;
      expect(() => validateModelSelectionInput(input)).toThrow();
    });

    test('MUST reject invalid time_window', () => {
      const input = createValidInput();
      input.time_window.end = input.time_window.start; // end <= start
      expect(() => validateModelSelectionInput(input)).toThrow();
    });

    test('MUST reject invalid task_context', () => {
      const input = createValidInput();
      (input.task_context as any).task_type = undefined;
      expect(() => validateModelSelectionInput(input)).toThrow();
    });
  });

  // ==========================================================================
  // Recommendation Generation Tests
  // ==========================================================================

  describe('Recommendation Generation', () => {
    test('MUST generate at least one recommendation', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      expect(output.primary_recommendation).toBeDefined();
      expect(output.primary_recommendation.model_id).toBeTruthy();
    });

    test('MUST rank recommendations correctly', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      expect(output.primary_recommendation.rank).toBe(1);

      for (let i = 0; i < output.alternative_recommendations.length; i++) {
        expect(output.alternative_recommendations[i].rank).toBe(i + 2);
      }
    });

    test('MUST include score breakdown', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      const scores = output.primary_recommendation.score_breakdown;
      expect(scores.quality_score).toBeGreaterThanOrEqual(0);
      expect(scores.quality_score).toBeLessThanOrEqual(1);
      expect(scores.latency_score).toBeGreaterThanOrEqual(0);
      expect(scores.latency_score).toBeLessThanOrEqual(1);
      expect(scores.cost_score).toBeGreaterThanOrEqual(0);
      expect(scores.cost_score).toBeLessThanOrEqual(1);
    });

    test('MUST include expected performance projections', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      const perf = output.primary_recommendation.expected_performance;
      expect(perf.latency_ms).toBeDefined();
      expect(perf.quality_score).toBeGreaterThanOrEqual(0);
      expect(perf.cost_per_request_usd).toBeGreaterThan(0);
      expect(perf.success_rate).toBeGreaterThanOrEqual(0);
    });

    test('MUST provide reasoning for recommendation', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      expect(output.primary_recommendation.reasoning).toBeTruthy();
      expect(output.primary_recommendation.reasoning.length).toBeGreaterThan(10);
    });
  });

  // ==========================================================================
  // Constraint Evaluation Tests
  // ==========================================================================

  describe('Constraint Evaluation', () => {
    test('MUST respect allowed_providers constraint', async () => {
      const input = createValidInput();
      input.constraints = [
        {
          type: 'allowed_providers',
          value: ['anthropic'],
          hard: true,
        },
      ];

      const output = await agent.recommend(input);

      expect(output.primary_recommendation.provider).toBe('anthropic');
      for (const alt of output.alternative_recommendations) {
        expect(alt.provider).toBe('anthropic');
      }
    });

    test('MUST respect blocked_models constraint', async () => {
      const input = createValidInput();
      input.constraints = [
        {
          type: 'blocked_models',
          value: ['claude-3-opus'],
          hard: true,
        },
      ];

      const output = await agent.recommend(input);

      expect(output.primary_recommendation.model_id).not.toBe('claude-3-opus');
      for (const alt of output.alternative_recommendations) {
        expect(alt.model_id).not.toBe('claude-3-opus');
      }
    });

    test('MUST respect max_cost_per_request constraint', async () => {
      const input = createValidInput();
      input.constraints = [
        {
          type: 'max_cost_per_request',
          value: 0.01, // Very low - should filter expensive models
          hard: true,
        },
      ];

      const output = await agent.recommend(input);

      // Should recommend cheaper models
      expect(output.primary_recommendation.expected_performance.cost_per_request_usd)
        .toBeLessThanOrEqual(0.01);
    });

    test('MUST report constraint evaluation results', async () => {
      const input = createValidInput();
      input.constraints = [
        {
          type: 'min_quality_threshold',
          value: 0.8,
          hard: true,
        },
      ];

      const output = await agent.recommend(input);

      expect(output.constraints_evaluated).toBeDefined();
      expect(output.constraints_evaluated.length).toBe(1);
      expect(output.constraints_evaluated[0].satisfied).toBeDefined();
    });
  });

  // ==========================================================================
  // Analysis Mode Tests
  // ==========================================================================

  describe('Analysis Mode', () => {
    test('analyze() should not generate recommendations', async () => {
      const input = createValidInput();
      const output = await agent.analyze(input);

      // Primary recommendation should be a placeholder
      expect(output.primary_recommendation.model_id).toBe('none');
      expect(output.alternative_recommendations).toHaveLength(0);
    });

    test('analyze() should still evaluate constraints', async () => {
      const input = createValidInput();
      const output = await agent.analyze(input);

      expect(output.constraints_evaluated).toBeDefined();
      expect(output.analysis_summary).toBeDefined();
    });
  });

  // ==========================================================================
  // Simulation Mode Tests
  // ==========================================================================

  describe('Simulation Mode', () => {
    test('simulate() should NOT persist DecisionEvent', async () => {
      const input = createValidInput();
      await agent.simulate(input);

      expect(mockRuVectorClient.persistedEvents).toHaveLength(0);
    });

    test('simulate() should generate recommendations', async () => {
      const input = createValidInput();
      const output = await agent.simulate(input);

      expect(output.primary_recommendation.model_id).not.toBe('none');
    });
  });

  // ==========================================================================
  // DecisionEvent Tests
  // ==========================================================================

  describe('DecisionEvent Persistence', () => {
    test('recommend() MUST persist DecisionEvent', async () => {
      const input = createValidInput();
      await agent.recommend(input);

      expect(mockRuVectorClient.persistedEvents).toHaveLength(1);
    });

    test('DecisionEvent MUST have correct structure', async () => {
      const input = createValidInput();
      await agent.recommend(input);

      const event = mockRuVectorClient.persistedEvents[0];

      expect(event.agent_id).toBe(AGENT_ID);
      expect(event.agent_version).toBe(AGENT_VERSION);
      expect(event.decision_type).toBe(DECISION_TYPE);
      expect(event.inputs_hash).toBeTruthy();
      expect(event.outputs).toBeDefined();
      expect(event.confidence).toBeGreaterThanOrEqual(0);
      expect(event.confidence).toBeLessThanOrEqual(1);
      expect(event.constraints_applied).toBeDefined();
      expect(event.execution_ref).toBe(input.execution_ref);
      expect(event.timestamp).toBeTruthy();
    });

    test('inputs_hash MUST be deterministic', async () => {
      const input = createValidInput();
      const hash1 = hashInputs(input);
      const hash2 = hashInputs(input);

      expect(hash1).toBe(hash2);
    });
  });

  // ==========================================================================
  // Output Validation Tests
  // ==========================================================================

  describe('Output Validation', () => {
    test('MUST produce valid output', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      expect(() => validateModelSelectionOutput(output)).not.toThrow();
    });

    test('MUST include analysis_summary', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      expect(output.analysis_summary).toBeDefined();
      expect(output.analysis_summary.models_evaluated).toBeGreaterThan(0);
      expect(output.analysis_summary.models_passing_constraints).toBeGreaterThanOrEqual(0);
    });

    test('MUST include metadata', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      expect(output.metadata).toBeDefined();
      expect(output.metadata.processing_duration_ms).toBeGreaterThanOrEqual(0);
      expect(output.metadata.models_considered).toBeDefined();
      expect(Array.isArray(output.metadata.models_considered)).toBe(true);
    });

    test('MUST include evidence for recommendations', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      if (output.primary_recommendation.model_id !== 'none') {
        expect(output.primary_recommendation.evidence).toBeDefined();
        expect(output.primary_recommendation.evidence.length).toBeGreaterThan(0);
      }
    });
  });

  // ==========================================================================
  // Task Type Routing Tests
  // ==========================================================================

  describe('Task Type Routing', () => {
    test('SHOULD recommend models suited for code_generation', async () => {
      const input = createValidInput();
      input.task_context.task_type = 'code_generation';
      input.task_context.complexity = 'expert';

      const output = await agent.recommend(input);

      // Should recommend high-capability models for code generation
      expect(output.primary_recommendation.score_breakdown.task_fit_score)
        .toBeGreaterThan(0.7);
    });

    test('SHOULD recommend different models for simple classification', async () => {
      const input = createValidInput();
      input.task_context.task_type = 'classification';
      input.task_context.complexity = 'simple';
      input.task_context.budget_constraints.cost_optimization_priority = 'high';

      const output = await agent.recommend(input);

      // Should favor cost-effective models for simple tasks
      expect(output.primary_recommendation.expected_performance.cost_per_request_usd)
        .toBeLessThan(0.2);
    });
  });

  // ==========================================================================
  // Non-Responsibility Tests (What Agent MUST NOT Do)
  // ==========================================================================

  describe('Non-Responsibilities', () => {
    test('Agent MUST NOT execute model routing', () => {
      // The agent only recommends - no routing methods exist
      expect((agent as any).routeTraffic).toBeUndefined();
      expect((agent as any).enforceSelection).toBeUndefined();
    });

    test('Agent MUST NOT modify runtime behavior', () => {
      // No methods that could modify external systems
      expect((agent as any).applyChanges).toBeUndefined();
      expect((agent as any).deployConfiguration).toBeUndefined();
    });

    test('Output MUST be advisory only', async () => {
      const input = createValidInput();
      const output = await agent.recommend(input);

      // Check that output only contains recommendations, not actions
      expect(output.primary_recommendation).toBeDefined();
      expect((output as any).executed).toBeUndefined();
      expect((output as any).applied).toBeUndefined();
    });
  });

  // ==========================================================================
  // Edge Case Tests
  // ==========================================================================

  describe('Edge Cases', () => {
    test('SHOULD handle empty performance history gracefully', async () => {
      const input = createValidInput();
      input.performance_history = {
        by_model: {},
        by_task_type: {},
        total_requests: 0,
        data_window: input.performance_history.data_window,
      };

      const output = await agent.recommend(input);

      // Should still produce recommendations based on benchmarks
      expect(output.primary_recommendation).toBeDefined();
    });

    test('SHOULD handle restrictive constraints that filter all models', async () => {
      const input = createValidInput();
      input.constraints = [
        {
          type: 'max_cost_per_request',
          value: 0.00001, // Impossibly low
          hard: true,
        },
      ];

      const output = await agent.recommend(input);

      // Should return placeholder recommendation
      expect(output.primary_recommendation.model_id).toBe('none');
      expect(output.primary_recommendation.confidence).toBe(0);
    });

    test('SHOULD handle conflicting constraints', async () => {
      const input = createValidInput();
      input.constraints = [
        {
          type: 'min_quality_threshold',
          value: 0.99, // Very high
          hard: true,
        },
        {
          type: 'max_cost_per_request',
          value: 0.001, // Very low
          hard: true,
        },
      ];

      const output = await agent.recommend(input);

      // Should handle gracefully (likely no models pass both)
      expect(output.analysis_summary.warnings.length).toBeGreaterThan(0);
    });
  });
});

// ============================================================================
// CLI Helper Tests
// ============================================================================

describe('Model Selection CLI Helpers', () => {
  const { buildInputFromCLI } = require('../../src/agents/model-selection-agent/handler');

  test('SHOULD build valid input from CLI params', () => {
    const params = {
      task_type: 'code_generation',
      complexity: 'complex',
      input_tokens: 1000,
      output_tokens: 500,
      max_latency: 2000,
    };

    const input = buildInputFromCLI(params);

    expect(() => validateModelSelectionInput(input)).not.toThrow();
    expect(input.task_context.task_type).toBe('code_generation');
    expect(input.task_context.complexity).toBe('complex');
  });

  test('SHOULD handle provider constraints from CLI', () => {
    const params = {
      providers: 'anthropic,openai',
    };

    const input = buildInputFromCLI(params);

    expect(input.constraints.some(c => c.type === 'allowed_providers')).toBe(true);
    const providerConstraint = input.constraints.find(c => c.type === 'allowed_providers');
    expect(providerConstraint?.value).toEqual(['anthropic', 'openai']);
  });

  test('SHOULD apply default values for missing params', () => {
    const params = {};

    const input = buildInputFromCLI(params);

    expect(input.task_context.task_type).toBe('general');
    expect(input.task_context.complexity).toBe('moderate');
  });
});
