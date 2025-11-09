# Contributing to LLM-Auto-Optimizer

Thank you for your interest in contributing to the LLM-Auto-Optimizer project! This document provides guidelines and instructions for contributing.

## Code of Conduct

This project adheres to a code of conduct. By participating, you are expected to uphold this code.

## How to Contribute

### Reporting Bugs

Before creating bug reports, please check existing issues. When creating a bug report, include:

- Clear description of the problem
- Steps to reproduce
- Expected vs actual behavior
- Environment details (OS, Rust version, etc.)
- Relevant logs or error messages

### Suggesting Enhancements

Enhancement suggestions are tracked as GitHub issues. When creating an enhancement suggestion, include:

- Clear description of the proposed feature
- Use case and motivation
- Proposed implementation approach (if applicable)
- Any alternative solutions considered

### Pull Requests

1. **Fork the repository** and create your branch from `main`
2. **Make your changes** following the code style guidelines
3. **Add tests** for any new functionality
4. **Update documentation** as needed
5. **Run the test suite** to ensure everything passes
6. **Submit your pull request**

## Development Setup

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone your fork
git clone https://github.com/YOUR_USERNAME/llm-auto-optimizer.git
cd llm-auto-optimizer

# Build the project
cargo build

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run
```

## Code Style

- Follow standard Rust formatting with `rustfmt`
- Run `cargo clippy` and address all warnings
- Write clear, descriptive commit messages
- Add documentation for public APIs
- Include unit tests for new functionality

## Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run with coverage
cargo tarpaulin
```

## Commit Message Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0.
