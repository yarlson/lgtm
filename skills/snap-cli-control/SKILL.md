---
name: snap-cli-control
description: "snap-rs local CLI/TUI control skill. Use only when a selected PLAN.md phase changes CLI/TUI behavior, terminal output, prompts, interrupts, hangs, resize behavior, or terminal demos and needs repeatable local evidence."
managed-by: snap-rs
---

# snap-rs CLI Control

Use this only when the selected phase needs user-visible CLI or TUI verification.

The goal is a repeatable local harness, not manual poking.

## Workflow

1. Identify the command, workspace, and user-visible behavior under test.
2. Prefer existing repo-native harnesses:
   - integration tests
   - e2e tests
   - demo scripts
   - PTY helpers
   - expect scripts
3. If no harness exists, use a temporary local harness under `/tmp`.
4. Drive one action at a time and wait for concrete output before the next action.
5. Capture the smallest transcript that proves or disproves the behavior.
6. Clean up temporary sessions, processes, and artifacts unless the user asked to keep them.
7. Convert findings into a selected-phase fix or explicit blocker.

## Harness Options

Prefer repo-native tools. If needed, use:

- `tmux` for managed terminal sessions
- a short PTY script for deterministic waits
- existing runtime profilers for startup, hangs, or memory behavior

Do not add a testing dependency just for a one-off probe unless the selected phase requires it.

## Guardrails

Do not send credentials or destructive commands into a harness.

Do not hardcode paths from another repository.

Do not keep harness code in the repo unless the selected phase requires a reusable test.

Do not treat screenshots or transcripts as sufficient when a stable automated test is practical.

## Completion Criteria

CLI control is complete when the CLI/TUI behavior is verified with local evidence, fixed within selected-phase scope, or blocked with a clear reason.
