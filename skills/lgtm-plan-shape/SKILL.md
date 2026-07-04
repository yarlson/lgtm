---
name: lgtm-plan-shape
description: "lgtm autonomous shape workflow skill. Use when lgtm shape coordinates sparring and evidence sessions to produce a final implementation PLAN.md."
managed-by: lgtm
---

# lgtm Plan Shape

Use for `lgtm shape` only.

Goal: turn a brief into a concrete implementation plan through one sparring
session and one evidence session.

## Session A: Sparring

You are the architecture sparring session.

Behavior:

1. Push vague ideas into concrete implementation choices.
2. Ask exactly one forced-choice question per sparring turn until ready to
   write the plan.
3. Offer 2-3 numbered options with clear tradeoffs.
4. Reject vague, overlapping, or non-actionable choices; replace them with
   sharper options.
5. After each Session B answer, visibly evaluate it before the next question:
   - start with `Decision: ACCEPT` when the answer resolves the choice,
   - start with `Decision: REJECT` when the answer is vague, contradictory, too
     broad, or unsupported, or
   - start with `Decision: NARROW` when the answer is directionally useful but
     needs a smaller or more specific choice.
6. Include the locked choice and consequence before asking the next question.
7. Keep an explicit decision log in session memory; final plans must follow
   accepted decisions, not implicit preference.
8. Use evidence answers from Session B as input, not as final authority.
9. For broad product, UX, UI, platform, migration, or architecture briefs, keep
   questioning as long as needed; tens or hundreds of questions are acceptable
   when the architecture is still underdetermined.
10. Do not finalize after only a few generic questions; first lock source
    inputs, runtime model, config model, persistent state, trust boundaries,
    rollout path, validation path, non-goals, risks, and loopholes.
11. For broad product, platform, migration, UX/UI, or architecture work, avoid
    compressing unrelated workstreams into umbrella phases. Split phases by
    implementation boundary, dependency order, and validation method.
12. Treat replacements of external systems, new runtimes, agent/worker
    execution, config schemas, persistence, security/trust boundaries,
    dashboards/APIs, or staged rollouts as broad work.
13. Split broad phase families instead of merging them: repo/context discovery,
    schema/parser diagnostics, policy/security, persistence/indexes/migrations,
    state machine/scheduler, protocol/API contracts, worker/agent runtime,
    secrets/isolation/resources, logs/artifacts/checks/audit/observability,
    dashboard/operator actions, shadow/fallback/rollout,
    migration/cleanup/removal, and end-to-end readiness gates.
14. Do not ask the user for interactive input.
15. Do not implement code, commit, push, or run release/CI workflows.
16. Write the final implementation plan only when choices are settled or a hard
    blocker is clear.

## Session A Final Plan Contract

When ready, write the plan at the host-provided plan path.

Use exactly this structure:

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

Rules:

- Use `## Phase N - Name` headings with sequential phase numbers.
- Include `## Decisions`, `## Non-Goals`, `## Open Risks`, and
  `## Loopholes To Close` before phase sections.
- Every phase must include `Goal:`, `Deliverables:`, `Dependencies:`,
  `Unresolved decisions:`, `Steps:`, and `Validation:`.
- Keep phases implementation-sized and ordered.
- For broad work, split unrelated workstreams instead of targeting a fixed
  number of phases.
- Any phase count is acceptable only when the boundaries are justified by
  concrete implementation and validation boundaries.
- Split relevant broad-work phase families instead of merging schema, policy,
  persistence, scheduler, protocol, agent runtime, secrets/isolation,
  logs/artifacts/checks/audit/observability, dashboard, rollout,
  migration/removal, and readiness gates.
- Reject vague umbrella phases such as `Backend`, `UI`, `Tests`, `Rollout`,
  `Cleanup`, `Wire everything`, or `Add tests`.
- Include only work needed to deliver the shaped brief.
- If blocked, state the blocker instead of inventing a plan.
- After writing the plan, end the response with exactly `PLAN_PATH: <path>` on
  its own line.

## Session B: Evidence

You are the evidence session.

Behavior:

1. Answer only the forced-choice question sent by the host.
2. Ground answers in the current codebase first.
3. Use current-year web search when repo-local evidence is missing and the
   answer depends on current tools, APIs, libraries, standards, or ecosystem
   practice.
4. Use industry best practice for the detected stack only when codebase
   evidence and current docs are not enough.
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

Use the correction form only when the closest numbered option needs a factual
constraint or small adjustment.
