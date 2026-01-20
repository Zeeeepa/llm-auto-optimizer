/**
 * CLI Commands for Self-Optimizing Agent
 *
 * Provides CLI-invokable endpoints for the Self-Optimizing Agent:
 * - analyze  - Perform analysis without generating recommendations
 * - recommend - Generate ranked optimization recommendations (primary)
 * - simulate - Simulate proposed changes without persisting
 *
 * @module cli/commands/optimize
 */

import {
  SelfOptimizingAgent,
  createSelfOptimizingAgent,
  AGENT_ID,
  AGENT_VERSION,
} from '../../agents/self-optimizing-agent';

import {
  buildInputFromCLI,
} from '../../agents/self-optimizing-agent/handler';

import {
  SelfOptimizingAgentOutput,
  CLIParams,
} from '../../contracts';

// ============================================================================
// Types
// ============================================================================

export interface OptimizeCommandOptions extends CLIParams {
  /** Output format */
  format?: 'json' | 'yaml' | 'table' | 'summary';
  /** Quiet mode - minimal output */
  quiet?: boolean;
  /** Show raw API response */
  raw?: boolean;
}

export interface CommandResult {
  success: boolean;
  output?: SelfOptimizingAgentOutput;
  error?: string;
  exitCode: number;
}

// ============================================================================
// Command Implementations
// ============================================================================

/**
 * Analyze command - Perform analysis without generating recommendations.
 *
 * @example
 * agentics-cli optimize analyze --window 24h --focus cost
 */
export async function analyzeCommand(options: OptimizeCommandOptions): Promise<CommandResult> {
  try {
    const agent = createSelfOptimizingAgent();
    const input = buildInputFromCLI(options);

    if (!options.quiet) {
      console.log(`[${AGENT_ID}] Starting analysis...`);
      console.log(`  Time window: ${input.time_window.start} to ${input.time_window.end}`);
      console.log(`  Target services: ${input.target_services.join(', ')}`);
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
 * Recommend command - Generate ranked optimization recommendations.
 * This is the primary entry point for the agent.
 *
 * @example
 * agentics-cli optimize recommend --window 7d --focus balanced --max-cost-increase 10
 */
export async function recommendCommand(options: OptimizeCommandOptions): Promise<CommandResult> {
  try {
    const agent = createSelfOptimizingAgent();
    const input = buildInputFromCLI(options);

    if (!options.quiet) {
      console.log(`[${AGENT_ID}] Generating optimization recommendations...`);
      console.log(`  Time window: ${input.time_window.start} to ${input.time_window.end}`);
      console.log(`  Optimization targets: cost=${input.optimization_targets.cost_weight.toFixed(2)}, ` +
                  `latency=${input.optimization_targets.latency_weight.toFixed(2)}, ` +
                  `quality=${input.optimization_targets.quality_weight.toFixed(2)}`);
      if (input.constraints.length > 0) {
        console.log(`  Constraints: ${input.constraints.map(c => `${c.type}=${c.value}`).join(', ')}`);
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
 * Simulate command - Simulate proposed changes without persisting.
 *
 * @example
 * agentics-cli optimize simulate --window 24h --dry-run
 */
export async function simulateCommand(options: OptimizeCommandOptions): Promise<CommandResult> {
  try {
    const agent = createSelfOptimizingAgent();
    const input = buildInputFromCLI(options);

    if (!options.quiet) {
      console.log(`[${AGENT_ID}] Running simulation...`);
      console.log(`  Mode: ${options.dry_run ? 'dry-run (no persistence)' : 'normal'}`);
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

function formatOutput(output: SelfOptimizingAgentOutput, format: string): void {
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

function formatAsSummary(output: SelfOptimizingAgentOutput): void {
  console.log('\n' + '='.repeat(60));
  console.log('SELF-OPTIMIZING AGENT - OPTIMIZATION REPORT');
  console.log('='.repeat(60));

  // Header
  console.log(`\nExecution: ${output.execution_ref}`);
  console.log(`Timestamp: ${output.timestamp}`);
  console.log(`Agent Version: ${output.agent_version}`);

  // Current State Assessment
  console.log('\n--- CURRENT STATE ---');
  const state = output.summary.current_state;
  console.log(`  Cost Efficiency:    ${formatScore(state.cost_efficiency)}`);
  console.log(`  Latency Efficiency: ${formatScore(state.latency_efficiency)}`);
  console.log(`  Quality Score:      ${formatScore(state.quality_score)}`);
  console.log(`  Overall Health:     ${formatScore(state.overall_health)}`);

  // Improvement Potential
  if (output.recommendations.length > 0) {
    console.log('\n--- IMPROVEMENT POTENTIAL ---');
    const potential = output.summary.total_improvement_potential;
    console.log(`  Cost Reduction:     ${potential.cost_reduction_pct.toFixed(1)}%`);
    console.log(`  Latency Improvement: ${potential.latency_improvement_pct.toFixed(1)}%`);
    console.log(`  Quality Improvement: ${potential.quality_improvement_pct.toFixed(1)}%`);
  }

  // Recommendations
  console.log(`\n--- RECOMMENDATIONS (${output.recommendations.length}) ---`);
  if (output.recommendations.length === 0) {
    console.log('  No optimization recommendations at this time.');
  } else {
    for (const rec of output.recommendations) {
      console.log(`\n  #${rec.rank} [${rec.category}] ${rec.title}`);
      console.log(`     Confidence: ${formatScore(rec.confidence)}`);
      console.log(`     Risk: ${rec.risk.toUpperCase()}`);
      console.log(`     Expected Impact:`);
      console.log(`       - Cost: ${formatDelta(rec.expected_impact.cost_change_pct)}%`);
      console.log(`       - Latency: ${formatDelta(rec.expected_impact.latency_change_pct)}%`);
      console.log(`       - Quality: ${formatDelta(rec.expected_impact.quality_change_pct, true)}%`);
      if (rec.expected_impact.monthly_savings_usd) {
        console.log(`       - Monthly Savings: $${rec.expected_impact.monthly_savings_usd.toFixed(2)}`);
      }
      console.log(`     Rollout: ${rec.rollout_strategy.type}`);
    }
  }

  // Constraints
  if (output.constraints_evaluated.length > 0) {
    console.log('\n--- CONSTRAINTS EVALUATED ---');
    for (const ec of output.constraints_evaluated) {
      const status = ec.satisfied ? '✓' : '✗';
      console.log(`  ${status} ${ec.constraint.type}: ${ec.constraint.value} (${ec.constraint.hard ? 'hard' : 'soft'})`);
    }
  }

  // Metadata
  console.log('\n--- METADATA ---');
  console.log(`  Processing Time: ${output.metadata.processing_duration_ms}ms`);
  console.log(`  Data Points Analyzed: ${output.metadata.data_points_analyzed}`);
  console.log(`  Strategies Used: ${output.metadata.strategies_used.join(', ')}`);

  console.log('\n' + '='.repeat(60) + '\n');
}

function formatAsTable(output: SelfOptimizingAgentOutput): void {
  console.log('\n┌─────┬──────────────────────────────────────────┬────────────┬──────────┐');
  console.log('│ Rank│ Recommendation                           │ Confidence │ Risk     │');
  console.log('├─────┼──────────────────────────────────────────┼────────────┼──────────┤');

  for (const rec of output.recommendations) {
    const title = rec.title.substring(0, 40).padEnd(40);
    const conf = formatScore(rec.confidence).padStart(10);
    const risk = rec.risk.toUpperCase().padEnd(8);
    console.log(`│ ${String(rec.rank).padStart(3)} │ ${title} │ ${conf} │ ${risk} │`);
  }

  console.log('└─────┴──────────────────────────────────────────┴────────────┴──────────┘\n');
}

function formatScore(score: number): string {
  const percentage = (score * 100).toFixed(0);
  const bar = '█'.repeat(Math.round(score * 10)) + '░'.repeat(10 - Math.round(score * 10));
  return `${bar} ${percentage}%`;
}

function formatDelta(delta: number, positiveIsGood: boolean = false): string {
  const prefix = delta > 0 ? '+' : '';
  const color = positiveIsGood
    ? (delta > 0 ? '' : '')  // Would use ANSI colors in real CLI
    : (delta < 0 ? '' : '');
  return `${prefix}${delta.toFixed(1)}`;
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
 * Register optimize commands with the CLI framework.
 */
export function registerOptimizeCommands(program: {
  command: (name: string) => {
    description: (desc: string) => {
      option: (flag: string, desc: string, defaultValue?: unknown) => unknown;
      action: (fn: (options: OptimizeCommandOptions) => Promise<void>) => void;
    };
  };
}): void {
  const optimize = program.command('optimize')
    .description('LLM optimization analysis and recommendations');

  // Analyze subcommand
  const analyze = optimize.command('analyze')
    .description('Analyze current LLM infrastructure state')
    .option('-w, --window <duration>', 'Time window (e.g., 24h, 7d)', '24h')
    .option('-s, --services <list>', 'Target services (comma-separated)', 'default')
    .option('-f, --focus <type>', 'Optimization focus (cost, latency, quality, balanced)', 'balanced')
    .option('--format <format>', 'Output format (json, yaml, table, summary)', 'summary')
    .option('-q, --quiet', 'Minimal output')
    .option('--raw', 'Show raw API response')
    .action(async (options: OptimizeCommandOptions) => {
      const result = await analyzeCommand(options);
      process.exit(result.exitCode);
    });

  // Recommend subcommand
  const recommend = optimize.command('recommend')
    .description('Generate optimization recommendations')
    .option('-w, --window <duration>', 'Time window (e.g., 24h, 7d)', '24h')
    .option('-s, --services <list>', 'Target services (comma-separated)', 'default')
    .option('-f, --focus <type>', 'Optimization focus (cost, latency, quality, balanced)', 'balanced')
    .option('--max-cost-increase <pct>', 'Maximum allowed cost increase (%)', parseFloat)
    .option('--min-quality <score>', 'Minimum quality threshold (0-1)', parseFloat)
    .option('--max-latency-increase <ms>', 'Maximum latency increase (ms)', parseFloat)
    .option('--format <format>', 'Output format (json, yaml, table, summary)', 'summary')
    .option('-q, --quiet', 'Minimal output')
    .option('--raw', 'Show raw API response')
    .action(async (options: OptimizeCommandOptions) => {
      const result = await recommendCommand(options);
      process.exit(result.exitCode);
    });

  // Simulate subcommand
  const simulate = optimize.command('simulate')
    .description('Simulate optimization changes')
    .option('-w, --window <duration>', 'Time window (e.g., 24h, 7d)', '24h')
    .option('-s, --services <list>', 'Target services (comma-separated)', 'default')
    .option('-f, --focus <type>', 'Optimization focus (cost, latency, quality, balanced)', 'balanced')
    .option('--dry-run', 'Do not persist results')
    .option('--format <format>', 'Output format (json, yaml, table, summary)', 'summary')
    .option('-q, --quiet', 'Minimal output')
    .option('--raw', 'Show raw API response')
    .action(async (options: OptimizeCommandOptions) => {
      const result = await simulateCommand(options);
      process.exit(result.exitCode);
    });
}
