# snap-rs

Rust port of `lnk/run-plan.sh`.

It keeps the same implementation/validation loop:

1. read `PLAN.md`, `AGENTS.md`, and `DESIGN.md`
2. detect `## Phase N - Title` headings
3. run Codex once to implement each phase
4. run Codex again to validate each phase
5. write raw Codex JSONL into `codex-logs/`
6. render the live JSONL stream as a readable terminal transcript

From this directory, drive the sibling `lnk` repo with:

```sh
cargo run -- --root ../lnk --start-phase 1 --end-phase 1 --sleep-seconds 0
```

Environment variables mirror the shell harness where practical:

- `PLAN_PATH`
- `REPO_AGENTS_PATH`
- `DESIGN_PATH`
- `START_PHASE`
- `END_PHASE`
- `SLEEP_SECONDS`
- `CODEX_BIN`
- `STREAM_MODE` (`pretty` or `raw`)
- `LOG_DIR`
- `RUN_STAMP`
- `ROOT_DIR`

The formatter is based on the current `codex exec --json` source from
`openai/codex` commit `de80fa6e3194d68b71b0f09be475179922e0f5b8`, especially
`codex-rs/exec/src/exec_events.rs` and
`codex-rs/exec/src/event_processor_with_jsonl_output.rs`. It intentionally uses
Ratatui text/style primitives for terminal rendering instead of a `jq` filter.
