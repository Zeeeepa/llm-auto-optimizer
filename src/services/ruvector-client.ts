/**
 * RuVector Service Client
 *
 * Thin client for persisting DecisionEvents to ruvector-service.
 * LLM-Auto-Optimizer NEVER connects directly to Google SQL.
 * ALL persistence occurs via this client ONLY.
 *
 * @module services/ruvector-client
 */

import { DecisionEvent } from '../contracts';

// ============================================================================
// Configuration
// ============================================================================

export interface RuVectorClientConfig {
  /** RuVector service base URL */
  baseUrl: string;
  /** Authentication API key */
  apiKey: string;
  /** Request timeout in milliseconds */
  timeout?: number;
  /** Enable retry on transient failures */
  enableRetry?: boolean;
  /** Maximum retry attempts */
  maxRetries?: number;
  /** Environment identifier */
  environment?: string;
}

const DEFAULT_CONFIG: Partial<RuVectorClientConfig> = {
  timeout: 30000,
  enableRetry: true,
  maxRetries: 3,
  environment: process.env.NODE_ENV || 'production',
};

// ============================================================================
// Types
// ============================================================================

export interface PersistResult {
  /** Whether persistence was successful */
  success: boolean;
  /** Record ID in ruvector-service */
  record_id: string;
  /** Timestamp of persistence */
  persisted_at: string;
  /** Any warnings (non-fatal) */
  warnings?: string[];
}

export interface QueryOptions {
  /** Filter by agent ID */
  agent_id?: string;
  /** Filter by decision type */
  decision_type?: string;
  /** Time range start (ISO 8601) */
  since?: string;
  /** Time range end (ISO 8601) */
  until?: string;
  /** Maximum results */
  limit?: number;
  /** Offset for pagination */
  offset?: number;
  /** Sort order */
  sort?: 'asc' | 'desc';
}

export interface QueryResult {
  /** Retrieved decision events */
  events: StoredDecisionEvent[];
  /** Total count (for pagination) */
  total_count: number;
  /** Whether more results exist */
  has_more: boolean;
}

export interface StoredDecisionEvent extends DecisionEvent {
  /** Storage record ID */
  record_id: string;
  /** When persisted */
  persisted_at: string;
  /** Storage version */
  storage_version: number;
}

export interface HealthStatus {
  /** Service health */
  status: 'healthy' | 'degraded' | 'unhealthy';
  /** Last health check */
  last_check: string;
  /** Backend status */
  backend: {
    postgres: 'up' | 'down' | 'degraded';
    cache: 'up' | 'down' | 'degraded';
  };
  /** Latency metrics */
  latency_ms: number;
}

// ============================================================================
// RuVector Client
// ============================================================================

/**
 * Client for interacting with ruvector-service.
 * All persistence operations for LLM-Auto-Optimizer go through this client.
 */
export class RuVectorClient {
  private config: Required<RuVectorClientConfig>;

  constructor(config: RuVectorClientConfig) {
    this.config = {
      ...DEFAULT_CONFIG,
      ...config,
    } as Required<RuVectorClientConfig>;
  }

  // --------------------------------------------------------------------------
  // DecisionEvent Persistence
  // --------------------------------------------------------------------------

  /**
   * Persist a DecisionEvent to ruvector-service.
   * This is the ONLY way DecisionEvents should be stored.
   *
   * @param event - The DecisionEvent to persist
   * @returns PersistResult with record ID
   * @throws Error if persistence fails after retries
   */
  async persistDecisionEvent(event: DecisionEvent): Promise<PersistResult> {
    this.validateDecisionEvent(event);

    const payload = {
      event,
      metadata: {
        source: 'llm-auto-optimizer',
        environment: this.config.environment,
        client_version: '1.0.0',
      },
    };

    const response = await this.makeRequest<{
      record_id: string;
      persisted_at: string;
      warnings?: string[];
    }>('/api/v1/decisions', {
      method: 'POST',
      body: JSON.stringify(payload),
    });

    return {
      success: true,
      record_id: response.record_id,
      persisted_at: response.persisted_at,
      warnings: response.warnings,
    };
  }

  /**
   * Persist multiple DecisionEvents in a single batch.
   * More efficient for bulk operations.
   *
   * @param events - Array of DecisionEvents to persist
   * @returns Array of PersistResults
   */
  async persistDecisionEventBatch(events: DecisionEvent[]): Promise<PersistResult[]> {
    if (events.length === 0) {
      return [];
    }

    if (events.length > 100) {
      throw new Error('Batch size exceeds maximum of 100 events');
    }

    for (const event of events) {
      this.validateDecisionEvent(event);
    }

    const payload = {
      events,
      metadata: {
        source: 'llm-auto-optimizer',
        environment: this.config.environment,
        client_version: '1.0.0',
        batch_size: events.length,
      },
    };

    const response = await this.makeRequest<{
      results: Array<{
        record_id: string;
        persisted_at: string;
        warnings?: string[];
      }>;
    }>('/api/v1/decisions/batch', {
      method: 'POST',
      body: JSON.stringify(payload),
    });

    return response.results.map(r => ({
      success: true,
      record_id: r.record_id,
      persisted_at: r.persisted_at,
      warnings: r.warnings,
    }));
  }

  // --------------------------------------------------------------------------
  // DecisionEvent Queries (for governance/audit views)
  // --------------------------------------------------------------------------

  /**
   * Query stored DecisionEvents.
   * Used by governance and audit consumers.
   *
   * @param options - Query filter options
   * @returns QueryResult with matching events
   */
  async queryDecisionEvents(options: QueryOptions = {}): Promise<QueryResult> {
    const params = new URLSearchParams();

    if (options.agent_id) params.set('agent_id', options.agent_id);
    if (options.decision_type) params.set('decision_type', options.decision_type);
    if (options.since) params.set('since', options.since);
    if (options.until) params.set('until', options.until);
    if (options.limit) params.set('limit', String(options.limit));
    if (options.offset) params.set('offset', String(options.offset));
    if (options.sort) params.set('sort', options.sort);

    return this.makeRequest<QueryResult>(
      `/api/v1/decisions?${params.toString()}`
    );
  }

  /**
   * Get a specific DecisionEvent by record ID.
   *
   * @param recordId - The storage record ID
   * @returns The stored DecisionEvent
   */
  async getDecisionEvent(recordId: string): Promise<StoredDecisionEvent> {
    return this.makeRequest<StoredDecisionEvent>(
      `/api/v1/decisions/${encodeURIComponent(recordId)}`
    );
  }

  /**
   * Get aggregate statistics for DecisionEvents.
   * Used for dashboards and monitoring.
   */
  async getDecisionStats(options: {
    agent_id?: string;
    since?: string;
    until?: string;
  } = {}): Promise<{
    total_decisions: number;
    by_type: Record<string, number>;
    by_agent: Record<string, number>;
    avg_confidence: number;
    success_rate: number;
  }> {
    const params = new URLSearchParams();
    if (options.agent_id) params.set('agent_id', options.agent_id);
    if (options.since) params.set('since', options.since);
    if (options.until) params.set('until', options.until);

    return this.makeRequest(`/api/v1/decisions/stats?${params.toString()}`);
  }

  // --------------------------------------------------------------------------
  // Health Check
  // --------------------------------------------------------------------------

  /**
   * Check ruvector-service health.
   */
  async checkHealth(): Promise<HealthStatus> {
    return this.makeRequest<HealthStatus>('/api/v1/health');
  }

  // --------------------------------------------------------------------------
  // Internal Helpers
  // --------------------------------------------------------------------------

  private validateDecisionEvent(event: DecisionEvent): void {
    const requiredFields: (keyof DecisionEvent)[] = [
      'agent_id',
      'agent_version',
      'decision_type',
      'inputs_hash',
      'outputs',
      'confidence',
      'constraints_applied',
      'execution_ref',
      'timestamp',
    ];

    for (const field of requiredFields) {
      if (!(field in event) || event[field] === undefined) {
        throw new Error(`Invalid DecisionEvent: missing required field '${field}'`);
      }
    }

    if (event.confidence < 0 || event.confidence > 1) {
      throw new Error('Invalid DecisionEvent: confidence must be between 0 and 1');
    }

    // Validate timestamp is valid ISO 8601
    const timestamp = new Date(event.timestamp);
    if (isNaN(timestamp.getTime())) {
      throw new Error('Invalid DecisionEvent: timestamp must be valid ISO 8601');
    }
  }

  private async makeRequest<T>(
    path: string,
    options: RequestInit = {},
    attempt: number = 0
  ): Promise<T> {
    const url = `${this.config.baseUrl}${path}`;
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.config.timeout);

    try {
      const response = await fetch(url, {
        ...options,
        headers: {
          'Authorization': `Bearer ${this.config.apiKey}`,
          'Content-Type': 'application/json',
          'X-Client': 'llm-auto-optimizer',
          'X-Client-Version': '1.0.0',
          'X-Environment': this.config.environment,
          ...options.headers,
        },
        signal: controller.signal,
      });

      if (!response.ok) {
        const errorBody = await response.text().catch(() => 'Unknown error');
        throw new Error(
          `RuVector API error: ${response.status} ${response.statusText} - ${errorBody}`
        );
      }

      return await response.json() as T;
    } catch (error) {
      if (
        this.config.enableRetry &&
        attempt < this.config.maxRetries &&
        this.isRetryable(error)
      ) {
        await this.sleep(this.calculateBackoff(attempt));
        return this.makeRequest<T>(path, options, attempt + 1);
      }
      throw error;
    } finally {
      clearTimeout(timeoutId);
    }
  }

  private isRetryable(error: unknown): boolean {
    if (error instanceof Error) {
      // Retry on network errors and 5xx
      return (
        error.name === 'AbortError' ||
        error.message.includes('502') ||
        error.message.includes('503') ||
        error.message.includes('504') ||
        error.message.includes('ECONNRESET') ||
        error.message.includes('ETIMEDOUT')
      );
    }
    return false;
  }

  private calculateBackoff(attempt: number): number {
    // Exponential backoff with jitter
    const baseDelay = 1000 * Math.pow(2, attempt);
    const jitter = Math.random() * 500;
    return Math.min(baseDelay + jitter, 30000);
  }

  private sleep(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }
}

// ============================================================================
// Factory Function
// ============================================================================

/**
 * Create a RuVector client from environment variables.
 */
export function createRuVectorClient(config?: Partial<RuVectorClientConfig>): RuVectorClient {
  const envConfig: RuVectorClientConfig = {
    baseUrl: process.env.RUVECTOR_SERVICE_URL || 'https://ruvector.googleapis.com',
    apiKey: process.env.RUVECTOR_API_KEY || '',
    ...config,
  };

  if (!envConfig.apiKey) {
    throw new Error('RUVECTOR_API_KEY environment variable is required');
  }

  return new RuVectorClient(envConfig);
}

/**
 * Singleton instance for convenience.
 */
let _defaultClient: RuVectorClient | null = null;

export function getDefaultRuVectorClient(): RuVectorClient {
  if (!_defaultClient) {
    _defaultClient = createRuVectorClient();
  }
  return _defaultClient;
}
