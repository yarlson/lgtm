---
name: lgtm-plan-update
description: "lgtm PLAN.md update skill. Use only when implementation or validation proves the selected PLAN.md phase has an incorrect order, missing validation gate, impossible instruction, or incomplete phase contract."
managed-by: lgtm
---

# lgtm PLAN.md Update

Use only when current `PLAN.md` wrong or incomplete for selected phase.

Not for documenting progress or nice-to-have tasks.

## Valid Reasons To Update PLAN.md

Update `PLAN.md` only when one true:

- selected phase cannot implement safely as written
- validation proves required step missing
- phase order wrong
- validation gates insufficient or impossible
- phase contradicts repo-local product or architecture contract
- implementation exposes prerequisite that must join this phase

## Workflow

1. Find exact phase, exact defect.
2. Confirm issue from repo-local evidence.
3. Make smallest correction.
4. Keep existing plan style.
5. No rewrite unrelated phases.
6. No future features.
7. After update, continue implement or validate only selected phase.

## Update Rules

Prefer:

- add missing validation command
- clarify ambiguous step
- correct phase order locally
- mark blocker explicitly

Avoid:

- broad plan rewrites
- new roadmap sections
- speculative future phases
- duplicate implementation details already obvious in code

## Completion Criteria

PLAN.md update OK only when it makes selected phase implementable or verifiable.
