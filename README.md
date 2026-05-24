<div align="center">

<img src="assets/banner.png" alt="lgtm workflow banner showing plan, PLAN.md, AGENTS.md, implementation, validation, and review" width="100%">

# lgtm

Plan the work. Execute one phase. Verify. Review. Repeat.

</div>

`lgtm` is a local Codex harness for repo-sized work that should not be done
as one giant prompt. It creates or refines a `PLAN.md`, then runs selected plan
phases through implementation, validation, and review passes.

Use it for migrations, cleanup, feature slices, test hardening, and docs drift
where Codex needs a repeatable local loop.

> [!WARNING]
> `lgtm` drives `codex app-server` with `danger-full-access` and approval
> policy `never` inside the target repository. Use it only where autonomous
> local file and command execution is acceptable.

## Overview

- `lgtm plan [BRIEF]` starts an interactive planning session.
- `lgtm run` executes selected phases from `PLAN.md`.
- Each run phase gets implement, validate, and review passes.
- Prompts are anchored to `PLAN.md`, `AGENTS.md`, and the exact phase heading.
- `PLAN.md` is reloaded before each phase so earlier phases can update later
  phases.
- Pretty output uses active spinner rows while Codex is thinking or running
  tools, then replaces them with final evidence.
- App-server protocol logs are written to `.codex-log/`.
- Managed skills are installed under `.agents/skills/lgtm-*`.

## Install

```bash
cargo install --path .
```

Or run from the repository:

```bash
cargo run -- --help
```

## Plan

Plan mode requires an interactive TTY. `PLAN.md` and `AGENTS.md` do not need to
exist before planning starts.

```bash
lgtm plan "split the migration into small reviewable phases"
```

Plan mode asks one question at a time. The inline composer supports normal
editing keys, cursor movement, paste, and multiline answers with `Ctrl+J`,
`Shift+Enter`, or `Alt+Enter`. Press `Ctrl+C` once to clear the current input;
press it twice quickly to quit.

Enter `/finish` to ask Codex to write the final `PLAN.md` from the current
session context, or `/quit` to exit without another Codex turn. If `AGENTS.md`
was missing at the start, plan mode keeps going until both `PLAN.md` and
`AGENTS.md` are complete.

## Run

For run mode, the target repository must contain:

- `PLAN.md`
- `AGENTS.md`
- a Git repository at the target root, or permission to initialize one

Run one phase:

```bash
lgtm run --root ../target-repo --start-phase 1 --end-phase 1 --sleep-seconds 0
```

Run from phase 2 through the detected final phase:

```bash
lgtm run --root ../target-repo --start-phase 2
```

Use raw protocol output:

```bash
lgtm run --stream-mode raw
```

## Options

Run options:

| Option | Environment | Default | Description |
| --- | --- | --- | --- |
| `--root` | `ROOT_DIR` | current directory | Target repository root |
| `--plan-path` | `PLAN_PATH` | `PLAN.md` | Plan file path under the root |
| `--agents-path` | `REPO_AGENTS_PATH` | `AGENTS.md` | Agent instruction path under the root |
| `--start-phase` | `START_PHASE` | `1` | First phase to run |
| `--end-phase` | `END_PHASE` | detected | Last phase to run |
| `--sleep-seconds` | `SLEEP_SECONDS` | `600` | Delay between phases |
| `--codex-bin` | `CODEX_BIN` | `codex` | Codex executable |
| `--stream-mode` | `STREAM_MODE` | `pretty` | `pretty` or `raw` |
| `--log-dir` | `LOG_DIR` | `.codex-log` | Log directory |
| `--run-stamp` | `RUN_STAMP` | timestamp | Log filename prefix |

Plan options:

| Option | Environment | Default | Description |
| --- | --- | --- | --- |
| `[BRIEF]` | | | Optional planning brief |
| `--root` | `ROOT_DIR` | current directory | Target repository root |
| `--plan-path` | `PLAN_PATH` | `PLAN.md` | Plan file path under the root |
| `--codex-bin` | `CODEX_BIN` | `codex` | Codex executable |
| `--log-dir` | `LOG_DIR` | `.codex-log` | Log directory |
| `--run-stamp` | `RUN_STAMP` | timestamp | Log filename prefix |

## Safety And Logs

Before planning or running, `lgtm` checks for unmanaged `lgtm-*` skills,
ensures the target root is a Git root, and installs bundled managed skills. If
Git is not initialized, it asks before running `git init` and `git branch -M
main`.

Logs are written as JSONL:

```text
.codex-log/<run-stamp>-plan-001.jsonl
.codex-log/<run-stamp>-phase-01-index.jsonl
.codex-log/<run-stamp>-phase-01-implement.jsonl
.codex-log/<run-stamp>-phase-01-validate.jsonl
.codex-log/<run-stamp>-phase-01-review.jsonl
```

Each log line records app-server protocol direction and payload. Managed skills
and logs are ignored in target repositories through:

```gitignore
.agents/skills/lgtm-*
.codex-log/
```

## Development

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

Release packaging is defined in `.github/workflows/release.yml`; tagged
releases build platform archives and can update the Homebrew formula through
`scripts/update-homebrew-formula.sh`.
