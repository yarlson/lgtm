# Token Usage Sentinel

Compares released `lgtm` binaries on the same fresh-repo task. This is a live
manual or release-check sentinel, not a default CI eval. It exits before
running binaries unless `LGTM_TOKEN_EVAL=1` is set.

The fixture starts with only `PLAN.md` and `AGENTS.md`. Each trial creates a
new temporary Git repo, runs one `lgtm` binary, validates the generated Rust CLI,
collects app-server token usage from `.lgtm/logs`, records metrics outside this
repository, then deletes the generated repo. Usage is parsed from
`turn/completed` and `thread/tokenUsage/updated` events.

Default data root:

```bash
~/lgtm-token-eval-data
```

Example:

```bash
LGTM_TOKEN_EVAL=1 python3 evals/token-usage/run_eval.py \
  --bin 0.15.0 ~/lgtm-token-eval-data/bin/lgtm-0.15.0 \
  --bin 0.18.0 ~/lgtm-token-eval-data/bin/lgtm-0.18.0 \
  --trials 1
```

Outputs:

- `runs/<run-id>/stdout.txt`
- `runs/<run-id>/stderr.txt`
- `runs/<run-id>/validation.json`
- `runs/<run-id>/metrics.json`
- `results.jsonl`
- `summary.md`

The summary reports input, cached input, output, reasoning, total tokens,
median and p75 totals, wall time, and candidate-vs-baseline binary delta when
exactly two versions are compared. Each trial fails if `.lgtm/logs`, usage
objects, or total token counts are missing. It does not call optional semantic
judges.
