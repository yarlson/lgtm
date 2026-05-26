---
name: lgtm-docs-drift-review
description: "lgtm documentation drift review skill. Use when implementation or validation may affect README, AGENTS.md, phase-linked project docs, API docs, operational docs, PLAN.md contract defects, or other repo-local documentation."
managed-by: lgtm
---

# lgtm Docs Drift Review

Use this when touched behavior may make documentation stale.

The goal is to update only directly affected docs and avoid stale parallel documentation.
`PLAN.md` is immutable after `/finish`; ordinary progress, execution
discoveries, verification notes, blockers, and phase closeout belong in
root-level `PLAN_STATUS.md`.

## Workflow

1. Identify behavior, commands, config, APIs, workflows, or contracts changed by the selected phase.
2. Search for repo-local docs that describe those areas.
3. Compare docs against actual implementation.
4. Update only docs directly affected by this phase.
5. Preserve the repo's documentation style.
6. Prefer canonical docs over adding new parallel docs.
7. Keep product-contract changes in docs explicitly linked from the selected phase, or use `lgtm-plan-update` only for an exceptional selected-phase `PLAN.md` contract defect.
8. Put selected-phase progress, verification, blockers, closeout, and execution discoveries in `PLAN_STATUS.md`, not in user-facing docs or `PLAN.md`.
9. Do not put implementation-plan details into user-facing docs unless the repo already does that.

## Docs To Consider

Depending on the repo, check:

- `README.md`
- `AGENTS.md`
- `PLAN.md` only when `lgtm-plan-update` is required for a selected-phase contract defect
- `PLAN_STATUS.md` for progress, verification, blockers, and phase status
- docs linked from the selected phase
- docs under `docs/`
- command help text
- API or schema docs
- examples
- configuration templates

## Guardrails

Do not add docs for unchanged behavior.

Do not rewrite docs for style alone.

Do not create a new documentation system.

Do not bury product-contract changes in chat only.

Do not edit `PLAN.md` for ordinary docs drift, progress notes, or future-work
ideas after `/finish`.

## Completion Criteria

Docs drift review is complete when directly affected documentation is correct, or no docs update is needed and that conclusion is supported by inspection.
