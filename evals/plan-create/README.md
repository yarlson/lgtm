# lgtm-plan-create Eval

Evaluates generated `PLAN.md` quality for the `lgtm-plan-create` prompt.

The runner creates a fresh temporary repo, asks Codex to write a final plan from
the current bundled planning skill, then scores the produced artifact. Results
are stored outside the repo by default under `~/lgtm-plan-create-eval-data`.

Run the broad Kargo job-system eval:

```bash
python3 evals/plan-create/run_eval.py --case kargo-job-system
```

Run the deterministic weak-plan control:

```bash
python3 evals/plan-create/run_eval.py \
  --score-only evals/plan-create/controls/weak-kargo-8-phase.md
```

The eval is intended for manual or release-check use, not as CI. It depends on
local Codex auth, model behavior, and network availability.
