---
name: lgtm-test-gap-review
description: "lgtm test and verification gap review skill. Use during validation to detect weak tests, missing behavior coverage, fake confidence, skipped checks, or verification that does not prove the selected PLAN.md phase works."
managed-by: lgtm
---

# lgtm Test Gap Review

Use during validation after inspecting implementation.

Goal: verify behavior, not implementation trivia.

## Workflow

1. Re-read selected phase's Validation + Web validation sections.
2. Identify behavior to prove.
3. Inspect existing tests/checks for that behavior.
4. Find gaps:
   - no test covers new behavior
   - assertions too weak
   - only happy path covered
   - test checks implementation details not behavior
   - test uses fixtures that cannot fail meaningfully
   - required command skipped
   - manual verification claimed without evidence
5. Add/strengthen tests only where they materially improve confidence.
6. Run relevant checks.
7. For measurable claims, restate claim in falsifiable form, classify result as `VERIFIED`, `NOT VERIFIED`, or `INCONCLUSIVE`.
8. If required check cannot run, report blocker + residual risk.

## Verification Preference

Prefer, in order:

1. existing project test command required by `AGENTS.md`
2. selected phase validation command
3. targeted unit or integration tests
4. focused manual verification with concrete evidence
5. explicit blocker report

## Verdict Shape

Use this shape when validating a measurable claim:

```md
VERIFIED | NOT VERIFIED | INCONCLUSIVE
Claim: ...
Evidence: ...
Reasoning: ...
```

Use `INCONCLUSIVE` when no valid baseline, signal is noisy, environment differs, or check failed for reasons unrelated to claim.

## Guardrails

Don't chase 100% coverage for its own sake.

Don't add fake-confidence tests.

Don't snapshot unstable output unless that's the established local pattern.

Don't broaden test infrastructure unless selected phase requires it.

## Completion Criteria

Review complete when selected phase's behavior is proven by meaningful checks, disproven clearly, or remaining verification gaps explicitly reported.