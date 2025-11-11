# NPM Publishing Guide

This guide explains how to publish the LLM Auto Optimizer CLI to npm under the `@llm-dev-ops` organization.

## Package Structure

We use a multi-package approach similar to esbuild:

- **Main package**: `@llm-dev-ops/optimizer` - Users install this
- **Platform packages**: Platform-specific binaries
  - `@llm-dev-ops/cli-darwin-x64` - macOS x64
  - `@llm-dev-ops/cli-darwin-arm64` - macOS ARM64
  - `@llm-dev-ops/cli-linux-x64` - Linux x64
  - `@llm-dev-ops/cli-linux-arm64` - Linux ARM64
  - `@llm-dev-ops/cli-win32-x64` - Windows x64

## Prerequisites

1. **Fix CLI compilation errors**:
   The CLI currently has compilation errors that need to be fixed first:
   ```bash
   cd /workspaces/llm-auto-optimizer
   cargo build --release -p llm-optimizer-cli
   ```

2. **npm authentication**:
   ```bash
   npm login
   # Or set NPM_TOKEN environment variable
   export NPM_TOKEN=your-token-here
   ```

3. **Access to @llm-dev-ops organization**:
   Ensure you have publishing rights to the @llm-dev-ops npm organization.

## Building Binaries

### Option 1: Build on each platform

For best compatibility, build on native hardware:

**On macOS (x64)**:
```bash
cargo build --release -p llm-optimizer-cli
cp target/release/llm-optimizer npm/packages/cli-darwin-x64/bin/
```

**On macOS (ARM64)**:
```bash
cargo build --release -p llm-optimizer-cli
cp target/release/llm-optimizer npm/packages/cli-darwin-arm64/bin/
```

**On Linux (x64)**:
```bash
cargo build --release -p llm-optimizer-cli
cp target/release/llm-optimizer npm/packages/cli-linux-x64/bin/
```

**On Linux (ARM64)**:
```bash
cargo build --release -p llm-optimizer-cli
cp target/release/llm-optimizer npm/packages/cli-linux-arm64/bin/
```

**On Windows (x64)**:
```bash
cargo build --release -p llm-optimizer-cli
copy target\release\llm-optimizer.exe npm\packages\cli-win32-x64\bin\
```

### Option 2: Use GitHub Actions (Recommended)

Create `.github/workflows/build-npm.yml`:

```yaml
name: Build NPM Packages

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            platform: linux-x64
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            platform: linux-arm64
          - os: macos-latest
            target: x86_64-apple-darwin
            platform: darwin-x64
          - os: macos-latest
            target: aarch64-apple-darwin
            platform: darwin-arm64
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            platform: win32-x64

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          toolchain: stable
          target: ${{ matrix.target }}

      - name: Build
        run: cargo build --release --target ${{ matrix.target }} -p llm-optimizer-cli

      - name: Prepare package
        run: |
          mkdir -p npm/packages/cli-${{ matrix.platform }}/bin
          if [ "${{ matrix.os }}" == "windows-latest" ]; then
            cp target/${{ matrix.target }}/release/llm-optimizer.exe npm/packages/cli-${{ matrix.platform }}/bin/
          else
            cp target/${{ matrix.target }}/release/llm-optimizer npm/packages/cli-${{ matrix.platform }}/bin/
            chmod +x npm/packages/cli-${{ matrix.platform }}/bin/llm-optimizer
          fi

      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: cli-${{ matrix.platform }}
          path: npm/packages/cli-${{ matrix.platform }}/
```

## Publishing Process

### 1. Generate Platform Packages

Run the build script to create package.json files:

```bash
cd npm
node build-platform-packages.js
```

This creates the package structure in `npm/packages/`.

### 2. Publish Platform Packages

After building all binaries, publish each platform package:

```bash
# Linux x64
cd npm/packages/cli-linux-x64
npm publish --access public

# Linux ARM64
cd ../cli-linux-arm64
npm publish --access public

# macOS x64
cd ../cli-darwin-x64
npm publish --access public

# macOS ARM64
cd ../cli-darwin-arm64
npm publish --access public

# Windows x64
cd ../cli-win32-x64
npm publish --access public
```

### 3. Publish Main Package

After all platform packages are published:

```bash
cd /workspaces/llm-auto-optimizer/npm
npm publish --access public
```

## Verification

After publishing, test the installation:

```bash
npm install -g @llm-dev-ops/optimizer
llm-optimizer --help
```

Or test without installing:

```bash
npx @llm-dev-ops/optimizer --help
```

## Version Updates

When releasing a new version:

1. Update version in:
   - `npm/package.json`
   - `npm/build-platform-packages.js` (VERSION constant)
   - `Cargo.toml` (workspace.package.version)

2. Rebuild and republish all packages

## Package Contents

### Main Package (`@llm-dev-ops/optimizer`)

```
npm/
├── package.json          # Main package manifest
├── index.js              # Programmatic API
├── install.js            # Post-install verification
├── bin/
│   └── llm-optimizer.js  # CLI wrapper script
└── README.md             # User documentation
```

### Platform Packages

```
npm/packages/cli-<platform>-<arch>/
├── package.json
├── README.md
└── bin/
    └── llm-optimizer(.exe)  # Native binary
```

## CLI Commands Available

Once installed, users get access to:

```bash
llm-optimizer init                  # Initialize configuration
llm-optimizer service start         # Start service
llm-optimizer optimize create       # Create optimization
llm-optimizer metrics get           # Get metrics
llm-optimizer config set            # Configure settings
llm-optimizer integration add       # Add integrations
llm-optimizer admin health          # System health
llm-optimizer doctor                # Diagnostics
llm-optimizer interactive           # Interactive mode
llm-optimizer completions bash      # Shell completions
```

Shorthand alias: `llmo` (same as `llm-optimizer`)

## Programmatic Usage

Users can also use the CLI programmatically:

```javascript
const llmOptimizer = require('@llm-dev-ops/optimizer');

// Execute command
const result = llmOptimizer.execSync(['metrics', 'get', '--service', 'my-service']);
console.log(result.stdout);
```

## Troubleshooting

### Binary not found

If users report "binary not found" errors:

1. Check that all platform packages are published
2. Verify the binary permissions (755 on Unix)
3. Test installation on the target platform

### Platform not supported

If a platform isn't supported, users will see:

```
ERROR: llm-optimizer does not support this platform.

Platform: freebsd
Architecture: x64
```

## Support

- GitHub Issues: https://github.com/globalbusinessadvisors/llm-auto-optimizer/issues
- Documentation: https://github.com/globalbusinessadvisors/llm-auto-optimizer
