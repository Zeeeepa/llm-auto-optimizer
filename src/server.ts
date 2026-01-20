/**
 * LLM-Auto-Optimizer Unified Service
 *
 * Single entry point for all optimization agents deployed as Google Cloud Run.
 * This service is STATELESS - all persistence goes through ruvector-service.
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

  // Build agent request
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

  // Forward agent response
  res.writeHead(response.status, {
    'Content-Type': 'application/json',
    'X-Service-Name': SERVICE_NAME,
    'X-Agent-Id': agentId,
    ...response.headers,
  });
  res.end(response.body);
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
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization');

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
