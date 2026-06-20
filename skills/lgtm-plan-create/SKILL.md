---
name: lgtm-plan-create
description: "lgtm planning skill. Use when lgtm start interactive Codex planning session to make final PLAN.md and, when missing, AGENTS.md from user brief + answers."
managed-by: lgtm
---

# lgtm Plan Create

You make repo-local `PLAN.md` for lgtm. If `AGENTS.md` missing, you make it too.

Goal = sharp implementation plan, not brainstorm transcript.

## Workflow

1. Inspect target repo only as needed for better questions.
2. Read `AGENTS.md` if exist, treat as authoritative when present.
3. Ask exactly one sharp question per turn.
4. Ask via normal assistant message only; no `request_user_input` or interactive input tool.
5. Prefer forced choices over open-ended.
6. If answer vague, reject it, ask one narrower follow-up.
7. Keep planning state in Codex session, not draft files.
8. Preserve existing `AGENTS.md`.
9. If `AGENTS.md` missing, detect stack from repo files and web-search current-year best practices for that stack before writing.
10. Write final artifacts only when plan ready to finish.

## PLAN.md Contract

`PLAN.md` = final-only sentinel.

No make `PLAN.md` as draft.

No modify `PLAN.md` while still asking planning questions.

When ready to finish, write full `PLAN.md` using exactly this structure:

```md
# Plan

## Phase 1 - Name

Goal: ...

Steps:

- ...

Validation:

- ...
```

Use `## Phase N - Name` headings with sequential phase numbers.

Every phase must include `Goal:`, `Steps:`, and `Validation:`.

## Completion Criteria

Planning pass complete only when:

- `PLAN.md` exist at requested path and hold final plan.
- `AGENTS.md` exist if it was missing when planning started.

Generated `AGENTS.md` must be practical, repo-local, focused on engineering workflow, coding rules, validation, and safety constraints for detected stack.
