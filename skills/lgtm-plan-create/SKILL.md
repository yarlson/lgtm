---
name: lgtm-plan-create
description: "lgtm planning skill. Use when lgtm starts an interactive Codex planning session to create a final PLAN.md and, when missing, AGENTS.md from a user brief and answers."
managed-by: lgtm
---

# lgtm Plan Create

You are creating a repo-local `PLAN.md` for lgtm. If `AGENTS.md` is missing,
you are also creating it.

The goal is a sharp implementation plan, not a brainstorming transcript.

## Workflow

1. Inspect the target repository only as needed to ask better questions.
2. Read `AGENTS.md` if it exists, and treat it as authoritative when present.
3. Ask exactly one sharp question per turn.
4. Ask questions by writing a normal assistant message only; do not call `request_user_input` or any interactive input tool.
5. Prefer forced choices over open-ended questions.
6. If the answer is vague, reject the vague answer and ask one narrower follow-up.
7. Keep planning state in the Codex session, not in draft files.
8. Preserve an existing `AGENTS.md`.
9. If `AGENTS.md` is missing, detect the project stack from repo files and
   web-search current-year best practices for that stack before writing it.
10. Write final artifacts only when the plan is ready to finish.

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

The planning pass is complete only when:

- `PLAN.md` exists at the requested path and contains the final plan.
- `AGENTS.md` exists if it was missing when planning started.

Generated `AGENTS.md` must be practical, repo-local, and focused on engineering
workflow, coding rules, validation, and safety constraints for the detected
stack.
