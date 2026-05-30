---
name: lgtm-phase-validate
description: "lgtm validation pass for exactly one PLAN.md phase. Use when lgtm asks Codex validate implemented phase. Independently re-read selected phase, compare implementation to Goal, Steps, Validation, Web validation sections, fix scoped gaps, verify concrete checks."
managed-by: lgtm
---

# lgtm Phase Validation

You validate exactly one selected phase from `PLAN.md`.

## Inputs

lgtm give:

- selected phase heading
- path to `PLAN.md`
- path to `AGENTS.md`

Treat validation as independent review, not continuation of implementation assumptions.

## Workflow

1. Re-open `AGENTS.md`, `PLAN.md`, context docs linked from selected phase.
2. Find exact selected phase heading.
3. Re-read selected phase Goal, Steps, Validation, Web validation sections.
4. Inspect files touched by implementation and surrounding modules.
5. Compare current behavior vs phase contract.
6. Look for:
   - missing behavior
   - incomplete edge cases
   - unsafe broad changes
   - weak or missing tests
   - stale docs or product-contract drift
   - security-sensitive surfaces from change
   - skipped required checks
7. Fix only gaps needed complete selected phase.
8. Strengthen tests/verification when existing checks not prove phase works.
9. Re-run required checks after fixes.
10. If compile or type-check fail, group errors by file and category, fix highest-confidence selected-phase issues first, rerun until clean or blocked.
11. Leave structural quality and final closeout to review pass.

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