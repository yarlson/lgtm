---
name: snap-security-review
description: "snap-rs focused security review skill. Use when a selected phase touches auth, secrets, shell commands, file IO, network calls, user input, dependency changes, MCP/tool configuration, agent boundaries, or other security-sensitive surfaces."
managed-by: snap-rs
---

# snap-rs Security Review

Use this when the selected phase touches security-sensitive behavior.

## Trigger Surfaces

Run this review when touched code or config involves:

- authentication or authorization
- secrets, tokens, credentials, private keys, or environment variables
- command execution or shell arguments
- file reads, writes, paths, archives, uploads, or downloads
- network calls, webhooks, callbacks, redirects, or user-controlled URLs
- user input parsing or interpolation
- database queries
- dependency, package, lockfile, or tool changes
- MCP server config, tool config, plugin config, or agent tool boundaries
- logs that may contain sensitive data
- permission, sandbox, or approval behavior

## Workflow

1. Identify security-sensitive touched surfaces.
2. Trace user-controlled or external input to dangerous sinks.
3. Check for secrets committed or newly exposed.
4. Check shell commands for injection, quoting, and untrusted arguments.
5. Check file paths for traversal, unintended overwrite, and unsafe deletion.
6. Check network calls for SSRF, open redirect, insecure transport, and credential leakage.
7. Check auth changes for missing checks, privilege escalation, and insecure defaults.
8. Check dependency changes for unpinned, unexpected, or vulnerable packages where practical.
9. Check MCP/tool config for hardcoded secrets, unsafe args, latest-style pinning, and broad permissions.
10. Fix confirmed issues that are in scope for the selected phase.
11. Report out-of-scope risks without expanding the implementation.

## Finding Standard

Do not report speculative issues as confirmed vulnerabilities.

For each confirmed finding, know:

- affected file
- vulnerable behavior
- exploit or failure path
- severity
- minimal fix
- verification performed

## Guardrails

Do not perform broad security rewrites.

Do not introduce security frameworks unless the selected phase requires them.

Do not rotate credentials or modify live services.

Do not remove test fixtures just because they look like secrets unless confirmed unsafe.

## Completion Criteria

Security review is complete when all touched security-sensitive surfaces have been checked and confirmed issues within phase scope are fixed or clearly reported.
