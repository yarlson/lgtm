---
name: snap-final-review
description: "snap-rs final phase closeout skill. Use at the end of the review pass to confirm the selected phase contract is satisfied, summarize verification, and flag out-of-scope follow-ups without expanding work."
managed-by: snap-rs
---

# snap-rs Final Review

Use this at the end of the review pass.

The goal is to close the selected phase cleanly.

## Workflow

1. Re-read the selected phase contract.
2. Review the final diff.
3. Confirm each required behavior is implemented.
4. Confirm tests or validation checks were run.
5. Confirm fixes stayed within selected-phase scope.
6. Confirm no later-phase work was added.
7. Confirm docs were updated only if directly affected.
8. Confirm security, dependency, rollout, test-gap, CLI-control, UI-control, and phase-review skills were used when triggered.
9. Identify out-of-scope issues separately without fixing them.
10. Produce a concise final summary.

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

Do not make new edits during final review unless they are required to complete the selected phase.

Do not hide failed or skipped checks.

Do not claim validation that was not performed.

Do not commit or push unless explicitly requested.

## Completion Criteria

Final review is complete when the selected phase can be honestly reported as complete, or the remaining blocker is explicit and actionable.
