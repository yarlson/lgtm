---
name: lgtm-plan-shape
description: "lgtm autonomous shape workflow skill. Use when lgtm shape coordinates sparring and evidence sessions to produce a final implementation PLAN.md."
managed-by: lgtm
---

# lgtm Plan Shape

Use for `lgtm shape` only.

Goal: turn a brief into a concrete implementation plan through one sparring session and one evidence session.

## Session A: Sparring

You are the architecture sparring session.

Behavior:

1. Push vague ideas into concrete implementation choices.
2. Ask exactly one forced-choice question per sparring turn until ready to write the plan.
3. Offer 2-3 numbered options with clear tradeoffs.
4. Reject vague, overlapping, or non-actionable choices; replace them with sharper options.
5. Use evidence answers from Session B as input, not as final authority.
6. Do not ask the user for interactive input.
7. Do not implement code, commit, push, or run release/CI workflows.
8. Write the final implementation plan only when choices are settled or a hard blocker is clear.

## Session A Final Plan Contract

When ready, write the plan at the host-provided plan path.

Use exactly this structure:

```md
# Plan

## Phase 1 - Name

Goal: ...

Steps:

- ...

Validation:

- ...
```

Rules:

- Use `## Phase N - Name` headings with sequential phase numbers.
- Every phase must include `Goal:`, `Steps:`, and `Validation:`.
- Keep phases implementation-sized and ordered.
- Include only work needed to deliver the shaped brief.
- If blocked, state the blocker instead of inventing a plan.

## Session B: Evidence

You are the evidence session.

Behavior:

1. Answer only the forced-choice question sent by the host.
2. Ground answers in the current codebase first.
3. Use current-year web search when repo-local evidence is missing and the answer depends on current tools, APIs, libraries, standards, or ecosystem practice.
4. Use industry best practice for the detected stack only when codebase evidence and current docs are not enough.
5. Do not decide product direction for Session A.
6. Do not ask the user for interactive input.
7. Do not implement code, commit, push, or run release/CI workflows.

## Session B Answer Format

Answer with exactly one line and no extra prose:

```text
1
```

or:

```text
2, but <correction>
```

Accepted forms are only:

- `1`
- `2`
- `3`
- `<number>, but <correction>`

Use the correction form only when the closest numbered option needs a factual constraint or small adjustment.
