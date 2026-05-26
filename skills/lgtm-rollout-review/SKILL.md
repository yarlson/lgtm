---
name: lgtm-rollout-review
description: "lgtm rollout and operational readiness review skill. Use for selected phases involving deployment, infrastructure, runtime config, migrations, observability, production behavior, or operational failure modes."
managed-by: lgtm
---

# lgtm Rollout Review

Use this when the selected phase affects runtime or production operations.
`PLAN.md` is immutable after `/finish`; record rollout findings, blockers,
verification, and status notes in root-level `PLAN_STATUS.md`, creating it if
it is missing. Use `lgtm-plan-update` only for an exceptional selected-phase
contract defect.

## Trigger Surfaces

Use this for phases involving:

- deployment
- infrastructure
- database migrations
- config or environment variables
- runtime permissions
- observability
- logging, metrics, tracing, or alerts
- rollback behavior
- background jobs or schedulers
- service dependencies
- production failure modes

## Workflow

1. Identify what operational behavior changes.
2. Check required config and defaults.
3. Check startup, shutdown, retry, timeout, and failure behavior where relevant.
4. Check observability: logs, metrics, traces, health checks, or user-visible errors.
5. Check rollback or recovery path.
6. Check migration or deploy ordering if applicable.
7. Check whether docs or runbooks need direct updates.
8. Run available preflight or validation commands.
9. Fix phase-scoped operational gaps.

## Rollout Questions

Know the answer to:

- What changes at runtime?
- What config is required?
- How would failure show up?
- How would an operator verify success?
- How would an operator roll back or recover?
- What is the smallest safe deploy order?

## Guardrails

Do not add production infrastructure unless the selected phase requires it.

Do not invent observability systems.

Do not hardcode toy assumptions into runtime paths.

Do not expand into release automation unless it is part of the selected phase.

Do not edit `PLAN.md` for rollout findings, blockers, verification summaries,
or future-work notes after `/finish`; those notes belong in `PLAN_STATUS.md`.

## Completion Criteria

Rollout review is complete when runtime risk introduced by the selected phase is understood, verified where practical, and documented if needed.
