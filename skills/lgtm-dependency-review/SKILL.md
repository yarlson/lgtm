---
name: lgtm-dependency-review
description: "lgtm dependency and supply-chain review skill. Use when a selected phase changes dependencies, lockfiles, package manager config, generated files, CI security config, tool versions, or plugin/MCP/tool installation."
managed-by: lgtm
---

# lgtm Dependency Review

Use this when the selected phase changes dependencies or tool supply chain.
`PLAN.md` is immutable after `/finish`; record dependency review findings,
blockers, verification, and status notes in root-level `PLAN_STATUS.md`,
creating it if it is missing. Use `lgtm-plan-update` only for an exceptional
selected-phase contract defect.

## Trigger Surfaces

Use this for changes to:

- package manifests
- lockfiles
- vendored code
- generated code
- build scripts
- CI workflows that install tools
- Dockerfiles or container images
- MCP servers or plugin config
- tool versions
- dependency update policy
- scripts downloaded from the network

## Workflow

1. Identify every dependency or toolchain change.
2. Check whether the change is required by the selected phase.
3. Confirm lockfiles or equivalent generated dependency state are updated consistently.
4. Prefer pinned versions over floating versions when the repo pattern allows.
5. Watch for `latest`, unpinned Git URLs, curl-to-shell, broad install scripts, or unknown registries.
6. Check for secrets or credentials in package, tool, or CI config.
7. Run dependency-related checks available in the repo.
8. Report out-of-scope supply-chain risks without expanding the phase.

## Guardrails

Do not upgrade unrelated dependencies.

Do not normalize the whole lockfile unless the selected phase requires it.

Do not add scanners or services unless already part of the repo or phase.

Do not trust generated code blindly; inspect whether it is intended to be committed.

Do not edit `PLAN.md` for dependency findings, blockers, verification
summaries, or future-work notes after `/finish`; those notes belong in
`PLAN_STATUS.md`.

## Completion Criteria

Dependency review is complete when dependency/tool changes are necessary, consistent, pinned where appropriate, and verified by available checks.
