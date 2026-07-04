---
name: lgtm-dependency-review
description: "lgtm dependency and supply-chain review skill. Use when a selected phase changes dependencies, lockfiles, package manager config, generated files, CI security config, tool versions, or plugin/MCP/tool installation."
managed-by: lgtm
---

# lgtm Dependency Review

Use when selected phase change deps or tool supply chain.

## Trigger Surfaces

Use for change to:

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
- scripts downloaded from network

## Workflow

1. Find every dep or toolchain change.
2. Check if change needed by selected phase.
3. Confirm lockfiles or equivalent generated dep state updated consistent.
4. Prefer pinned versions over floating when repo pattern allows.
5. Watch for `latest`, unpinned Git URLs, curl-to-shell, broad install scripts, unknown registries.
6. Check secrets or credentials in package, tool, or CI config.
7. Run dep checks available in repo.
8. Report out-of-scope supply-chain risk, no expand phase.

## Guardrails

No upgrade unrelated deps.

No normalize whole lockfile unless phase need it.

No add scanners or services unless already in repo or phase.

No trust generated code blind; inspect if meant to be committed.

## Completion Criteria

Done when dep/tool change necessary, consistent, pinned where right, verified by available checks.
