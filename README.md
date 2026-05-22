# snap-rs

<div align="center">

**Run Codex through plan-driven implementation, validation, and review passes.**

![Rust](https://img.shields.io/badge/Rust-2024-orange?style=flat-square&logo=rust)
![CLI](https://img.shields.io/badge/CLI-Codex-blue?style=flat-square)

[Overview](#overview) • [Getting Started](#getting-started) • [Usage](#usage) • [How It Works](#how-it-works) • [Development](#development)

</div>

`snap-rs` is a small Rust CLI for running a repo-local `PLAN.md` through a
repeatable Codex execution loop. For each selected phase, it asks Codex to:

1. implement the phase
2. validate the implementation
3. review the result and fix only small, phase-scoped findings

It is designed for repositories that use `PLAN.md` as the implementation order
and `AGENTS.md` as the run instructions.

> [!WARNING]
> `snap-rs` runs `codex exec` with
> `--dangerously-bypass-approvals-and-sandbox` inside the target repository.
> Use it only on repositories where fully autonomous local file and command
> execution is acceptable.

## Overview

`snap-rs` provides a controlled local harness around Codex:

- detects phase headings such as `## Phase 1 - Skeleton` or
  `## Phase 1: Skeleton`
- runs implementation, validation, and review prompts for each selected phase
- reloads `PLAN.md` before each phase so earlier work can adjust later phases
- installs snap-managed project skills into `.agents/skills/snap-*`
- refuses to overwrite project-owned `snap-*` skills
- writes raw Codex JSONL logs into `.codex-log/`
- renders the live JSONL stream as a compact terminal transcript
- prompts before initializing Git in a target root that is not already a Git
  repository

## Getting Started

### Prerequisites

- Rust with Cargo and Rust 2024 edition support
- Git
- the Codex CLI installed and authenticated as `codex`

### Install Locally

From this repository:

```bash
cargo install --path .
```

Or run without installing:

```bash
cargo run -- --help
```

### Prepare a Target Repository

The target repository must contain:

- `PLAN.md`
- `AGENTS.md`
- Git initialized at the target root

Minimal `PLAN.md` example:

```md
# Plan

## Phase 1 - Skeleton

Create the initial implementation and verification path.
```

> [!NOTE]
> If the target root is not a Git repository, `snap-rs` asks before running
> `git init` and `git branch -M main`. Declining the prompt aborts the run.

## Usage

Run a single phase in a sibling repository:

```bash
cargo run -- --root ../lnk --start-phase 1 --end-phase 1 --sleep-seconds 0
```

Run phases from the current directory:

```bash
snap-rs --start-phase 1 --end-phase 3
```

Let `snap-rs` detect the final phase from `PLAN.md`:

```bash
snap-rs --root /path/to/repo --start-phase 2
```

Use raw JSONL output instead of the formatted transcript:

```bash
snap-rs --stream-mode raw
```

### Options

| Option | Environment variable | Default | Description |
| --- | --- | --- | --- |
| `--root` | `ROOT_DIR` | current directory | Target repository root |
| `--plan-path` | `PLAN_PATH` | `PLAN.md` | Plan file path under the root |
| `--agents-path` | `REPO_AGENTS_PATH` | `AGENTS.md` | Agent instruction file path under the root |
| `--start-phase` | `START_PHASE` | `1` | First phase number to run |
| `--end-phase` | `END_PHASE` | detected from `PLAN.md` | Last phase number to run |
| `--sleep-seconds` | `SLEEP_SECONDS` | `600` | Delay between phases |
| `--codex-bin` | `CODEX_BIN` | `codex` | Codex executable |
| `--stream-mode` | `STREAM_MODE` | `pretty` | `pretty` or `raw` |
| `--log-dir` | `LOG_DIR` | `.codex-log` | Raw JSONL log directory |
| `--run-stamp` | `RUN_STAMP` | current timestamp | Prefix for log filenames |

## How It Works

For each phase, `snap-rs` constructs three prompts from the selected phase
heading and the configured context files:

1. **Implement** uses `snap-phase-implement` and related phase-scoped skills.
2. **Validate** independently checks the phase against the plan and fixes
   correctness, test, docs, security, dependency, or rollout gaps.
3. **Review** checks maintainability, scope control, and closeout, then fixes
   only small high-confidence findings.

Logs are written as:

```text
.codex-log/<run-stamp>-phase-<NN>-implement.jsonl
.codex-log/<run-stamp>-phase-<NN>-validate.jsonl
.codex-log/<run-stamp>-phase-<NN>-review.jsonl
```

Managed skills are embedded into the binary at compile time from
`skills/snap-*/SKILL.md`. During preflight, `snap-rs` refreshes those skills in
the target repository and ensures these entries exist in the target
`.gitignore`:

```gitignore
.agents/skills/snap-*
.codex-log/
```

## Development

Common checks are available through the `Makefile`:

```bash
make check
```

Equivalent commands:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --all-targets --all-features
```

Build a release binary:

```bash
make release
```
