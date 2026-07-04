# lgtm-shape-quality Eval

Evaluates generated `PLAN.md` quality for the real `lgtm shape` workflow.

The runner creates a fresh temporary repo, runs `lgtm shape`, scores the final
plan with the shared plan-create scorer, and stores results outside the repo by
default under `~/lgtm-shape-quality-eval-data`.

Build `lgtm` first so the eval uses current embedded skills:

```bash
cargo build
```

Run the live Kargo job-system eval:

```bash
LGTM_LIVE_EVAL=1 python3 evals/shape-quality/run_eval.py \
  --case kargo-job-system \
  --iterations 1 \
  --max-rounds 8
```

Run the deterministic weak-shape control:

```bash
python3 evals/shape-quality/run_eval.py \
  --score-only evals/shape-quality/controls/weak-shape-finalizes-too-early.md \
  --expect-fail
```

The scorer is intentionally shared with `evals/plan-create`: shape is judged by
the same final artifact contract as plan creation, including concrete
workstream coverage, specific validation, dependency shape, and weak top-level
section checks.

Live shape generation is intended for manual or release-check use, not as CI.
It depends on local Codex auth, model behavior, and network availability.
Optional judge prompts in `evals/prompts` are secondary semantic review inputs
only; the deterministic scorer remains the pass authority for score-only
controls.
