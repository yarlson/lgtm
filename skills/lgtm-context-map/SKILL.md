---
name: lgtm-context-map
description: "lgtm context discovery skill. Use before implement or validate to find files, docs, commands, risks, unknowns, local patterns for selected PLAN.md phase."
managed-by: lgtm
---

# lgtm Context Map

Use before edit or validate selected phase.

Goal: gather enough local context to work safe without read whole repo.

## Workflow

1. Read selected `PLAN.md` phase.
2. Read `AGENTS.md` for repo instructions.
3. Read context docs linked from phase.
4. Search files, modules, tests, commands, docs, config for phase.
5. Inspect nearby code patterns before decide how to implement or validate.
6. Find unknowns that affect implementation correctness.
7. Resolve discoverable unknowns via repo-local files, config, tests, or installed tool versions.
8. Use official docs only when local evidence not enough for unfamiliar or version-sensitive behavior.

## Output To Keep In Working Memory

Before proceed, know:

- relevant source files
- relevant tests
- relevant commands
- local conventions to follow
- likely risk areas
- implementation assumptions, if any
- validation evidence needed

## Guardrails

No turn context mapping into broad doc work.

No inspect generated output, build artifacts, vendored deps, or unrelated modules unless phase needs it.

No ask user for file locations findable locally.

## Completion Criteria

Context mapping done when you can explain:

- what files you need to touch
- what files you need to verify
- what repo conventions constrain change
- what risks or unknowns remain
