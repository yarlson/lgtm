---
name: lgtm-phase-review
description: "lgtm strict local phase review pass. Use after implementation and validation for exactly one PLAN.md phase to audit structural quality, identify ambitious behavior-preserving simplifications, and fix all safe phase-scoped findings before commit."
managed-by: lgtm
---

# lgtm Phase Review

You review exactly one selected phase after implementation and validation.

This strict maintainability review, not PR/CI/shipping workflow or broad redesign. Selected phase = only authorized scope.

## Inputs

lgtm give:

- selected phase heading
- path to `PLAN.md`
- path to `AGENTS.md`

These files authoritative.

## Review Standard

Do deep code-quality audit of selected phase final diff. Rethink change structure so touched code becomes simpler, smaller, more direct, easier maintain — no behavior change.

Be ambitious about structural simplification. Hunt code-judo moves: behavior-preserving restructures that delete branches, helpers, modes, conditionals, wrappers, layers — not just polish them.

No approve just because behavior seems correct. Phase review passes only when no clear selected-phase structural regression remains.

## Workflow

1. Use current session context for `AGENTS.md`, `PLAN.md`, selected phase, and prior validation result.
2. Re-open plan docs only when context missing, stale, or contradicted by current diff.
3. Inspect current diff, staged diff, changed files, surrounding modules.
4. Review diff against strict standards below.
5. Fix every safe, phase-scoped finding you find.
6. Use `$lgtm-refactor-plan` before fix needing non-trivial behavior-preserving restructure.
7. Re-run affected checks after review fixes.
8. Stop and report blocker only when finding real but cannot fix safely inside selected phase.

## Strict Review Questions

For every meaningful change, ask:

- Is there code-judo move making this dramatically simpler?
- Can reframe so fewer concepts, branches, helpers, modes exist?
- Did change improve or worsen local architecture?
- Did it add ad-hoc conditionals, one-off flags, nullable modes, scattered special cases?
- Is logic in canonical layer, file, module, helper?
- Did it duplicate existing helper or invent near-duplicate?
- Did it add unnecessary optionality, casts, loose data shapes, silent fallback, unclear invariants?
- Did it add wrappers or abstractions that no earn keep?
- Did file cross or near 1000 lines because decomposition skipped?
- Are mechanical churn and behavior changes mixed so review harder than needed?
- Do tests and docs prove changed behavior without fake confidence?

## Findings To Fix Aggressively

Treat these as presumptive blockers until fixed or explicitly blocked:

- complicated implementation where cleaner framing deletes complexity
- spaghetti growth from branches bolted onto unrelated flows
- feature-specific logic leaking into shared paths
- thin wrappers, identity abstractions, generic magic hiding simple structure
- unnecessary casts, optional params, loose types, unclear boundaries
- duplicated helpers instead of local canonical utilities
- large-file growth that should decompose before hardens
- unrelated cleanup, noisy formatting, implementation chatter, AI slop
- refactors moving complexity around without reducing it
- partial or sequential orchestration harder to reason about than simpler atomic flow

## Preferred Fixes

Prefer fixes that:

- delete layer of indirection instead of polish it
- reframe state model so conditionals disappear
- collapse duplicate branches into one direct flow
- move logic to module that already owns concept
- extract focused helper or module when it materially cuts file pressure
- replace special-case chains with small typed model or explicit dispatcher
- reuse existing canonical helpers
- make boundaries explicit so control flow simpler
- separate orchestration from business logic
- remove unrelated churn from phase

## Scope And Safety

Fix all findings that selected-phase scoped and safe to change now.

No add new product behavior.

No broaden implementation into later phases or unrelated cleanup.

No rewrite subsystem just because cleaner design imaginable. If finding real but fix needs broad redesign, later-phase work, missing product decisions, or unrelated files, report it blocked or out of scope.

No commit, push, create branches, open PRs, manage CI, tag releases, or inspect PR comments. Commit pass owns committing.

## Completion Criteria

Phase review complete only when:

- all safe selected-phase findings fixed
- remaining findings explicitly blocked or out of scope
- no obvious structural regression remains
- no obvious AI slop remains
- no later-phase or unrelated work introduced
- review fixes stayed phase-scoped and behavior-preserving
- affected checks reran after review fixes

Final response concise: findings fixed, verification, blocked/out-of-scope items only.
