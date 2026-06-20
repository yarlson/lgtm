# Plan

## Context

`lgtm` currently has two top-level commands: `plan` for interactive planning and `run` for executing phases from a plan. The requested feature is a new autonomous planning mode that takes a brief, starts two Codex sessions, lets one session run architecture sparring, lets the other answer each forced-choice question from evidence, and saves a multi-phase implementation plan.

The command name is `shape`.

`shape` is intentionally separate from `plan`. It is not normal human-in-the-loop planning with a flag; it is an autonomous idea-shaping workflow with two isolated Codex sessions and host-managed transcript handoff.

UI contract:

- When `shape` starts, it prints the normal banner and exactly one clear status line after both sessions are ready: `Started 2 Codex sessions; gathering context`.
- While the evidence session is doing initial codebase or web discovery, the UI should stay compact with existing status-line/spinner behavior, not full transcript dumping.
- Once sparring starts, `shape` streams Codex output using the same renderer behavior as a `run` implementation phase. The user should see the sparring agent's Codex messages, commands, web searches, file observations, and spinner updates through the existing terminal transcript UI, not a separate summary-only UI.
- Evidence-agent answers may remain compact in stdout, but the sparring agent output must be visible exactly through the implementation-phase rendering path.

Target user commands:

```bash
lgtm shape "brief idea"
lgtm shape --brief-file docs/brief.md
lgtm run --plan-path PLAN.md
```

## Non-goals

- Do not change the existing `lgtm plan` interactive contract.
- Do not add branch, PR, CI, release, or remote workflow automation.
- Do not replace Codex app-server or add another model-provider abstraction.
- Do not run both Codex turns concurrently in v1; sequential host orchestration is enough.
- Do not create broad generic agent orchestration framework code.
- Do not add many role-specific managed skills before one `shape` skill proves insufficient.

## Phase 1 - CLI Shape Command

Goal:
Add a first-class `lgtm shape` command with the smallest CLI surface needed for autonomous plan shaping.

Steps:

- Add a `Shape` variant to the CLI command enum.
- Add `ShapeArgs` with positional `brief`, `--brief-file`, `--root`, `--plan-path`, `--codex-bin`, flattened execution args, `--stream-mode`, `--log-dir`, `--run-stamp`, and `--max-rounds`.
- Default `--plan-path` to `PLAN.md`.
- Default `--max-rounds` to a finite value such as `12`.
- Dispatch `Command::Shape(args)` from `main`.
- Add `src/commands/shape.rs` and wire it through `src/commands/mod.rs`.

Validation:

- `cargo test cli`
- Parser tests prove `lgtm shape "brief"` and `lgtm shape --brief-file docs/brief.md` parse correctly.
- Parser tests prove shared execution, log, stream, root, plan path, and run stamp args are accepted.

## Phase 2 - Brief Source Validation

Goal:
Make brief intake deterministic and explicit.

Steps:

- Require exactly one brief source: positional brief string or `--brief-file`.
- Reject missing brief source with an actionable error.
- Reject both brief sources together with an actionable error.
- Resolve relative `--brief-file` under the target root; accept absolute paths unchanged.
- Read the brief file as UTF-8 text.
- Reject empty or whitespace-only brief content.
- Keep normal `plan` brief behavior unchanged.

Validation:

- Unit tests cover string brief, file brief, missing brief, both brief sources, missing file, and empty brief.
- Integration test proves `shape` does not require a TTY for brief intake.

## Phase 3 - Shape Runtime Preflight

Goal:
Reuse the existing lgtm runtime safety and setup behavior for the new command.

Steps:

- Build `ShapeConfig` with `CommandRuntime::new`.
- Reuse existing managed skill preflight, git initialization, and skill installation behavior.
- Render a startup banner for `shape`.
- Add a `Shape` banner mode if needed.
- Immediately after both app-server clients and threads start, print exactly one startup status line: `Started 2 Codex sessions; gathering context`.
- Keep the initial context-gathering period compact with a waiting/status line rather than dumping both sessions' full transcripts.
- Preserve existing host and Apple Container execution behavior.
- Preserve raw protocol logging behavior under the configured log directory.

Validation:

- Unit tests prove relative and absolute log paths resolve consistently with existing commands.
- Integration test proves `shape` installs managed skills and writes logs under `.lgtm/logs`.
- Integration test proves pretty stdout includes `Started 2 Codex sessions; gathering context` exactly once before sparring output begins.
- Existing `plan` and `run` tests remain unchanged in behavior.

## Phase 4 - Managed Shape Skill

Goal:
Add a dedicated managed skill for the shape workflow without overloading `lgtm-plan-create`.

Steps:

- Add `skills/lgtm-plan-shape/SKILL.md`.
- Use managed frontmatter with `name: lgtm-plan-shape` and `managed-by: lgtm`.
- Define Session A behavior: architecture sparring, forced-choice questions, sharp rejection of vague choices, final implementation plan writing.
- Define Session B behavior: evidence-only answers based on current codebase, current-year web search when needed, or industry best practice for the stack.
- Register the skill in `src/skills.rs`.
- Keep one new skill for v1; use prompt functions for role-specific turn contracts.

Validation:

- Existing bundled-skill frontmatter test includes the new skill.
- Unit test proves the new skill is installed into `.agents/skills/lgtm-plan-shape/SKILL.md`.
- Skill text contains the strict answer format for the evidence agent and the final plan contract for the sparring agent.

## Phase 5 - Shape Prompt Builders

Goal:
Create explicit prompt contracts for the two-session shape workflow.

Steps:

- Add prompt builder for Session A initial turn.
- Add prompt builder for Session B initial evidence-discovery turn.
- Add prompt builder for sending a Session A question to Session B.
- Add prompt builder for sending a Session B answer back to Session A.
- Add prompt builder for finalization when the host reaches `--max-rounds`.
- In Session A prompts, require exactly one forced-choice question per sparring turn until ready to write the plan.
- In Session B prompts, require output to be only `1`, `2`, `3`, or `<number>, but <correction>`.
- In final Session A prompt, require writing the plan at the configured plan path unless there is a hard blocker.

Validation:

- Unit tests prove prompts include the brief, root-relative plan path, `lgtm-plan-shape`, one-question rule, evidence-answer format, and final `PLAN.md` contract.
- Unit tests prove prompts forbid implementation edits, commits, pushes, and interactive input tools.

## Phase 6 - Two Session Orchestrator

Goal:
Start and coordinate two isolated Codex sessions through app-server.

Steps:

- Start two logged app-server clients using `CommandRuntime`.
- Start one thread per client.
- Treat Session A as the sparring session and Session B as the evidence session.
- Run Session B initial discovery before the first sparring answer is needed.
- Run Session A initial turn with the brief and instructions to produce the first forced-choice question or final plan if no question is needed.
- Before every turn, set a role-specific log sink.
- Use log names like `<run-stamp>-shape-a-001.jsonl` and `<run-stamp>-shape-b-001.jsonl`.
- During initial discovery, render a compact waiting status instead of full Session B transcript output.
- Once Session A starts sparring, stream Session A events to stdout using the shared Codex transcript renderer.
- Keep Session B evidence-answer turns mostly hidden from stdout except concise status and errors; the answer itself is host-transferred to Session A.
- Stop both clients explicitly on success and on failure.

Validation:

- Integration test with fake Codex proves two app-server sessions start.
- Integration test proves both sessions receive distinct role prompts.
- Integration test proves role-specific log files are created.
- Integration test proves initial discovery does not dump the full evidence transcript to pretty stdout.
- Integration test proves Session A sparring output is streamed to pretty stdout.
- Failure test proves the command exits non-zero with role and round context when one session fails.

## Phase 7 - Shared Shape Output Renderer

Goal:
Reuse the implementation-phase Codex streaming UI for sparring instead of inventing a separate transcript display.

Steps:

- Extract or expose the smallest reusable command-output helper currently embedded in `run` so `shape` can use the same `Renderer` event path.
- Preserve existing `run` output behavior while extracting the helper: banner rendering, pretty/raw suppression, status lines, streamed turn events, idle ticks, finish clearing, flushing, and token summaries.
- Keep the rendered sparring UI visually consistent with implementation phases: header, `Codex` message blocks, command rows, file-change rows, web-search rows, active spinner rows, and completion cleanup.
- Add a shape-specific header label such as `Shape 01 sparring` only if needed, but do not create a new renderer style.
- Do not add a duplicate shape-only transcript renderer or println-based transcript formatting.
- Use the same `render_event` behavior for Session A sparring turns that implementation passes use today.
- Use `start_status_line` or the same status-line machinery for the initial two-session gathering message.
- Preserve raw mode behavior: raw mode should echo app-server protocol logs and suppress pretty-only UI.

Validation:

- Unit tests prove the reusable output helper renders a Session A agent message with the existing `Codex` block style.
- Unit tests prove the initial waiting/status line uses existing spinner/status behavior.
- Unit tests prove raw mode suppresses shape banner/status transcript UI through the shared helper.
- Integration test proves fake Session A command, web-search, and agent-message events appear in stdout with the same labels as implementation phase output.
- Integration test proves raw mode does not print the pretty waiting/header transcript UI.

## Phase 8 - Bounded Transcript Handoff

Goal:
Pass only the useful assistant output between sessions without token blowup.

Steps:

- Add a small shape-local helper for bounded role-labeled excerpts.
- Use `CompletedTurn.transcript.response_text()` as the source for handoff text.
- Reject empty assistant responses before passing text to the other session.
- Truncate long handoff text by character budget with a clear marker.
- Do not feed raw command output, full transcript activity, protocol logs, or file patches into the other session by default.

Validation:

- Unit tests cover excerpt formatting, empty response rejection, and truncation.
- Integration test proves Session B receives Session A's question, and Session A receives Session B's answer.
- Integration test proves large Session B output is bounded before being sent to Session A.

## Phase 9 - Evidence Answer Validation And Retry

Goal:
Keep the evidence agent from drifting into prose or pretending to decide architecture.

Steps:

- Parse Session B response as one of the accepted formats: `1`, `2`, `3`, or `<number>, but <correction>`.
- If Session B returns invalid output, send one repair prompt that includes the original question and invalid answer.
- Fail the run if the repaired answer is still invalid.
- Preserve the corrected answer text exactly when sending it to Session A.
- Do not validate whether the chosen number is semantically correct; Session B owns evidence judgment.

Validation:

- Unit tests cover accepted answer formats and rejected prose.
- Integration test proves invalid Session B answer triggers one repair turn.
- Integration test proves a second invalid answer fails with a useful error.

## Phase 10 - Sparring Loop Completion

Goal:
Make the autonomous loop finite, observable, and deterministic enough to operate.

Steps:

- Track shape rounds from `1` to `--max-rounds`.
- After each Session A turn, classify output as either a question for Session B or final plan completion.
- Require Session A final output to contain a parseable final marker such as `PLAN_PATH: <path>`.
- Verify the reported plan path exists under the target root unless it is absolute and explicitly allowed by args.
- If max rounds is reached without final completion, send one finalization prompt to Session A.
- If finalization still does not produce a plan, fail with a blocker message.

Validation:

- Unit tests cover final marker parsing and target-root path validation.
- Integration test proves a normal A/B loop exits after final marker and plan file creation.
- Integration test proves max-round finalization happens exactly once.
- Integration test proves missing final plan fails clearly.

## Phase 11 - Final Plan Contract Checks

Goal:
Ensure `shape` produces a plan that `lgtm run` can consume.

Steps:

- After final plan write, read the plan file.
- Check that it contains `# Plan`.
- Check that it contains at least one `## Phase N - Name` heading.
- Check that each detected phase block contains `Goal:`, `Steps:`, and `Validation:`.
- Keep checks deterministic and local; do not invoke the phase-index model from `shape`.
- Report the final plan path in stdout.

Validation:

- Unit tests cover valid plan, missing `# Plan`, missing phase heading, and missing required block label.
- Integration test proves fake Codex-created plan passes final checks.
- Integration test proves an invalid fake plan fails before reporting success.

## Phase 12 - Shape Output And Token Summary

Goal:
Keep terminal output useful while preserving the implementation-phase transcript experience during sparring.

Steps:

- Pretty mode renders the `shape` banner, the two-session gathering-context status, sparring round headers, streamed Session A Codex transcript output, and final plan path.
- Pretty mode shows the startup status line `Started 2 Codex sessions; gathering context` exactly once after both sessions and threads are created.
- Pretty mode uses compact status labels for evidence work, such as `gathering evidence` or `answering shape round 1 from evidence`.
- Pretty mode prints aggregate token usage when app-server reports it.
- Raw mode echoes app-server protocol lines consistently with existing command behavior.
- Do not hide Session A sparring prose behind summaries; it should render through the same transcript UI as implementation phases.
- Keep Session B evidence-agent transcript output compact unless an error or repair is needed.
- Include role and round labels in errors.

Validation:

- Unit tests cover token summary formatting if added outside existing helpers.
- Integration test proves stdout includes `mode: shape`, `Started 2 Codex sessions; gathering context` once, Session A `Codex` output, round labels, and final plan path.
- Integration test proves Session B strict answers are passed to Session A but are not dumped as separate verbose prose transcripts in pretty mode.
- Raw-mode integration test proves protocol output is available and pretty banner is suppressed if that matches existing `run` behavior.

## Phase 13 - README Documentation

Goal:
Document the new command accurately without overselling it.

Steps:

- Update README overview to mention `shape` as autonomous plan shaping.
- Add usage examples for string brief and markdown brief file.
- Add an options table for `shape`.
- Document that `shape` starts two Codex sessions and may use web search through Codex when current-year or stack guidance is needed.
- Document the UI behavior: `mode: shape`, `Started 2 Codex sessions; gathering context`, compact evidence-gathering status, streamed sparring transcript using the same UI style as implementation phases, and final plan path.
- Document log naming for shape sessions.
- Document that `shape` writes a plan and does not implement it; implementation remains `lgtm run`.
- Keep safety messaging about `danger-full-access` and approval policy intact.

Validation:

- `npx --yes markdownlint-cli2 README.md` if available, otherwise inspect formatting manually.
- README examples match actual CLI args.
- Existing README plan and run docs remain accurate.

## Phase 14 - Integration Harness Coverage

Goal:
Prove the full shape workflow with fake Codex app-server scripts.

Steps:

- Add a `tests/shape_app_server.rs` integration test file or extend existing app-server tests if local style favors that.
- Build fake Codex script that supports two sessions and records each turn prompt.
- Simulate Session B initial discovery.
- Simulate Session A question.
- Simulate Session B strict answer.
- Simulate Session A final plan write and final marker.
- Simulate Session A streamed transcript items matching implementation-phase UI cases: agent message, plan update, command execution, and web search.
- Assert prompt handoff, log files, `Started 2 Codex sessions; gathering context` exactly once, streamed Session A sparring output with implementation-phase markers such as `Codex` and `Ran`, hidden Session B prose in pretty mode, final plan path, and stdout.

Validation:

- `cargo test --test shape_app_server`
- `cargo test --all-features`
- Fake harness proves no real Codex or network access is needed for tests.

## Phase 15 - Full Verification

Goal:
Finish with the same confidence bar as other meaningful lgtm code changes.

Steps:

- Run formatting check.
- Run clippy with warnings denied.
- Run all tests.
- Run all-target build.
- Run the project `make check` target.
- Fix only issues caused by this feature.

Validation:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo build --all-targets --all-features`
- `make check`
