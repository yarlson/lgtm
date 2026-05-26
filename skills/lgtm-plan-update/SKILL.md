---
name: lgtm-plan-update
description: "lgtm exceptional PLAN.md repair skill. Use only when implementation or validation proves the selected PLAN.md phase has an incorrect order, missing validation gate, impossible instruction, or incomplete phase contract."
managed-by: lgtm
---

# lgtm PLAN.md Update

Use this only as an exceptional repair path when the current immutable `PLAN.md`
is wrong or incomplete for the selected phase.

This skill is not for documenting progress, recording execution discoveries,
adding nice-to-have tasks, or updating future work. Put ordinary progress,
verification, blockers, and phase status in root-level `PLAN_STATUS.md`.

## Valid Reasons To Update PLAN.md

Update `PLAN.md` only when one of these is true:

- the selected phase cannot be implemented safely as written
- validation proves a required step is missing
- phase order is incorrect
- validation gates are insufficient or impossible
- the phase contradicts a repo-local product or architecture contract
- implementation exposes a prerequisite that must be part of this phase

If the issue can be handled as a progress note, validation result, blocker, or
out-of-scope follow-up, update `PLAN_STATUS.md` instead of `PLAN.md`.

## Workflow

1. Identify the exact phase and exact defect in the plan.
2. Confirm the issue from repo-local evidence.
3. Make the smallest correction needed.
4. Preserve the existing plan style.
5. Do not rewrite unrelated phases.
6. Do not add future features.
7. Record the repair and any resulting blocker or verification note in `PLAN_STATUS.md`.
8. After updating, continue implementing or validating only the selected phase.

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
- progress logs, closeout notes, validation summaries, or future-work notes that belong in `PLAN_STATUS.md`

## Completion Criteria

A PLAN.md update is acceptable only when it repairs the selected phase contract enough to make the phase implementable or verifiable.
