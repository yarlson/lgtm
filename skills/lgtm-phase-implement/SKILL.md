---
name: lgtm-phase-implement
description: "lgtm implementation pass for exactly one PLAN.md phase. Use when lgtm asks Codex to implement a selected phase. Reads PLAN.md, AGENTS.md, and phase-linked context docs, maps relevant files, implements only the selected phase, keeps changes surgical, and verifies before finishing."
managed-by: lgtm
---

# lgtm Phase Implementation

You are implementing exactly one selected phase from a repo-local `PLAN.md`.

## Inputs

lgtm will provide:

- the target phase heading, for example `## Phase 4 - Path And Environment Resolution`
- the path to `PLAN.md`
- the path to `AGENTS.md`

Treat these files as authoritative.

`PLAN.md` is immutable after `/finish`. Do not edit it for ordinary progress,
status, discoveries, or later-phase notes. Keep implementation progress and
closeout notes in root-level `PLAN_STATUS.md`, creating it if it is missing.
Use `lgtm-plan-update` only for an exceptional selected-phase contract defect
that makes `PLAN.md` impossible or unsafe to execute as written.

## Workflow

1. Open `AGENTS.md`, `PLAN.md`, and context docs linked from the selected phase.
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
12. Update `PLAN_STATUS.md` with concise implementation progress, verification, blockers, and current phase status.
13. Before finishing, confirm that the selected phase is complete end to end.

## Scope Rules

Do not add unrelated features, commands, flags, workflows, release automation, CI, configuration, abstractions, or documentation.

Update a phase-linked contract doc only when implementation exposes a real product-contract gap.

Update `PLAN.md` only through `lgtm-plan-update`, and only when the selected phase needs a corrected implementation order, missing validation gate, impossible instruction, or other contract repair.

Do not use `PLAN.md` or phase-linked contract docs as an implementation log; put progress, execution discoveries, verification notes, blockers, and closeout status in `PLAN_STATUS.md`.

Do not commit or push unless explicitly requested.

## Completion Criteria

The implementation pass is complete only when:

- the selected phase's Goal and Steps are satisfied
- required validation commands were run or a blocker is clearly reported
- touched code follows local patterns
- `PLAN_STATUS.md` contains current implementation progress, verification, blockers, or status notes
- no later-phase work was introduced
- no unrelated cleanup was included
