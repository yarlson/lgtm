# Plan

## Decisions

- Parse `.kargo.yml` with a versioned schema before scheduling any job.
- Preserve Jenkins compatibility through explicit diagnostics until cutover.
- Store job state in Mongo with indexes that support scheduler and dashboard reads.
- Treat agent protocol, runtime execution, and artifact status as separate contracts.

## Non-Goals

- Do not build a Jenkins replacement UI.
- Do not silently accept unsupported Jenkins compatibility behavior.
- Do not remove Jenkins compatibility until the shadow rollout proves readiness.

## Open Risks

- Migration diagnostics may expose legacy Jenkins jobs that cannot map cleanly.
- Scheduler lease behavior can create duplicate execution if retry state is unclear.
- Agent runtime sandbox failures can look like protocol failures without distinct logs.
- Dashboard rerun and cancel actions can drift from backend authorization policy.

## Loopholes To Close

- Define exact unsupported Jenkins fields before enabling migration diagnostics.
- Make artifact blob status observable before dashboard actions are enabled.
- Keep the shadow rollout feature flag reversible until end-to-end readiness passes.

## Phase 1 - Kargo Manifest Schema Parser

Goal:
Add a versioned `.kargo.yml` manifest schema parser with diagnostics.

Deliverables:
- Define manifest schema structs for job name, command, agent requirements, and artifact declarations.
- Implement parser diagnostics for unknown fields, invalid types, and missing required keys.
- Add repository fixtures for valid and invalid `.kargo.yml` manifests.

Dependencies:
- None.

Unresolved decisions:
- None.

Steps:
- Add schema types and parser entrypoints.
- Wire diagnostic rendering for parser failures.
- Add fixture-based parser tests.

Validation:
- Run unit tests for manifest schema parser fixtures.
- Run a command-level parser check against valid and invalid repository fixtures.

## Phase 2 - Jenkins Legacy Compatibility Diagnostics

Goal:
Expose unsupported Jenkins legacy compatibility cases before scheduling migration jobs.

Deliverables:
- Map Jenkins job fields that can be migrated to Kargo manifest fields.
- Emit migration diagnostic errors for unsupported Jenkins plugins and legacy triggers.
- Add compatibility fixture coverage for supported and unsupported Jenkins jobs.

Dependencies:
- Phase 1 manifest parser.

Unresolved decisions:
- None.

Steps:
- Add Jenkins compatibility inspection module.
- Convert supported fields into manifest diagnostics.
- Add unsupported plugin and trigger fixtures.

Validation:
- Run compatibility fixture tests for Jenkins supported and unsupported jobs.
- Assert migration diagnostic messages include the unsupported legacy field names.

## Phase 3 - Policy Authorization And Secret Trust

Goal:
Enforce policy authorization, allowlist trust, and secret handling before jobs run.

Deliverables:
- Add policy checks for repository allowlist and trusted agent labels.
- Reject manifests that reference secrets outside the authorized trust boundary.
- Record authorization decisions for later dashboard diagnostics.

Dependencies:
- Phase 1 manifest parser.

Unresolved decisions:
- None.

Steps:
- Implement policy evaluation over parsed manifests.
- Add allowlist and secret reference validation.
- Surface policy diagnostics through the parser result.

Validation:
- Run policy unit tests for allowlist, authorization, trust, and secret cases.
- Run integration fixture checks for accepted and rejected manifests.

## Phase 4 - Mongo Persistence Model

Goal:
Persist Kargo job state in Mongo collections with indexes and retention rules.

Deliverables:
- Add Mongo collection models for job, run, artifact, and agent heartbeat records.
- Define indexes for scheduler queries, dashboard reads, and retention cleanup.
- Add migration or initialization checks for required indexes.

Dependencies:
- Phase 1 manifest parser.
- Phase 3 policy authorization.

Unresolved decisions:
- None.

Steps:
- Define persistence model structs and collection names.
- Add index creation logic with idempotent startup checks.
- Add retention metadata to stored run records.

Validation:
- Run repository integration tests against a Mongo fixture or test container.
- Assert expected collection indexes and retention fields are created.

## Phase 5 - Scheduler State Machine And Leases

Goal:
Implement scheduler state machine transitions with leases, ready state, retry, and cancel handling.

Deliverables:
- Add job states for queued, ready, leased, running, retrying, canceling, canceled, failed, and succeeded.
- Implement lease acquisition and expiry rules.
- Add retry and cancel transition checks.

Dependencies:
- Phase 4 Mongo persistence model.

Unresolved decisions:
- None.

Steps:
- Add scheduler state machine transition functions.
- Persist lease owner and expiry values.
- Add retry and cancel transition tests.

Validation:
- Run scheduler state machine unit tests.
- Run integration tests proving lease expiry returns jobs to ready state.

## Phase 6 - Agent WebSocket RPC Protocol

Goal:
Define the agent protocol for WebSocket RPC dispatch, heartbeat, and ack handling.

Deliverables:
- Add protocol messages for dispatch, heartbeat, ack, log status, and artifact status.
- Validate inbound WebSocket RPC payloads against the protocol contract.
- Add timeout handling for missing heartbeat and missing ack events.

Dependencies:
- Phase 5 scheduler state machine.

Unresolved decisions:
- None.

Steps:
- Define protocol structs and JSON encoding.
- Add WebSocket dispatch and heartbeat handling.
- Add ack timeout paths that update scheduler state.

Validation:
- Run protocol serialization and deserialization tests.
- Run WebSocket fixture tests for dispatch, heartbeat, ack, and timeout cases.

## Phase 7 - Agent Runner Runtime Sandbox

Goal:
Execute dispatched jobs in the agent runner runtime with workspace sandbox isolation.

Deliverables:
- Add agent runner workspace setup and command execution.
- Isolate runtime environment variables and filesystem paths.
- Report execution status back through the protocol.

Dependencies:
- Phase 6 agent WebSocket RPC protocol.

Unresolved decisions:
- None.

Steps:
- Implement workspace preparation and cleanup.
- Run commands inside the sandboxed execution directory.
- Send status updates for started, completed, and failed executions.

Validation:
- Run agent runtime tests for workspace creation and command execution.
- Assert sandbox tests do not write outside the workspace fixture.

## Phase 8 - Logs Artifacts And Check Status

Goal:
Capture logs, artifact blobs, and check status updates for every job run.

Deliverables:
- Store log chunks and artifact blob metadata.
- Link check status updates to run and artifact records.
- Add artifact upload failure handling.

Dependencies:
- Phase 4 Mongo persistence model.
- Phase 7 agent runner runtime sandbox.

Unresolved decisions:
- None.

Steps:
- Add log and artifact persistence paths.
- Wire runtime status updates to artifact and check records.
- Add blob status diagnostics for failed uploads.

Validation:
- Run artifact persistence integration tests.
- Assert log, artifact, check, blob, and status records are visible after a fixture run.

## Phase 9 - Dashboard Operator API

Goal:
Expose dashboard API endpoints for operator diagnostics, rerun, and cancel actions.

Deliverables:
- Add dashboard API reads for job, run, artifact, and diagnostic records.
- Add authorized operator rerun and cancel endpoints.
- Return policy and migration diagnostics in API responses.

Dependencies:
- Phase 3 policy authorization.
- Phase 8 logs artifacts and check status.

Unresolved decisions:
- None.

Steps:
- Implement dashboard read endpoints over persisted records.
- Add operator authorization checks for rerun and cancel.
- Add API response fixtures for diagnostics.

Validation:
- Run dashboard API contract tests for read, rerun, cancel, and diagnostic responses.
- Assert unauthorized operator actions fail with policy diagnostics.

## Phase 10 - Shadow Rollout Feature Flag

Goal:
Enable shadow rollout with fallback behavior behind a reversible feature flag.

Deliverables:
- Add feature flag checks for Kargo job execution.
- Run Jenkins fallback when shadow execution is disabled or fails readiness checks.
- Record shadow comparison status for operators.

Dependencies:
- Phase 9 dashboard operator API.

Unresolved decisions:
- None.

Steps:
- Add feature flag read path to scheduler decisions.
- Implement fallback routing for disabled or failed shadow runs.
- Record comparison results for dashboard visibility.

Validation:
- Run rollout tests for enabled, disabled, fallback, and comparison status cases.
- Assert the feature flag can disable Kargo execution without code changes.

## Phase 11 - Jenkins Removal Cutover

Goal:
Remove Jenkins execution after migration cutover while preserving cleanup diagnostics.

Deliverables:
- Add cutover checks that prove no active jobs require Jenkins.
- Remove Jenkins fallback execution paths after cutover.
- Keep migration cleanup diagnostics for archived legacy jobs.

Dependencies:
- Phase 10 shadow rollout feature flag.

Unresolved decisions:
- None.

Steps:
- Add cutover readiness checks.
- Remove Jenkins fallback execution branches.
- Keep archived migration diagnostic reporting.

Validation:
- Run cutover tests proving Jenkins removal is blocked until readiness checks pass.
- Run cleanup diagnostics tests for archived Jenkins jobs.

## Phase 12 - End-To-End Readiness Gate

Goal:
Add end-to-end readiness gates and smoke checks for Kargo job execution.

Deliverables:
- Add e2e smoke that parses a manifest, schedules a job, dispatches an agent, captures artifacts, and reports dashboard status.
- Add readiness gate checks for policy, protocol, runtime, persistence, dashboard, and rollout status.
- Publish operator-facing failure diagnostics.

Dependencies:
- Phase 11 Jenkins removal cutover.

Unresolved decisions:
- None.

Steps:
- Build the end-to-end fixture repository.
- Wire smoke execution through scheduler, agent, artifact, and dashboard paths.
- Add readiness gate reporting for each subsystem.

Validation:
- Run end-to-end smoke tests against the fixture repository.
- Assert readiness gate output includes e2e, smoke, readiness, and gate status for every subsystem.
