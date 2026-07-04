# Plan

## Decisions

- Build a dependency-free Rust calculator binary named `calc_cli`.
- Support only integer `add` and `sub` commands in this fixture.
- Treat invalid commands and invalid arguments as user errors printed to stderr.

## Non-Goals

- Do not add multiplication, division, floating point support, or external dependencies.
- Do not change repository automation outside the calculator crate.

## Open Risks

- Argument parsing mistakes can make invalid input look successful.
- Fixture validation depends on exact stdout for successful calculator commands.

## Loopholes To Close

- Keep error handling explicit for missing, extra, and non-integer arguments.

## Phase 1 - Calculator CLI

Goal:
Create a minimal Rust CLI calculator in this empty repository.

Deliverables:
- Add a Cargo binary crate named `calc_cli`.
- Implement `add A B` and `sub A B` integer commands.
- Add behavior tests for valid and invalid CLI input.

Dependencies:
- None.

Unresolved decisions:
- None.

Steps:
- Create the Cargo package manifest and `src/main.rs`.
- Parse exactly three CLI arguments: command, first integer, second integer.
- Print `add` and `sub` results to stdout.
- Return a nonzero exit and usage/error text for invalid command, missing args, extra args, or non-integer args.
- Keep implementation dependency-free.
- Add focused tests for add, sub, invalid command, missing args, extra args, and invalid integer input.

Validation:
- Run `cargo fmt --all --check`.
- Run `cargo test --all`.
- Run `cargo run --quiet -- add 2 3` and verify stdout is `5`.
- Run `cargo run --quiet -- sub 9 4` and verify stdout is `5`.
