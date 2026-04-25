# Terminology

## Core Concepts

**Task** — Individual feature or requirement to implement, usually stored as a separate file (TASK1.md, TASK2.md, etc.) in a tasks directory. Directory-scanned task files are case-sensitive and uppercase-required. `snap run --task-file <path>` also supports a single ad hoc task file with any filename.

**Task Discovery Diagnostics** — Automated checks that identify common task discovery failures: detects case-mismatched filenames (task1.md vs TASK1.md) and PRD-embedded task headers (## TASK1: Feature in PRD.md). Returns user-friendly hints with corrective actions. Triggered when task scanner finds no valid files.

**PRD** (Product Requirements Document) — Optional overview document at `docs/tasks/PRD.md` providing context and scope. Tasks are defined as individual files, not as sections within PRD.

**Iteration** — Single execution of the multi-step workflow for one task, spanning code implementation, validation, review, documentation, and commit phases. See [`workflow/runner.md`](workflow/runner.md) for the complete step sequence.

**Step** — One atomic operation within an iteration (e.g., "Implement", "Validate", "Code review").

**Workflow** — Complete orchestration of multiple iterations (one per task) until all tasks complete or user stops.

**State** — Current progress snapshot saved to `.snap/state.json` (default workflow), `.snap/sessions/<name>/state.json` (named session), or `.snap/adhoc/<hash>/state.json` (single ad hoc task file), enables resumable execution across interruptions.

**Session** — Named project workspace with isolated task directory, separate state tracking, and independent workflow execution. Each session has its own task files at `.snap/sessions/<name>/tasks/` and state at `.snap/sessions/<name>/state.json`.

**Named Session** — Session created via `snap new <name>` command, allowing multiple independent projects or features to be managed within a single repository.

**Default Session** — Special session named "default" automatically created by `snap plan` when no sessions exist on a fresh project. Enables planning workflow without requiring explicit `snap new <name>` first. Created via `session.EnsureDefault()` which is idempotent.

**Auto-detection** — Feature of `snap run` (without session name) that automatically selects the workflow session when exactly one session exists. If zero sessions exist, falls back to legacy layout. If multiple sessions exist, returns error with list.

**Legacy fallback** — Workflow mode used when no named sessions exist, falling back to task files in `docs/tasks/` directory or `--tasks-dir` flag. Uses global `.snap/state.json` for state tracking, not session-scoped state.

**Ad hoc task mode** — Workflow mode triggered by `snap run --task-file <path>`. Runs the full 10-step flow against exactly one task file, with no PRD, TECHNOLOGY, DESIGN, or session required. Uses a task-file-scoped state directory under `.snap/adhoc/`.

**Display Name** — Session name, tasks directory path, or ad hoc task file path shown in startup summary. For named sessions: shows session name (e.g., "auth"). For legacy layout: shows tasks directory path (e.g., "docs/tasks"). For ad hoc mode: shows the resolved task file path. Used in startup summary and --show-state output.

## Execution & Control

**Project Context** — `docs/context/` directory storing project context for coding agents and LLMs: conventions, terminology, and architecture decisions persistent across workflow runs. Not user-facing documentation.

**Provider** — LLM service used to generate implementations (Claude or Codex, set via `SNAP_PROVIDER` env var).

**Provider CLI validation** — Pre-flight check that verifies the selected provider's CLI binary exists in PATH before workflow execution. Fails early with helpful error message including installation link and alternative provider suggestion.

**Resume** — Ability to continue from exact step where workflow was interrupted, without repeating completed work.

**Startup Summary** — Plain-text line displayed at workflow start, showing display name (session name or tasks directory), provider name, total and completed task counts, and current action (starting/resuming). Format: `snap: <display-name> | <provider> | <N> tasks (<M> done) | <action>`.

**Prompt Hint** — Informational message shown on fresh workflow starts (TTY-only) reminding user they can type directives between steps. Suppressed on resume and in non-interactive environments.

**Plan command** — `snap plan [session]` generates planning documents (PRD, TECHNOLOGY, DESIGN, TASK files) for a session through a two-phase pipeline: Phase 1 (interactive requirements gathering) and Phase 2 (autonomous document generation).

**Two-phase planning** — Planning pipeline consisting of Phase 1 (interactive chat with Claude) and Phase 2 (autonomous document generation). Phase 1 gathers requirements, Phase 2 generates structured planning documents based on requirements.

**Phase 1 (Requirements gathering)** — Interactive phase of planning where user chats with Claude. User types messages to provide context and answer clarifying questions, then types `/done` to complete Phase 1 and advance to Phase 2. Skipped if `--from` flag provides input file.

**Phase 2 (Document generation)** — Autonomous phase of planning where Claude generates planning documents: PRD.md (requirements), TECHNOLOGY.md (architecture), DESIGN.md (specifications), TASKS.md (task summary), and individual TASK<N>.md files. Document generation uses a 4-step pipeline: (1) PRD generated sequentially, (2) TECHNOLOGY and DESIGN generated concurrently via errgroup, (3) Analyze tasks in a fresh conversation (no -c) — creates the task list, enforces traceability, assesses against six anti-patterns, refines via merge/absorb/split/rework, and validates context alignment, (4) Generate tasks via -c continuation — writes TASKS.md (sections A–J) and spawns subagents to write each TASK<N>.md. All Phase 2 prompts are prepended with shared engineering principles to guide consistent decision-making across all generated documents.

**Engineering Principles** — Shared decision-making guidelines (KISS, DRY, SOLID, YAGNI) that steer all Phase 2 planning document generation. Principles resolve conflicts by favoring simplicity: straightforward solutions that work today are better than elegant abstractions that anticipate tomorrow.

**Principles Preamble** — The prepended engineering principles text added to all Phase 2 plan prompts (PRD, Technology, Design, Tasks). Guides LLM decision-making consistently across all generated planning documents. Defined in `internal/plan/prompts/principles.md`.

**UI Surface Awareness** — Set of clarifying questions in Phase 1 requirements gathering that ensures the planning process accounts for user-facing output characteristics. Asks about primary UI surface (CLI/TUI/Web/API output), states that must be handled (success, error, empty, loading), accessibility requirements (keyboard navigation, contrast, screen reader), terminal width/viewport expectations, and UI anti-pattern preferences. Confirms if project is headless or API-only to skip UI questions if appropriate.

**Contract Rules** — Testable assertions in DESIGN.md phrased as MUST / MUST NOT statements covering: terminology rules (which terms to use/avoid), content and message patterns, formatting and layout rules, accessibility requirements, and anti-patterns. Capped at 30 rules prioritizing prevention of most common quality failures. Enables verification that generated content conforms to design language.

**UI State Matrix** — Table in DESIGN.md with one row per (flow × state) combination, showing expected behavior or message for each state (success, error, empty, loading). Auto-generated from PRD core flow and use cases. Defines exactly what output the user sees in each scenario, enabling consistent behavior across all flows.

**UI-Undefined Task** — Anti-pattern #6 detected during task analysis. A task has user-facing impact (modifies terminal output, user-visible messages, or interactive prompts) but lacks concrete UI deliverables or measurable UI acceptance criteria. Section 4 (UI Deliverables) is empty/vague, and section 11 (Acceptance Criteria) has no UI-specific pass/fail assertions. Verdict: REWORK to include concrete UI specifications tied to DESIGN.md contract rules and state matrix.

**Context Alignment Check** — Validation step during task analysis that compares each task's scope against `docs/context/*` constraints (practices.md, terminology.md, domain files). Ensures tasks don't silently diverge from established project conventions. Tasks can either follow existing conventions or include updating `docs/context/` as part of their scope.

**User-facing flag** — Binary indicator (yes/no) in task specification section 0 that denotes whether a task produces user-visible output. Used to filter tasks requiring UI deliverables and DESIGN.md compliance from infrastructure-only tasks.

**Plan marker** — Hidden file `.plan-started` created in session directory when plan command starts, used for session status tracking to distinguish planning-in-progress from completed planning.

**Planning Artifacts** — Generated planning documents stored in a session's tasks directory: TASK\*.md (task files), PRD.md (product requirements), TECHNOLOGY.md (technology decisions), DESIGN.md (design specifications). Detected by `HasArtifacts()` function to prevent accidental overwriting.

**Artifact Conflict** — Occurs when `snap plan` is run on a session that already contains planning artifacts. Prevents accidental overwriting of existing planning documents. In TTY mode, uses `tap.Select` to let user choose between cleaning up and re-planning, or creating a new session. In non-TTY mode, returns error with cleanup instructions.

**Conflict Guard** — Safety mechanism in `snap plan` that detects existing planning artifacts before starting or resuming planning. Implemented via `checkPlanConflict()` function. Handles TTY vs. non-TTY scenarios differently to prevent automated environments from silently overwriting artifacts. In TTY mode, uses `tap.Select` to present two options (clean up and re-plan, or create a new session); on "new session" choice, uses `tap.Text` for name entry with inline validation.

**Raw mode** — Terminal mode where input is not line-buffered; each keystroke is immediately available to the application. Implemented via `termios` on Unix. Used by the run command's input reader for interactive step control.

**Interactive input** — User input from a TTY terminal via tap components (`tap.Text`, `tap.Textarea`, `tap.Select`). Provides styled text input with placeholder text, multiline text input for requirements gathering, selection prompts with arrow-key navigation, Ctrl+C and Escape (abort), validation callbacks, and context cancellation.

**TAP Text** — Component from `github.com/yarlson/tap` library providing styled interactive text input with validation. Supports Ctrl+C (cancel), Escape (cancel), validation callbacks, and context cancellation. Used in conflict guard for session name entry.

**TAP Textarea** — Component from `github.com/yarlson/tap` library providing styled multiline text input with validation. Supports line-by-line editing, cursor navigation, Shift+Return for new lines, Ctrl+C (cancel), Escape (cancel), validation callbacks, and context cancellation. Used in Phase 1 of plan command for interactive requirements gathering.

**TAP Select** — Component from `github.com/yarlson/tap` library providing styled selection prompt with arrow-key navigation. First option is pre-selected; user navigates with up/down arrows, confirms with Enter, cancels with Ctrl+C or Escape (returns zero value). Used in plan command conflict guard for replan/new-session choice.

**TAP Confirm** — Component from `github.com/yarlson/tap` library providing styled Yes/No confirmation prompt with radio buttons. User toggles with left/right arrows, confirms with Enter, or presses y/n directly. Cancels with Ctrl+C (returns false). Used in delete command for session deletion confirmation.

**TAP Mock Pattern** — Testing strategy for tap components using `tap.NewMockReadable()`, `tap.NewMockWritable()`, and `tap.SetTermIO(in, out)`. Tests run the component in a goroutine and emit keypresses asynchronously with `time.Sleep` for synchronization. Tests using `SetTermIO` must NOT run in parallel (global state). After validation error, tap keeps field content — tests must emit backspace characters to clear before typing corrected input.

**Brief** — Extracted requirements from Phase 1 conversation or provided via `--from` file, used as input to Phase 2 document generation prompts.

**Snapshot** — Git stash checkpoint created after each workflow step, capturing working tree state for debugging and recovery (optional, disabled by default).

**Signal handling** — OS signal (SIGINT from Ctrl+C, SIGTERM from system) interrupts the workflow. Runner writes interrupt message via `SwitchWriter.Direct()` to ensure visibility, then cancels context. Root-level handler maps `context.Canceled` to exit code 130 (standard SIGINT convention). All deferred cleanup runs before process exit, preserving state for resumability.

## UI & Output

**Styling** — Terminal text formatting using ANSI color codes and weight styles (bold, dim, normal).

**Duration** — Elapsed time for completed task, formatted as "Xh Ym" or "Xm Ys" and displayed in task completion message.

**Task Summary** — One-line description of a task, max 60 characters, generated by fast LLM from task content and displayed below task header in dim styling.

**State Display** — Human-readable one-line summary of workflow progress (current task, step, completion counts). Can be inspected at any time via `--show-state` flag or output as raw JSON with `--json`. Format: "TASK_ID in progress — step X/Y: StepName — Z tasks completed" or "No active task — Z tasks completed".

**Color codes** — ANSI escape sequences for terminal colors (Primary, Secondary, Success, Error, Celebrate, Dim).

## Build & Release

**Version** — Semantic version string displayed via `--version` flag. Set to "dev" by default, injected at build time via `go build -ldflags "-X github.com/yarlson/snap/cmd.Version=vX.Y.Z"`.

**ldflags** — Linker flags passed to `go build` to inject version and other compile-time constants into the binary.

**Version Tags** — Git tags matching `v*` pattern (e.g., `v1.0.0`, `v2.1.3`). Push of version tag triggers automated release workflow.

**Release Workflow** — GitHub Actions workflow (`.github/workflows/release.yml`) triggered by version tag push. Runs lint, test, then GoReleaser to build and publish binaries.

**GoReleaser** — Tool that automates binary cross-compilation, packaging, and GitHub Releases publishing. Configured via `.goreleaser.yaml`. Builds for multiple OS/architecture combinations.

**Release Job** — Step in release workflow that runs GoReleaser after lint and test jobs pass. Builds binaries for linux/darwin on amd64/arm64 and publishes to GitHub Releases.

## Color Output Control

**NO_COLOR** — Environment variable following the [NO_COLOR](https://no-color.org/) standard. When set to any non-empty value, disables ANSI color output. Useful for CI/CD pipelines, log files, and environments that don't support color. Respected by all output formatting functions. Automatically disabled in non-TTY environments (piped output, file redirection).

**TTY Detection** — Automatic detection of whether output is connected to a terminal (TTY) or piped/redirected. Colors are automatically disabled in non-TTY contexts, enabling clean output for log aggregation and file storage without requiring environment variable configuration.
