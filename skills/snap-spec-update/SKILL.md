---
name: snap-spec-update
description: "snap-rs spec update skill. Use only when implementing or validating the selected phase exposes a real product, architecture, or behavior-contract gap that belongs in PLAN.md or a phase-linked project doc."
managed-by: snap-rs
---

# snap-rs Spec Update

Use this only when the selected phase exposes a real product or architecture decision that must be recorded in `PLAN.md` or a phase-linked project doc.

This skill is not for general documentation polish.

## Valid Reasons To Update Specs

Update `PLAN.md` or a phase-linked project doc only when:

- implementation exposes an undefined product behavior
- current code and phase plan reveal an architecture contradiction
- a phase requires a decision that belongs in the product contract
- validation cannot determine correctness without a missing contract
- a phase-linked contract doc is stale in a way directly affecting the selected phase

## Workflow

1. Identify the exact missing or incorrect product or architecture contract.
2. Confirm it is required by the selected phase.
3. Make the smallest possible update to `PLAN.md` or the phase-linked project doc.
4. Preserve the existing document style and structure.
5. Avoid implementation chatter unless the doc already uses that style.
6. Return to the selected phase after the update.

## Guardrails

Do not use contract docs as an implementation log.

Do not add speculative product features.

Do not rewrite unrelated contract sections.

Do not make product or architecture decisions silently if the correct behavior cannot be inferred from the phase, code, or existing docs. Mark the gap clearly instead.

## Completion Criteria

A spec update is acceptable only when it clarifies the product or architecture contract needed to complete the selected phase.
