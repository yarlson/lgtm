---
name: lgtm-plan-create
description: "lgtm planning skill. Use when lgtm start interactive Codex planning session to make final PLAN.md and, when missing, AGENTS.md from user brief + answers."
managed-by: lgtm
---

# lgtm Plan Create

You make repo-local `PLAN.md` for lgtm. If `AGENTS.md` missing, you make it too.

Goal = sharp implementation plan, not brainstorm transcript or roadmap summary.

## Workflow

1. Inspect target repo only as needed for better questions.
2. Read `AGENTS.md` if exist, treat as authoritative when present.
3. Ask exactly one sharp question per turn until the plan can be implemented
   without guessing.
4. Ask via normal assistant message only; no `request_user_input` or
   interactive input tool.
5. Prefer forced choices over open-ended.
6. If answer vague, reject it, ask one narrower follow-up.
7. Keep an explicit decision log in session memory.
8. Keep planning state in Codex session, not draft files.
9. Preserve existing `AGENTS.md`.
10. If `AGENTS.md` missing, detect stack from repo files and web-search
    current-year best practices for that stack before writing.
11. For broad product, platform, migration, UX/UI, or architecture work, keep
    asking as many questions as needed; do not optimize for a short question
    count.
12. Before writing the plan, lock the source inputs, ownership boundaries,
    runtime model, data/config model, persistence, security/trust boundaries,
    rollout order, validation gates, non-goals, risks, loopholes, and
    unresolved decisions.
13. Write final artifacts only when plan ready to finish.

## PLAN.md Contract

`PLAN.md` = final-only sentinel.

Do not create `PLAN.md` as a draft.

Do not modify `PLAN.md` while still asking planning questions.

When ready to finish, write full `PLAN.md` using at least this structure:

```md
# Plan

## Decisions

- ...

## Non-Goals

- ...

## Open Risks

- ...

## Loopholes To Close

- ...

## Phase 1 - Name

Goal: ...

Deliverables:

- ...

Dependencies:

- ...

Unresolved decisions:

- ...

Steps:

- ...

Validation:

- ...
```

Use `## Phase N - Name` headings with sequential phase numbers.

Include `## Decisions`, `## Non-Goals`, `## Open Risks`, and
`## Loopholes To Close` before phase sections.

Every phase must include `Goal:`, `Deliverables:`, `Dependencies:`,
`Unresolved decisions:`, `Steps:`, and `Validation:`.

## Plan Quality Bar

Plan phases must be implementation-sized, not umbrella roadmap buckets.

For large tasks, prefer phases that follow real implementation boundaries:
ownership, data model, runtime boundary, dependency order, rollout risk, and
validation method. Broad product, platform, migration, UX/UI, or architecture
work often needs many phases, but do not target a fixed phase count. Do not
compress unrelated workstreams into broad phases just to look concise.

Treat a brief as broad when it replaces an external system, introduces a new
runtime, changes agent/worker execution, adds a config schema, creates storage
or persistence, changes security/trust boundaries, adds dashboards/APIs, or
needs staged rollout. For broad work, split separate phases for the relevant
families instead of merging them:

- repo/context discovery and compatibility gates;
- schema, parser, and diagnostics;
- policy, authorization, and trust boundaries;
- persistence models, indexes, and migrations;
- state machine and scheduler/core runtime;
- protocol/API contracts;
- worker/agent/runtime implementation;
- secrets, isolation, and resource enforcement;
- logs, artifacts, checks, audit, and observability;
- dashboard/UI and operator actions;
- shadow mode, fallback, and rollout controls;
- migration, cleanup, and removal of legacy paths;
- end-to-end smoke and release/readiness gates.

Each phase must:

- name the concrete subsystem, file area, API, model, UI surface, migration,
  or test layer it changes;
- state the contract or behavior it establishes;
- list concrete deliverables;
- list dependencies on earlier phases or say `None`;
- list unresolved decisions or say `None`;
- list ordered implementation steps specific enough for another agent to
  execute;
- include validation that proves the phase works, not just generic "add tests";
- keep rollout, compatibility, data migration, observability, docs, and cleanup
  as separate phases when they carry different risk.

Split a phase when it spans multiple layers, mixes product decisions with
implementation, combines infra/UI/docs/tests as one blob, depends on unresolved
research, or cannot be validated without later phases.

Reject or continue questioning instead of writing a plan when phases would read
like:

- "Build backend";
- "Add UI";
- "Wire everything";
- "Add tests";
- "Roll out";
- "Clean up".

Also reject vague verbs without concrete targets: "improve", "support",
"handle", "integrate", "make robust", "wire up", "polish", or "finish".

Validation must name concrete checks: exact repo commands when known, test
files or test names when discoverable, manual smoke evidence only when
automated checks are unavailable, and docs/config checks when behavior depends
on docs or runtime setup. "Run tests", "verify it works", and "manual QA" are
not enough by themselves.

If the user asks to finish while important decisions remain unresolved, write
the best detailed plan possible and mark unresolved decisions explicitly in the
relevant phase goals or risk notes. Do not hide unknowns behind vague wording.

## Completion Criteria

Planning pass complete only when:

- `PLAN.md` exist at requested path and hold final plan.
- `AGENTS.md` exist if it was missing when planning started.

Generated `AGENTS.md` must be practical, repo-local, focused on engineering
workflow, coding rules, validation, and safety constraints for detected stack.
