---
name: lgtm-cli-control
description: "lgtm local CLI/TUI control skill. Use only when a selected PLAN.md phase changes CLI/TUI behavior, terminal output, prompts, interrupts, hangs, resize behavior, or terminal demos and needs repeatable local evidence."
managed-by: lgtm
---

# lgtm CLI Control

Use only when selected phase need user-visible CLI/TUI verify.

Goal: repeatable local harness, not manual poke.

## Workflow

1. ID command, workspace, behavior under test.
2. Prefer repo-native harness:
   - integration tests
   - e2e tests
   - demo scripts
   - PTY helpers
   - expect scripts
3. No harness? Use temp local harness under `/tmp`.
4. Drive one action at a time. Wait for concrete output before next.
5. Capture smallest transcript that prove/disprove behavior.
6. Clean up temp sessions, processes, artifacts unless user say keep.
7. Turn findings into selected-phase fix or explicit blocker.

## Harness Options

Prefer repo-native tools. If need:

- `tmux` for managed terminal sessions
- short PTY script for deterministic waits
- existing runtime profilers for startup, hangs, memory

No add testing dependency for one-off probe unless selected phase require.

## Guardrails

No send credentials or destructive commands into harness.

No hardcode paths from another repo.

No keep harness code in repo unless selected phase need reusable test.

No treat screenshots/transcripts as enough when stable automated test practical.

## Completion Criteria

CLI control done when CLI/TUI behavior verified with local evidence, fixed within selected-phase scope, or blocked with clear reason.