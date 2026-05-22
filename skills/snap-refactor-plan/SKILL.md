---
name: snap-refactor-plan
description: "snap-rs refactor planning skill. Use when the selected PLAN.md phase is a refactor, cleanup, migration, decomposition, rename, or behavior-preserving change. Builds a minimal safe edit sequence before code changes."
managed-by: snap-rs
---

# snap-rs Refactor Plan

Use this when the selected phase is primarily a refactor or migration.

The goal is to preserve behavior while making the requested structural change.

## Workflow

1. Re-read the selected phase and identify the intended behavior-preserving boundary.
2. Identify current tests or commands that can detect regressions.
3. Inspect existing code shape and local patterns.
4. Define the smallest safe edit sequence.
5. Prefer mechanical, reversible steps.
6. Avoid changing public behavior unless the selected phase explicitly requires it.
7. Run checks after the refactor.
8. If behavior changes are necessary, state why they are required by the selected phase.

## Refactor Plan Shape

Before editing, form a short plan:

```md
## Refactor Plan

Goal: ...
Behavior that must not change: ...
Files likely touched: ...
Safe sequence:

1. ...
2. ...
3. ...
   Verification: ...
```

## Guardrails

Do not combine unrelated cleanup with the refactor.

Do not introduce a new abstraction unless it clearly reduces real complexity in the touched area.

Do not move code across ownership boundaries unless the phase requires it.

Do not split or rename files just to make the diff look cleaner.

## Completion Criteria

The refactor is complete when:

- requested structure is achieved
- behavior is preserved or intentionally changed per phase contract
- checks pass
- diff remains focused
