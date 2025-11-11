#!/usr/bin/env node

const { spawnSync } = require('child_process');
const { join } = require('path');
const { existsSync } = require('fs');

const PLATFORMS = {
  'darwin-x64': '@llm-dev-ops/cli-darwin-x64',
  'darwin-arm64': '@llm-dev-ops/cli-darwin-arm64',
  'linux-x64': '@llm-dev-ops/cli-linux-x64',
  'linux-arm64': '@llm-dev-ops/cli-linux-arm64',
  'win32-x64': '@llm-dev-ops/cli-win32-x64',
};

function getBinaryPath() {
  const platform = process.platform;
  const arch = process.arch;

  // Normalize arch
  let normalizedArch = arch;
  if (arch === 'x64') {
    normalizedArch = 'x64';
  } else if (arch === 'arm64' || arch === 'aarch64') {
    normalizedArch = 'arm64';
  } else {
    console.error(`Unsupported architecture: ${arch}`);
    process.exit(1);
  }

  const key = `${platform}-${normalizedArch}`;
  const packageName = PLATFORMS[key];

  if (!packageName) {
    console.error(`
ERROR: llm-optimizer does not support this platform.

Platform: ${platform}
Architecture: ${arch}

Supported platforms:
- macOS (x64, arm64)
- Linux (x64, arm64)
- Windows (x64)

Please open an issue if you need support for this platform:
https://github.com/globalbusinessadvisors/llm-auto-optimizer/issues
`);
    process.exit(1);
  }

  // Try to find the binary in node_modules
  const [scope, name] = packageName.split('/');
  const binaryName = platform === 'win32' ? 'llm-optimizer.exe' : 'llm-optimizer';

  // Check relative to this file
  const paths = [
    // Installed as dependency
    join(__dirname, '..', 'node_modules', scope, name, 'bin', binaryName),
    // Monorepo/development setup
    join(__dirname, '..', '..', scope, name, 'bin', binaryName),
  ];

  for (const path of paths) {
    if (existsSync(path)) {
      return path;
    }
  }

  console.error(`
ERROR: Could not find llm-optimizer binary.

Expected package: ${packageName}
Searched paths:
${paths.map(p => `  - ${p}`).join('\n')}

This usually means the installation failed. Try:
  npm install --force

Or install the platform package manually:
  npm install ${packageName}
`);
  process.exit(1);
}

function main() {
  const binaryPath = getBinaryPath();

  // Forward all arguments to the binary
  const result = spawnSync(binaryPath, process.argv.slice(2), {
    stdio: 'inherit',
    env: process.env,
  });

  // Exit with the same code as the binary
  process.exit(result.status || 0);
}

main();
