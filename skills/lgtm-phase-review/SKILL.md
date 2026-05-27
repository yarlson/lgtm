---
name: lgtm-phase-review
description: "lgtm strict local phase review pass. Use after implementation and validation for exactly one PLAN.md phase to audit structural quality, identify ambitious behavior-preserving simplifications, and fix all safe phase-scoped findings before commit."
managed-by: lgtm
---

# lgtm Phase Review

You are reviewing exactly one selected phase after implementation and
validation.

This is a strict maintainability review, not a PR workflow, CI workflow,
shipping workflow, or broad redesign pass. The selected phase is the only
authorized scope.

## Inputs

lgtm will provide:

- the selected phase heading
- the path to `PLAN.md`
- the path to `AGENTS.md`

Treat these files as authoritative.

## Review Standard

Perform a deep code-quality audit of the selected phase's final diff. Rethink
how the change is structured and implemented so the touched code becomes
simpler, smaller, more direct, and easier to maintain without changing behavior.

Be ambitious about structural simplification. Actively look for code-judo moves:
behavior-preserving restructurings that delete branches, helpers, modes,
conditionals, wrappers, or layers instead of merely polishing them.

Do not approve merely because behavior seems correct. The phase review only
passes when no clear selected-phase structural regression remains.

## Workflow

1. Re-open `AGENTS.md`, `PLAN.md`, and context docs linked from the selected
   phase.
2. Locate the exact selected phase heading.
3. Re-read the selected phase's Goal, Steps, and Validation sections.
4. Inspect the current diff, staged diff, changed files, and surrounding modules.
5. Review the diff against the strict standards below.
6. Fix every safe, phase-scoped finding you identify.
7. Use `$lgtm-refactor-plan` before a fix that needs non-trivial
   behavior-preserving restructuring.
8. Re-run affected checks after review fixes.
9. Stop and report a blocker only when a finding is real but cannot be fixed
   safely inside the selected phase.

## Strict Review Questions

For every meaningful change, ask:

- Is there a code-judo move that would make this dramatically simpler?
- Can this be reframed so fewer concepts, branches, helpers, or modes exist?
- Did the change improve or worsen the local architecture?
- Did it add ad-hoc conditionals, one-off flags, nullable modes, or scattered
  special cases?
- Is logic living in the canonical layer, file, module, or helper?
- Did it duplicate an existing helper or invent a near-duplicate?
- Did it introduce unnecessary optionality, casts, loose data shapes, silent
  fallback, or unclear invariants?
- Did it add wrappers or abstractions that do not earn their keep?
- Did a file cross or approach 1000 lines because decomposition was skipped?
- Are mechanical churn and behavior changes mixed in a way that makes review
  harder than necessary?
- Are tests and docs proving the changed behavior without fake confidence?

## Findings To Fix Aggressively

Treat these as presumptive blockers until fixed or explicitly blocked:

- complicated implementation where a cleaner framing would delete complexity
- spaghetti growth from branches bolted onto unrelated flows
- feature-specific logic leaking into shared paths
- thin wrappers, identity abstractions, or generic magic that hide simple
  structure
- unnecessary casts, optional params, loose types, or unclear boundaries
- duplicated helpers instead of local canonical utilities
- large-file growth that should be decomposed before it hardens
- unrelated cleanup, noisy formatting, implementation chatter, or AI slop
- refactors that move complexity around without reducing it
- partial or sequential orchestration that is harder to reason about than a
  simpler atomic flow

## Preferred Fixes

Prefer fixes that:

- delete a layer of indirection instead of polishing it
- reframe the state model so conditionals disappear
- collapse duplicate branches into one direct flow
- move logic to the module that already owns the concept
- extract a focused helper or module when it materially reduces file pressure
- replace special-case chains with a small typed model or explicit dispatcher
- reuse existing canonical helpers
- make boundaries explicit so control flow gets simpler
- separate orchestration from business logic
- remove unrelated churn introduced by the phase

## Scope And Safety

Fix all findings that are selected-phase scoped and safe to change now.

Do not add new product behavior.

Do not broaden the implementation into later phases or unrelated cleanup.

Do not rewrite a subsystem just because a cleaner design is imaginable. If the
finding is real but the fix requires broad redesign, later-phase work, missing
product decisions, or unrelated files, report it as blocked or out of scope.

Do not commit, push, create branches, open PRs, manage CI, tag releases, or
inspect PR comments. The commit pass owns committing.

## Completion Criteria

The phase review is complete only when:

- all safe selected-phase findings were fixed
- remaining findings are explicitly blocked or out of scope
- no obvious structural regression remains
- no obvious AI slop remains
- no later-phase or unrelated work was introduced
- review fixes stayed phase-scoped and behavior-preserving
- affected checks were rerun after review fixes
