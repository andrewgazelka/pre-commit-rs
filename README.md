# pre-commit-rs

A fast, parallel pre-commit hook runner written in Rust, compatible with the Python pre-commit framework configuration format.

## Features

- **Blazingly fast**: Written in Rust for maximum performance
- **Parallel execution**: Run hooks in parallel while respecting dependencies
- **DAG-based scheduling**: Automatically builds and executes dependency graphs
- **Python pre-commit compatible**: Uses the same `.pre-commit-config.yaml` format
- **Atomic crates**: Highly modular and testable architecture
- **CI-friendly**: JSON and human-readable output formats
- **Comprehensive testing**: Extensive test coverage for all components

## Architecture

The project is organized as a Cargo workspace with multiple atomic crates:

- **pre-commit-core**: Shared types, traits, and error handling
- **pre-commit-parser**: YAML configuration parsing and validation
- **pre-commit-dag**: Dependency graph construction and topological sorting
- **pre-commit-executor-sync**: Sequential hook execution
- **pre-commit-executor-parallel**: Parallel hook execution with dependency resolution
- **pre-commit-cli**: User-friendly command-line interface
- **pre-commit-ci**: CI-optimized binary with JSON output

## Installation

### From Release

Download the latest release for your platform from the [releases page](https://github.com/andrewgazelka/pre-commit-rs/releases):

- `pre-commit-rs-aarch64-apple-darwin` (Apple Silicon macOS)
- `pre-commit-rs-x86_64-unknown-linux-gnu` (Linux x86_64)

```bash
# macOS (Apple Silicon)
curl -L https://github.com/andrewgazelka/pre-commit-rs/releases/latest/download/pre-commit-rs-aarch64-apple-darwin -o pre-commit-rs
chmod +x pre-commit-rs
sudo mv pre-commit-rs /usr/local/bin/

# Linux
curl -L https://github.com/andrewgazelka/pre-commit-rs/releases/latest/download/pre-commit-rs-x86_64-unknown-linux-gnu -o pre-commit-rs
chmod +x pre-commit-rs
sudo mv pre-commit-rs /usr/local/bin/
```

### From Source

```bash
cargo install --path crates/cli
```

## Usage

### Command Line Interface

```bash
# Run hooks on staged files (default)
pre-commit-rs run

# Run hooks in parallel mode
pre-commit-rs run --parallel

# Run hooks on specific files
pre-commit-rs run file1.rs file2.rs

# Install as git pre-commit hook
pre-commit-rs install

# Uninstall git pre-commit hook
pre-commit-rs uninstall
```

### CI Usage

The `pre-commit-ci` binary is optimized for CI environments:

```bash
# Run with JSON output
pre-commit-ci --format json

# Run with human-readable output
pre-commit-ci --format human

# Run in parallel mode
pre-commit-ci --parallel --format json
```

## Configuration

Uses the same `.pre-commit-config.yaml` format as Python's pre-commit:

```yaml
repos:
  - repo: local
    hooks:
      - id: cargo-fmt
        name: Rust formatting
        entry: cargo +nightly fmt
        language: system
        files: \.rs$
        pass_filenames: false

      - id: cargo-check
        name: Rust linting
        entry: cargo check
        language: system
        files: \.rs$
        pass_filenames: false
        depends_on:
          - cargo-fmt

      - id: cargo-test
        name: Rust tests
        entry: cargo test
        language: system
        files: \.rs$
        pass_filenames: false
        depends_on:
          - cargo-check
```

### Dependency Management

The `depends_on` field allows you to specify hook dependencies. pre-commit-rs will:

1. Build a dependency graph (DAG)
2. Detect cycles and fail fast
3. Execute hooks in topological order
4. Run independent hooks in parallel (when `--parallel` is used)

## Example Workflow

### Local Development

```bash
# Install the pre-commit hook
pre-commit-rs install

# Now hooks run automatically on git commit
git commit -m "Your changes"
```

### CI Pipeline

```yaml
name: Pre-commit Checks

on: [push, pull_request]

jobs:
  pre-commit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Download pre-commit-ci
        run: |
          curl -L https://github.com/andrewgazelka/pre-commit-rs/releases/latest/download/pre-commit-ci-x86_64-unknown-linux-gnu -o pre-commit-ci
          chmod +x pre-commit-ci
      - name: Run pre-commit hooks
        run: ./pre-commit-ci --parallel --format json
```

## Performance

Thanks to Rust's performance and parallel execution, pre-commit-rs significantly outperforms the Python implementation:

- **Parallel execution**: Hooks without dependencies run concurrently
- **Zero-overhead abstractions**: Rust's compile-time optimizations
- **Efficient DAG scheduling**: Smart execution planning

## Development

### Building

```bash
cargo build --workspace
```

### Testing

```bash
# Run all tests
cargo test --workspace

# Run specific crate tests
cargo test -p pre-commit-dag
```

### Running Examples

```bash
# Build and run the CLI
cargo run --package pre-commit-cli -- run --parallel

# Build and run the CI binary
cargo run --package pre-commit-ci -- --format json
```

## Design Principles

1. **Functional composition**: Pure functions where possible
2. **Atomic crates**: Each crate has a single, well-defined responsibility
3. **Comprehensive testing**: Every component is thoroughly tested
4. **Type safety**: Leverage Rust's type system for correctness
5. **Clear separation**: Planning (DAG) is separate from execution

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
