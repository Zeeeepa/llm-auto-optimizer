# LLM Auto Optimizer

[![npm version](https://badge.fury.io/js/%40llm-dev-ops%2Foptimizer.svg)](https://www.npmjs.com/package/@llm-dev-ops/optimizer)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Production-ready CLI tool for intelligent cost optimization of LLM applications.

## Features

- **Intelligent Optimization**: Automatically optimize LLM costs while maintaining quality
- **Real-time Monitoring**: Track performance, costs, and quality metrics
- **Multiple Strategies**: Cost-performance scoring, Bayesian optimization, and more
- **Integration Support**: GitHub, Slack, Jira, and more
- **Interactive Mode**: User-friendly TUI for managing optimizations
- **REST & gRPC APIs**: Programmatic access to all features

## Installation

```bash
npm install -g @llm-dev-ops/optimizer
```

Or use with npx:

```bash
npx @llm-dev-ops/optimizer --help
```

## Quick Start

Initialize the CLI:

```bash
llm-optimizer init --api-url http://your-api-url
```

Create an optimization:

```bash
llm-optimizer optimize create \
  --service my-llm-service \
  --strategy cost-performance-scoring
```

View metrics:

```bash
llm-optimizer metrics get --service my-llm-service
```

## Commands

### Service Management
- `llm-optimizer service start` - Start the optimizer service
- `llm-optimizer service stop` - Stop the service
- `llm-optimizer service status` - Check service status

### Optimization
- `llm-optimizer optimize create` - Create new optimization
- `llm-optimizer optimize list` - List optimizations
- `llm-optimizer optimize get <id>` - Get optimization details
- `llm-optimizer optimize deploy <id>` - Deploy optimization
- `llm-optimizer optimize rollback <id>` - Rollback optimization

### Metrics
- `llm-optimizer metrics get` - Get metrics for a service
- `llm-optimizer metrics cost` - Get cost analytics
- `llm-optimizer metrics performance` - Get performance metrics
- `llm-optimizer metrics quality` - Get quality metrics

### Configuration
- `llm-optimizer config show` - Show current configuration
- `llm-optimizer config set <key> <value>` - Set configuration value
- `llm-optimizer config list` - List all configuration options

### Integration
- `llm-optimizer integration add` - Add new integration
- `llm-optimizer integration list` - List integrations
- `llm-optimizer integration test <name>` - Test integration

### Admin
- `llm-optimizer admin health` - Check system health
- `llm-optimizer admin users` - Manage users
- `llm-optimizer admin backup` - Backup system data

### Utilities
- `llm-optimizer init` - Initialize CLI configuration
- `llm-optimizer doctor` - Run system diagnostics
- `llm-optimizer interactive` - Start interactive mode
- `llm-optimizer completions <shell>` - Generate shell completions

## Programmatic Usage

You can also use the CLI programmatically in your Node.js applications:

```javascript
const llmOptimizer = require('@llm-dev-ops/optimizer');

// Execute command synchronously
const result = llmOptimizer.execSync(['metrics', 'get', '--service', 'my-service']);
console.log(result.stdout);

// Spawn command asynchronously
const child = llmOptimizer.spawn(['optimize', 'create', '--service', 'my-service'], {
  stdio: 'inherit'
});
```

## Environment Variables

- `LLM_OPTIMIZER_API_URL` - API base URL
- `LLM_OPTIMIZER_API_KEY` - API key for authentication

## Configuration File

The CLI looks for configuration in:
- `~/.config/llm-optimizer/config.yaml` (Linux/macOS)
- `%APPDATA%\llm-optimizer\config.yaml` (Windows)

Example configuration:

```yaml
api_url: http://localhost:8080
api_key: your-api-key
output_format: table
verbose: false
timeout: 30
```

## Supported Platforms

- macOS (x64, arm64)
- Linux (x64, arm64)
- Windows (x64)

## Documentation

For full documentation, visit: https://github.com/globalbusinessadvisors/llm-auto-optimizer

## License

Apache-2.0

## Support

- [GitHub Issues](https://github.com/globalbusinessadvisors/llm-auto-optimizer/issues)
- [Documentation](https://github.com/globalbusinessadvisors/llm-auto-optimizer/blob/main/README.md)
