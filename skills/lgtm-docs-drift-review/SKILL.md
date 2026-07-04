---
name: lgtm-docs-drift-review
description: "lgtm documentation drift review skill. Use when implementation or validation may affect README, AGENTS.md, PLAN.md, phase-linked project docs, API docs, operational docs, or other repo-local documentation."
managed-by: lgtm
---

# lgtm Docs Drift Review

Use when changed behavior make docs stale.

Goal: update only directly affected docs, avoid stale parallel docs.

## Workflow

1. Find behavior, commands, config, APIs, workflows, contracts changed by phase.
2. Search repo-local docs describing those areas.
3. Compare docs vs implementation.
4. Update only docs directly affected by phase.
5. Keep repo doc style.
6. Prefer canonical docs over new parallel docs.
7. Keep product-contract changes in `PLAN.md` or docs linked from phase.
8. No implementation-plan details in user-facing docs unless repo already does.

## Docs To Consider

Per repo, check:

- `README.md`
- `AGENTS.md`
- `PLAN.md`
- docs linked from phase
- docs under `docs/`
- command help text
- API or schema docs
- examples
- configuration templates

## Guardrails

No docs for unchanged behavior.

No doc rewrites for style alone.

No new documentation system.

No burying product-contract changes in chat only.

## Completion Criteria

Done when directly affected docs correct, or no update needed and inspection supports that.
