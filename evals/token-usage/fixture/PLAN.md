# Plan

## Phase 1 - Calculator CLI

Goal:

Create a minimal Rust CLI calculator in this empty repository.

Steps:

1. Add a Cargo binary crate named `calc_cli`.
2. Implement `src/main.rs` with these commands:
   - `add A B` prints the integer sum.
   - `sub A B` prints the integer difference.
   - invalid command, missing args, extra args, or non-integer args exits nonzero and prints a usage/error message to stderr.
3. Keep implementation dependency-free.
4. Add focused tests for add, sub, invalid command, missing args, extra args, and invalid integer input.

Validation:

```bash
cargo fmt --all --check
cargo test --all
cargo run --quiet -- add 2 3
cargo run --quiet -- sub 9 4
```
