---
name: lgtm-security-review
description: "lgtm focused security review skill. Use when a selected phase touches auth, secrets, shell commands, file IO, network calls, user input, dependency changes, MCP/tool configuration, agent boundaries, or other security-sensitive surfaces."
managed-by: lgtm
---

# lgtm Security Review

Use when selected phase touch security-sensitive behavior.

## Trigger Surfaces

Run review when touched code/config involve:

- auth or authz
- secrets, tokens, credentials, private keys, env vars
- command execution or shell args
- file read, write, paths, archives, uploads, downloads
- network calls, webhooks, callbacks, redirects, user-controlled URLs
- user input parse or interpolation
- database queries
- dependency, package, lockfile, tool changes
- MCP server config, tool config, plugin config, agent tool boundaries
- logs maybe hold sensitive data
- permission, sandbox, approval behavior

## Workflow

1. Find security-sensitive touched surfaces.
2. Trace user/external input to dangerous sinks.
3. Check secrets committed or newly exposed.
4. Check shell commands for injection, quoting, untrusted args.
5. Check file paths for traversal, unintended overwrite, unsafe deletion.
6. Check network calls for SSRF, open redirect, insecure transport, credential leak.
7. Check auth changes for missing checks, privilege escalation, insecure defaults.
8. Check dependency changes for unpinned, unexpected, vulnerable packages where practical.
9. Check MCP/tool config for hardcoded secrets, unsafe args, latest-style pinning, broad permissions.
10. Fix confirmed issues in scope for selected phase.
11. Report out-of-scope risks, no expand implementation.

## Finding Standard

No report speculative issues as confirmed vulns.

Each confirmed finding, know:

- affected file
- vulnerable behavior
- exploit or failure path
- severity
- minimal fix
- verification done

## Guardrails

No broad security rewrites.

No security frameworks unless selected phase need them.

No rotate credentials or modify live services.

No remove test fixtures just for look like secrets unless confirmed unsafe.

## Completion Criteria

Security review done when all touched security-sensitive surfaces checked and confirmed issues in phase scope fixed or clearly reported.