---
name: lgtm-phase-validate
description: "lgtm validation pass for exactly one PLAN.md phase. Use when lgtm asks Codex to validate an implemented phase. Independently re-reads the selected phase, compares implementation to Goal, Steps, Validation, and Web validation sections, fixes scoped gaps, and verifies concrete checks."
managed-by: lgtm
---

# lgtm Phase Validation

You are validating exactly one selected phase from `PLAN.md`.

## Inputs

lgtm will provide:

- the selected phase heading
- the path to `PLAN.md`
- the path to `AGENTS.md`

Treat validation as an independent review, not a continuation of implementation assumptions.

`PLAN.md` is immutable after `/finish`. Do not edit it for ordinary progress,
status, discoveries, or later-phase notes. Keep validation progress and
closeout notes in root-level `PLAN_STATUS.md`, creating it if it is missing.
Use `lgtm-plan-update` only for an exceptional selected-phase contract defect
that makes `PLAN.md` impossible or unsafe to validate as written.

## Workflow

1. Re-open `AGENTS.md`, `PLAN.md`, and context docs linked from the selected phase.
2. Locate the exact selected phase heading.
3. Re-read the selected phase's Goal, Steps, Validation, and Web validation sections.
4. Inspect files touched by the implementation and surrounding modules.
5. Compare current behavior against the phase contract.
6. Look for:
   - missing behavior
   - incomplete edge cases
   - unsafe broad changes
   - weak or missing tests
   - stale docs or product-contract drift
   - security-sensitive surfaces introduced by the change
   - required checks that were skipped
7. Fix only gaps needed to complete the selected phase.
8. Strengthen tests or verification when existing checks do not prove the phase works.
9. Run required checks again after fixes.
10. If compile or type-check commands fail, group errors by file and category, fix the highest-confidence selected-phase issues first, and rerun until clean or blocked.
11. Update `PLAN_STATUS.md` with concise validation progress, verification, blockers, and current phase status.
12. Leave structural quality and final closeout to the review pass.

## Validation Standard

Do not accept a phase because code exists. Accept it only when behavior is verified against the phase contract.

If the phase cannot be validated because a tool, service, credential, fixture, or environment is missing, report the blocker explicitly and explain what remains unverified.

Record blockers and unverified areas in `PLAN_STATUS.md`; do not rewrite `PLAN.md` unless `lgtm-plan-update` is required for a selected-phase contract defect.

## Compiler And Typecheck Failures

When validation fails at compile or type-check time:

1. Identify the failing command.
2. Summarize errors by file and category.
3. Fix the highest-confidence selected-phase errors first.
4. Re-run the same command after each focused fix.
5. Stop and report a blocker if the remaining failure requires unrelated work or missing environment.

## Completion Criteria

Validation is complete only when:

- the selected phase is implemented fully and correctly
- concrete checks were run or blockers were reported
- `PLAN_STATUS.md` contains current validation progress, verification, blockers, or status notes
- any fixes stayed within selected-phase scope
- no later-phase work was added
- compile or type-check failures were resolved or explicitly blocked
