---
name: lgtm-context-map
description: "lgtm context discovery skill. Use before implementation or validation to identify files, docs, commands, risks, unknowns, and local patterns relevant to the selected PLAN.md phase."
managed-by: lgtm
---

# lgtm Context Map

Use this before editing or validating a selected phase.

The goal is to gather enough local context to work safely without reading the whole repository.

## Workflow

1. Read the selected `PLAN.md` phase.
2. Read `AGENTS.md` for repo instructions.
3. Read context docs linked from the selected phase.
4. Search for files, modules, tests, commands, docs, and config relevant to the selected phase.
5. Inspect nearby code patterns before deciding how to implement or validate.
6. Identify unknowns that affect implementation correctness.
7. Resolve discoverable unknowns through repo-local files, config, tests, or installed tool versions.
8. Use official docs only when local evidence is insufficient for unfamiliar or version-sensitive behavior.

## Output To Keep In Working Memory

Before proceeding, know:

- relevant source files
- relevant tests
- relevant commands
- local conventions to follow
- likely risk areas
- implementation assumptions, if any
- validation evidence needed

## Guardrails

Do not turn context mapping into broad documentation work.

Do not inspect generated output, build artifacts, vendored dependencies, or unrelated modules unless the selected phase requires it.

Do not ask the user for file locations that can be discovered locally.

## Completion Criteria

Context mapping is complete when you can explain:

- what files you need to touch
- what files you need to verify
- what repo conventions constrain the change
- what risks or unknowns remain
