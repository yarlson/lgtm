# Plan

## Phase 1 - Run Command

Goal: Execute `PLAN.md` phases through Codex-backed implementation,
validation, and review passes.

Steps:

- Parse the current target repository `PLAN.md` into phase identifiers.
- Reload `PLAN.md` before every phase so earlier phases can update later work.
- Run implementation, validation, and review passes through `codex app-server`.
- Keep phase prompts anchored to `PLAN.md`, `AGENTS.md`, and the exact selected
  phase heading.
- Store app-server protocol logs under `.codex-log/`.

Validation:

- Fake app-server integration test proves indexing, pass prompts, skill
  installation, logging, and mid-run `PLAN.md` reload.

## Phase 2 - Plan Command

Goal: Create or refine `PLAN.md` through an interactive Codex planning session.

Steps:

- Add `lgtm plan [BRIEF]` with root, plan path, Codex binary, log directory,
  and run stamp options.
- Install managed planning skills and perform Git preflight without requiring an
  existing `PLAN.md` or `AGENTS.md`.
- Keep planning state in the Codex thread.
- Stop only when `PLAN.md` changes and a missing initial `AGENTS.md` has been
  created.
- Support exact `/finish` and `/quit` submissions.

Validation:

- Unit tests cover artifact completion detection and planning prompt contracts.
- Fake app-server tests cover plan turn logging and non-TTY preflight order.

## Phase 3 - Interactive Composer

Goal: Make plan-mode answers comfortable in a terminal.

Steps:

- Use crossterm raw mode without entering an alternate screen.
- Support normal text editing, cursor movement, multiline answers, and
  bracketed paste.
- Treat one `Ctrl+C` as input clear and a quick second `Ctrl+C` as quit.
- Restore terminal modes on submit, quit, and error.

Validation:

- Composer unit tests cover input editing, paste normalization, slash commands,
  shifted letters, cursor movement, and `Ctrl+C` behavior.

## Phase 4 - Output And Spinner DX

Goal: Match the useful `snap-rs` terminal feedback while preserving the
app-server architecture.

Steps:

- Emit idle events while waiting on app-server output.
- Show active spinner rows during long-running turns and tool activity.
- Replace active rows with final evidence once the relevant item completes.
- Render plan-mode final agent messages through the Markdown renderer.
- Restore the cursor on normal completion, drop, and SIGINT.

Validation:

- Unit tests cover idle events, spinner formatting, active command replacement,
  and Markdown rendering.
- `make check` verifies formatting, clippy, tests, and build.

## Phase 5 - Repository DX And Release Surface

Goal: Keep the repository usable for contributors and releasable like the source
project it was adapted from.

Steps:

- Document setup, usage, safety, and development checks in README.
- Keep root `AGENTS.md` accurate for future agents.
- Provide a `Makefile` with the standard local verification commands.
- Add release packaging workflow and Homebrew formula update helper.
- Keep Cargo metadata suitable for publishing and release validation.

Validation:

- `make check`
- `target/debug/lgtm --help`
- `target/debug/lgtm plan --help`
- `target/debug/lgtm run --help`
- Diff source skills and Makefile against `../snap-rs` where parity is expected.
