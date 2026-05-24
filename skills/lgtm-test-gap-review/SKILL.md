---
name: lgtm-test-gap-review
description: "lgtm test and verification gap review skill. Use during validation to detect weak tests, missing behavior coverage, fake confidence, skipped checks, or verification that does not prove the selected PLAN.md phase works."
managed-by: lgtm
---

# lgtm Test Gap Review

Use this during validation after inspecting the implementation.

The goal is to verify behavior, not implementation trivia.

## Workflow

1. Re-read the selected phase's Validation and Web validation sections.
2. Identify what behavior must be proven.
3. Inspect existing tests and checks for that behavior.
4. Identify gaps:
   - no test covers the new behavior
   - assertions are too weak
   - only happy path is covered
   - test checks implementation details instead of behavior
   - test uses fixtures that cannot fail meaningfully
   - required command was skipped
   - manual verification is claimed without evidence
5. Add or strengthen tests only where they materially improve confidence.
6. Run the relevant checks.
7. For measurable claims, restate the claim in falsifiable form and classify the result as `VERIFIED`, `NOT VERIFIED`, or `INCONCLUSIVE`.
8. If a required check cannot run, report the blocker and residual risk.

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

Use `INCONCLUSIVE` when there is no valid baseline, the signal is noisy, the environment differs, or the check failed for reasons unrelated to the claim.

## Guardrails

Do not chase 100% coverage for its own sake.

Do not add fake-confidence tests.

Do not snapshot unstable output unless that is the established local pattern.

Do not broaden test infrastructure unless the selected phase requires it.

## Completion Criteria

This review is complete when the selected phase's behavior is proven by meaningful checks, disproven clearly, or remaining verification gaps are explicitly reported.
