---
name: lgtm-rollout-review
description: "lgtm rollout + operational readiness review skill. Use for phases involving deployment, infra, runtime config, migrations, observability, production behavior, or operational failure modes."
managed-by: lgtm
---

# lgtm Rollout Review

Use when selected phase affects runtime or production ops.

## Trigger Surfaces

Use for phases involving:

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
2. Check required config + defaults.
3. Check startup, shutdown, retry, timeout, failure behavior where relevant.
4. Check observability: logs, metrics, traces, health checks, user-visible errors.
5. Check rollback or recovery path.
6. Check migration or deploy ordering if applicable.
7. Check whether docs or runbooks need direct updates.
8. Run available preflight or validation commands.
9. Fix phase-scoped operational gaps.

## Rollout Questions

Know answer to:

- What changes at runtime?
- What config required?
- How would failure show up?
- How would operator verify success?
- How would operator roll back or recover?
- Smallest safe deploy order?

## Guardrails

No add production infra unless selected phase requires it.

No invent observability systems.

No hardcode toy assumptions into runtime paths.

No expand into release automation unless part of selected phase.

## Completion Criteria

Rollout review done when runtime risk introduced by selected phase is understood, verified where practical, documented if needed.