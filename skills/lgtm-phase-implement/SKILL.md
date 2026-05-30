---
name: lgtm-phase-implement
description: "lgtm implementation pass for exactly one PLAN.md phase. Use when lgtm asks Codex to implement a selected phase. Reads PLAN.md, AGENTS.md, and phase-linked context docs, maps relevant files, implements only the selected phase, keeps changes surgical, and verifies before finishing."
managed-by: lgtm
---

# lgtm Phase Implementation

You implement exactly one selected phase from repo-local `PLAN.md`.

## Inputs

lgtm gives:

- target phase heading, e.g. `## Phase 4 - Path And Environment Resolution`
- path to `PLAN.md`
- path to `AGENTS.md`

These files authoritative.

## Workflow

1. Open `AGENTS.md`, `PLAN.md`, context docs linked from selected phase.
2. Find exact selected phase heading in `PLAN.md`.
3. Read only selected phase plus directly referenced sections needed to understand it.
4. Map files relevant to phase before editing.
5. Inspect current patterns in those files and nearby modules.
6. State assumptions only when they affect implementation.
7. Implement only selected phase.
8. No skip ahead to later phases unless selected phase explicitly needs small prerequisite.
9. Keep diff surgical, consistent with existing codebase.
10. Run checks required by `AGENTS.md` and selected phase.
11. Fix failures within selected-phase scope.
12. Before finishing, confirm selected phase complete end to end.

## Scope Rules

No add unrelated features, commands, flags, workflows, release automation, CI, configuration, abstractions, or documentation.

Update `PLAN.md` or phase-linked contract doc only when implementation exposes real product-contract gap.

Update `PLAN.md` only when selected phase needs corrected implementation order or validation gate.

No commit or push unless explicitly requested.

## Completion Criteria

Implementation pass complete only when:

- selected phase Goal and Steps satisfied
- required validation commands run, or blocker clearly reported
- touched code follows local patterns
- no later-phase work introduced
- no unrelated cleanup included