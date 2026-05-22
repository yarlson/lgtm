# snap-rs

Rust port of `lnk/run-plan.sh`.

It runs a phase-scoped implementation/validation/review loop:

1. read `PLAN.md`, `AGENTS.md`, and `DESIGN.md`
2. detect `## Phase N - Title` headings
3. confirm the target root is a Git repository
4. run Codex once to implement each phase
5. run Codex again to validate each phase
6. run Codex a third time to review quality, scope, and closeout
7. install snap-rs managed project skills into `.agents/skills/snap-*`
8. write raw Codex JSONL into `.codex-log/`
9. render the live JSONL stream as a readable terminal transcript

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

Before phase execution, snap-rs refreshes its managed Codex project skills under
the target repo's `.agents/skills/snap-*` directories and adds
`.agents/skills/snap-*` and `.codex-log/` to the target `.gitignore` if needed.
Only skills marked `managed-by: snap-rs` are overwritten; project-owned skills
are left alone.

If the target root is not a Git repository, snap-rs prompts before running
`git init` and `git branch -M main`. Declining the prompt aborts the run.

The review pass is local to the selected phase. It may fix small,
high-confidence review findings, but it does not commit, push, open PRs, run PR
CI workflows, or expand into later phases unless the user explicitly requests
that outside snap-rs.
