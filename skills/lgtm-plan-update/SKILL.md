---
name: lgtm-plan-update
description: "lgtm PLAN.md update skill. Use only when implementation or validation proves the selected PLAN.md phase has an incorrect order, missing validation gate, impossible instruction, or incomplete phase contract."
managed-by: lgtm
---

# lgtm PLAN.md Update

Use this only when the current `PLAN.md` is wrong or incomplete for the selected phase.

This skill is not for documenting progress or adding nice-to-have tasks.

## Valid Reasons To Update PLAN.md

Update `PLAN.md` only when one of these is true:

- the selected phase cannot be implemented safely as written
- validation proves a required step is missing
- phase order is incorrect
- validation gates are insufficient or impossible
- the phase contradicts a repo-local product or architecture contract
- implementation exposes a prerequisite that must be part of this phase

## Workflow

1. Identify the exact phase and exact defect in the plan.
2. Confirm the issue from repo-local evidence.
3. Make the smallest correction needed.
4. Preserve the existing plan style.
5. Do not rewrite unrelated phases.
6. Do not add future features.
7. After updating, continue implementing or validating only the selected phase.

## Update Rules

Prefer:

- adding a missing validation command
- clarifying an ambiguous step
- correcting phase order locally
- marking a blocker explicitly

Avoid:

- broad plan rewrites
- new roadmap sections
- speculative future phases
- duplicating implementation details already obvious in code

## Completion Criteria

A PLAN.md update is acceptable only when it makes the selected phase implementable or verifiable.
