---
name: lgtm-phase-review
description: "lgtm local phase review pass. Use after implementation and validation for exactly one PLAN.md phase. Reviews final diff for structural regressions, AI slop, reviewability, scope drift, and maintainability issues; fixes only small, high-confidence phase-scoped findings."
managed-by: lgtm
---

# lgtm Phase Review

You are reviewing exactly one selected phase after implementation and validation.

This is not a PR workflow, CI workflow, shipping workflow, or broad redesign pass.

## Inputs

lgtm will provide:

- the selected phase heading
- the path to `PLAN.md`
- the path to `AGENTS.md`

Treat the selected phase as the only authorized scope.

## Workflow

1. Re-open `AGENTS.md`, `PLAN.md`, and context docs linked from the selected phase.
2. Locate the exact selected phase heading.
3. Review the current diff and changed files against the selected phase.
4. Look for structural code-quality regressions:
   - unnecessary abstraction or wrappers
   - spaghetti conditionals or one-off branches
   - logic in the wrong layer or module
   - duplicated helpers instead of local canonical helpers
   - needless optionality, casts, loose types, or unclear invariants
   - large-file growth that should be decomposed before it hardens
5. Remove AI slop introduced by the phase:
   - unnecessary comments
   - abnormal defensive checks
   - unrelated cleanup
   - noisy formatting or churn
   - implementation chatter in user-facing docs
6. Check reviewability:
   - the diff is understandable
   - mechanical and behavior changes are not confusingly mixed when avoidable
   - tests and docs make the changed behavior clear
7. Fix only small, high-confidence findings inside selected-phase scope.
8. Re-run affected checks after any review fix.
9. Report broad redesign, unrelated refactors, PR work, CI work, or later-phase work as out-of-scope or blocked.

## Approval Bar

Do not accept the phase if the final diff clearly makes the touched area harder to maintain.

The phase review passes only when:

- no obvious structural regression remains
- no obvious AI slop remains
- no later-phase or unrelated work was introduced
- review fixes stayed small and phase-scoped
- affected checks were rerun after review fixes

## Guardrails

Do not add new product behavior.

Do not broaden the implementation to satisfy a review idea.

Do not rewrite a subsystem just because a cleaner design is imaginable.

Do not commit, push, create branches, open PRs, or inspect PR comments unless the user explicitly requested that outside lgtm.
