# Architecture

This document describes the architecture and design decisions of pre-commit-rs.

## Overview

pre-commit-rs is a high-performance, parallel pre-commit hook runner written in Rust. It is designed to be a drop-in replacement for the Python pre-commit framework while offering significant performance improvements through parallel execution and native code.

## Design Principles

### 1. Functional Composition

The codebase emphasizes functional programming patterns:
- Pure functions where possible
- Immutable data structures
- Explicit error handling using `Result<T, E>`
- Separation of pure logic from I/O

### 2. Atomic Crates

Each crate has a single, well-defined responsibility:

```
core        → Type definitions and traits
parser      → Configuration parsing
dag         → Dependency graph construction
executor-*  → Hook execution strategies
cli         → User-facing interface
ci          → CI-optimized interface
```

This modular design allows:
- Independent testing of each component
- Easy extension with new executors
- Clear separation of concerns
- Reusability in other projects

### 3. Type Safety

Leverages Rust's type system for correctness:
- Compile-time guarantees about data flow
- No null pointer exceptions
- Exhaustive pattern matching
- Trait-based abstractions

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────┐
│                      User Interface                      │
│  ┌─────────────────────┐   ┌────────────────────────┐  │
│  │   CLI (main.rs)     │   │  CI Binary (main.rs)   │  │
│  │  - Human output     │   │  - JSON/Human output   │  │
│  │  - Git integration  │   │  - CI optimized        │  │
│  └─────────────────────┘   └────────────────────────┘  │
└────────────────┬────────────────────┬───────────────────┘
                 │                    │
                 ├────────────────────┘
                 │
┌────────────────▼────────────────────────────────────────┐
│                  Core Components                         │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────┐   │
│  │  Parser  │→ │   DAG    │→ │    Executors       │   │
│  │  (YAML)  │  │ Builder  │  │  ┌──────────────┐  │   │
│  └──────────┘  └──────────┘  │  │ Sync (Seq.)  │  │   │
│                               │  └──────────────┘  │   │
│                               │  ┌──────────────┐  │   │
│                               │  │ Parallel     │  │   │
│                               │  └──────────────┘  │   │
│                               └────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. Configuration Parsing

```rust
YAML file → Parser → Config → Validation → Vec<Hook>
```

The parser crate:
- Reads `.pre-commit-config.yaml`
- Deserializes using `serde_yaml`
- Validates hook IDs are unique
- Validates dependencies exist

### 2. Execution Planning

```rust
Vec<Hook> → DAG Builder → Execution Plan (levels)
```

The DAG crate:
- Builds a directed acyclic graph from dependencies
- Performs topological sort
- Detects cycles (fails fast)
- Groups hooks into execution levels

Execution levels allow parallel execution:
```
Level 0: [hook-a, hook-b]       // Can run in parallel
Level 1: [hook-c]               // Depends on hook-a
Level 2: [hook-d, hook-e]       // Can run in parallel
```

### 3. Hook Execution

```rust
Execution Plan → Executor → Results
```

Two execution strategies:

**Sequential (Sync)**:
- Executes hooks one by one
- Simple, predictable
- Good for debugging

**Parallel**:
- Executes levels sequentially
- Within each level, hooks run in parallel
- Respects dependencies
- Maximum performance

## Key Components

### Core (`pre-commit-core`)

Defines shared types:

```rust
pub struct Hook { ... }           // Hook configuration
pub struct ExecutionResult { ... } // Execution results
pub trait Executor { ... }        // Execution strategy
pub trait PlanBuilder { ... }     // Planning strategy
```

### Parser (`pre-commit-parser`)

Functions:
- `parse_config_file(path)` → Parse YAML
- `validate_config(config)` → Validate
- `extract_hooks(config)` → Get all hooks

All functions are pure (no side effects except I/O).

### DAG (`pre-commit-dag`)

The DAG builder:
1. Creates a graph with hooks as nodes
2. Adds edges for dependencies
3. Topologically sorts the graph
4. Computes execution levels by depth

Algorithm for level computation:
```
For each node in topological order:
    depth = max(parent depths) + 1
    assign to level[depth - 1]
```

This ensures:
- All dependencies run before dependents
- Maximum parallelism within constraints

### Executors

**Sync Executor** (`pre-commit-executor-sync`):
- Executes hooks sequentially
- Simple implementation
- Predictable execution order

**Parallel Executor** (`pre-commit-executor-parallel`):
- Uses tokio for async execution
- Executes levels sequentially
- Within level: parallel execution
- Waits for level completion before next

## Testing Strategy

### Unit Tests

Each crate has comprehensive unit tests:

```rust
#[test]
fn test_cycle_detection() {
    // Given hooks with circular dependency
    // When building plan
    // Then error is returned
}
```

### Integration Tests

Test cross-crate functionality:
- Parser + DAG integration
- Executor + DAG integration
- End-to-end CLI tests

### Property-Based Testing

Potential future additions:
- Random DAG generation
- Property: no cycles detected = successful plan
- Property: all dependencies satisfied

## Performance Considerations

### Why Rust?

1. **Zero-cost abstractions**: Traits have no runtime overhead
2. **No GC pauses**: Predictable performance
3. **Fearless concurrency**: Safe parallel execution
4. **Small binaries**: Native code, no runtime

### Parallel Execution

Benefits:
- Independent hooks run simultaneously
- Reduced total execution time
- Better CPU utilization

Tradeoffs:
- Slightly more complex implementation
- Non-deterministic output order (by hook ID)
- Requires careful DAG construction

## Extension Points

### Adding New Executors

Implement the `Executor` trait:

```rust
pub trait Executor {
    fn execute(&self, hooks: &[Hook], files: &[PathBuf])
        -> Result<ExecutionResult>;
}
```

Example: Remote executor, distributed executor, cached executor.

### Adding New Output Formats

Extend the CI binary with new `OutputFormat` variants.

### Adding New Hook Types

Currently supports `language: system`. Could add:
- Docker containers
- Python virtual environments
- Node.js executors

## Error Handling

Consistent error handling using `thiserror`:

```rust
#[derive(Error, Debug)]
pub enum PreCommitError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Cycle detected in hook dependencies")]
    CycleDetected,

    // ...
}
```

All functions return `Result<T, PreCommitError>`.

## Future Improvements

### Potential Enhancements

1. **Caching**: Cache hook results based on file hashes
2. **Remote execution**: Distribute hooks across machines
3. **Language support**: Native runners for Python, Node, etc.
4. **Incremental execution**: Only run hooks on changed files
5. **Watch mode**: Re-run hooks on file changes
6. **Plugin system**: Allow custom executors via dynamic linking

### Performance Optimizations

1. **Lazy file filtering**: Only filter files when needed
2. **Smart scheduling**: Consider hook duration for better parallelism
3. **Process pooling**: Reuse processes for multiple hooks
4. **Incremental compilation**: Cache compiled state

## Comparison with Python pre-commit

| Feature | Python pre-commit | pre-commit-rs |
|---------|------------------|---------------|
| Language | Python | Rust |
| Execution | Sequential | Parallel (optional) |
| Config format | YAML | YAML (compatible) |
| Dependencies | Yes | Yes (DAG-based) |
| Performance | Good | Excellent |
| Binary size | Large (Python runtime) | Small (native) |
| Startup time | ~100ms | <10ms |

## Conclusion

pre-commit-rs demonstrates how functional programming principles, type safety, and modular design can create a maintainable, high-performance tool. The atomic crate architecture ensures each component is independently testable while the trait-based design allows for easy extension.
