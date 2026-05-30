---
name: lgtm-spec-update
description: "lgtm spec update skill. Use only when implementing or validating the selected phase exposes a real product, architecture, or behavior-contract gap that belongs in PLAN.md or a phase-linked project doc."
managed-by: lgtm
---

# lgtm Spec Update

Use only when selected phase exposes real product or architecture decision that must go in `PLAN.md` or phase-linked project doc.

Not for general doc polish.

## Valid Reasons To Update Specs

Update `PLAN.md` or phase-linked project doc only when:

- implementation exposes undefined product behavior
- code and phase plan reveal architecture contradiction
- phase needs decision belonging in product contract
- validation cannot judge correctness without missing contract
- phase-linked contract doc stale in way directly affecting selected phase

## Workflow

1. Identify exact missing or wrong product/architecture contract.
2. Confirm selected phase requires it.
3. Make smallest update to `PLAN.md` or phase-linked project doc.
4. Keep existing doc style and structure.
5. No implementation chatter unless doc already uses that style.
6. Return to selected phase after update.

## Guardrails

No using contract docs as implementation log.

No speculative product features.

No rewriting unrelated contract sections.

No silent product/architecture decisions when correct behavior cannot be inferred from phase, code, or docs. Mark gap clearly instead.

## Completion Criteria

Spec update acceptable only when it clarifies product/architecture contract needed to complete selected phase.