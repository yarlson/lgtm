# lgtm-plan-create Eval

Evaluates generated `PLAN.md` quality for the real `lgtm plan` workflow.

The runner creates a fresh temporary repo, drives `lgtm plan` through a PTY,
sends `/finish`, exits at the post-plan prompt, then scores the produced
artifact. Results are stored outside the repo by default under
`~/lgtm-plan-create-eval-data`.

Build `lgtm` first so the eval uses current embedded skills:

```bash
cargo build
```

Run the live Kargo job-system eval:

```bash
LGTM_LIVE_EVAL=1 python3 evals/plan-create/run_eval.py --case kargo-job-system
```

Run the deterministic weak-plan control:

```bash
python3 evals/plan-create/run_eval.py \
  --score-only evals/plan-create/controls/weak-kargo-8-phase.md \
  --expect-fail
```

Run the keyword-stuffed weak control:

```bash
python3 evals/plan-create/run_eval.py \
  --score-only evals/plan-create/controls/weak-kargo-keyword-stuffed-14-phase.md \
  --expect-fail
```

Run the deterministic good control:

```bash
python3 evals/plan-create/run_eval.py \
  --score-only evals/plan-create/controls/good-kargo-job-system.md
```

The scorer treats phase count as telemetry only. It fails weak plans through
missing concrete workstream coverage, keyword-only coverage, umbrella phases,
fake dependencies, generic validation, and top-level sections that hide
unresolved decisions.

Live generation is intended for manual or release-check use, not as CI. It
depends on local Codex auth, model behavior, and network availability. Optional
judge prompts in `evals/prompts` are secondary semantic review inputs only; the
deterministic scorer remains the pass authority for these controls.
