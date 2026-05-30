---
name: lgtm-phase-validate
description: "lgtm validation pass for exactly one PLAN.md phase. Use when lgtm asks Codex validate implemented phase in the same session, compare implementation to Goal, Steps, Validation, Web validation sections, fix scoped gaps, verify concrete checks."
managed-by: lgtm
---

# lgtm Phase Validation

You validate exactly one selected phase from `PLAN.md`.

## Inputs

lgtm give:

- selected phase heading
- path to `PLAN.md`
- path to `AGENTS.md`

Treat validation as independent judgment, not blind trust in implementation assumptions. Use session context already gathered.

## Workflow

1. Use current session context for `AGENTS.md`, `PLAN.md`, and selected phase.
2. Re-open plan docs only when context missing, stale, or contradicted by implementation.
3. Inspect files touched by implementation and surrounding modules.
4. Compare current behavior vs phase contract.
5. Look for:
   - missing behavior
   - incomplete edge cases
   - unsafe broad changes
   - weak or missing tests
   - stale docs or product-contract drift
   - security-sensitive surfaces from change
   - skipped required checks
6. Fix only gaps needed complete selected phase.
7. Strengthen tests/verification when existing checks not prove phase works.
8. Re-run required checks after fixes.
9. If compile or type-check fail, group errors by file and category, fix highest-confidence selected-phase issues first, rerun until clean or blocked.
10. Leave structural quality and final closeout to review pass.

## Validation Standard

Do not accept phase because code exists. Accept only when behavior verified vs phase contract.

If phase cannot validate because tool, service, credential, fixture, or environment missing, report blocker explicitly and explain what stays unverified.

## Compiler And Typecheck Failures

When validation fails at compile or type-check:

1. Identify failing command.
2. Summarize errors by file and category.
3. Fix highest-confidence selected-phase errors first.
4. Re-run same command after each focused fix.
5. Stop and report blocker if remaining failure needs unrelated work or missing environment.

## Completion Criteria

Validation complete only when:

- selected phase implemented fully and correctly
- concrete checks run or blockers reported
- any fixes stayed in selected-phase scope
- no later-phase work added
- compile or type-check failures resolved or explicitly blocked
