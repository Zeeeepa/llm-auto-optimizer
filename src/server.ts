/**
 * LLM-Auto-Optimizer Unified Service
 *
 * Single entry point for all optimization agents deployed as Google Cloud Run.
 * This service is STATELESS - all persistence goes through ruvector-service.
 *
 * AGENTICS INSTRUMENTATION:
 * This service is a Foundational Execution Unit within the Agentics execution
 * system. All agent routes MUST carry an ExecutionContext (execution_id +
 * parent_span_id) and produce hierarchical execution spans.
 *
 * Endpoints:
 * - /health                                    - Service health check
 * - /ready                                     - Readiness probe
 * - /agents                                    - List available agents
 * - /agents/self-optimizing-agent/*            - Self-Optimizing Agent
 * - /agents/token-optimization-agent/*         - Token Optimization Agent
 * - /agents/model-selection-agent/*            - Model Selection Agent
 *
 * @module server
 */

import { createServer, IncomingMessage, ServerResponse } from 'http';
import { URL } from 'url';

// Agent handlers
import { SelfOptimizingAgentHandler } from './agents/self-optimizing-agent/handler';
import { TokenOptimizationAgentHandler } from './agents/token-optimization-agent/handler';
import { ModelSelectionAgentHandler } from './agents/model-selection-agent/handler';
import { listAgents } from './agents';
import { createRuVectorClient } from './services/ruvector-client';

// Agentics execution instrumentation
import {
  extractExecutionContext,
  ExecutionContextError,
  SpanManager,
  type RepoExecutionResult,
} from './agentics';

// ============================================================================
// Configuration
// ============================================================================

const PORT = parseInt(process.env.PORT || '8080', 10);
const SERVICE_NAME = process.env.SERVICE_NAME || 'llm-auto-optimizer';
const SERVICE_VERSION = process.env.SERVICE_VERSION || '1.0.0';
const PLATFORM_ENV = process.env.PLATFORM_ENV || 'production';

// ============================================================================
// Initialization
// ============================================================================

// Initialize agent handlers (lazy, singleton pattern)
let selfOptimizingHandler: SelfOptimizingAgentHandler | null = null;
let tokenOptimizationHandler: TokenOptimizationAgentHandler | null = null;
let modelSelectionHandler: ModelSelectionAgentHandler | null = null;

function getSelfOptimizingHandler(): SelfOptimizingAgentHandler {
  if (!selfOptimizingHandler) {
    selfOptimizingHandler = new SelfOptimizingAgentHandler();
  }
  return selfOptimizingHandler;
}

function getTokenOptimizationHandler(): TokenOptimizationAgentHandler {
  if (!tokenOptimizationHandler) {
    tokenOptimizationHandler = new TokenOptimizationAgentHandler();
  }
  return tokenOptimizationHandler;
}

function getModelSelectionHandler(): ModelSelectionAgentHandler {
  if (!modelSelectionHandler) {
    modelSelectionHandler = new ModelSelectionAgentHandler();
  }
  return modelSelectionHandler;
}

// ============================================================================
// HTTP Request Handling
// ============================================================================

interface ParsedRequest {
  method: string;
  path: string;
  headers: Record<string, string>;
  body?: unknown;
  query: Record<string, string>;
}

async function parseRequest(req: IncomingMessage): Promise<ParsedRequest> {
  const url = new URL(req.url || '/', `http://${req.headers.host || 'localhost'}`);

  const headers: Record<string, string> = {};
  for (const [key, value] of Object.entries(req.headers)) {
    if (typeof value === 'string') {
      headers[key] = value;
    }
  }

  const query: Record<string, string> = {};
  url.searchParams.forEach((value, key) => {
    query[key] = value;
  });

  let body: unknown = undefined;
  if (req.method === 'POST' || req.method === 'PUT') {
    body = await new Promise((resolve, reject) => {
      const chunks: Buffer[] = [];
      req.on('data', (chunk: Buffer) => chunks.push(chunk));
      req.on('end', () => {
        const raw = Buffer.concat(chunks).toString('utf-8');
        try {
          resolve(raw ? JSON.parse(raw) : undefined);
        } catch {
          resolve(raw);
        }
      });
      req.on('error', reject);
    });
  }

  return {
    method: req.method || 'GET',
    path: url.pathname,
    headers,
    body,
    query,
  };
}

function sendJson(res: ServerResponse, status: number, data: unknown): void {
  res.writeHead(status, {
    'Content-Type': 'application/json',
    'X-Service-Name': SERVICE_NAME,
    'X-Service-Version': SERVICE_VERSION,
  });
  res.end(JSON.stringify(data));
}

function sendError(res: ServerResponse, status: number, code: string, message: string): void {
  sendJson(res, status, {
    error: { code, message },
    service: SERVICE_NAME,
    timestamp: new Date().toISOString(),
  });
}

// ============================================================================
// Route Handlers
// ============================================================================

async function handleHealth(res: ServerResponse): Promise<void> {
  // Check ruvector-service connectivity
  let ruvectorStatus = 'unknown';
  try {
    const client = createRuVectorClient();
    const health = await client.checkHealth();
    ruvectorStatus = health.status;
  } catch {
    ruvectorStatus = 'unreachable';
  }

  const status = ruvectorStatus === 'healthy' ? 'healthy' : 'degraded';

  sendJson(res, status === 'healthy' ? 200 : 503, {
    status,
    service: SERVICE_NAME,
    version: SERVICE_VERSION,
    environment: PLATFORM_ENV,
    timestamp: new Date().toISOString(),
    checks: {
      ruvector_service: ruvectorStatus,
    },
  });
}

async function handleReady(res: ServerResponse): Promise<void> {
  // Lightweight readiness check
  sendJson(res, 200, {
    status: 'ready',
    service: SERVICE_NAME,
    version: SERVICE_VERSION,
    timestamp: new Date().toISOString(),
  });
}

async function handleListAgents(res: ServerResponse): Promise<void> {
  sendJson(res, 200, {
    service: SERVICE_NAME,
    version: SERVICE_VERSION,
    agents: listAgents(),
    timestamp: new Date().toISOString(),
  });
}

/**
 * Route to agent with full Agentics execution span instrumentation.
 *
 * ENFORCEMENT:
 * - Extracts ExecutionContext from headers/body (rejects if parent_span_id missing)
 * - Creates repo-level span
 * - Creates agent-level span for the invoked agent
 * - Attaches artifacts to agent span
 * - Returns hierarchical span structure alongside agent response
 */
async function routeToAgent(
  parsed: ParsedRequest,
  res: ServerResponse
): Promise<void> {
  const pathParts = parsed.path.split('/').filter(Boolean);

  // Expected: /agents/{agent-id}/{action}
  if (pathParts.length < 2 || pathParts[0] !== 'agents') {
    sendError(res, 404, 'NOT_FOUND', 'Invalid agent path');
    return;
  }

  const agentId = pathParts[1];
  const action = pathParts[2] || 'health';

  // ---- AGENTICS: Extract execution context ----
  // Health checks are exempt from execution context requirement
  if (action === 'health') {
    const agentRequest = {
      method: parsed.method,
      path: `/${action}`,
      headers: parsed.headers,
      body: parsed.body as string | object | undefined,
      query: parsed.query,
    };

    let response;
    switch (agentId) {
      case 'self-optimizing-agent':
        response = await getSelfOptimizingHandler().handle(agentRequest);
        break;
      case 'token-optimization-agent':
        response = await getTokenOptimizationHandler().handle(agentRequest);
        break;
      case 'model-selection-agent':
        response = await getModelSelectionHandler().handle(agentRequest);
        break;
      default:
        sendError(res, 404, 'AGENT_NOT_FOUND', `Unknown agent: ${agentId}`);
        return;
    }

    res.writeHead(response.status, {
      'Content-Type': 'application/json',
      'X-Service-Name': SERVICE_NAME,
      'X-Agent-Id': agentId,
      ...response.headers,
    });
    res.end(response.body);
    return;
  }

  // Non-health agent routes REQUIRE execution context
  let executionContext;
  try {
    executionContext = extractExecutionContext(parsed.headers, parsed.body);
  } catch (error) {
    if (error instanceof ExecutionContextError) {
      sendJson(res, 400, {
        error: {
          code: error.code,
          message: error.message,
        },
        service: SERVICE_NAME,
        timestamp: new Date().toISOString(),
      });
      return;
    }
    throw error;
  }

  // ---- AGENTICS: Create SpanManager with repo-level span ----
  const spanManager = new SpanManager(executionContext);

  // ---- AGENTICS: Start agent-level span ----
  const agentSpanId = spanManager.startAgentSpan(agentId);

  // Build agent request
  const agentRequest = {
    method: parsed.method,
    path: `/${action}`,
    headers: parsed.headers,
    body: parsed.body as string | object | undefined,
    query: parsed.query,
  };

  let response;
  try {
    switch (agentId) {
      case 'self-optimizing-agent':
        response = await getSelfOptimizingHandler().handle(agentRequest);
        break;
      case 'token-optimization-agent':
        response = await getTokenOptimizationHandler().handle(agentRequest);
        break;
      case 'model-selection-agent':
        response = await getModelSelectionHandler().handle(agentRequest);
        break;
      default:
        spanManager.failAgentSpan(agentSpanId, [`Unknown agent: ${agentId}`]);
        const failedResult = spanManager.finalize();
        sendJson(res, 404, {
          error: { code: 'AGENT_NOT_FOUND', message: `Unknown agent: ${agentId}` },
          execution: failedResult,
          service: SERVICE_NAME,
          timestamp: new Date().toISOString(),
        });
        return;
    }

    // ---- AGENTICS: Attach agent response as artifact ----
    const agentOutput = safeParseJson(response.body);

    // Attach the decision event / agent output as an artifact
    const artifactRef = String(
      agentOutput?.output_id || agentOutput?.execution_ref || `${agentId}-${action}-${Date.now()}`
    );
    spanManager.attachArtifact(agentSpanId, {
      type: 'decision_event',
      reference: artifactRef,
      metadata: {
        agent_id: agentId,
        action,
        status: response.status,
      },
    });

    // Attach evidence: response hash for verifiability
    const responseHash = simpleHash(response.body);
    spanManager.attachEvidence(agentSpanId, {
      type: 'hash',
      value: responseHash,
      description: `SHA-256 hash of ${agentId} ${action} response body`,
    });

    if (response.status >= 200 && response.status < 400) {
      spanManager.completeAgentSpan(agentSpanId);
    } else {
      const errorObj = agentOutput?.error as Record<string, unknown> | undefined;
      spanManager.failAgentSpan(agentSpanId, [
        `Agent returned HTTP ${response.status}`,
        String(errorObj?.message || 'Unknown agent error'),
      ]);
    }
  } catch (error) {
    // ---- AGENTICS: Record failure in span ----
    const errorMessage = error instanceof Error ? error.message : 'Unknown error';
    spanManager.failAgentSpan(agentSpanId, [errorMessage]);

    const failedResult = spanManager.finalize();
    sendJson(res, 500, {
      error: {
        code: 'AGENT_EXECUTION_FAILED',
        message: errorMessage,
      },
      execution: failedResult,
      service: SERVICE_NAME,
      timestamp: new Date().toISOString(),
    });
    return;
  }

  // ---- AGENTICS: Finalize and return with execution spans ----
  const executionResult = spanManager.finalize();

  // Enrich response with execution spans
  const enrichedResponse = {
    data: safeParseJson(response.body),
    execution: executionResult,
  };

  res.writeHead(response.status, {
    'Content-Type': 'application/json',
    'X-Service-Name': SERVICE_NAME,
    'X-Agent-Id': agentId,
    'X-Execution-Id': executionContext.execution_id,
    'X-Repo-Span-Id': spanManager.repoSpanId,
    'X-Agent-Span-Id': agentSpanId,
    ...response.headers,
  });
  res.end(JSON.stringify(enrichedResponse));
}

// ============================================================================
// Main Request Handler
// ============================================================================

async function handleRequest(req: IncomingMessage, res: ServerResponse): Promise<void> {
  const startTime = Date.now();

  try {
    const parsed = await parseRequest(req);

    // CORS headers for all responses
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization, X-Execution-Id, X-Parent-Span-Id');

    // Handle preflight
    if (parsed.method === 'OPTIONS') {
      res.writeHead(204);
      res.end();
      return;
    }

    // Route request
    switch (true) {
      case parsed.path === '/health':
        await handleHealth(res);
        break;
      case parsed.path === '/ready':
        await handleReady(res);
        break;
      case parsed.path === '/agents' && parsed.method === 'GET':
        await handleListAgents(res);
        break;
      case parsed.path.startsWith('/agents/'):
        await routeToAgent(parsed, res);
        break;
      default:
        sendError(res, 404, 'NOT_FOUND', `Path not found: ${parsed.path}`);
    }
  } catch (error) {
    console.error('Request error:', error);
    sendError(res, 500, 'INTERNAL_ERROR', 'Internal server error');
  } finally {
    const duration = Date.now() - startTime;
    console.log(JSON.stringify({
      level: 'info',
      message: 'request_completed',
      method: req.method,
      path: req.url,
      duration_ms: duration,
      timestamp: new Date().toISOString(),
    }));
  }
}

// ============================================================================
// Utility Functions
// ============================================================================

function safeParseJson(str: string): Record<string, unknown> | null {
  try {
    return JSON.parse(str);
  } catch {
    return null;
  }
}

function simpleHash(input: string): string {
  const crypto = require('crypto');
  return crypto.createHash('sha256').update(input).digest('hex');
}

// ============================================================================
// Server Startup
// ============================================================================

const server = createServer(handleRequest);

server.listen(PORT, () => {
  console.log(JSON.stringify({
    level: 'info',
    message: 'server_started',
    service: SERVICE_NAME,
    version: SERVICE_VERSION,
    port: PORT,
    environment: PLATFORM_ENV,
    agentics_instrumented: true,
    repo_name: 'llm-auto-optimizer',
    timestamp: new Date().toISOString(),
  }));
});

// Graceful shutdown
process.on('SIGTERM', () => {
  console.log(JSON.stringify({
    level: 'info',
    message: 'shutdown_initiated',
    timestamp: new Date().toISOString(),
  }));
  server.close(() => {
    console.log(JSON.stringify({
      level: 'info',
      message: 'shutdown_complete',
      timestamp: new Date().toISOString(),
    }));
    process.exit(0);
  });
});

export { server };
