const { execFileSync, spawnSync } = require('child_process');
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
    throw new Error(`Unsupported architecture: ${arch}`);
  }

  const key = `${platform}-${normalizedArch}`;
  const packageName = PLATFORMS[key];

  if (!packageName) {
    throw new Error(`Unsupported platform: ${platform}-${arch}`);
  }

  // Try to find the binary in node_modules
  const [scope, name] = packageName.split('/');
  const binaryName = platform === 'win32' ? 'llm-optimizer.exe' : 'llm-optimizer';

  // Check relative to this file
  const paths = [
    // Installed as dependency
    join(__dirname, 'node_modules', scope, name, 'bin', binaryName),
    // Monorepo/development setup
    join(__dirname, '..', scope, name, 'bin', binaryName),
  ];

  for (const path of paths) {
    if (existsSync(path)) {
      return path;
    }
  }

  throw new Error(`Could not find llm-optimizer binary for ${packageName}`);
}

/**
 * Execute llm-optimizer CLI command synchronously
 * @param {string[]} args - Command arguments
 * @param {object} options - Execution options
 * @returns {object} - Result object with stdout, stderr, and status
 */
function execSync(args, options = {}) {
  const binaryPath = getBinaryPath();

  try {
    const stdout = execFileSync(binaryPath, args, {
      encoding: 'utf8',
      ...options,
    });

    return {
      success: true,
      stdout,
      stderr: '',
      status: 0,
    };
  } catch (error) {
    return {
      success: false,
      stdout: error.stdout || '',
      stderr: error.stderr || error.message,
      status: error.status || 1,
    };
  }
}

/**
 * Spawn llm-optimizer CLI command asynchronously
 * @param {string[]} args - Command arguments
 * @param {object} options - Spawn options
 * @returns {ChildProcess} - Child process object
 */
function spawn(args, options = {}) {
  const binaryPath = getBinaryPath();
  return spawnSync(binaryPath, args, options);
}

module.exports = {
  getBinaryPath,
  execSync,
  spawn,
};
