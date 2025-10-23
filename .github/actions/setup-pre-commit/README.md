# Setup pre-commit-ci Action

Download and setup the `pre-commit-ci` binary from GitHub releases.

## Usage

```yaml
- uses: andrewgazelka/pre-commit/.github/actions/setup-pre-commit@main

- name: Run pre-commit hooks
  run: ./pre-commit-ci --parallel --format json
```

## Inputs

| Input | Description | Required | Default |
|-------|-------------|----------|---------|
| `version` | Version to download | No | `latest` |
| `token` | GitHub token for API requests (avoids rate limits) | No | `${{ github.token }}` |

## Outputs

| Output | Description |
|--------|-------------|
| `binary_path` | Path to the downloaded pre-commit-ci binary |
| `version` | The version that was downloaded |

## Supported Platforms

- Linux x64 (`x86_64-unknown-linux-gnu`)
- Linux ARM64 (`aarch64-unknown-linux-gnu`)
- macOS x64 (`x86_64-apple-darwin`)
- macOS ARM64 (`aarch64-apple-darwin`)

## Example

```yaml
name: Pre-commit Checks

on: [push, pull_request]

jobs:
  pre-commit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5

      - uses: andrewgazelka/pre-commit/.github/actions/setup-pre-commit@main

      - name: Run hooks
        run: ./pre-commit-ci --parallel

      - name: Run with JSON output
        run: ./pre-commit-ci --parallel --format json > results.json
```

## Why use this?

- Fast: Downloads pre-compiled binaries instead of building from source
- Cross-platform: Automatically detects and downloads the right binary for your runner
- Simple: Just one step to get `pre-commit-ci` ready to use
