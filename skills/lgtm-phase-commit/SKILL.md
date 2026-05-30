---
name: lgtm-phase-commit
description: "lgtm after-phase commit pass. Use after impl, validation, review for exactly one PLAN.md phase to stage all changes and create one local git commit without pushing."
managed-by: lgtm
---

# lgtm Phase Commit

Commit all uncommitted changes after the one `PLAN.md` phase just finished this session (impl, validation, review). lgtm gives selected phase heading. Local commit only — no PR, CI, branch, release, push.

## Workflow

You already know from this session what changed and why. Do not re-read plan docs or re-run checks.

1. `git status --short` + diff to see uncommitted changes.
2. Stage all changes.
3. One local commit.
4. Report commit hash + one-line summary.

## Commit Message

Conventional Commits. `<type>(<scope>): <imperative summary>`, scope optional, subject ≤50 char, no trailing period, imperative mood.

Types: `feat`, `fix`, `refactor`, `perf`, `docs`, `test`, `chore`, `build`, `ci`, `style`, `revert`.

Body only when why not obvious. Always body for breaking, security, migration, revert. Wrap 72. Bullets `-`. Issues at end: `Closes #42`.

Never: "this commit", "I/we/now", AI attribution, emoji, file name when scope says it.

## Guardrails

- No empty commit.
- Stage all changes.
- No push, branch, PR, CI, release tag.

Final response concise: commit hash and one-line summary only.
