/**
 * Agent Registry - LLM-Auto-Optimizer
 *
 * Central registry for all optimization agents in the platform.
 * Each agent is a stateless, deterministic unit that:
 * - Imports schemas from agentics-contracts
 * - Emits DecisionEvents to ruvector-service
 * - Exposes CLI-invokable endpoints
 *
 * @module agents
 */

// ============================================================================
// Agent Exports
// ============================================================================

// Self-Optimizing Agent
export {
  SelfOptimizingAgent,
  createSelfOptimizingAgent,
  AGENT_ID as SELF_OPTIMIZING_AGENT_ID,
  AGENT_VERSION as SELF_OPTIMIZING_AGENT_VERSION,
} from './self-optimizing-agent';

export type {
  SelfOptimizingAgentConfig,
} from './self-optimizing-agent';

// Self-Optimizing Agent Handler (Edge Function)
export {
  SelfOptimizingAgentHandler,
  selfOptimizingAgent,
  handler as selfOptimizingAgentHandler,
  buildInputFromCLI,
} from './self-optimizing-agent/handler';

// Token Optimization Agent
export {
  TokenOptimizationAgent,
  createTokenOptimizationAgent,
  AGENT_ID as TOKEN_OPTIMIZATION_AGENT_ID,
  AGENT_VERSION as TOKEN_OPTIMIZATION_AGENT_VERSION,
  DECISION_TYPE_RECOMMEND as TOKEN_OPTIMIZATION_DECISION_TYPE,
} from './token-optimization-agent';

export type {
  TokenOptimizationAgentConfig,
} from './token-optimization-agent';

// Token Optimization Agent Handler (Edge Function)
export {
  TokenOptimizationAgentHandler,
  tokenOptimizationAgent,
  handler as tokenOptimizationAgentHandler,
  buildInputFromCLI as buildTokenOptimizationInputFromCLI,
  formatOutputForCLI as formatTokenOptimizationOutputForCLI,
} from './token-optimization-agent/handler';

export type {
  TokenOptimizationCLIParams,
} from './token-optimization-agent/handler';

// Token Optimization Agent Types
export * from './token-optimization-agent/types';

// Model Selection Agent
export {
  ModelSelectionAgent,
  createModelSelectionAgent,
  AGENT_ID as MODEL_SELECTION_AGENT_ID,
  AGENT_VERSION as MODEL_SELECTION_AGENT_VERSION,
} from './model-selection-agent';

export type {
  ModelSelectionAgentConfig,
} from './model-selection-agent';

// Model Selection Agent Handler (Edge Function)
export {
  ModelSelectionAgentHandler,
  modelSelectionAgent,
  handler as modelSelectionAgentHandler,
  buildInputFromCLI as buildModelSelectionInputFromCLI,
} from './model-selection-agent/handler';

export type {
  ModelSelectCLIParams,
} from './model-selection-agent/handler';

// ============================================================================
// Agent Registry
// ============================================================================

export interface AgentRegistryEntry {
  /** Unique agent identifier */
  id: string;
  /** Agent version */
  version: string;
  /** Agent classification */
  classification: 'analysis' | 'recommendation' | 'bounded_adjustment';
  /** Decision type emitted */
  decision_type: string;
  /** Available CLI commands */
  commands: string[];
  /** Agent description */
  description: string;
  /** Edge function endpoint path */
  endpoint: string;
}

/**
 * Registry of all available agents.
 */
export const AGENT_REGISTRY: AgentRegistryEntry[] = [
  {
    id: 'self-optimizing-agent',
    version: '1.0.0',
    classification: 'recommendation',
    decision_type: 'system_optimization_recommendation',
    commands: ['analyze', 'recommend', 'simulate'],
    description: 'Generate holistic optimization recommendations across cost, latency, and quality',
    endpoint: '/agents/self-optimizing-agent',
  },
  {
    id: 'token-optimization-agent',
    version: '1.0.0',
    classification: 'recommendation',
    decision_type: 'token_optimization_recommendation',
    commands: ['analyze', 'recommend', 'simulate'],
    description: 'Analyze token usage patterns and recommend strategies to reduce consumption without degrading quality',
    endpoint: '/agents/token-optimization-agent',
  },
  {
    id: 'model-selection-agent',
    version: '1.0.0',
    classification: 'recommendation',
    decision_type: 'model_selection_recommendation',
    commands: ['analyze', 'recommend', 'simulate'],
    description: 'Recommend optimal model selection based on historical performance signals',
    endpoint: '/agents/model-selection-agent',
  },
];

/**
 * Get agent registry entry by ID.
 */
export function getAgent(agentId: string): AgentRegistryEntry | undefined {
  return AGENT_REGISTRY.find(a => a.id === agentId);
}

/**
 * List all available agents.
 */
export function listAgents(): AgentRegistryEntry[] {
  return AGENT_REGISTRY;
}

/**
 * Check if an agent ID is valid.
 */
export function isValidAgent(agentId: string): boolean {
  return AGENT_REGISTRY.some(a => a.id === agentId);
}
