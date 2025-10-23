# Contributing to pre-commit-rs

Thank you for your interest in contributing to pre-commit-rs! This document provides guidelines and instructions for contributing.

## Development Setup

1. **Install Rust**: Make sure you have Rust installed (1.70 or later)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Clone the repository**:
   ```bash
   git clone https://github.com/andrewgazelka/pre-commit-rs.git
   cd pre-commit-rs
   ```

3. **Build the project**:
   ```bash
   cargo build --workspace
   ```

4. **Run tests**:
   ```bash
   cargo test --workspace
   ```

## Project Structure

```
pre-commit-rs/
├── crates/
│   ├── core/              # Shared types and traits
│   ├── parser/            # YAML config parsing
│   ├── dag/              # Dependency graph construction
│   ├── executor-sync/    # Sequential execution
│   ├── executor-parallel/ # Parallel execution
│   ├── cli/              # User-facing CLI
│   └── ci/               # CI-optimized binary
├── .github/
│   └── workflows/        # CI/CD pipelines
└── README.md
```

## Design Principles

1. **Functional Programming**: Prefer pure functions and immutable data structures
2. **Atomic Crates**: Each crate should have a single, well-defined responsibility
3. **Comprehensive Testing**: All new features must include tests
4. **Type Safety**: Leverage Rust's type system for correctness
5. **Performance**: Keep performance in mind, but prioritize correctness first

## Making Changes

### Before You Start

- Check existing issues to see if someone is already working on it
- For large changes, open an issue first to discuss the approach
- Make sure all tests pass before submitting a PR

### Development Workflow

1. **Create a branch**:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**:
   - Write tests for new functionality
   - Update documentation as needed
   - Follow Rust naming conventions
   - Run `cargo fmt` to format your code
   - Run `cargo clippy` to catch common mistakes

3. **Test your changes**:
   ```bash
   # Run all tests
   cargo test --workspace

   # Run specific crate tests
   cargo test -p pre-commit-dag

   # Run with coverage (optional)
   cargo tarpaulin --workspace
   ```

4. **Commit your changes**:
   ```bash
   git add .
   git commit -m "Add feature: your feature description"
   ```

5. **Push and create a PR**:
   ```bash
   git push origin feature/your-feature-name
   ```

## Code Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Write clear, descriptive commit messages
- Add comments for complex logic
- Document public APIs with doc comments

## Testing Guidelines

### Unit Tests

Each crate should have comprehensive unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        // Arrange
        let input = setup_input();

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected);
    }
}
```

### Integration Tests

For cross-crate functionality, add integration tests in the `tests/` directory.

## Pull Request Process

1. Ensure all tests pass
2. Update documentation if needed
3. Add a clear description of your changes
4. Reference any related issues
5. Wait for review and address feedback

## Reporting Bugs

When reporting bugs, please include:

- A clear description of the issue
- Steps to reproduce
- Expected behavior
- Actual behavior
- Your environment (OS, Rust version)
- Relevant logs or error messages

## Feature Requests

Feature requests are welcome! Please:

- Check if the feature has already been requested
- Provide a clear use case
- Explain why it would be useful
- Consider submitting a PR if you can implement it

## Questions?

- Open an issue for general questions
- Tag issues with `question` label
- Check existing issues and documentation first

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
