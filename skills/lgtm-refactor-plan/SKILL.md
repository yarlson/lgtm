---
name: lgtm-refactor-plan
description: "lgtm refactor plan skill. Use when selected PLAN.md phase be refactor, cleanup, migration, decomposition, rename, or behavior-preserving change. Build minimal safe edit sequence before code change."
managed-by: lgtm
---

# lgtm Refactor Plan

Use when selected phase mainly refactor or migration.

Goal: preserve behavior while make requested structural change.

## Workflow

1. Re-read selected phase, find intended behavior-preserving boundary.
2. Find current tests or commands that detect regressions.
3. Inspect existing code shape and local patterns.
4. Define smallest safe edit sequence.
5. Prefer mechanical, reversible steps.
6. No change public behavior unless selected phase require it.
7. Run checks after refactor.
8. If behavior change needed, state why phase require it.

## Refactor Plan Shape

Before edit, form short plan:

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

No combine unrelated cleanup with refactor.

No add new abstraction unless it clearly cut real complexity in touched area.

No move code across ownership boundaries unless phase require it.

No split or rename files just to make diff look cleaner.

## Completion Criteria

Refactor done when:

- requested structure achieved
- behavior preserved or intentionally changed per phase contract
- checks pass
- diff stay focused