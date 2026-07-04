# lgtm-run-gate-negative Eval

Exercises deterministic `lgtm run` gate failures with a fake Codex app-server.
This is the default no-token gate eval lane.

The runner creates a fresh repo, runs one phase, and verifies that validation or
review gate failures stop before the commit pass and persist gate artifacts
under `.lgtm/gates`. It scores base behavior, structurally parsed app-server
trajectory logs, and git state diffs by default. This eval does not require
Codex auth, network access, or model tokens.

Build `lgtm` first:

```bash
cargo build
```

Run the negative cases with default trajectory and state-diff scoring:

```bash
python3 evals/run-gate-negative/run_eval.py
```

Include the passing control:

```bash
python3 evals/run-gate-negative/run_eval.py --include-pass-control
```

Temporarily isolate base gate checks by disabling one scorer:

```bash
python3 evals/run-gate-negative/run_eval.py --no-score-trajectory
python3 evals/run-gate-negative/run_eval.py --no-score-state-diff
```

Validate optional judge prompt assets without calling a model:

```bash
LGTM_LIVE_EVAL_JUDGE=1 python3 evals/run-gate-negative/run_eval.py \
  --judge-prompts \
  --dry-run
```

Results are stored outside the repo by default under
`~/lgtm-run-gate-negative-eval-data`.
