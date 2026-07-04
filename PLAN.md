# PLAN.md - Low-Token LGTM Eval Coverage

## Summary

Add the important missing eval coverage for `lgtm` without making normal development or CI burn real Codex tokens.

The eval suite has three lanes:

- Default deterministic lane: fake Codex and score-only evals, no model calls, suitable for local checks and CI.
- Manual live smoke lane: one tightly capped real `lgtm shape` run, opt-in only.
- Release token sentinel lane: candidate-vs-baseline token reporting, opt-in only.

The plan intentionally avoids live eval matrices, prompt goldens, broad LLM judging, and repeated live plan/run attempts. Optional judge prompts exist as assets, but they are only executed behind explicit environment flags and only over reduced summaries.

## Key Interface Changes

- Add `--expect-fail` to `evals/plan-create/run_eval.py` and `evals/shape-quality/run_eval.py`.
- Add shared log and usage parsing in `evals/common/lgtm_logs.py`.
- Add shared deterministic scorer helpers in `evals/common/scoring.py`.
- Keep `make check` and normal CI token-free.
- Require `LGTM_LIVE_EVAL=1` for live plan-create and shape-quality eval runs.
- Require `LGTM_LIVE_EVAL_JUDGE=1` for optional judge-prompt checks.

## Non-Goals

- Do not add live evals to default CI.
- Do not make an LLM judge the sole authority for pass/fail.
- Do not add a broad eval orchestration framework.
- Do not change normal `lgtm plan`, `lgtm shape`, or `lgtm run` runtime behavior beyond eval harness integration.

## Phase 1 - Eval Baseline And Low-Token Policy

Goal:
Make the eval policy explicit before expanding coverage.

Deliverables:
- Document the deterministic, live smoke, and token sentinel lanes.
- Document that optional judge prompts are semantic-only secondary checks.
- Keep default developer verification token-free.

Dependencies:
- None.

Unresolved decisions:
- None.

Steps:
- Add a top-level eval README.
- Add a README eval section that points to logs and gates.
- Add AGENTS guidance for default deterministic eval behavior.

Validation:
- Run `python3 -m compileall evals`.
- Confirm docs name `.lgtm/logs` and `.lgtm/gates`.

## Phase 2 - Deterministic Run Gate And State-Diff Eval

Goal:
Expand fake-Codex run evals so gate failures, commit failures, and generated-state failures are scored without model calls.

Deliverables:
- Cases for block, missing verdict, malformed verdict, missing commit, generated `.lgtm/` commit, and pass control.
- Trajectory scoring over reduced log facts.
- State-diff scoring over committed files, dirty status, generated-state checks, and expected terminal status.

Dependencies:
- Phase 1 eval policy.

Unresolved decisions:
- None.

Steps:
- Extend `evals/run-gate-negative/run_eval.py` cases.
- Add baseline commits to fixture repos so phase commits are distinguishable.
- Copy logs and gates into eval result directories.
- Score pass order, gate status, commit-pass reachability, and generated-state handling.

Validation:
- Run `cargo build`.
- Run `python3 evals/run-gate-negative/run_eval.py`.
- Run `python3 evals/run-gate-negative/run_eval.py --include-pass-control`.
- Run `python3 evals/run-gate-negative/run_eval.py --score-trajectory`.
- Run `python3 evals/run-gate-negative/run_eval.py --score-state-diff`.

## Phase 3 - Score-Only Plan And Shape Controls

Goal:
Catch scorer regressions without invoking Codex.

Deliverables:
- `--expect-fail` support for weak plan and shape controls.
- Positive good control for the Kargo job-system plan scorer.
- README examples that separate score-only controls from live runs.

Dependencies:
- Phase 1 eval policy.

Unresolved decisions:
- None.

Steps:
- Add expectation-aware exit behavior while preserving raw `score["passed"]`.
- Add `evals/plan-create/controls/good-kargo-job-system.md`.
- Keep shape-quality score-only mode using the shared plan-create scorer.

Validation:
- Run `python3 evals/plan-create/run_eval.py --score-only evals/plan-create/controls/weak-kargo-keyword-stuffed-14-phase.md --expect-fail`.
- Run `python3 evals/plan-create/run_eval.py --score-only evals/plan-create/controls/good-kargo-job-system.md`.
- Run `python3 evals/shape-quality/run_eval.py --score-only evals/shape-quality/controls/weak-shape-finalizes-too-early.md --expect-fail`.

## Phase 4 - Optional Eval Judge Prompts

Goal:
Provide judge prompts for manual semantic review without making them default pass gates.

Deliverables:
- `evals/prompts/trajectory_judge.md`
- `evals/prompts/state_diff_judge.md`
- `evals/prompts/diff_vs_plan_judge.md`
- `evals/prompts/guardrail_judge.md`
- Dry-run validation that prompt assets exist without calling a model.

Dependencies:
- Phase 2 deterministic trajectory and state-diff facts.

Unresolved decisions:
- None.

Steps:
- Store the prompt text as eval assets.
- Add `--judge-prompts --dry-run` validation behind `LGTM_LIVE_EVAL_JUDGE=1`.
- Keep judge inputs constrained to reduced summaries, not raw logs or full transcripts.

Validation:
- Run `LGTM_LIVE_EVAL_JUDGE=1 python3 evals/run-gate-negative/run_eval.py --judge-prompts --dry-run`.

## Phase 5 - Capped Live Shape Smoke

Goal:
Keep one real semantic smoke for `lgtm shape` while preventing routine token burn.

Deliverables:
- `evals/shape-quality/run_eval.py` requires `LGTM_LIVE_EVAL=1` unless `--score-only` is used.
- Live shape results include parsed token usage when logs contain usage events.
- The recommended live command uses one case, one iteration, and bounded rounds.

Dependencies:
- Phase 3 score-only shape controls.

Unresolved decisions:
- None.

Steps:
- Gate live shape execution with `LGTM_LIVE_EVAL=1`.
- Parse usage from copied `.lgtm/logs`.
- Print token totals in result and summary output.

Validation:
- Run `LGTM_LIVE_EVAL=1 python3 evals/shape-quality/run_eval.py --case kargo-job-system --iterations 1 --max-rounds 8` only when live token spend is explicitly requested.

## Phase 6 - Token Regression Sentinel

Goal:
Keep release token comparison focused on candidate-vs-baseline usage, not semantic judging.

Deliverables:
- Shared token parser reused from `evals/common/lgtm_logs.py`.
- Token eval fails when usage metadata is missing.
- Token fixture plan matches the current plan contract.

Dependencies:
- Phase 1 eval policy.

Unresolved decisions:
- None.

Steps:
- Extract token parsing from token-usage into the shared helper.
- Keep token-usage live/manual and release-oriented.
- Update the fixture `PLAN.md` with top-level sections and phase labels.

Validation:
- Run `cargo build --release`.
- Run `python3 evals/token-usage/run_eval.py --bin baseline ~/lgtm-token-eval-data/bin/lgtm-baseline --bin candidate target/release/lgtm --trials 1` only during release/token checks.

## Phase 7 - Final Documentation And Verification

Goal:
Make the implementation reproducible and keep the final check path token-free by default.

Deliverables:
- README and eval README commands for every lane.
- Eval-specific README updates for plan-create, shape-quality, run-gate-negative, and token-usage.
- Final deterministic verification command list.

Dependencies:
- Phases 1 through 6.

Unresolved decisions:
- None.

Steps:
- Document deterministic commands.
- Document live commands with required env flags.
- Document prompt-judge opt-in policy.
- Document logs and gates as scorer inputs.

Validation:
- Run `cargo build`.
- Run `python3 -m compileall evals`.
- Run `python3 evals/run-gate-negative/run_eval.py`.
- Run `python3 evals/run-gate-negative/run_eval.py --include-pass-control`.
- Run `python3 evals/run-gate-negative/run_eval.py --score-trajectory`.
- Run `python3 evals/run-gate-negative/run_eval.py --score-state-diff`.
- Run `python3 evals/plan-create/run_eval.py --score-only evals/plan-create/controls/weak-kargo-keyword-stuffed-14-phase.md --expect-fail`.
- Run `python3 evals/plan-create/run_eval.py --score-only evals/plan-create/controls/good-kargo-job-system.md`.
- Run `python3 evals/shape-quality/run_eval.py --score-only evals/shape-quality/controls/weak-shape-finalizes-too-early.md --expect-fail`.
- Run `LGTM_LIVE_EVAL_JUDGE=1 python3 evals/run-gate-negative/run_eval.py --judge-prompts --dry-run`.

## Eval Prompt Assets

The prompt assets live under `evals/prompts/` and are not prompt goldens. They are manual semantic-review inputs only. The deterministic lane remains authoritative for exact gate, state, and token checks.
