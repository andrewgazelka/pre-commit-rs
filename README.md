# pre-commit-rs

Fast parallel pre-commit hook runner in Rust.

## Quick Start (CI)

```yaml
- uses: andrewgazelka/pre-commit-rs/.github/actions/setup-pre-commit@main
- run: ./pre-commit-ci --parallel --format json
```

## Features

- Parallel execution with dependency resolution
- `.pre-commit-config.yaml` compatible
- DAG-based scheduling

## Example Config

```yaml
repos:
  - repo: local
    hooks:
      - id: fmt
        entry: cargo fmt
        language: system
        pass_filenames: false

      - id: test
        entry: cargo test
        language: system
        pass_filenames: false
        depends_on: [fmt]
```

## License

MIT
