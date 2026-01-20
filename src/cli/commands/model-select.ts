/**
 * CLI Commands for Model Selection Agent
 *
 * Provides CLI-invokable endpoints for the Model Selection Agent:
 * - analyze   - Analyze model landscape for a task type
 * - recommend - Get model selection recommendations (primary)
 * - simulate  - Simulate model selection with different parameters
 *
 * @module cli/commands/model-select
 */

import {
  ModelSelectionAgent,
  createModelSelectionAgent,
  AGENT_ID,
  AGENT_VERSION,
} from '../../agents/model-selection-agent';

import {
  buildInputFromCLI,
  ModelSelectCLIParams,
} from '../../agents/model-selection-agent/handler';

import {
  ModelSelectionAgentOutput,
  ModelRecommendation,
} from '../../contracts';

// ============================================================================
// Types
// ============================================================================

export interface ModelSelectCommandOptions extends ModelSelectCLIParams {
  /** Output format */
  format?: 'json' | 'yaml' | 'table' | 'summary';
  /** Quiet mode - minimal output */
  quiet?: boolean;
  /** Show raw API response */
  raw?: boolean;
}

export interface CommandResult {
  success: boolean;
  output?: ModelSelectionAgentOutput;
  error?: string;
  exitCode: number;
}

// ============================================================================
// Command Implementations
// ============================================================================

/**
 * Analyze command - Analyze model landscape without generating recommendations.
 *
 * @example
 * agentics-cli model-select analyze --task-type code_generation --complexity complex
 */
export async function analyzeCommand(options: ModelSelectCommandOptions): Promise<CommandResult> {
  try {
    const agent = createModelSelectionAgent();
    const input = buildInputFromCLI(options);

    if (!options.quiet) {
      console.log(`[${AGENT_ID}] Analyzing model landscape...`);
      console.log(`  Task type: ${input.task_context.task_type}`);
      console.log(`  Complexity: ${input.task_context.complexity}`);
      console.log(`  Time window: ${input.time_window.start} to ${input.time_window.end}`);
    }

    const output = await agent.analyze(input);

    if (options.raw) {
      console.log(JSON.stringify(output, null, 2));
    } else {
      formatOutput(output, options.format || 'summary');
    }

    return { success: true, output, exitCode: 0 };
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Unknown error';
    console.error(`[${AGENT_ID}] Error: ${message}`);
    return { success: false, error: message, exitCode: 1 };
  }
}

/**
 * Recommend command - Generate model selection recommendations.
 * This is the primary entry point for the agent.
 *
 * @example
 * agentics-cli model-select recommend --task-type code_generation --complexity complex --max-latency 2000
 */
export async function recommendCommand(options: ModelSelectCommandOptions): Promise<CommandResult> {
  try {
    const agent = createModelSelectionAgent();
    const input = buildInputFromCLI(options);

    if (!options.quiet) {
      console.log(`[${AGENT_ID}] Generating model recommendations...`);
      console.log(`  Task type: ${input.task_context.task_type}`);
      console.log(`  Complexity: ${input.task_context.complexity}`);
      console.log(`  Input tokens: ~${input.task_context.expected_input_tokens.avg}`);
      console.log(`  Output tokens: ~${input.task_context.expected_output_tokens.avg}`);

      if (input.constraints.length > 0) {
        console.log(`  Constraints: ${input.constraints.map(c => `${c.type}=${JSON.stringify(c.value)}`).join(', ')}`);
      }
    }

    const output = await agent.recommend(input);

    if (options.raw) {
      console.log(JSON.stringify(output, null, 2));
    } else {
      formatOutput(output, options.format || 'summary');
    }

    return { success: true, output, exitCode: 0 };
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Unknown error';
    console.error(`[${AGENT_ID}] Error: ${message}`);
    return { success: false, error: message, exitCode: 1 };
  }
}

/**
 * Simulate command - Simulate model selection without persisting.
 *
 * @example
 * agentics-cli model-select simulate --task-type reasoning --complexity expert --providers anthropic
 */
export async function simulateCommand(options: ModelSelectCommandOptions): Promise<CommandResult> {
  try {
    const agent = createModelSelectionAgent();
    const input = buildInputFromCLI(options);

    if (!options.quiet) {
      console.log(`[${AGENT_ID}] Running model selection simulation...`);
      console.log(`  Mode: simulation (no persistence)`);
      console.log(`  Task type: ${input.task_context.task_type}`);
    }

    const output = await agent.simulate(input);

    if (options.raw) {
      console.log(JSON.stringify(output, null, 2));
    } else {
      formatOutput(output, options.format || 'summary');
    }

    return { success: true, output, exitCode: 0 };
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Unknown error';
    console.error(`[${AGENT_ID}] Error: ${message}`);
    return { success: false, error: message, exitCode: 1 };
  }
}

// ============================================================================
// Output Formatting
// ============================================================================

function formatOutput(output: ModelSelectionAgentOutput, format: string): void {
  switch (format) {
    case 'json':
      console.log(JSON.stringify(output, null, 2));
      break;

    case 'yaml':
      console.log(toYaml(output));
      break;

    case 'table':
      formatAsTable(output);
      break;

    case 'summary':
    default:
      formatAsSummary(output);
      break;
  }
}

function formatAsSummary(output: ModelSelectionAgentOutput): void {
  console.log('\n' + '='.repeat(70));
  console.log('MODEL SELECTION AGENT - RECOMMENDATION REPORT');
  console.log('='.repeat(70));

  // Header
  console.log(`\nExecution: ${output.execution_ref}`);
  console.log(`Timestamp: ${output.timestamp}`);
  console.log(`Agent Version: ${output.agent_version}`);

  // Analysis Summary
  console.log('\n--- ANALYSIS SUMMARY ---');
  const summary = output.analysis_summary;
  console.log(`  Models Evaluated:           ${summary.models_evaluated}`);
  console.log(`  Models Passing Constraints: ${summary.models_passing_constraints}`);
  console.log(`  Task Compatibility:         ${summary.task_compatibility.toUpperCase()}`);
  console.log(`  Confidence Level:           ${summary.confidence_level.toUpperCase()}`);

  if (summary.key_factors.length > 0) {
    console.log(`\n  Key Factors:`);
    for (const factor of summary.key_factors) {
      console.log(`    - ${factor}`);
    }
  }

  // Primary Recommendation
  console.log('\n--- PRIMARY RECOMMENDATION ---');
  const primary = output.primary_recommendation;

  if (primary.model_id === 'none') {
    console.log('  No suitable model found. Consider relaxing constraints.');
  } else {
    console.log(`  Model:       ${primary.model_id} (${primary.provider})`);
    console.log(`  Suitability: ${formatScore(primary.suitability_score)}`);
    console.log(`  Confidence:  ${formatScore(primary.confidence)}`);
    console.log(`\n  Reasoning: ${primary.reasoning}`);

    console.log('\n  Score Breakdown:');
    const scores = primary.score_breakdown;
    console.log(`    Quality:     ${formatScore(scores.quality_score)} (weight: ${(scores.weights.quality * 100).toFixed(0)}%)`);
    console.log(`    Latency:     ${formatScore(scores.latency_score)} (weight: ${(scores.weights.latency * 100).toFixed(0)}%)`);
    console.log(`    Cost:        ${formatScore(scores.cost_score)} (weight: ${(scores.weights.cost * 100).toFixed(0)}%)`);
    console.log(`    Task Fit:    ${formatScore(scores.task_fit_score)} (weight: ${(scores.weights.task_fit * 100).toFixed(0)}%)`);
    console.log(`    Reliability: ${formatScore(scores.reliability_score)} (weight: ${(scores.weights.reliability * 100).toFixed(0)}%)`);

    console.log('\n  Expected Performance:');
    const perf = primary.expected_performance;
    console.log(`    Latency:  ${perf.latency_ms.min}-${perf.latency_ms.max}ms (avg: ${perf.latency_ms.avg}ms)`);
    console.log(`    Quality:  ${(perf.quality_score * 100).toFixed(0)}%`);
    console.log(`    Cost:     $${perf.cost_per_request_usd.toFixed(4)}/request`);
    console.log(`    Success:  ${(perf.success_rate * 100).toFixed(1)}%`);

    if (primary.suggested_config) {
      console.log('\n  Suggested Configuration:');
      const config = primary.suggested_config;
      if (config.temperature !== undefined) {
        console.log(`    Temperature: ${config.temperature}`);
      }
      if (config.max_tokens !== undefined) {
        console.log(`    Max Tokens: ${config.max_tokens}`);
      }
      if (config.system_prompt_notes) {
        console.log(`    Note: ${config.system_prompt_notes}`);
      }
    }
  }

  // Alternative Recommendations
  if (output.alternative_recommendations.length > 0) {
    console.log(`\n--- ALTERNATIVE RECOMMENDATIONS (${output.alternative_recommendations.length}) ---`);
    for (const alt of output.alternative_recommendations) {
      console.log(`\n  #${alt.rank} ${alt.model_id} (${alt.provider})`);
      console.log(`     Suitability: ${formatScore(alt.suitability_score)}`);
      console.log(`     Cost: $${alt.expected_performance.cost_per_request_usd.toFixed(4)}/req`);
      console.log(`     Latency: ~${alt.expected_performance.latency_ms.avg}ms`);
    }
  }

  // Constraints Evaluated
  if (output.constraints_evaluated.length > 0) {
    console.log('\n--- CONSTRAINTS EVALUATED ---');
    for (const ce of output.constraints_evaluated) {
      const status = ce.satisfied ? '✓' : '✗';
      const hardness = ce.constraint.hard ? 'hard' : 'soft';
      console.log(`  ${status} ${ce.constraint.type}: ${JSON.stringify(ce.constraint.value)} (${hardness})`);
      if (ce.models_blocked > 0) {
        console.log(`     Blocked ${ce.models_blocked} models`);
      }
    }
  }

  // Warnings
  if (summary.warnings.length > 0) {
    console.log('\n--- WARNINGS ---');
    for (const warning of summary.warnings) {
      console.log(`  ⚠ ${warning}`);
    }
  }

  // Metadata
  console.log('\n--- METADATA ---');
  console.log(`  Processing Time:     ${output.metadata.processing_duration_ms}ms`);
  console.log(`  Data Points Analyzed: ${output.metadata.data_points_analyzed}`);
  console.log(`  Models Considered:    ${output.metadata.models_considered.join(', ')}`);

  console.log('\n' + '='.repeat(70) + '\n');
}

function formatAsTable(output: ModelSelectionAgentOutput): void {
  const allRecs = [output.primary_recommendation, ...output.alternative_recommendations];

  console.log('\n┌─────┬──────────────────────────┬────────────┬────────────┬────────────────┬────────────┐');
  console.log('│ Rank│ Model                    │ Suitability│ Confidence │ Cost/Request   │ Provider   │');
  console.log('├─────┼──────────────────────────┼────────────┼────────────┼────────────────┼────────────┤');

  for (const rec of allRecs) {
    if (rec.model_id === 'none') continue;

    const model = rec.model_id.substring(0, 24).padEnd(24);
    const suitability = formatScoreCompact(rec.suitability_score).padStart(10);
    const confidence = formatScoreCompact(rec.confidence).padStart(10);
    const cost = `$${rec.expected_performance.cost_per_request_usd.toFixed(4)}`.padStart(14);
    const provider = rec.provider.padEnd(10);

    console.log(`│ ${String(rec.rank).padStart(3)} │ ${model} │ ${suitability} │ ${confidence} │ ${cost} │ ${provider} │`);
  }

  console.log('└─────┴──────────────────────────┴────────────┴────────────┴────────────────┴────────────┘\n');
}

function formatScore(score: number): string {
  const percentage = (score * 100).toFixed(0);
  const bar = '█'.repeat(Math.round(score * 10)) + '░'.repeat(10 - Math.round(score * 10));
  return `${bar} ${percentage}%`;
}

function formatScoreCompact(score: number): string {
  return `${(score * 100).toFixed(0)}%`;
}

function toYaml(obj: unknown, indent: number = 0): string {
  const spaces = '  '.repeat(indent);

  if (obj === null) return 'null';
  if (obj === undefined) return '';

  if (Array.isArray(obj)) {
    if (obj.length === 0) return '[]';
    return obj.map(item => `${spaces}- ${toYaml(item, indent + 1).trim()}`).join('\n');
  }

  if (typeof obj === 'object') {
    const entries = Object.entries(obj);
    if (entries.length === 0) return '{}';
    return entries
      .map(([key, value]) => {
        if (typeof value === 'object' && value !== null) {
          return `${spaces}${key}:\n${toYaml(value, indent + 1)}`;
        }
        return `${spaces}${key}: ${toYaml(value, indent)}`;
      })
      .join('\n');
  }

  if (typeof obj === 'string') {
    if (obj.includes('\n') || obj.includes(':')) {
      return `"${obj.replace(/"/g, '\\"')}"`;
    }
    return obj;
  }

  return String(obj);
}

// ============================================================================
// CLI Registration
// ============================================================================

/**
 * Register model-select commands with the CLI framework.
 */
export function registerModelSelectCommands(program: {
  command: (name: string) => {
    description: (desc: string) => {
      option: (flag: string, desc: string, defaultValue?: unknown) => unknown;
      action: (fn: (options: ModelSelectCommandOptions) => Promise<void>) => void;
    };
  };
}): void {
  const modelSelect = program.command('model-select')
    .description('Recommend optimal model selection based on task context and historical performance');

  // Analyze subcommand
  const analyze = modelSelect.command('analyze')
    .description('Analyze model landscape for a task type')
    .option('-t, --task-type <type>', 'Task type (code_generation, reasoning, etc.)', 'general')
    .option('-c, --complexity <level>', 'Complexity (simple, moderate, complex, expert)', 'moderate')
    .option('--input-tokens <count>', 'Expected input tokens', parseInt)
    .option('--output-tokens <count>', 'Expected output tokens', parseInt)
    .option('-w, --window <duration>', 'Time window (e.g., 24h, 7d)', '24h')
    .option('--format <format>', 'Output format (json, yaml, table, summary)', 'summary')
    .option('-q, --quiet', 'Minimal output')
    .option('--raw', 'Show raw API response')
    .action(async (options: ModelSelectCommandOptions) => {
      const result = await analyzeCommand(options);
      process.exit(result.exitCode);
    });

  // Recommend subcommand
  const recommend = modelSelect.command('recommend')
    .description('Get model selection recommendations')
    .option('-t, --task-type <type>', 'Task type (code_generation, reasoning, etc.)', 'general')
    .option('-c, --complexity <level>', 'Complexity (simple, moderate, complex, expert)', 'moderate')
    .option('--input-tokens <count>', 'Expected input tokens', parseInt)
    .option('--output-tokens <count>', 'Expected output tokens', parseInt)
    .option('--max-latency <ms>', 'Maximum acceptable latency (ms)', parseInt)
    .option('--min-quality <score>', 'Minimum quality threshold (0-1)', parseFloat)
    .option('--max-cost <usd>', 'Maximum cost per request (USD)', parseFloat)
    .option('--cost-priority <level>', 'Cost optimization priority (high, medium, low)', 'medium')
    .option('--providers <list>', 'Allowed providers (comma-separated)')
    .option('--models <list>', 'Allowed models (comma-separated)')
    .option('-w, --window <duration>', 'Time window (e.g., 24h, 7d)', '24h')
    .option('--format <format>', 'Output format (json, yaml, table, summary)', 'summary')
    .option('-q, --quiet', 'Minimal output')
    .option('--raw', 'Show raw API response')
    .action(async (options: ModelSelectCommandOptions) => {
      const result = await recommendCommand(options);
      process.exit(result.exitCode);
    });

  // Simulate subcommand
  const simulate = modelSelect.command('simulate')
    .description('Simulate model selection with different parameters')
    .option('-t, --task-type <type>', 'Task type (code_generation, reasoning, etc.)', 'general')
    .option('-c, --complexity <level>', 'Complexity (simple, moderate, complex, expert)', 'moderate')
    .option('--input-tokens <count>', 'Expected input tokens', parseInt)
    .option('--output-tokens <count>', 'Expected output tokens', parseInt)
    .option('--max-latency <ms>', 'Maximum acceptable latency (ms)', parseInt)
    .option('--min-quality <score>', 'Minimum quality threshold (0-1)', parseFloat)
    .option('--max-cost <usd>', 'Maximum cost per request (USD)', parseFloat)
    .option('--providers <list>', 'Allowed providers (comma-separated)')
    .option('--models <list>', 'Allowed models (comma-separated)')
    .option('-w, --window <duration>', 'Time window (e.g., 24h, 7d)', '24h')
    .option('--format <format>', 'Output format (json, yaml, table, summary)', 'summary')
    .option('-q, --quiet', 'Minimal output')
    .option('--raw', 'Show raw API response')
    .action(async (options: ModelSelectCommandOptions) => {
      const result = await simulateCommand(options);
      process.exit(result.exitCode);
    });
}
