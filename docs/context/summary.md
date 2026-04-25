# snap — Project Summary

## What

**snap** is an autonomous CLI tool for feature implementation from requirements to production. It discovers and implements individual task files with full TDD workflow, validates changes, performs code review, and commits results — all unattended in a continuous loop.

## Architecture

```
snap CLI Entry Point (main.go)
  ↓
Workflow Runner (orchestrates 10-step iteration workflow)
  ├─ Step Runner (executes individual workflow steps)
  ├─ Prompt Queue (manages LLM call sequencing)
  ├─ State Manager (tracks progress, resumable)
  ├─ Model Layer (Task, Result definitions)
  └─ Output Formatting (UI package)
      ├─ Format functions (text styling, progress)
      ├─ Color & style codes (terminal styling)
      └─ Duration formatting (task timing display)
```

## Core Flow

1. **Load State** — Resume from prior progress or start fresh
2. **Select Task** — Discover and pick next incomplete task from task files
3. **Run Iteration** (10 steps per task):
   - Implement feature (LLM)
   - Check completeness
   - Validate (lint, test)
   - Code review (LLM review + feedback)
   - Fix issues if needed
   - Re-validate fixes
   - Update user-facing docs
   - Commit implementation
   - Update context docs
   - Commit context
4. **Loop** — Repeat until all tasks done or interrupted
5. **Save State** — State persists after every step for resumability

## System State

- **Task files**: Stored in `docs/tasks/` (legacy) or `.snap/sessions/<name>/tasks/` (session-scoped); naming: TASK1.md, TASK2.md, etc. (case-sensitive, uppercase required). `snap run --task-file <path>` also supports one ad hoc task file with any filename in any location
- **Task discovery**: Scanner finds and orders valid task files; detects case mismatches and PRD-embedded headers; PRD.md is optional overview document. In ad hoc mode, runner synthesizes a single-task inventory from the provided file path
- **Sessions**: Named project workspaces, each with isolated tasks directory and independent state; auto-detection uses exactly-one-session heuristic
- **Session state**: Saved to `.snap/sessions/<name>/state.json` for named sessions, `.snap/state.json` for legacy layout, or `.snap/adhoc/<hash>/state.json` for ad hoc single-task runs (tracks completed tasks, current task, step progress)
- **Run command**: Supports `snap run [session]` with four modes: named session, auto-detection (single session), legacy fallback (docs/tasks/), or ad hoc single-task mode via `--task-file`
- **Context**: `docs/context/` stores project context for future runs
- **Terminal output**: Uses ANSI colors, styled headers, progress indicators, task durations, diagnostic hints, and a boxed 10-line viewport for oversized tool output

## Capabilities

- **TDD workflow**: Write tests, implement, validate, review
- **Planning workflow**: `snap plan [session]` generates structured planning documents (PRD, TECHNOLOGY, DESIGN, TASK files) through two-phase pipeline (interactive requirements gathering with tap.Textarea multiline input for TTY or buffered input for pipes, then autonomous document generation with parallel execution of TECHNOLOGY and DESIGN documents); includes conflict guard to detect and handle existing planning artifacts; auto-creates "default" session on fresh projects with no sessions
- **Session management**: `snap new <name>` creates named workspaces; `snap list` displays all sessions with task counts and status; `snap delete <name>` removes sessions (with confirmation or --force flag); `snap status [session]` shows detailed progress with task state and current step; each session has isolated tasks directory and state at `.snap/sessions/<name>/`
- **Plan command**: `snap plan [session]` with two-phase pipeline: Phase 1 (interactive requirements gathering via tap.Textarea on TTY or buffered input on pipes), Phase 2 (4-step autonomous document generation: 1) PRD sequential, 2) TECHNOLOGY + DESIGN parallel, 3) Analyze tasks in fresh conversation (creates task list, runs anti-pattern assessment, refines, validates context alignment), 4) Generate tasks via -c continuation (writes TASKS.md and spawns subagents for individual TASK<N>.md files)); guided by engineering principles (KISS, DRY, SOLID, YAGNI); conflict guard detects existing planning artifacts with TTY choice (clean up and re-plan vs. create new session) or non-TTY error; supports `--from <file>` to skip Phase 1; auto-creates "default" session if none exist; session auto-detection for single-session projects; Ctrl+C aborts planning gracefully
- **Run command**: `snap run [session]` with four modes: explicit session, auto-detect (single session), legacy fallback (docs/tasks/), or `--task-file <path>` for a single arbitrary task file; supports `--fresh` flag for state reset
- **Multi-provider support**: Claude (default) or Codex via env var
- **Provider validation**: Pre-flight check ensures provider CLI is available in PATH before execution
- **Task discovery diagnostics**: Detects case-mismatched filenames (task1.md vs TASK1.md) and PRD-embedded task headers, provides corrective hints
- **Resumable execution**: Survives interruption, resumes exactly where left off; state persists independently per session
- **Startup summary**: Displays workflow state (session name or tasks directory, provider, task counts, action) on startup
- **Prompt hint**: Interactive reminder on fresh starts (TTY-only) that user can type directives between steps
- **Automated code review**: Built-in review step with feedback loop
- **Task duration tracking**: Shows elapsed time for each completed task
- **Persistent context**: `docs/context/` accessible to every task implementation
- **Snapshot capture**: Optional git stash snapshots after each workflow step for debugging and recovery
- **Task summaries**: Auto-generates one-line task descriptions during iteration setup for better context
- **Version flag**: `--version` displays snap version (set at build time via ldflags)
- **State inspection**: `--show-state` displays workflow progress in human-readable format; `--show-state --json` outputs raw state JSON; works with sessions and `--task-file`
- **Color output control**: NO_COLOR environment variable support (follows https://no-color.org/ standard); automatic color disabling in non-TTY contexts
- **Bounded tool output**: Multiline tool results are capped to the most recent 10 lines in a boxed viewport so long command output does not flood the main transcript
- **Auto-push and PR creation**: Upon workflow completion, automatically pushes commits to configured git remote (origin); on GitHub remotes, creates a pull request with LLM-generated title and description (using PRD context); skips PR if on default branch or PR already exists
- **CI workflow monitoring**: Detects GitHub Actions workflows triggered by push or pull_request events; polls CI status after push/PR creation; displays status updates and final results; tracks individual checks (≤5) or summary view (>5 checks); automatically attempts to fix failing checks with LLM-generated minimal code changes (up to 10 attempts)
- **CI/CD integration**: GitHub Actions workflow validates lint and race conditions on every push and PR
- **Release automation**: Automated binary builds and GitHub Releases via GoReleaser (linux/darwin, amd64/arm64); triggered on version tag push; workflow enforces lint and test before release

## Tech Stack

- **Language**: Go 1.25.6
- **CLI Framework**: Cobra
- **Testing**: testify (assert/mock)
- **Markdown rendering**: charmbracelet/glamour
- **Styling**: charmbracelet/lipgloss (ANSI colors/styles)
- **Terminal I/O**: golang.org/x/term
- **Concurrency**: golang.org/x/sync (errgroup for parallel document generation)
- **Interactive terminal components**: github.com/yarlson/tap (Text input with validation, Textarea multiline input, Select prompts with arrow-key navigation)
