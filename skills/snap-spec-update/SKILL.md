---
name: snap-spec-update
description: "snap-rs DESIGN.md update skill. Use only when implementing or validating the selected phase exposes a real product, architecture, or behavior-contract gap in DESIGN.md."
managed-by: snap-rs
---

# snap-rs Spec Update

Use this only when `DESIGN.md` is missing or contradicting a real product or architecture decision needed by the selected phase.

This skill is not for general documentation polish.

## Valid Reasons To Update DESIGN.md

Update `DESIGN.md` only when:

- implementation exposes an undefined product behavior
- current code and phase plan reveal a design contradiction
- a phase requires a decision that belongs in the product contract
- validation cannot determine correctness without a missing contract
- the design doc is stale in a way directly affecting the selected phase

## Workflow

1. Identify the exact missing or incorrect design contract.
2. Confirm it is required by the selected phase.
3. Make the smallest possible update to `DESIGN.md`.
4. Preserve the existing document style and structure.
5. Avoid implementation chatter unless the doc already uses that style.
6. Return to the selected phase after the update.

## Guardrails

Do not use `DESIGN.md` as an implementation log.

Do not add speculative product features.

Do not rewrite unrelated design sections.

Do not make design decisions silently if the correct product behavior cannot be inferred from the phase, code, or existing docs. Mark the gap clearly instead.

## Completion Criteria

A DESIGN.md update is acceptable only when it clarifies the product or architecture contract needed to complete the selected phase.
