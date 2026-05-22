---
name: snap-docs-drift-review
description: "snap-rs documentation drift review skill. Use when implementation or validation may affect README, AGENTS.md, DESIGN.md, PLAN.md, API docs, operational docs, or other repo-local documentation."
managed-by: snap-rs
---

# snap-rs Docs Drift Review

Use this when touched behavior may make documentation stale.

The goal is to update only directly affected docs and avoid stale parallel documentation.

## Workflow

1. Identify behavior, commands, config, APIs, workflows, or contracts changed by the selected phase.
2. Search for repo-local docs that describe those areas.
3. Compare docs against actual implementation.
4. Update only docs directly affected by this phase.
5. Preserve the repo's documentation style.
6. Prefer canonical docs over adding new parallel docs.
7. Do not move product contract out of `DESIGN.md` if that is the established source.
8. Do not put implementation-plan details into user-facing docs unless the repo already does that.

## Docs To Consider

Depending on the repo, check:

- `README.md`
- `AGENTS.md`
- `DESIGN.md`
- `PLAN.md`
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

## Completion Criteria

Docs drift review is complete when directly affected documentation is correct, or no docs update is needed and that conclusion is supported by inspection.
