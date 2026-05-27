---
name: lgtm-phase-commit
description: "lgtm after-phase commit pass. Use after implementation, validation, and review for exactly one PLAN.md phase to inspect the final diff, stage only selected-phase changes, and create a rich local git commit without pushing."
managed-by: lgtm
---

# lgtm Phase Commit

You are committing exactly one selected phase from `PLAN.md` after its
implementation, validation, and review passes have completed.

This is a local git commit pass, not a PR, CI, branch, release, or push
workflow.

## Inputs

lgtm will provide:

- the selected phase heading
- the path to `PLAN.md`
- the path to `AGENTS.md`

Treat these files as authoritative.

## Workflow

1. Re-open `AGENTS.md` and `PLAN.md`.
2. Locate the exact selected phase heading.
3. Re-read the selected phase's Goal, Steps, and Validation sections.
4. Inspect `git status --short`, unstaged diff, staged diff, and recent commits.
5. Confirm the remaining changes belong to the selected phase.
6. Stage only selected-phase changes.
7. Create one local git commit for the selected phase.
8. Report the commit hash and concise summary.

## Commit Message Standard

Use a rich commit message:

- subject line: concise, imperative, and phase-specific
- body: summarize the phase scope, key changes, verification performed, and any
  known blockers or skipped checks

Do not use vague subjects such as `update`, `changes`, or `phase work`.

## Guardrails

Do not create an empty commit.

Do not commit unrelated changes, generated logs, ignored state, later-phase work,
or broad cleanup outside the selected phase.

If unrelated changes are mixed into the diff and cannot be safely separated,
stop and report a blocker instead of committing them.

Do not push, create branches, open PRs, manage CI, tag releases, or inspect PR
comments.

Do not rerun broad checks unless needed to understand the commit state. The
validation and review passes own verification.

## Completion Criteria

The commit pass is complete only when:

- selected-phase changes are committed locally, or no selected-phase changes
  exist and that is reported explicitly
- the commit message explains scope and verification
- unrelated work was not staged or committed
- no push, PR, branch, CI, or release workflow was started
