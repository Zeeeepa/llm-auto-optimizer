#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const PLATFORMS = [
  { platform: 'darwin', arch: 'x64', rust_target: 'x86_64-apple-darwin' },
  { platform: 'darwin', arch: 'arm64', rust_target: 'aarch64-apple-darwin' },
  { platform: 'linux', arch: 'x64', rust_target: 'x86_64-unknown-linux-gnu' },
  { platform: 'linux', arch: 'arm64', rust_target: 'aarch64-unknown-linux-gnu' },
  { platform: 'win32', arch: 'x64', rust_target: 'x86_64-pc-windows-msvc' },
];

const VERSION = '0.1.2';

function createPlatformPackage(platform) {
  const packageName = `@llm-dev-ops/llm-auto-optimizer-cli-${platform.platform}-${platform.arch}`;
  const packageDir = path.join(__dirname, 'packages', `llm-auto-optimizer-cli-${platform.platform}-${platform.arch}`);

  // Create package directory
  if (!fs.existsSync(packageDir)) {
    fs.mkdirSync(packageDir, { recursive: true });
  }

  // Create bin directory
  const binDir = path.join(packageDir, 'bin');
  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
  }

  // Create package.json
  const packageJson = {
    name: packageName,
    version: VERSION,
    description: `LLM Auto Optimizer CLI for ${platform.platform}-${platform.arch}`,
    keywords: ['llm', 'optimization', 'ai', 'devops', 'monitoring'],
    homepage: 'https://github.com/globalbusinessadvisors/llm-auto-optimizer',
    repository: {
      type: 'git',
      url: 'https://github.com/globalbusinessadvisors/llm-auto-optimizer.git',
    },
    license: 'Apache-2.0',
    author: 'LLM Auto Optimizer Contributors <noreply@llmdevops.dev>',
    os: [platform.platform],
    cpu: [platform.arch],
    files: ['bin/'],
  };

  fs.writeFileSync(
    path.join(packageDir, 'package.json'),
    JSON.stringify(packageJson, null, 2) + '\n'
  );

  // Create README
  const readme = `# ${packageName}

This package contains the precompiled binary for LLM Auto Optimizer CLI on ${platform.platform}-${platform.arch}.

You should not install this package directly. Install \`@llm-dev-ops/optimizer\` instead.

\`\`\`bash
npm install -g @llm-dev-ops/optimizer
\`\`\`

## License

Apache-2.0
`;

  fs.writeFileSync(path.join(packageDir, 'README.md'), readme);

  console.log(`✓ Created platform package: ${packageName}`);
  return packageDir;
}

function buildBinary(platform, outputDir) {
  console.log(`\nBuilding binary for ${platform.rust_target}...`);

  const binaryName = platform.platform === 'win32' ? 'llm-optimizer.exe' : 'llm-optimizer';
  const binDir = path.join(outputDir, 'bin');

  try {
    // Install rust target if needed
    console.log(`  Installing Rust target ${platform.rust_target}...`);
    execSync(`rustup target add ${platform.rust_target}`, {
      stdio: 'inherit',
      cwd: path.join(__dirname, '..'),
    });

    // Build the binary
    console.log(`  Building Rust binary...`);
    execSync(
      `cargo build --release --target ${platform.rust_target} -p llm-optimizer-cli`,
      {
        stdio: 'inherit',
        cwd: path.join(__dirname, '..'),
      }
    );

    // Copy binary to package
    const sourcePath = path.join(
      __dirname,
      '..',
      'target',
      platform.rust_target,
      'release',
      binaryName
    );

    if (!fs.existsSync(sourcePath)) {
      throw new Error(`Binary not found at ${sourcePath}`);
    }

    fs.copyFileSync(sourcePath, path.join(binDir, binaryName));

    // Make binary executable on Unix
    if (platform.platform !== 'win32') {
      fs.chmodSync(path.join(binDir, binaryName), 0o755);
    }

    console.log(`✓ Built binary for ${platform.rust_target}`);
  } catch (error) {
    console.error(`✗ Failed to build binary for ${platform.rust_target}:`, error.message);
    throw error;
  }
}

function main() {
  console.log('Building platform packages...\n');

  // Get current platform for testing
  const currentPlatform = process.platform;
  const currentArch = process.arch === 'x64' ? 'x64' : 'arm64';

  for (const platform of PLATFORMS) {
    console.log(`\n=== Processing ${platform.platform}-${platform.arch} ===`);

    // Create package structure
    const packageDir = createPlatformPackage(platform);

    // Only build binary for current platform (cross-compilation can be added later)
    if (platform.platform === currentPlatform && platform.arch === currentArch) {
      console.log('  (Building for current platform)');
      buildBinary(platform, packageDir);
    } else {
      console.log('  (Skipping build - cross-compilation not configured)');
      console.log('  Note: You can manually build and copy the binary to:');
      console.log(`        ${path.join(packageDir, 'bin')}`);
    }
  }

  console.log('\n✓ All platform packages created!');
  console.log('\nNext steps:');
  console.log('1. Build binaries for all platforms (use CI/CD or cross-compilation)');
  console.log('2. Test each platform package');
  console.log('3. Publish platform packages:');
  console.log('   cd npm/packages/cli-<platform>-<arch> && npm publish --access public');
  console.log('4. Publish main package:');
  console.log('   cd npm && npm publish --access public');
}

main();
