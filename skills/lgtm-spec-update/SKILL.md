---
name: lgtm-spec-update
description: "lgtm spec update skill. Use only when implementing or validating the selected phase exposes a real product, architecture, or behavior-contract gap that belongs in a phase-linked project doc or exceptional PLAN.md repair."
managed-by: lgtm
---

# lgtm Spec Update

Use this only when the selected phase exposes a real product or architecture
decision that must be recorded in a phase-linked project doc, or in `PLAN.md`
only through an exceptional `lgtm-plan-update` repair.

This skill is not for general documentation polish, implementation progress, or
execution notes. Put ordinary progress, verification, blockers, and phase status
in root-level `PLAN_STATUS.md`.

## Valid Reasons To Update Specs

Update a phase-linked project doc, or use `lgtm-plan-update` for an exceptional
`PLAN.md` repair, only when:

- implementation exposes an undefined product behavior
- current code and phase plan reveal an architecture contradiction
- a phase requires a decision that belongs in the product contract
- validation cannot determine correctness without a missing contract
- a phase-linked contract doc is stale in a way directly affecting the selected phase

If the discovery is only about what happened during execution, what remains
blocked, what was verified, or what might be useful future work, update
`PLAN_STATUS.md` instead.

## Workflow

1. Identify the exact missing or incorrect product or architecture contract.
2. Confirm it is required by the selected phase.
3. Make the smallest possible update to the phase-linked project doc, or use `lgtm-plan-update` for the smallest possible `PLAN.md` repair.
4. Preserve the existing document style and structure.
5. Avoid implementation chatter unless the doc already uses that style.
6. Return to the selected phase after the update.

## Guardrails

Do not use contract docs as an implementation log.

Do not use `PLAN.md` for progress notes or ordinary execution discoveries after
`/finish`; use `PLAN_STATUS.md` for those notes.

Do not add speculative product features.

Do not rewrite unrelated contract sections.

Do not make product or architecture decisions silently if the correct behavior cannot be inferred from the phase, code, or existing docs. Mark the gap clearly instead.

## Completion Criteria

A spec update is acceptable only when it clarifies the product or architecture contract needed to complete the selected phase. Progress, verification, blockers, and closeout notes belong in `PLAN_STATUS.md`.
