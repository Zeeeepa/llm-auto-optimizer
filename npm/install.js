#!/usr/bin/env node

const { existsSync } = require('fs');
const { join } = require('path');

const PLATFORMS = {
  'darwin-x64': '@llm-dev-ops/cli-darwin-x64',
  'darwin-arm64': '@llm-dev-ops/cli-darwin-arm64',
  'linux-x64': '@llm-dev-ops/cli-linux-x64',
  'linux-arm64': '@llm-dev-ops/cli-linux-arm64',
  'win32-x64': '@llm-dev-ops/cli-win32-x64',
};

function getPlatform() {
  const platform = process.platform;
  const arch = process.arch;

  // Normalize arch
  let normalizedArch = arch;
  if (arch === 'x64') {
    normalizedArch = 'x64';
  } else if (arch === 'arm64' || arch === 'aarch64') {
    normalizedArch = 'arm64';
  } else {
    throw new Error(`Unsupported architecture: ${arch}`);
  }

  const key = `${platform}-${normalizedArch}`;
  return PLATFORMS[key];
}

function verifyInstallation() {
  const platformPackage = getPlatform();

  if (!platformPackage) {
    console.error(`
ERROR: llm-optimizer does not support this platform.

Platform: ${process.platform}
Architecture: ${process.arch}

Supported platforms:
- macOS (x64, arm64)
- Linux (x64, arm64)
- Windows (x64)

Please open an issue if you need support for this platform:
https://github.com/globalbusinessadvisors/llm-auto-optimizer/issues
`);
    process.exit(1);
  }

  try {
    // Check if the platform-specific package is installed
    const packagePath = join(
      __dirname,
      'node_modules',
      platformPackage.split('/')[0],
      platformPackage.split('/')[1]
    );

    if (!existsSync(packagePath)) {
      console.error(`
ERROR: Platform-specific package not installed.

Expected package: ${platformPackage}

This usually means the installation failed. Try:
  npm install --force

Or install the platform package manually:
  npm install ${platformPackage}
`);
      process.exit(1);
    }

    console.log('✓ llm-optimizer installed successfully');
    console.log(`  Platform: ${platformPackage}`);
    console.log('\nGet started:');
    console.log('  llm-optimizer init');
    console.log('  llm-optimizer --help');
  } catch (error) {
    console.warn('Warning: Could not verify installation:', error.message);
  }
}

verifyInstallation();
