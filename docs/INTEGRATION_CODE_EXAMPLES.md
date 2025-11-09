# LLM DevOps Integration Examples
## Code Templates and Implementation Patterns

**Companion Document to:** ECOSYSTEM_RESEARCH.md
**Version:** 1.0
**Date:** 2025-11-09

---

## Table of Contents

1. [OpenTelemetry Integration](#opentelemetry-integration)
2. [gRPC Service Implementation](#grpc-service-implementation)
3. [Event Publishing & Consumption](#event-publishing--consumption)
4. [Auto-Optimizer Core Examples](#auto-optimizer-core-examples)
5. [Testing Patterns](#testing-patterns)
6. [Deployment Configurations](#deployment-configurations)

---

## OpenTelemetry Integration

### Complete Auto-Instrumentation Setup

```typescript
// src/telemetry/setup.ts
import { NodeSDK } from '@opentelemetry/sdk-node';
import { getNodeAutoInstrumentations } from '@opentelemetry/auto-instrumentations-node';
import { OTLPTraceExporter } from '@opentelemetry/exporter-trace-otlp-grpc';
import { OTLPMetricExporter } from '@opentelemetry/exporter-metrics-otlp-grpc';
import { OTLPLogExporter } from '@opentelemetry/exporter-logs-otlp-grpc';
import { PeriodicExportingMetricReader } from '@opentelemetry/sdk-metrics';
import { Resource } from '@opentelemetry/resources';
import { SemanticResourceAttributes } from '@opentelemetry/semantic-conventions';

export function initializeTelemetry() {
  const resource = Resource.default().merge(
    new Resource({
      [SemanticResourceAttributes.SERVICE_NAME]: 'llm-auto-optimizer',
      [SemanticResourceAttributes.SERVICE_VERSION]: process.env.APP_VERSION || '1.0.0',
      [SemanticResourceAttributes.SERVICE_NAMESPACE]: 'llm-platform',
      [SemanticResourceAttributes.DEPLOYMENT_ENVIRONMENT]: process.env.NODE_ENV || 'development',
    })
  );

  const sdk = new NodeSDK({
    resource,
    traceExporter: new OTLPTraceExporter({
      url: process.env.OTEL_EXPORTER_OTLP_ENDPOINT || 'grpc://observatory:4317',
      headers: {
        'x-api-key': process.env.OBSERVATORY_API_KEY || '',
      },
    }),
    metricReader: new PeriodicExportingMetricReader({
      exporter: new OTLPMetricExporter({
        url: process.env.OTEL_EXPORTER_OTLP_ENDPOINT || 'grpc://observatory:4317',
        headers: {
          'x-api-key': process.env.OBSERVATORY_API_KEY || '',
        },
      }),
      exportIntervalMillis: 5000, // Export every 5 seconds
    }),
    logRecordProcessor: new BatchLogRecordProcessor(
      new OTLPLogExporter({
        url: process.env.OTEL_EXPORTER_OTLP_ENDPOINT || 'grpc://observatory:4317',
      })
    ),
    instrumentations: [
      getNodeAutoInstrumentations({
        '@opentelemetry/instrumentation-fs': { enabled: false },
      }),
    ],
  });

  sdk.start();

  process.on('SIGTERM', () => {
    sdk
      .shutdown()
      .then(() => console.log('Telemetry terminated'))
      .catch((error) => console.log('Error terminating telemetry', error))
      .finally(() => process.exit(0));
  });

  return sdk;
}
```

### Custom LLM Instrumentation

```typescript
// src/telemetry/llm-instrumentation.ts
import { trace, metrics, SpanStatusCode, context } from '@opentelemetry/api';
import { SemanticAttributes } from '@opentelemetry/semantic-conventions';

const tracer = trace.getTracer('llm-auto-optimizer', '1.0.0');
const meter = metrics.getMeter('llm-auto-optimizer', '1.0.0');

// Define custom metrics
const optimizationCounter = meter.createCounter('llm.optimization.runs', {
  description: 'Number of optimization runs',
  unit: '1',
});

const optimizationDuration = meter.createHistogram('llm.optimization.duration', {
  description: 'Duration of optimization operations',
  unit: 'ms',
});

const tokenSavingsCounter = meter.createCounter('llm.optimization.tokens_saved', {
  description: 'Total tokens saved through optimization',
  unit: 'tokens',
});

const costSavingsCounter = meter.createCounter('llm.optimization.cost_saved', {
  description: 'Total cost saved through optimization',
  unit: 'USD',
});

export interface LLMOperationAttributes {
  'gen_ai.system': string;
  'gen_ai.request.model': string;
  'gen_ai.request.temperature'?: number;
  'gen_ai.request.max_tokens'?: number;
  'llm.request_id': string;
  'llm.user_id'?: string;
  'llm.application_name'?: string;
  'optimization.enabled': boolean;
  'optimization.technique'?: string;
}

export interface OptimizationResult {
  success: boolean;
  originalTokens: number;
  optimizedTokens: number;
  tokensSaved: number;
  costSavedUSD: number;
  compressionRatio: number;
  qualityScore: number;
  technique: string;
}

export async function instrumentLLMRequest<T>(
  operationName: string,
  attributes: LLMOperationAttributes,
  operation: (span: any) => Promise<T>
): Promise<T> {
  const span = tracer.startSpan(operationName, {
    attributes: attributes as any,
  });

  const startTime = Date.now();

  return context.with(trace.setSpan(context.active(), span), async () => {
    try {
      const result = await operation(span);

      span.setStatus({ code: SpanStatusCode.OK });

      return result;
    } catch (error: any) {
      span.recordException(error);
      span.setStatus({
        code: SpanStatusCode.ERROR,
        message: error.message,
      });
      throw error;
    } finally {
      const duration = Date.now() - startTime;

      // Record duration metric
      optimizationDuration.record(duration, {
        'optimization.type': attributes['optimization.technique'] || 'unknown',
        'gen_ai.request.model': attributes['gen_ai.request.model'],
      });

      span.end();
    }
  });
}

export function recordOptimizationMetrics(
  result: OptimizationResult,
  model: string,
  applicationId: string
) {
  const baseAttributes = {
    'gen_ai.request.model': model,
    'llm.application_name': applicationId,
    'optimization.technique': result.technique,
  };

  // Record optimization run
  optimizationCounter.add(1, {
    ...baseAttributes,
    'optimization.status': result.success ? 'success' : 'failure',
  });

  if (result.success) {
    // Record tokens saved
    tokenSavingsCounter.add(result.tokensSaved, baseAttributes);

    // Record cost saved
    costSavingsCounter.add(result.costSavedUSD, baseAttributes);
  }
}

// Usage Example
export async function optimizePromptWithTelemetry(
  prompt: string,
  model: string,
  requestId: string,
  userId?: string
): Promise<OptimizationResult> {
  const attributes: LLMOperationAttributes = {
    'gen_ai.system': model.split('-')[0], // e.g., "gpt" from "gpt-4"
    'gen_ai.request.model': model,
    'llm.request_id': requestId,
    'llm.user_id': userId,
    'llm.application_name': 'auto-optimizer',
    'optimization.enabled': true,
    'optimization.technique': 'prompt_compression',
  };

  return instrumentLLMRequest(
    'llm.optimization.prompt_compression',
    attributes,
    async (span) => {
      // Perform optimization
      const originalTokens = countTokens(prompt);
      const optimizedPrompt = await compressPrompt(prompt);
      const optimizedTokens = countTokens(optimizedPrompt);

      const result: OptimizationResult = {
        success: true,
        originalTokens,
        optimizedTokens,
        tokensSaved: originalTokens - optimizedTokens,
        costSavedUSD: calculateCostSavings(originalTokens - optimizedTokens, model),
        compressionRatio: optimizedTokens / originalTokens,
        qualityScore: await evaluateQuality(prompt, optimizedPrompt),
        technique: 'llmlingua',
      };

      // Add result to span
      span.setAttributes({
        'llm.optimization.original_tokens': result.originalTokens,
        'llm.optimization.optimized_tokens': result.optimizedTokens,
        'llm.optimization.tokens_saved': result.tokensSaved,
        'llm.optimization.cost_saved_usd': result.costSavedUSD,
        'llm.optimization.compression_ratio': result.compressionRatio,
        'llm.optimization.quality_score': result.qualityScore,
      });

      // Record metrics
      recordOptimizationMetrics(result, model, 'auto-optimizer');

      return result;
    }
  );
}

// Helper functions (stubs)
function countTokens(text: string): number {
  // Use tiktoken or similar
  return Math.ceil(text.length / 4); // Rough estimate
}

async function compressPrompt(prompt: string): Promise<string> {
  // Implement LLMLingua or other compression
  return prompt; // Stub
}

function calculateCostSavings(tokensSaved: number, model: string): number {
  const costPer1kTokens = model.includes('gpt-4') ? 0.03 : 0.01;
  return (tokensSaved / 1000) * costPer1kTokens;
}

async function evaluateQuality(original: string, optimized: string): Promise<number> {
  // Implement semantic similarity check
  return 0.95; // Stub
}
```

---

## gRPC Service Implementation

### Protocol Buffer Definitions

```protobuf
// protos/auto-optimizer.proto
syntax = "proto3";

package llm.auto_optimizer.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/struct.proto";

service AutoOptimizerService {
  // Optimize a prompt
  rpc OptimizePrompt(OptimizePromptRequest) returns (OptimizePromptResponse);

  // Select optimal model
  rpc SelectModel(SelectModelRequest) returns (SelectModelResponse);

  // Get optimization strategy
  rpc GetStrategy(GetStrategyRequest) returns (GetStrategyResponse);

  // Create A/B test experiment
  rpc CreateExperiment(CreateExperimentRequest) returns (CreateExperimentResponse);

  // Stream optimization results
  rpc StreamOptimizations(StreamOptimizationsRequest) returns (stream OptimizationEvent);
}

message OptimizePromptRequest {
  string request_id = 1;
  string prompt = 2;
  string target_model = 3;
  OptimizationType optimization_type = 4;
  OptimizationConstraints constraints = 5;
  RequestContext context = 6;
}

enum OptimizationType {
  OPTIMIZATION_TYPE_UNSPECIFIED = 0;
  OPTIMIZATION_TYPE_COMPRESSION = 1;
  OPTIMIZATION_TYPE_ENHANCEMENT = 2;
  OPTIMIZATION_TYPE_RESTRUCTURING = 3;
}

message OptimizationConstraints {
  optional int32 max_tokens = 1;
  repeated string preserve_sections = 2;
  optional double min_quality_score = 3;
  optional double max_cost_increase_usd = 4;
}

message RequestContext {
  string user_id = 1;
  string team_id = 2;
  string application_id = 3;
  string environment = 4;
  map<string, string> metadata = 5;
}

message OptimizePromptResponse {
  string request_id = 1;
  bool success = 2;
  string optimized_prompt = 3;
  OptimizationMetrics metrics = 4;
  repeated string warnings = 5;
  optional string error_message = 6;
}

message OptimizationMetrics {
  int32 original_tokens = 1;
  int32 optimized_tokens = 2;
  int32 tokens_saved = 3;
  double compression_ratio = 4;
  double cost_saved_usd = 5;
  double quality_score = 6;
  string technique_used = 7;
  google.protobuf.Timestamp timestamp = 8;
}

message SelectModelRequest {
  string request_id = 1;
  ModelRequirements requirements = 2;
  ModelConstraints constraints = 3;
  ModelPreferences preferences = 4;
  RequestContext context = 5;
}

message ModelRequirements {
  string task_type = 1; // "classification", "generation", "reasoning", "coding"
  int32 min_context_window = 2;
  repeated string required_features = 3;
  optional string reasoning_level = 4;
}

message ModelConstraints {
  optional double max_cost_per_request_usd = 1;
  optional int32 max_latency_ms = 2;
  repeated string compliance_requirements = 3;
  repeated string allowed_regions = 4;
}

message ModelPreferences {
  string optimize_for = 1; // "cost", "latency", "quality"
  repeated string excluded_providers = 2;
}

message SelectModelResponse {
  string request_id = 1;
  string recommended_model = 2;
  repeated ModelAlternative alternatives = 3;
  EstimatedMetrics estimated_metrics = 4;
  string selection_rationale = 5;
}

message ModelAlternative {
  string model_id = 1;
  double match_score = 2;
  string rationale = 3;
  EstimatedMetrics estimated_metrics = 4;
}

message EstimatedMetrics {
  double cost_per_request_usd = 1;
  int32 latency_p95_ms = 2;
  double quality_score = 3;
}

message GetStrategyRequest {
  string request_id = 1;
  RequestContext context = 2;
  CurrentMetrics current_metrics = 3;
  repeated string goals = 4;
}

message CurrentMetrics {
  string current_model = 1;
  int32 average_prompt_tokens = 2;
  int32 average_completion_tokens = 3;
  int32 requests_per_day = 4;
  double current_daily_cost_usd = 5;
  optional double current_quality_score = 6;
}

message GetStrategyResponse {
  string request_id = 1;
  repeated StrategyRecommendation recommendations = 2;
}

message StrategyRecommendation {
  string recommendation_id = 1;
  string strategy_type = 2;
  string description = 3;
  double confidence = 4;
  ImpactEstimate estimated_impact = 5;
  google.protobuf.Struct implementation_details = 6;
  repeated string based_on_experiments = 7;
}

message ImpactEstimate {
  double cost_reduction_percentage = 1;
  double cost_savings_daily_usd = 2;
  double quality_impact = 3;
  int32 latency_impact_ms = 4;
}

message CreateExperimentRequest {
  string experiment_id = 1;
  string name = 2;
  string description = 3;
  repeated ExperimentVariant variants = 4;
  map<string, double> traffic_allocation = 5;
  SuccessCriteria success_criteria = 6;
  int32 duration_hours = 7;
  RequestContext context = 8;
}

message ExperimentVariant {
  string variant_id = 1;
  string description = 2;
  google.protobuf.Struct configuration = 3;
}

message SuccessCriteria {
  string primary_metric = 1;
  double min_improvement_percentage = 2;
  double min_statistical_significance = 3;
}

message CreateExperimentResponse {
  string experiment_id = 1;
  string status = 2;
  google.protobuf.Timestamp start_time = 3;
  google.protobuf.Timestamp estimated_end_time = 4;
}

message StreamOptimizationsRequest {
  RequestContext context = 1;
  repeated string event_types = 2;
}

message OptimizationEvent {
  string event_id = 1;
  string event_type = 2;
  google.protobuf.Timestamp timestamp = 3;
  google.protobuf.Struct data = 4;
}
```

### gRPC Service Implementation

```typescript
// src/grpc/auto-optimizer-service.ts
import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';
import { trace, context } from '@opentelemetry/api';
import path from 'path';

const PROTO_PATH = path.join(__dirname, '../../protos/auto-optimizer.proto');

const packageDefinition = protoLoader.loadSync(PROTO_PATH, {
  keepCase: true,
  longs: String,
  enums: String,
  defaults: true,
  oneofs: true,
});

const protoDescriptor = grpc.loadPackageDefinition(packageDefinition) as any;
const autoOptimizerProto = protoDescriptor.llm.auto_optimizer.v1;

export class AutoOptimizerServiceImpl {
  private tracer = trace.getTracer('auto-optimizer-grpc', '1.0.0');

  async optimizePrompt(
    call: grpc.ServerUnaryCall<any, any>,
    callback: grpc.sendUnaryData<any>
  ): Promise<void> {
    const span = this.tracer.startSpan('grpc.OptimizePrompt');

    try {
      const request = call.request;

      span.setAttributes({
        'rpc.service': 'AutoOptimizerService',
        'rpc.method': 'OptimizePrompt',
        'llm.request_id': request.request_id,
        'llm.target_model': request.target_model,
        'llm.optimization_type': request.optimization_type,
      });

      // Perform optimization
      const result = await this.performOptimization(request);

      const response = {
        request_id: request.request_id,
        success: result.success,
        optimized_prompt: result.optimizedPrompt,
        metrics: {
          original_tokens: result.originalTokens,
          optimized_tokens: result.optimizedTokens,
          tokens_saved: result.tokensSaved,
          compression_ratio: result.compressionRatio,
          cost_saved_usd: result.costSavedUSD,
          quality_score: result.qualityScore,
          technique_used: result.technique,
          timestamp: { seconds: Math.floor(Date.now() / 1000) },
        },
        warnings: result.warnings || [],
        error_message: result.errorMessage,
      };

      callback(null, response);
      span.setStatus({ code: 0 }); // OK
    } catch (error: any) {
      span.recordException(error);
      span.setStatus({ code: 2, message: error.message }); // ERROR
      callback({
        code: grpc.status.INTERNAL,
        message: error.message,
      });
    } finally {
      span.end();
    }
  }

  async selectModel(
    call: grpc.ServerUnaryCall<any, any>,
    callback: grpc.sendUnaryData<any>
  ): Promise<void> {
    const span = this.tracer.startSpan('grpc.SelectModel');

    try {
      const request = call.request;

      span.setAttributes({
        'rpc.service': 'AutoOptimizerService',
        'rpc.method': 'SelectModel',
        'llm.request_id': request.request_id,
        'llm.task_type': request.requirements.task_type,
        'llm.optimize_for': request.preferences.optimize_for,
      });

      // Perform model selection
      const result = await this.performModelSelection(request);

      const response = {
        request_id: request.request_id,
        recommended_model: result.recommendedModel,
        alternatives: result.alternatives,
        estimated_metrics: result.estimatedMetrics,
        selection_rationale: result.rationale,
      };

      callback(null, response);
      span.setStatus({ code: 0 });
    } catch (error: any) {
      span.recordException(error);
      span.setStatus({ code: 2, message: error.message });
      callback({
        code: grpc.status.INTERNAL,
        message: error.message,
      });
    } finally {
      span.end();
    }
  }

  streamOptimizations(call: grpc.ServerWritableStream<any, any>): void {
    const span = this.tracer.startSpan('grpc.StreamOptimizations');

    const request = call.request;

    span.setAttributes({
      'rpc.service': 'AutoOptimizerService',
      'rpc.method': 'StreamOptimizations',
      'llm.event_types': request.event_types.join(','),
    });

    // Subscribe to optimization events
    const subscription = this.subscribeToEvents(request.event_types, (event) => {
      call.write({
        event_id: event.id,
        event_type: event.type,
        timestamp: { seconds: Math.floor(event.timestamp / 1000) },
        data: event.data,
      });
    });

    call.on('cancelled', () => {
      subscription.unsubscribe();
      span.setStatus({ code: 1 }); // CANCELLED
      span.end();
    });

    call.on('error', (error) => {
      subscription.unsubscribe();
      span.recordException(error);
      span.setStatus({ code: 2, message: error.message });
      span.end();
    });
  }

  // Implementation methods
  private async performOptimization(request: any): Promise<any> {
    // Implementation here
    return {
      success: true,
      optimizedPrompt: request.prompt,
      originalTokens: 500,
      optimizedTokens: 250,
      tokensSaved: 250,
      compressionRatio: 0.5,
      costSavedUSD: 0.0075,
      qualityScore: 0.95,
      technique: 'llmlingua',
      warnings: [],
    };
  }

  private async performModelSelection(request: any): Promise<any> {
    // Implementation here
    return {
      recommendedModel: 'gpt-3.5-turbo',
      alternatives: [],
      estimatedMetrics: {
        cost_per_request_usd: 0.015,
        latency_p95_ms: 1800,
        quality_score: 0.88,
      },
      rationale: 'Cost-optimized selection based on requirements',
    };
  }

  private subscribeToEvents(eventTypes: string[], callback: (event: any) => void): any {
    // Implementation here
    return {
      unsubscribe: () => {},
    };
  }
}

// Server setup
export function createGrpcServer(): grpc.Server {
  const server = new grpc.Server();
  const serviceImpl = new AutoOptimizerServiceImpl();

  server.addService(autoOptimizerProto.AutoOptimizerService.service, {
    optimizePrompt: serviceImpl.optimizePrompt.bind(serviceImpl),
    selectModel: serviceImpl.selectModel.bind(serviceImpl),
    getStrategy: async (call: any, callback: any) => {
      callback(null, { request_id: call.request.request_id, recommendations: [] });
    },
    createExperiment: async (call: any, callback: any) => {
      callback(null, {
        experiment_id: call.request.experiment_id,
        status: 'CREATED',
        start_time: { seconds: Math.floor(Date.now() / 1000) },
      });
    },
    streamOptimizations: serviceImpl.streamOptimizations.bind(serviceImpl),
  });

  return server;
}

export function startGrpcServer(port: number = 50051): grpc.Server {
  const server = createGrpcServer();

  server.bindAsync(
    `0.0.0.0:${port}`,
    grpc.ServerCredentials.createInsecure(),
    (err, boundPort) => {
      if (err) {
        console.error('Failed to bind gRPC server:', err);
        throw err;
      }

      console.log(`gRPC server listening on port ${boundPort}`);
      server.start();
    }
  );

  return server;
}
```

---

## Event Publishing & Consumption

### CloudEvents Publisher

```typescript
// src/events/cloudevents-publisher.ts
import { Kafka, Producer, ProducerRecord } from 'kafkajs';
import { v4 as uuidv4 } from 'uuid';

export interface CloudEvent {
  specversion: '1.0';
  type: string;
  source: string;
  id: string;
  time: string;
  datacontenttype?: string;
  dataschema?: string;
  subject?: string;
  data: any;
}

export class CloudEventsPublisher {
  private kafka: Kafka;
  private producer: Producer;
  private connected: boolean = false;

  constructor(brokers: string[]) {
    this.kafka = new Kafka({
      clientId: 'llm-auto-optimizer',
      brokers,
    });

    this.producer = this.kafka.producer();
  }

  async connect(): Promise<void> {
    if (!this.connected) {
      await this.producer.connect();
      this.connected = true;
      console.log('CloudEvents producer connected');
    }
  }

  async disconnect(): Promise<void> {
    if (this.connected) {
      await this.producer.disconnect();
      this.connected = false;
      console.log('CloudEvents producer disconnected');
    }
  }

  async publishEvent(
    topic: string,
    eventType: string,
    source: string,
    data: any,
    options?: {
      subject?: string;
      datacontenttype?: string;
      dataschema?: string;
    }
  ): Promise<void> {
    const event: CloudEvent = {
      specversion: '1.0',
      type: eventType,
      source,
      id: uuidv4(),
      time: new Date().toISOString(),
      datacontenttype: options?.datacontenttype || 'application/json',
      dataschema: options?.dataschema,
      subject: options?.subject,
      data,
    };

    const record: ProducerRecord = {
      topic,
      messages: [
        {
          key: event.id,
          value: JSON.stringify(event),
          headers: {
            'ce-specversion': event.specversion,
            'ce-type': event.type,
            'ce-source': event.source,
            'ce-id': event.id,
            'ce-time': event.time,
            'content-type': event.datacontenttype || 'application/json',
          },
        },
      ],
    };

    await this.producer.send(record);
    console.log(`Published event ${event.id} to topic ${topic}`);
  }

  // Convenience methods for specific event types
  async publishOptimizationApplied(data: {
    optimization_id: string;
    request_id: string;
    optimization_type: string;
    technique: string;
    original_metrics: any;
    optimized_metrics: any;
    improvement: any;
    quality_metrics: any;
    metadata: any;
  }): Promise<void> {
    await this.publishEvent(
      'llm.optimizations',
      'io.llm-platform.optimization.applied',
      '/auto-optimizer',
      data,
      {
        subject: `request/${data.request_id}`,
      }
    );
  }

  async publishExperimentCompleted(data: {
    experiment_id: string;
    experiment_name: string;
    experiment_type: string;
    status: string;
    duration_hours: number;
    variants: any[];
    winner: any;
    conclusion: string;
    recommendation: string;
    next_actions: any[];
  }): Promise<void> {
    await this.publishEvent(
      'llm.optimizations',
      'io.llm-platform.experiment.completed',
      '/auto-optimizer/experiments',
      data,
      {
        subject: `experiment/${data.experiment_id}`,
      }
    );
  }

  async publishOptimizationFailed(data: {
    optimization_id: string;
    request_id: string;
    optimization_type: string;
    error_message: string;
    error_details: any;
    metadata: any;
  }): Promise<void> {
    await this.publishEvent(
      'llm.optimizations',
      'io.llm-platform.optimization.failed',
      '/auto-optimizer',
      data,
      {
        subject: `request/${data.request_id}`,
      }
    );
  }
}

// Usage example
export async function publishOptimizationEvent() {
  const publisher = new CloudEventsPublisher(['kafka-broker:9092']);
  await publisher.connect();

  try {
    await publisher.publishOptimizationApplied({
      optimization_id: 'opt-12345',
      request_id: 'req-abc123',
      optimization_type: 'prompt_compression',
      technique: 'llmlingua',
      original_metrics: {
        prompt_tokens: 850,
        estimated_cost_usd: 0.0255,
      },
      optimized_metrics: {
        prompt_tokens: 442,
        estimated_cost_usd: 0.0132,
      },
      improvement: {
        tokens_saved: 408,
        cost_saved_usd: 0.0123,
        reduction_percentage: 48.0,
      },
      quality_metrics: {
        compression_quality_score: 0.94,
        semantic_similarity: 0.96,
      },
      metadata: {
        model: 'gpt-4',
        application_id: 'chatbot-v2',
        user_id: 'user-456',
      },
    });
  } finally {
    await publisher.disconnect();
  }
}
```

### CloudEvents Consumer

```typescript
// src/events/cloudevents-consumer.ts
import { Kafka, Consumer, EachMessagePayload } from 'kafkajs';

export type CloudEventHandler = (event: CloudEvent) => Promise<void>;

export class CloudEventsConsumer {
  private kafka: Kafka;
  private consumer: Consumer;
  private handlers: Map<string, CloudEventHandler[]> = new Map();
  private connected: boolean = false;

  constructor(brokers: string[], groupId: string) {
    this.kafka = new Kafka({
      clientId: 'llm-auto-optimizer-consumer',
      brokers,
    });

    this.consumer = this.kafka.consumer({ groupId });
  }

  async connect(): Promise<void> {
    if (!this.connected) {
      await this.consumer.connect();
      this.connected = true;
      console.log('CloudEvents consumer connected');
    }
  }

  async disconnect(): Promise<void> {
    if (this.connected) {
      await this.consumer.disconnect();
      this.connected = false;
      console.log('CloudEvents consumer disconnected');
    }
  }

  async subscribe(topics: string[]): Promise<void> {
    for (const topic of topics) {
      await this.consumer.subscribe({ topic, fromBeginning: false });
      console.log(`Subscribed to topic: ${topic}`);
    }
  }

  registerHandler(eventType: string, handler: CloudEventHandler): void {
    if (!this.handlers.has(eventType)) {
      this.handlers.set(eventType, []);
    }
    this.handlers.get(eventType)!.push(handler);
    console.log(`Registered handler for event type: ${eventType}`);
  }

  async start(): Promise<void> {
    await this.consumer.run({
      eachMessage: async (payload: EachMessagePayload) => {
        await this.processMessage(payload);
      },
    });
  }

  private async processMessage(payload: EachMessagePayload): Promise<void> {
    const { topic, partition, message } = payload;

    try {
      const event: CloudEvent = JSON.parse(message.value!.toString());

      console.log(
        `Received event: ${event.type} (id: ${event.id}) from topic ${topic}, partition ${partition}`
      );

      const handlers = this.handlers.get(event.type) || [];

      for (const handler of handlers) {
        try {
          await handler(event);
        } catch (error) {
          console.error(`Error in handler for event ${event.id}:`, error);
          // Continue processing other handlers
        }
      }
    } catch (error) {
      console.error('Error processing message:', error);
    }
  }
}

// Usage example
export async function startEventConsumer() {
  const consumer = new CloudEventsConsumer(
    ['kafka-broker:9092'],
    'auto-optimizer-consumer-group'
  );

  await consumer.connect();
  await consumer.subscribe(['llm.metrics', 'llm.anomalies', 'llm.governance.alerts']);

  // Register handlers
  consumer.registerHandler('io.llm-platform.sentinel.alert', async (event) => {
    console.log('Received sentinel alert:', event.data);
    // Trigger optimization based on anomaly
    await handleAnomalyAlert(event.data);
  });

  consumer.registerHandler('io.llm-platform.governance.budget-alert', async (event) => {
    console.log('Received budget alert:', event.data);
    // Trigger cost optimization
    await handleBudgetAlert(event.data);
  });

  consumer.registerHandler(
    'io.llm-platform.observatory.threshold-exceeded',
    async (event) => {
      console.log('Received metric threshold alert:', event.data);
      // Analyze metrics and potentially optimize
      await handleMetricThreshold(event.data);
    }
  );

  await consumer.start();
}

async function handleAnomalyAlert(data: any): Promise<void> {
  // Implementation
  console.log('Handling anomaly alert...');
}

async function handleBudgetAlert(data: any): Promise<void> {
  // Implementation
  console.log('Handling budget alert...');
}

async function handleMetricThreshold(data: any): Promise<void> {
  // Implementation
  console.log('Handling metric threshold...');
}
```

---

## Auto-Optimizer Core Examples

### Prompt Compression Engine

```typescript
// src/optimization/prompt-compressor.ts
import { instrumentLLMRequest, recordOptimizationMetrics } from '../telemetry/llm-instrumentation';

export interface CompressionOptions {
  compressionRatio: number;
  preserveQuestions: boolean;
  preserveSections?: string[];
  minPromptLength?: number;
}

export interface CompressionResult {
  success: boolean;
  originalPrompt: string;
  compressedPrompt: string;
  originalTokens: number;
  compressedTokens: number;
  compressionRatio: number;
  tokensSaved: number;
  qualityScore: number;
  technique: string;
}

export class PromptCompressor {
  async compress(
    prompt: string,
    model: string,
    options: CompressionOptions,
    requestId: string
  ): Promise<CompressionResult> {
    return instrumentLLMRequest(
      'llm.optimization.prompt_compression',
      {
        'gen_ai.system': model.split('-')[0],
        'gen_ai.request.model': model,
        'llm.request_id': requestId,
        'llm.application_name': 'auto-optimizer',
        'optimization.enabled': true,
        'optimization.technique': 'llmlingua',
      },
      async (span) => {
        const originalTokens = this.countTokens(prompt);

        // Skip compression if prompt is too short
        if (options.minPromptLength && originalTokens < options.minPromptLength) {
          return {
            success: false,
            originalPrompt: prompt,
            compressedPrompt: prompt,
            originalTokens,
            compressedTokens: originalTokens,
            compressionRatio: 1.0,
            tokensSaved: 0,
            qualityScore: 1.0,
            technique: 'skipped',
          };
        }

        // Perform compression using LLMLingua or similar
        const compressed = await this.performCompression(prompt, options);

        const compressedTokens = this.countTokens(compressed);
        const tokensSaved = originalTokens - compressedTokens;
        const actualRatio = compressedTokens / originalTokens;

        // Evaluate quality
        const qualityScore = await this.evaluateQuality(prompt, compressed);

        const result: CompressionResult = {
          success: true,
          originalPrompt: prompt,
          compressedPrompt: compressed,
          originalTokens,
          compressedTokens,
          compressionRatio: actualRatio,
          tokensSaved,
          qualityScore,
          technique: 'llmlingua',
        };

        // Record metrics
        recordOptimizationMetrics(
          {
            success: true,
            originalTokens,
            optimizedTokens: compressedTokens,
            tokensSaved,
            costSavedUSD: this.calculateCostSavings(tokensSaved, model),
            compressionRatio: actualRatio,
            qualityScore,
            technique: 'llmlingua',
          },
          model,
          'auto-optimizer'
        );

        return result;
      }
    );
  }

  private async performCompression(
    prompt: string,
    options: CompressionOptions
  ): Promise<string> {
    // Implement LLMLingua compression
    // This is a simplified version

    let compressed = prompt;

    // Preserve questions if requested
    if (options.preserveQuestions) {
      const questions = this.extractQuestions(prompt);
      const nonQuestions = this.removeQuestions(prompt);
      const compressedNonQuestions = this.applyCompression(
        nonQuestions,
        options.compressionRatio
      );
      compressed = this.mergeQuestionsAndText(questions, compressedNonQuestions);
    } else {
      compressed = this.applyCompression(prompt, options.compressionRatio);
    }

    return compressed;
  }

  private applyCompression(text: string, ratio: number): string {
    // Simplified compression logic
    // In production, use LLMLingua or similar
    const words = text.split(/\s+/);
    const targetLength = Math.floor(words.length * ratio);
    return words.slice(0, targetLength).join(' ');
  }

  private extractQuestions(text: string): string[] {
    return text.match(/[^.!?]*\?[^.!?]*/g) || [];
  }

  private removeQuestions(text: string): string {
    return text.replace(/[^.!?]*\?[^.!?]*/g, '');
  }

  private mergeQuestionsAndText(questions: string[], text: string): string {
    return [...questions, text].join(' ').trim();
  }

  private countTokens(text: string): number {
    // Use tiktoken in production
    return Math.ceil(text.length / 4);
  }

  private async evaluateQuality(original: string, compressed: string): Promise<number> {
    // Implement semantic similarity check
    // Use embeddings to compare similarity
    return 0.95; // Stub
  }

  private calculateCostSavings(tokensSaved: number, model: string): number {
    const costPer1kTokens = model.includes('gpt-4') ? 0.03 : 0.01;
    return (tokensSaved / 1000) * costPer1kTokens;
  }
}
```

### Model Selection Engine

```typescript
// src/optimization/model-selector.ts
import { Registry } from '../registry/registry-client';

export interface ModelSelectionCriteria {
  taskType: string;
  minContextWindow: number;
  requiredFeatures: string[];
  reasoningLevel?: string;
}

export interface ModelSelectionConstraints {
  maxCostPerRequestUSD?: number;
  maxLatencyMs?: number;
  complianceRequirements?: string[];
  allowedRegions?: string[];
}

export interface ModelSelectionPreferences {
  optimizeFor: 'cost' | 'latency' | 'quality';
  excludedProviders?: string[];
}

export interface ModelRecommendation {
  modelId: string;
  matchScore: number;
  estimatedCostUSD: number;
  estimatedLatencyMs: number;
  estimatedQualityScore: number;
  rationale: string;
}

export class ModelSelector {
  private registry: Registry;

  constructor(registryUrl: string) {
    this.registry = new Registry(registryUrl);
  }

  async selectOptimalModel(
    criteria: ModelSelectionCriteria,
    constraints: ModelSelectionConstraints,
    preferences: ModelSelectionPreferences
  ): Promise<ModelRecommendation> {
    // Search registry for matching models
    const candidates = await this.registry.searchModels({
      required_capabilities: {
        min_context_window: criteria.minContextWindow,
        features: criteria.requiredFeatures,
        reasoning_capability: criteria.reasoningLevel,
      },
      constraints: {
        max_cost_per_request_usd: constraints.maxCostPerRequestUSD,
        max_latency_p95_ms: constraints.maxLatencyMs,
        required_compliance: constraints.complianceRequirements,
        allowed_regions: constraints.allowedRegions,
      },
      preferences: {
        optimize_for: preferences.optimizeFor,
        exclude_providers: preferences.excludedProviders,
      },
    });

    if (candidates.matches.length === 0) {
      throw new Error('No models match the specified criteria');
    }

    // Score and rank candidates
    const scored = candidates.matches.map((candidate: any) => {
      const score = this.calculateScore(candidate, preferences);
      return {
        modelId: candidate.model_id,
        matchScore: score,
        estimatedCostUSD: candidate.estimated_cost_per_request,
        estimatedLatencyMs: candidate.estimated_latency_p95_ms,
        estimatedQualityScore: candidate.match_score,
        rationale: this.generateRationale(candidate, preferences),
      };
    });

    // Sort by score (descending)
    scored.sort((a, b) => b.matchScore - a.matchScore);

    return scored[0];
  }

  private calculateScore(candidate: any, preferences: ModelSelectionPreferences): number {
    let score = candidate.match_score;

    // Adjust score based on optimization preference
    if (preferences.optimizeFor === 'cost') {
      // Lower cost = higher score
      const costFactor = 1 / (1 + candidate.estimated_cost_per_request);
      score *= 1 + costFactor;
    } else if (preferences.optimizeFor === 'latency') {
      // Lower latency = higher score
      const latencyFactor = 1 / (1 + candidate.estimated_latency_p95_ms / 1000);
      score *= 1 + latencyFactor;
    } else if (preferences.optimizeFor === 'quality') {
      // Quality is already in match_score
      score *= 1.5;
    }

    return score;
  }

  private generateRationale(candidate: any, preferences: ModelSelectionPreferences): string {
    const reasons: string[] = [];

    reasons.push(`Matches required capabilities`);

    if (preferences.optimizeFor === 'cost') {
      reasons.push(
        `Low cost: $${candidate.estimated_cost_per_request.toFixed(4)} per request`
      );
    } else if (preferences.optimizeFor === 'latency') {
      reasons.push(`Low latency: ${candidate.estimated_latency_p95_ms}ms p95`);
    } else {
      reasons.push(`High quality score: ${candidate.match_score.toFixed(2)}`);
    }

    return reasons.join('; ');
  }
}
```

---

## Testing Patterns

### Unit Testing with OpenTelemetry

```typescript
// src/__tests__/telemetry.test.ts
import { trace, metrics } from '@opentelemetry/api';
import { NodeSDK } from '@opentelemetry/sdk-node';
import { InMemorySpanExporter } from '@opentelemetry/sdk-trace-base';
import { InMemoryMetricExporter } from '@opentelemetry/sdk-metrics';

describe('Telemetry Integration', () => {
  let sdk: NodeSDK;
  let spanExporter: InMemorySpanExporter;
  let metricExporter: InMemoryMetricExporter;

  beforeAll(() => {
    spanExporter = new InMemorySpanExporter();
    metricExporter = new InMemoryMetricExporter();

    sdk = new NodeSDK({
      traceExporter: spanExporter,
      metricReader: new PeriodicExportingMetricReader({
        exporter: metricExporter,
        exportIntervalMillis: 100,
      }),
    });

    sdk.start();
  });

  afterAll(async () => {
    await sdk.shutdown();
  });

  afterEach(() => {
    spanExporter.reset();
    metricExporter.reset();
  });

  it('should create spans for optimization operations', async () => {
    const tracer = trace.getTracer('test-tracer');
    const span = tracer.startSpan('test.optimization');

    span.setAttributes({
      'llm.optimization.type': 'prompt_compression',
      'llm.optimization.tokens_saved': 250,
    });

    span.end();

    const spans = spanExporter.getFinishedSpans();
    expect(spans).toHaveLength(1);
    expect(spans[0].name).toBe('test.optimization');
    expect(spans[0].attributes['llm.optimization.type']).toBe('prompt_compression');
    expect(spans[0].attributes['llm.optimization.tokens_saved']).toBe(250);
  });

  it('should record optimization metrics', async () => {
    const meter = metrics.getMeter('test-meter');
    const counter = meter.createCounter('test.optimization.runs');

    counter.add(1, { 'optimization.type': 'prompt_compression', 'optimization.status': 'success' });

    // Wait for export
    await new Promise((resolve) => setTimeout(resolve, 200));

    const { resourceMetrics } = await metricExporter.collect();
    expect(resourceMetrics.scopeMetrics.length).toBeGreaterThan(0);
  });
});
```

### Integration Testing with gRPC

```typescript
// src/__tests__/grpc-integration.test.ts
import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';
import { createGrpcServer } from '../grpc/auto-optimizer-service';
import path from 'path';

describe('gRPC Service Integration', () => {
  let server: grpc.Server;
  let client: any;

  beforeAll((done) => {
    server = createGrpcServer();
    server.bindAsync('localhost:50052', grpc.ServerCredentials.createInsecure(), () => {
      server.start();

      const PROTO_PATH = path.join(__dirname, '../../protos/auto-optimizer.proto');
      const packageDefinition = protoLoader.loadSync(PROTO_PATH, {});
      const proto = grpc.loadPackageDefinition(packageDefinition) as any;

      client = new proto.llm.auto_optimizer.v1.AutoOptimizerService(
        'localhost:50052',
        grpc.credentials.createInsecure()
      );

      done();
    });
  });

  afterAll((done) => {
    client.close();
    server.tryShutdown(done);
  });

  it('should optimize a prompt', (done) => {
    const request = {
      request_id: 'test-123',
      prompt: 'This is a test prompt that should be compressed',
      target_model: 'gpt-4',
      optimization_type: 'OPTIMIZATION_TYPE_COMPRESSION',
      constraints: {
        min_quality_score: 0.9,
      },
      context: {
        application_id: 'test-app',
      },
    };

    client.optimizePrompt(request, (error: any, response: any) => {
      expect(error).toBeNull();
      expect(response).toBeDefined();
      expect(response.request_id).toBe('test-123');
      expect(response.success).toBe(true);
      expect(response.metrics).toBeDefined();
      expect(response.metrics.tokens_saved).toBeGreaterThan(0);
      done();
    });
  });

  it('should select optimal model', (done) => {
    const request = {
      request_id: 'test-456',
      requirements: {
        task_type: 'classification',
        min_context_window: 4000,
        required_features: ['function_calling'],
      },
      constraints: {
        max_cost_per_request_usd: 0.05,
        max_latency_ms: 2000,
      },
      preferences: {
        optimize_for: 'cost',
      },
      context: {
        application_id: 'test-app',
      },
    };

    client.selectModel(request, (error: any, response: any) => {
      expect(error).toBeNull();
      expect(response).toBeDefined();
      expect(response.request_id).toBe('test-456');
      expect(response.recommended_model).toBeDefined();
      expect(response.estimated_metrics).toBeDefined();
      done();
    });
  });
});
```

---

## Deployment Configurations

### Kubernetes Deployment

```yaml
# k8s/auto-optimizer-deployment.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: auto-optimizer-config
  namespace: llm-platform
data:
  OTEL_EXPORTER_OTLP_ENDPOINT: "grpc://observatory:4317"
  KAFKA_BROKERS: "kafka-broker-0:9092,kafka-broker-1:9092,kafka-broker-2:9092"
  REGISTRY_URL: "https://registry.llm-platform.internal"
  GOVERNANCE_URL: "https://governance.llm-platform.internal"
  NODE_ENV: "production"
---
apiVersion: v1
kind: Secret
metadata:
  name: auto-optimizer-secrets
  namespace: llm-platform
type: Opaque
stringData:
  OBSERVATORY_API_KEY: "your-api-key-here"
  KAFKA_USERNAME: "auto-optimizer"
  KAFKA_PASSWORD: "your-password-here"
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: auto-optimizer
  namespace: llm-platform
  labels:
    app: auto-optimizer
    version: v1
spec:
  replicas: 3
  selector:
    matchLabels:
      app: auto-optimizer
  template:
    metadata:
      labels:
        app: auto-optimizer
        version: v1
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9090"
        prometheus.io/path: "/metrics"
    spec:
      serviceAccountName: auto-optimizer
      containers:
      - name: auto-optimizer
        image: llm-platform/auto-optimizer:1.0.0
        imagePullPolicy: IfNotPresent
        ports:
        - name: grpc
          containerPort: 50051
          protocol: TCP
        - name: http
          containerPort: 8080
          protocol: TCP
        - name: metrics
          containerPort: 9090
          protocol: TCP
        env:
        - name: OTEL_EXPORTER_OTLP_ENDPOINT
          valueFrom:
            configMapKeyRef:
              name: auto-optimizer-config
              key: OTEL_EXPORTER_OTLP_ENDPOINT
        - name: KAFKA_BROKERS
          valueFrom:
            configMapKeyRef:
              name: auto-optimizer-config
              key: KAFKA_BROKERS
        - name: REGISTRY_URL
          valueFrom:
            configMapKeyRef:
              name: auto-optimizer-config
              key: REGISTRY_URL
        - name: GOVERNANCE_URL
          valueFrom:
            configMapKeyRef:
              name: auto-optimizer-config
              key: GOVERNANCE_URL
        - name: NODE_ENV
          valueFrom:
            configMapKeyRef:
              name: auto-optimizer-config
              key: NODE_ENV
        - name: OBSERVATORY_API_KEY
          valueFrom:
            secretKeyRef:
              name: auto-optimizer-secrets
              key: OBSERVATORY_API_KEY
        - name: KAFKA_USERNAME
          valueFrom:
            secretKeyRef:
              name: auto-optimizer-secrets
              key: KAFKA_USERNAME
        - name: KAFKA_PASSWORD
          valueFrom:
            secretKeyRef:
              name: auto-optimizer-secrets
              key: KAFKA_PASSWORD
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "2000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: auto-optimizer
  namespace: llm-platform
  labels:
    app: auto-optimizer
spec:
  type: ClusterIP
  selector:
    app: auto-optimizer
  ports:
  - name: grpc
    port: 50051
    targetPort: 50051
    protocol: TCP
  - name: http
    port: 8080
    targetPort: 8080
    protocol: TCP
  - name: metrics
    port: 9090
    targetPort: 9090
    protocol: TCP
---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: auto-optimizer
  namespace: llm-platform
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: auto-optimizer
  namespace: llm-platform
rules:
- apiGroups: [""]
  resources: ["configmaps", "secrets"]
  verbs: ["get", "list", "watch"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: auto-optimizer
  namespace: llm-platform
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: Role
  name: auto-optimizer
subjects:
- kind: ServiceAccount
  name: auto-optimizer
  namespace: llm-platform
```

### Docker Compose for Local Development

```yaml
# docker-compose.yml
version: '3.8'

services:
  auto-optimizer:
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "50051:50051"  # gRPC
      - "8080:8080"    # HTTP
      - "9090:9090"    # Metrics
    environment:
      - NODE_ENV=development
      - OTEL_EXPORTER_OTLP_ENDPOINT=grpc://observatory:4317
      - KAFKA_BROKERS=kafka:9092
      - REGISTRY_URL=http://registry:8080
      - GOVERNANCE_URL=http://governance:8080
    depends_on:
      - kafka
      - observatory
      - registry
      - governance
    volumes:
      - ./src:/app/src
      - /app/node_modules

  observatory:
    image: otel/opentelemetry-collector:latest
    ports:
      - "4317:4317"  # OTLP gRPC
      - "4318:4318"  # OTLP HTTP
      - "8888:8888"  # Prometheus metrics
    volumes:
      - ./config/otel-collector-config.yaml:/etc/otel-collector-config.yaml
    command: ["--config=/etc/otel-collector-config.yaml"]

  kafka:
    image: confluentinc/cp-kafka:latest
    ports:
      - "9092:9092"
    environment:
      KAFKA_BROKER_ID: 1
      KAFKA_ZOOKEEPER_CONNECT: zookeeper:2181
      KAFKA_ADVERTISED_LISTENERS: PLAINTEXT://kafka:9092
      KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR: 1
    depends_on:
      - zookeeper

  zookeeper:
    image: confluentinc/cp-zookeeper:latest
    environment:
      ZOOKEEPER_CLIENT_PORT: 2181
      ZOOKEEPER_TICK_TIME: 2000

  registry:
    image: llm-platform/registry:latest
    ports:
      - "8081:8080"
    environment:
      - DATABASE_URL=postgresql://postgres:password@postgres:5432/registry

  governance:
    image: llm-platform/governance:latest
    ports:
      - "8082:8080"
    environment:
      - DATABASE_URL=postgresql://postgres:password@postgres:5432/governance

  postgres:
    image: postgres:15
    ports:
      - "5432:5432"
    environment:
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: password
      POSTGRES_DB: llm_platform
    volumes:
      - postgres-data:/var/lib/postgresql/data

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9091:9090"
    volumes:
      - ./config/prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana-data:/var/lib/grafana

volumes:
  postgres-data:
  prometheus-data:
  grafana-data:
```

---

**Document End**

This companion document provides concrete implementation examples that can be used as templates for building the LLM-Auto-Optimizer integration with the ecosystem components.
