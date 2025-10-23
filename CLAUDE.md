# Pre-commit Rust Implementation

Fast parallel pre-commit hook runner in Rust.

## Atomicity

Each crate is ATOMIC and self-contained:
- Changes to one crate should NOT affect unrelated crates
- Keep changes minimal and focused
- Test changes within the crate you're modifying
