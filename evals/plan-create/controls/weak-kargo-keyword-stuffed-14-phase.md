# Plan

## Decisions

- Kargo replaces Jenkins.
- Agents run jobs.
- YAML has DAGs.

## Non-Goals

- No Jenkinsfile compatibility.

## Open Risks

- Migration may be risky.

## Loopholes To Close

- Details can be handled during implementation.

## Phase 1 - Manifest Schema

Goal: Support `.kargo.yml` manifest schema parser diagnostics.

Deliverables:

- Support.

Dependencies:

- None

Unresolved decisions:

- None

Steps:

- Implement support.

Validation:

- Run tests.

## Phase 2 - Jenkins Compatibility

Goal: Handle Jenkins legacy unsupported compatibility migration diagnostic behavior.

Deliverables:

- Support.

Dependencies:

- Phase 1

Unresolved decisions:

- None

Steps:

- Implement support.

Validation:

- Verify it works.

## Phase 3 - Policy Security

Goal: Support policy authorization allowlist trust secret behavior.

Deliverables:

- Support.

Dependencies:

- Phase 1

Unresolved decisions:

- None

Steps:

- Implement support.

Validation:

- Run tests.

## Phase 4 - Persistence

Goal: Add Mongo collection index persistence retention.

Deliverables:

- Support.

Dependencies:

- Phase 1

Unresolved decisions:

- None

Steps:

- Implement support.

Validation:

- Run tests.

## Phase 5 - Scheduler

Goal: Add scheduler state machine lease ready retry cancel.

Deliverables:

- Support.

Dependencies:

- Phase 1

Unresolved decisions:

- None

Steps:

- Implement support.

Validation:

- Verify it works.

## Phase 6 - Protocol

Goal: Add protocol websocket rpc dispatch heartbeat ack.

Deliverables:

- Support.

Dependencies:

- Phase 1

Unresolved decisions:

- None

Steps:

- Implement support.

Validation:

- Run tests.

## Phase 7 - Agent Runtime

Goal: Add agent runner runtime workspace sandbox execution.

Deliverables:

- Support.

Dependencies:

- Phase 1

Unresolved decisions:

- None

Steps:

- Implement support.

Validation:

- Manual QA.

## Phase 8 - Logs Artifacts Checks

Goal: Add log artifact check blob status.

Deliverables:

- Support.

Dependencies:

- Phase 1

Unresolved decisions:

- None

Steps:

- Implement support.

Validation:

- Run tests.

## Phase 9 - Dashboard API

Goal: Add dashboard api operator rerun cancel diagnostic.

Deliverables:

- Support.

Dependencies:

- Phase 1

Unresolved decisions:

- None

Steps:

- Implement support.

Validation:

- Verify it works.

## Phase 10 - Shadow Rollout

Goal: Add shadow fallback rollout feature flag enable.

Deliverables:

- Support.

Dependencies:

- Phase 1

Unresolved decisions:

- None

Steps:

- Implement support.

Validation:

- Run tests.

## Phase 11 - Jenkins Removal

Goal: Add Jenkins removal cutover migration cleanup.

Deliverables:

- Support.

Dependencies:

- Phase 1

Unresolved decisions:

- None

Steps:

- Implement support.

Validation:

- Run tests.

## Phase 12 - Readiness

Goal: Add end-to-end e2e smoke readiness gate.

Deliverables:

- Support.

Dependencies:

- Phase 1

Unresolved decisions:

- None

Steps:

- Implement support.

Validation:

- Manual QA.

## Phase 13 - Tests

Goal: Add tests.

Deliverables:

- Tests.

Dependencies:

- Phase 1

Unresolved decisions:

- None

Steps:

- Add tests.

Validation:

- Run tests.

## Phase 14 - Cleanup

Goal: Clean up.

Deliverables:

- Cleanup.

Dependencies:

- Phase 1

Unresolved decisions:

- None

Steps:

- Clean up.

Validation:

- Verify it works.
