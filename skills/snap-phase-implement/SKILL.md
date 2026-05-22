---
name: snap-phase-implement
description: "snap-rs implementation pass for exactly one PLAN.md phase. Use when snap-rs asks Codex to implement a selected phase. Reads PLAN.md, AGENTS.md, and DESIGN.md, maps relevant files, implements only the selected phase, keeps changes surgical, and verifies before finishing."
managed-by: snap-rs
---

# snap-rs Phase Implementation

You are implementing exactly one selected phase from a repo-local `PLAN.md`.

## Inputs

snap-rs will provide:

- the target phase heading, for example `## Phase 4 - Path And Environment Resolution`
- the path to `PLAN.md`
- the path to `AGENTS.md`
- the path to `DESIGN.md`

Treat these files as authoritative.

## Workflow

1. Open `AGENTS.md`, `DESIGN.md`, and `PLAN.md`.
2. Locate the exact selected phase heading in `PLAN.md`.
3. Read only the selected phase plus any directly referenced sections needed to understand it.
4. Map the files relevant to this phase before editing.
5. Inspect current implementation patterns in those files and nearby modules.
6. State assumptions only when they affect implementation.
7. Implement only the selected phase.
8. Do not skip ahead into later phases unless the selected phase explicitly requires a small prerequisite.
9. Keep the diff surgical and consistent with the existing codebase.
10. Run the checks required by `AGENTS.md` and the selected phase.
11. Fix failures within selected-phase scope.
12. Before finishing, confirm that the selected phase is complete end to end.

## Scope Rules

Do not add unrelated features, commands, flags, workflows, release automation, CI, configuration, abstractions, or documentation.

Update `DESIGN.md` only when implementation exposes a real product-contract gap.

Update `PLAN.md` only when the selected phase needs a corrected implementation order or validation gate.

Do not commit or push unless explicitly requested.

## Completion Criteria

The implementation pass is complete only when:

- the selected phase's Goal and Steps are satisfied
- required validation commands were run or a blocker is clearly reported
- touched code follows local patterns
- no later-phase work was introduced
- no unrelated cleanup was included
