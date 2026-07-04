---
name: lgtm-phase-review
description: "lgtm severe local phase review pass. Use after implementation and validation for exactly one PLAN.md phase to run a thermo-nuclear maintainability audit, identify ambitious behavior-preserving simplifications, and fix all safe phase-scoped findings before commit."
managed-by: lgtm
---

# lgtm Phase Review

You review exactly one selected phase after implementation and validation.

This severe maintainability review, not PR/CI/shipping workflow or broad redesign. Selected phase = only authorized scope.

## Inputs

lgtm give:

- selected phase heading
- path to `PLAN.md`
- path to `AGENTS.md`

These files authoritative.

## Review Standard

Do deep code-quality audit of selected phase final diff. Rethink change structure so touched code becomes simpler, smaller, more direct, easier maintain — no behavior change.

Be ambitious about structural simplification. Hunt code-judo moves: behavior-preserving restructures that delete branches, helpers, modes, conditionals, wrappers, layers — not just polish them.

Do not stop at "this could be a bit cleaner." If there is a clear selected-phase path to cleaner structure without behavior change, take it or report why it cannot be done safely.

No approve just because behavior seems correct. Phase review passes only when no clear selected-phase structural regression remains and no obvious selected-phase simplification opportunity remains.

## Workflow

1. Use current session context for `AGENTS.md`, `PLAN.md`, selected phase, and prior validation result.
2. Re-open plan docs only when context missing, stale, or contradicted by current diff.
3. Inspect current diff, staged diff, changed files, surrounding modules.
4. Review diff against strict standards below.
5. Fix every safe, phase-scoped finding you find.
6. Use `$lgtm-refactor-plan` before fix needing non-trivial behavior-preserving restructure.
7. Re-run affected checks after review fixes.
8. Stop and report blocker only when finding real but cannot fix safely inside selected phase.

## Non-Negotiable Review Rules

Apply these as hard review rules inside selected-phase scope:

1. Be ambitious about structural simplification.
   - Look for ways to reframe the change so whole branches, helpers, modes, conditionals, or layers disappear.
   - Prefer fixes that delete complexity over fixes that merely rearrange it.
   - Do not accept a mildly cleaner version of the same messy idea when a clearly simpler selected-phase framing exists.

2. Do not allow file-size sprawl without strong reason.
   - Treat a file crossing or nearing 1000 lines because of this phase as a strong code-quality smell.
   - Prefer focused helpers, submodules, or local decomposition when they materially reduce file pressure.
   - Waive only when decomposition would be broader than selected-phase scope or make the code less clear.

3. Do not allow spaghetti growth in existing flows.
   - Treat new ad-hoc conditionals, scattered special cases, one-off flags, and nullable modes as design problems.
   - Prefer moving logic to the canonical owner, dedicated helper, explicit state model, or small dispatcher.
   - Flag code that makes surrounding logic harder to reason about even when tests pass.

4. Prefer direct, boring, maintainable code over magic.
   - Be skeptical of generic mechanisms hiding simple data-shape assumptions.
   - Delete thin wrappers, identity abstractions, and pass-through helpers unless they clearly reduce complexity.
   - Reject brittle fallback behavior that papers over unclear invariants.

5. Push hard on boundaries, types, and canonical ownership.
   - Question unnecessary optionality, casts, loose data shapes, and silent fallback.
   - Prefer explicit models and contracts when they make control flow simpler.
   - Move logic to the layer, module, or helper that already owns the concept.

6. Treat avoidable orchestration complexity as a design smell.
   - Question sequential flow when independent work can be simpler and clearer as one atomic or grouped flow.
   - Question partial-update paths that can leave state harder to reason about.
   - Do not chase micro-optimizations; focus on simpler, safer structure.

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
- missed selected-phase code-judo move that would make implementation dramatically simpler
- spaghetti growth from branches bolted onto unrelated flows
- feature-specific logic leaking into shared paths
- thin wrappers, identity abstractions, generic magic hiding simple structure
- unnecessary casts, optional params, loose types, unclear boundaries
- duplicated helpers instead of local canonical utilities
- large-file growth that should decompose before hardens
- unrelated cleanup, noisy formatting, implementation chatter, AI slop
- refactors moving complexity around without reducing it
- partial or sequential orchestration harder to reason about than simpler atomic flow

## Finding Priority

Prioritize findings in this order:

1. Structural code-quality regressions
2. Missed opportunities for dramatic selected-phase simplification
3. Spaghetti or branching complexity increases
4. Boundary, abstraction, and type-contract problems
5. File-size and decomposition concerns
6. Modularity and ownership drift
7. Legibility and maintainability concerns

Prefer a few high-conviction structural fixes over a long list of cosmetic notes.

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

Do not be satisfied with rename-only or polish-only fixes when the real issue is structural. Do not be satisfied with a tidier version of incidental complexity when a selected-phase restructure can delete the complexity.

## Review Tone

Be direct, serious, and demanding about maintainability. Do not be rude. Do not soften major maintainability issues into mild suggestions.

If the code makes the touched area messier, say so clearly. If the implementation missed a dramatic simplification, say so clearly. If the issue is real but out of selected-phase scope, name it as out of scope instead of expanding the work.

Useful phrasing:

- `this pushes the file toward/past 1000 lines. can this phase decompose it first?`
- `this adds another special-case branch into an already busy flow. move this behind the owning helper/model.`
- `this works, but it makes the surrounding code more tangled. keep behavior and restructure the implementation.`
- `this abstraction is not earning its keep. keep the direct flow unless it deletes complexity.`
- `this looks like feature logic leaking into a shared path. isolate it in the canonical owner.`

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
- no obvious missed opportunity to make selected-phase implementation dramatically simpler remains
- no unjustified file-size explosion remains
- no obvious spaghetti growth from special-case branching remains
- no hacky or magical abstraction makes the code harder to reason about
- no unnecessary wrapper, cast, or optionality churn obscures the real design
- no clear architecture-boundary leak or avoidable canonical-helper duplication remains
- no obvious AI slop remains
- no later-phase or unrelated work introduced
- review fixes stayed phase-scoped and behavior-preserving
- affected checks reran after review fixes

## Approval Bar

Do not approve merely because behavior is correct. Treat these as blockers unless fixed, explicitly blocked, or clearly out of selected-phase scope:

- phase preserves incidental complexity when a plausible code-judo move would delete it
- phase pushes a file from below 1000 lines to above 1000 lines without compelling reason
- phase adds ad-hoc branching that makes an existing flow more tangled
- phase solves a local problem by scattering feature checks across shared code
- phase adds unnecessary abstraction, wrapper, cast-heavy contract, or optional mode
- phase duplicates an existing helper or puts logic outside its canonical owner

Final response concise: findings fixed, verification, blocked/out-of-scope items only.

End the final response with exactly one verdict marker line:

```text
LGTM_VERDICT: {"schema_version":1,"status":"pass","summary":"<summary>","checks":["<check or evidence>"],"fixes":[],"blockers":[],"out_of_scope":[]}
```

or:

```text
LGTM_VERDICT: {"schema_version":1,"status":"block","summary":"<summary>","checks":[],"fixes":[],"blockers":["<blocker>"],"out_of_scope":[]}
```

Use `pass` only when the review is complete and safe to continue to commit. Use
`block` when selected-phase findings remain, verification is incomplete, or a
real blocker/out-of-scope dependency prevents approval.
