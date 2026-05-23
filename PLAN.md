# Plan

## Phase 1 - Explicit CLI Subcommands

Goal: Replace the current bare `lgtm` command shape with explicit `run` and `plan` subcommands while preserving the existing phase-run behavior under `lgtm run`.

Steps:

- Change CLI parsing so `lgtm run` owns the existing options: `--root`, `--plan-path`, `--agents-path`, `--start-phase`, `--end-phase`, `--sleep-seconds`, `--codex-bin`, `--stream-mode`, `--log-dir`, and `--run-stamp`.
- Add `lgtm plan [BRIEF]` with shared options: `--root`, `--plan-path`, `--codex-bin`, `--log-dir`, and `--run-stamp`.
- Keep existing `Config` behavior for run mode, but split config construction so plan mode does not require run-only fields.
- Update `lib::run()` to dispatch by subcommand.
- Remove support for bare `lgtm --start-phase ...`; pre-v1 compatibility is not required.
- Keep error messages direct when no subcommand is provided.

Validation:

- `cargo test cli`
- Verify `lgtm run --help` shows phase-run options.
- Verify `lgtm plan --help` shows planning options and optional brief.
- Verify bare `lgtm --start-phase 1` fails with clap usage.
- Verify existing run-mode config tests still pass after moving under `run`.

## Phase 2 - Planning Skill And Prompt Contract

Goal: Add the `lgtm-plan-create` managed skill and initial/resume prompt builders for plan mode.

Steps:

- Add bundled skill `skills/lgtm-plan-create/SKILL.md` with `name: lgtm-plan-create` and `managed-by: lgtm`.
- Register the skill in the existing skill registry so install/preflight/gitignore behavior remains centralized.
- The skill must instruct Codex to ask exactly one sharp question per turn, prefer forced choices, reject vague answers with a narrower follow-up, and write `PLAN.md` only when ready to finish.
- The skill must define `PLAN.md` as a final-only sentinel: Codex must not create or modify it as a draft.
- Add plan-mode prompt generation:
  - Initial prompt includes optional user brief when provided.
  - Initial prompt references target `PLAN.md` path.
  - Initial prompt tells Codex to read `AGENTS.md` only if it exists.
  - Resume prompts pass only the user answer, except `/finish`.
- The final `PLAN.md` contract must use:
  - `# Plan`
  - `## Phase N - Name`
  - `Goal:`
  - `Steps:`
  - `Validation:`

Validation:

- Unit-test that all bundled skills have valid managed frontmatter.
- Unit-test that `lgtm-plan-create` is installed with the other managed skills.
- Unit-test initial plan prompt with and without brief.
- Unit-test prompt text includes the final-only `PLAN.md` sentinel rule.
- Unit-test prompt text does not require `AGENTS.md`.

## Phase 3 - Codex Turn Runner For Plan Mode

Goal: Add a reusable Codex JSONL turn runner that supports both first-turn `exec` and explicit-session `exec resume`, captures the thread id, and returns the last agent message.

Steps:

- Extract the current Codex process spawning and JSONL streaming behavior enough to reuse it for plan mode without changing run-mode output behavior.
- For the first plan turn, run:
  - `codex exec -C <root> --dangerously-bypass-approvals-and-sandbox --json -`
- Capture `thread.started.thread_id` from JSONL.
- Capture the last `agent_message` text from each turn.
- For resume turns, run:
  - `codex exec resume <thread_id> --dangerously-bypass-approvals-and-sandbox --json -`
  - Set the child process current directory to the target root.
  - Do not use `--last`.
- Write plan-mode raw logs under `.codex-log/` using names that distinguish turns, for example:
  - `<run-stamp>-plan-001.jsonl`
  - `<run-stamp>-plan-002.jsonl`
- In plan mode, suppress the pretty transcript and print only the final agent message/question for each turn.
- If a Codex turn exits successfully but emits no thread id on the first turn, return a clear error.
- If a Codex turn exits successfully but emits no agent message and `PLAN.md` did not change, return a clear error.

Validation:

- Integration-test with a fake Codex binary that first emits `thread.started` and an `agent_message`.
- Verify resume invocation uses the captured session id, not `--last`.
- Verify raw JSONL logs are written for each turn.
- Verify the last agent message is selected when multiple agent messages appear.
- Verify missing first-turn thread id is an error.
- Verify missing agent message before completion is an error.

## Phase 4 - Plan Mode Preflight And Completion Detection

Goal: Implement the plan-mode loop around Codex turns, with Git/skill preflight and deterministic `PLAN.md` completion detection.

Steps:

- Add plan-mode preflight:
  - Resolve root.
  - Reject unmanaged existing `lgtm-*` skills.
  - Ensure the root is a Git root or prompt to initialize Git.
  - Install managed skills and `.gitignore` entries.
  - Do not require `PLAN.md`.
  - Do not require `AGENTS.md`.
- Before the first Codex turn, snapshot `<root>/<plan-path>`:
  - absent
  - present with enough metadata/content to detect modification
- After every Codex turn, check whether `<root>/<plan-path>` was created or modified.
- Stop immediately when `PLAN.md` is created or modified.
- Continue the question/answer loop while `PLAN.md` is absent or unchanged.
- Treat `PLAN.md` as final-only; no draft path is supported in v1.
- Require interactive stdin/stdout for plan mode; fail clearly when not running in a TTY.
- Support exact submitted `/quit` by exiting without another Codex turn.
- Support exact submitted `/finish` by sending a finalization prompt to Codex:
  - produce the best `PLAN.md` now
  - mark unresolved risks explicitly
  - do not invent certainty

Validation:

- Integration-test that plan mode does not require existing `PLAN.md`.
- Integration-test that plan mode does not require `AGENTS.md`.
- Integration-test that unmanaged `lgtm-*` skills abort before Codex starts.
- Integration-test that loop stops when fake Codex creates `PLAN.md`.
- Integration-test that loop stops when fake Codex modifies an existing `PLAN.md`.
- Unit-test `/quit` and `/finish` command classification.
- Verify non-TTY plan mode returns a clear error.

## Phase 5 - Minimal Inline Composer

Goal: Add a small crossterm-based inline composer for plan-mode answers with multiline input and bracketed paste support.

Steps:

- Add `crossterm` as a direct dependency.
- Create a testable composer input-state layer that handles:
  - character insertion
  - newline insertion
  - backspace
  - submitted text
  - exact command classification for `/finish` and `/quit`
  - CRLF/CR paste normalization to LF
- Create a thin terminal wrapper that:
  - enables raw mode while reading an answer
  - enables bracketed paste
  - disables raw mode and bracketed paste on exit or error
  - does not enter alternate screen
  - redraws a compact inline input area
- Key behavior:
  - Enter submits
  - Ctrl+J inserts newline
  - Shift+Enter inserts newline when delivered distinctly by the terminal
  - Alt+Enter inserts newline when delivered distinctly by the terminal
  - bracketed paste inserts text and preserves multiline content
- Do not implement Codex-style paste-burst timing fallback in v1.
- Do not implement full cursor movement, history, external editor, or image paste in v1 unless needed to keep the composer usable.

Validation:

- Unit-test text input plus Enter submits.
- Unit-test Ctrl+J inserts newline.
- Unit-test pasted CRLF and CR normalize to LF.
- Unit-test exact `/finish` and `/quit` are recognized only after submission.
- Unit-test non-command slash-prefixed answers are sent as normal answers.
- Manual smoke test in a real terminal:
  - type single-line answer and submit
  - type multiline answer with Ctrl+J
  - paste multiline text
  - submit `/quit`

## Phase 6 - Documentation And End-To-End Verification

Goal: Update user-facing docs and verify both `run` and `plan` workflows.

Steps:

- Update README usage to document:
  - `lgtm run`
  - `lgtm plan [BRIEF]`
  - plan-mode TTY requirement
  - `/finish` and `/quit`
  - `PLAN.md` final-only behavior
  - raw JSONL logs for plan turns
- Update examples that currently use bare `lgtm --start-phase ...` to use `lgtm run`.
- Update safety model to cover both run mode and plan mode.
- Keep docs concise; do not add a second documentation system.
- Ensure tests do not depend on a real Codex binary by using fake Codex scripts.

Validation:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo build --all-targets --all-features`
- Run fake-Codex integration tests for both `lgtm run` and `lgtm plan`.
