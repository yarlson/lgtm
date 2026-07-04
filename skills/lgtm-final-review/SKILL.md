---
name: lgtm-final-review
description: "lgtm final phase closeout skill. Use at end of review pass: confirm selected phase contract satisfied, summarize verification, flag out-of-scope follow-ups, no scope expansion."
managed-by: lgtm
---

# lgtm Final Review

Use at end of review pass.

Goal: close selected phase cleanly.

## Workflow

1. Re-read selected phase contract.
2. Review final diff.
3. Confirm each required behavior implemented.
4. Confirm tests/validation ran.
5. Confirm fixes stayed in selected-phase scope.
6. Confirm no later-phase work added.
7. Confirm docs updated only if directly affected.
8. Confirm security, dependency, rollout, test-gap, CLI-control, UI-control, phase-review skills used when triggered.
9. Identify out-of-scope issues separately, no fixing.
10. Produce concise final summary.

## Final Summary Shape

Use this structure:

```md
## Phase Closeout

Implemented:

- ...

Verified:

- ...

Changed docs:

- ...

Not done / blocked:

- ...

Out-of-scope follow-ups:

- ...
```

Omit sections that do not apply.

## Guardrails

No new edits during final review unless required to complete selected phase.

No hiding failed/skipped checks.

No claiming validation not performed.

No commit/push unless explicitly requested.

## Completion Criteria

Complete when selected phase can be honestly reported complete, or remaining blocker is explicit and actionable.
