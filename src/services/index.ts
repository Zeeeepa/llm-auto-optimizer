/**
 * Services Index - LLM-Auto-Optimizer
 *
 * Central exports for all service clients used by agents.
 *
 * @module services
 */

// RuVector Service Client
export {
  RuVectorClient,
  createRuVectorClient,
  getDefaultRuVectorClient,
} from './ruvector-client';

export type {
  RuVectorClientConfig,
  PersistResult,
  QueryOptions,
  QueryResult,
  StoredDecisionEvent,
  HealthStatus,
} from './ruvector-client';
