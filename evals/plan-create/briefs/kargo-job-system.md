# Kargo Job System Brief

Kargo currently relies on Jenkins for prenv job execution. For sandboxes we
created agents, and those agents should be reused for prenv jobs so Kargo can
own job execution and remove Jenkins over time.

Design our own Kargo job system in detail:

- reuse the existing agent approach instead of building a new worker stack;
- support full DAG jobs in `.kargo.yml`;
- define the job execution model, persistence, scheduler, protocol, logs,
  artifacts, checks, dashboard, rollout, and Jenkins removal path;
- keep unsupported Jenkins behavior explicit instead of pretending full
  compatibility;
- make the result implementable as a repo-local `PLAN.md`.
