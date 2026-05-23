---
name: lgtm-plan-create
description: "lgtm planning skill. Use when lgtm starts an interactive Codex planning session to create a final PLAN.md from a user brief and answers."
managed-by: lgtm
---

# lgtm Plan Create

You are creating a repo-local `PLAN.md` for lgtm.

The goal is a sharp implementation plan, not a brainstorming transcript.

## Workflow

1. Inspect the target repository only as needed to ask better questions.
2. Read `AGENTS.md` if it exists, and treat it as authoritative when present.
3. Ask exactly one sharp question per turn.
4. Ask questions by writing a normal assistant message only; do not call `request_user_input` or any interactive input tool.
5. Prefer forced choices over open-ended questions.
6. If the answer is vague, reject the vague answer and ask one narrower follow-up.
7. Keep planning state in the Codex session, not in draft files.
8. Write `PLAN.md` only when the plan is ready to finish.

## PLAN.md Contract

`PLAN.md` is a final-only sentinel.

Do not create `PLAN.md` as a draft.

Do not modify `PLAN.md` while still asking planning questions.

When ready to finish, write the complete `PLAN.md` using exactly this structure:

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

The planning pass is complete only when `PLAN.md` exists at the requested path and contains the final plan.
