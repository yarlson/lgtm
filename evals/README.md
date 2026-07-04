# LGTM Evals

The eval suite is split to keep normal development token-free.

## Deterministic Lane

These checks do not require Codex auth, network access, or model tokens:

```bash
make eval-check
```

Expanded:

```bash
cargo build
python3 -B -m compileall -q evals
python3 -m unittest discover -s evals/tests
python3 evals/run-gate-negative/run_eval.py --include-pass-control
python3 evals/plan-create/run_eval.py --score-only evals/plan-create/controls/weak-kargo-keyword-stuffed-14-phase.md --expect-fail
python3 evals/plan-create/run_eval.py --score-only evals/plan-create/controls/good-kargo-job-system.md
python3 evals/shape-quality/run_eval.py --score-only evals/shape-quality/controls/weak-shape-finalizes-too-early.md --expect-fail
```

`evals/run-gate-negative` uses a fake Codex app-server to exercise real
`lgtm run` behavior. It scores base gate behavior, structural trajectory logs,
and git state diffs by default. Use `--no-score-trajectory` or
`--no-score-state-diff` only when isolating a scorer failure. Score-only plan
and shape controls exercise local artifact scorers.

## Manual Live Lane

Live plan and shape evals call real Codex and must be explicitly enabled:

```bash
LGTM_LIVE_EVAL=1 python3 evals/plan-create/run_eval.py --case kargo-job-system --iterations 1
LGTM_LIVE_EVAL=1 python3 evals/shape-quality/run_eval.py --case kargo-job-system --iterations 1 --max-rounds 8
```

Use these manually or in release checks only.

## Token Sentinel

`evals/token-usage` compares candidate and baseline binaries over the same fixture and parses token usage from `.lgtm/logs`.

```bash
cargo build --release
LGTM_TOKEN_EVAL=1 python3 evals/token-usage/run_eval.py \
  --bin baseline ~/lgtm-token-eval-data/bin/lgtm-baseline \
  --bin candidate target/release/lgtm \
  --trials 1
```

The sentinel reports input, cached input, output, reasoning, total tokens, median and p75 totals, and candidate-vs-baseline delta. It does not run optional semantic judges.
It fails before running binaries unless `LGTM_TOKEN_EVAL=1` is set, and each
trial fails if log files, usage objects, or total token counts are missing.

## Optional Judge Prompts

Prompt assets in `evals/prompts` are optional secondary semantic review inputs. They are not pass authorities for gate, state, or token checks. Validate prompt assets without a model call:

```bash
LGTM_LIVE_EVAL_JUDGE=1 python3 evals/run-gate-negative/run_eval.py --judge-prompts --dry-run
```

If a future runner executes a judge, it must pass reduced summaries only, not raw transcripts or full logs.

## Artifacts

- `.lgtm/logs` contains app-server JSONL protocol logs and token usage events.
- `.lgtm/gates` contains validate and review gate decisions.
- Eval data roots default outside this repository under `~/lgtm-*-eval-data`.
- `make eval-check` removes Python `__pycache__` directories created by syntax
  checking so compile artifacts do not pollute review diffs.
