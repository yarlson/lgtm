# Token Usage Eval

Compares released `lgtm` binaries on the same fresh-repo task.

The fixture starts with only `PLAN.md` and `AGENTS.md`. Each trial creates a
new temporary Git repo, runs one `lgtm` binary, validates the generated Rust CLI,
collects app-server token usage from `.lgtm/logs`, records metrics outside this
repository, then deletes the generated repo.

Default data root:

```bash
~/lgtm-token-eval-data
```

Example:

```bash
python3 evals/token-usage/run_eval.py \
  --bin 0.15.0 ~/lgtm-token-eval-data/bin/lgtm-0.15.0 \
  --bin 0.18.0 ~/lgtm-token-eval-data/bin/lgtm-0.18.0 \
  --trials 10
```

Outputs:

- `runs/<run-id>/stdout.txt`
- `runs/<run-id>/stderr.txt`
- `runs/<run-id>/validation.json`
- `runs/<run-id>/metrics.json`
- `results.jsonl`
- `summary.md`

