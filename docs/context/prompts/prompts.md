# Prompts: LLM Prompt Templates

Package `internal/prompts` manages all embedded prompt templates used throughout the workflow. Each prompt is a Go text template that's embedded at compile time for use during workflow execution.

## Embedded Prompt Templates

### Implement

**File**: `implement.md`
**Purpose**: Generate implementation code for a task
**Parameters**: `PRDPath`, `TaskPath`, `TaskID`
**Function**: `Implement(ImplementData) (string, error)`
**Usage**: Step 1 of workflow iteration
**Key Sections**:

- Context — read CLAUDE.md, docs/context/, task file, and existing code patterns; read PRD when provided, and TECHNOLOGY.md / DESIGN.md / TASKS.md if present
- **Pre-Implementation Alignment** — build internal constraint checklist covering naming conventions from `docs/context/practices.md`, UI rules from DESIGN.md, accessibility requirements, and domain patterns; detect conflicts between context and design documents using resolution rule (context wins for established patterns, DESIGN.md wins for new patterns)
- Scope — implement only what task defines, follow established patterns, do not update project context
- Process — start with failing E2E/integration test, write minimal code to pass, run full test suite, verify all acceptance criteria met
- Quality Guardrails — security (no secrets, validate input), reliability (close resources, handle errors), performance (no N+1), simplicity (no premature abstractions), dependencies (prefer stdlib, check active maintenance), architecture (separate business logic from I/O)

### Ensure Completeness

**File**: `ensure_completeness.md`
**Purpose**: Verify task implementation covers all requirements
**Parameters**: `TaskPath`, `TaskID`
**Function**: `EnsureCompleteness(EnsureCompletenessData) (string, error)`
**Usage**: Step 2 of workflow iteration
**Key Sections**:

- Context — read CLAUDE.md, docs/context/, task file, implementation code and tests
- **Criterion-to-Evidence Mapping** — for each acceptance criterion, identify covering evidence (passing test or artifact), produce mapping table with columns: criterion text, evidence (test name or artifact), status (COVERED / MISSING); for missing criteria write failing test then minimal code to pass; after all criteria mapped, run full test suite
- **UI Verification** — conditional on task's `user-facing: yes/no` flag (from task section 0); for user-facing tasks: verify UI states from section 4 (UI Deliverables) are implemented, verify DESIGN.md contract rules applicable to task are followed, verify accessibility requirements from DESIGN.md are met, capture actual output and verify against expected behavior; any unmapped or failing UI criterion must be addressed with failing test then minimal code
- Scope — complete only current task work, do not refactor or start next task, do not update project context

### Lint and Test

**File**: `lint_and_test.md`
**Purpose**: Guide linting and testing validation
**Parameters**: None (plain string)
**Function**: `LintAndTest() string`
**Usage**: Step 3 of workflow iteration

### Code Review

**File**: `code_review.md`
**Purpose**: Perform automated code review with feedback
**Parameters**: None (plain string)
**Function**: `CodeReview() string`
**Usage**: Step 4 of workflow iteration
**Key Sections**:

- Phases 1–5 — Security, bugs, logic, performance, architecture, testing categories with severity levels (CRITICAL, HIGH, MEDIUM, LOW)
- **Phase 6: UI Compliance** — Conditional on task's `user-facing: yes/no` flag; validates user-facing implementations against DESIGN.md and `docs/context/` conventions; checks missing required states, formatting/hierarchy violations, accessibility failures, context violations, and task scope mismatches; categories include `ui-compliance` with severity HIGH or CRITICAL

### Apply Fixes

**File**: `apply_fixes.md`
**Purpose**: Address code review feedback and fix issues
**Parameters**: None (plain string)
**Function**: `ApplyFixes() string`
**Usage**: Step 5 of workflow iteration

### Update Docs

**File**: `update_docs.md`
**Purpose**: Update user-facing documentation based on code changes
**Parameters**: `UpdateDocsData{TaskPath, TaskID}` (optional — empty when no specific task)
**Function**: `UpdateDocs(data UpdateDocsData) (string, error)`
**Usage**: Step 7 of workflow iteration

### Commit

**File**: `commit.md`
**Purpose**: Generate conventional commit messages
**Parameters**: None (plain string)
**Function**: `Commit() string`
**Usage**: Step 8 of workflow iteration

### Memory Update

**File**: `memory_update.md`
**Purpose**: Update `docs/context/` with current project state
**Parameters**: None (plain string)
**Function**: `MemoryUpdate() string`
**Usage**: Step 9 of workflow iteration
**Key Sections**:

- Standard memory vault workflow — identify changes, map to context topics, update terminology, practices, summary, and context-map
- **What to Record** — Proven patterns (implemented in source code and validated by tests), rejected anti-patterns (patterns considered and deliberately rejected with rationale)
- **What NOT to Record** — Speculative design intent, planned-but-unimplemented UI conventions, DESIGN.md rules not exercised by code, aspirational standards not yet enforced

### Task Summary

**File**: `task_summary.md`
**Purpose**: Generate one-line task description (max 60 characters)
**Parameters**: `TaskContent` (task file content, truncated to 2000 bytes)
**Function**: `TaskSummary(TaskSummaryData) (string, error)`
**Usage**: Workflow runner displays task summary in header before iteration starts
**Output**: Single sentence, no jargon, plain language, max 60 characters

## Planning Prompts

Package `internal/plan` manages the prompts used by `snap plan`. Every Phase 2 generator now produces sections ending in a `Grounded in:` footer (BRIEF section + repo file/symbol citation). A per-artifact critic deletes any uncited content; the engineering-principles preamble (`principles.md`) and the legacy 6-anti-pattern / traceability-gate / context-alignment ceremonies have been removed.

### Requirements Prompt

**File**: `internal/plan/prompts/requirements.md`
**Purpose**: Guide Phase 1 interactive requirements gathering and instruct Claude to write `BRIEF.md` after `/done`.
**Usage**: Phase 1 of `snap plan`.
**Key Sections**:

- Context — read CLAUDE.md, docs/context/, scan codebase
- Process — focused questions, one or two at a time, build on prior answers
- Scope Lock — explicit constraints/exclusions are fixed; do not suggest adjacent features or future phases
- Final Step — when the user types `/done`, immediately write `{{.BriefPath}}` with the seven required sections (Problem / Users / In scope / Non-goals / Success criteria / Constraints / Open questions); empty sections emit `(none)`

### Brief Synthesis Prompt

**File**: `internal/plan/prompts/brief.md`
**Purpose**: Reinforces the BRIEF.md write step in case the requirements prompt is consumed mid-conversation.
**Usage**: Sent with `-c` immediately after `/done` so Claude has the chat history for synthesis.

### Triage Prompt

**File**: `internal/plan/prompts/triage.md`
**Purpose**: Classify BRIEF.md into tier `tiny|small|full` plus `has_architecture` and `has_ui` flags.
**Usage**: Run after BRIEF.md is finalised, before Phase 2; `model.Fast`, no `-c`.
**Output**: Exactly one JSON line: `{"tier":"...","has_architecture":bool,"has_ui":bool,"rationale":"..."}`. Unparseable output falls back to `tier=full` with both flags true.

### Critic Prompt

**File**: `internal/plan/prompts/critic.md`
**Purpose**: Per-artifact reviewer that deletes uncited content in place.
**Usage**: Run on `model.Fast` (fresh, no `-c`) after each writer step. Reads BRIEF + the artifact + every repo file cited in `Grounded in:` footers; deletes sections without supporting evidence and rewrites the file via the LLM's Write tool.
**Forbidden patterns enforced**: "could", "might", "consider", "future", "later phase", "stretch goal", "nice-to-have"; sections labeled "Optional" / "Future enhancements" / "Nice to have"; `Grounded in:` footers citing files without specific line ranges or symbols.

### PRD Prompt

**File**: `internal/plan/prompts/prd.md`
**Purpose**: Produce `PRD.md` from BRIEF + repo evidence.
**Key changes from prior version**: Stripped the "make assumptions to fill blanks" license. Required `## Repo Evidence` section (3–5 cited file paths). Every section ends with `Grounded in:` footer. Forbids "consider", "could", "future", "later", "nice-to-have", "stretch".

### Technology Prompt

**File**: `internal/plan/prompts/technology.md`
**Purpose**: Produce `TECHNOLOGY.md` (full tier with `has_architecture=true` only).
**Key changes**: Same input model (BRIEF + PRD + Repo Evidence + Grounded-in footer). The 30-line embedded testing-philosophy block is replaced by a one-line "Follow CLAUDE.md/AGENTS.md testing conventions" reference.

### Design Prompt

**File**: `internal/plan/prompts/design.md`
**Purpose**: Produce `DESIGN.md` (full tier with `has_ui=true` only).
**Key changes**: Same input model. Contract Rules and UI State Matrix preserved (cap at 30 rules). Each rule and each State Matrix row carries its own `Grounded in:` citation.

### Analyze Tasks Prompt

**File**: `internal/plan/prompts/analyze-tasks.md`
**Purpose**: Create the task list (in conversation) from BRIEF + PRD + TECHNOLOGY + DESIGN at full tier.
**Key changes**: Cut to ~60 lines. **Removed** the 6-anti-pattern block, "Context Alignment Check", "Traceability Gate", and conflict-resolution boilerplate (the critic replaces these). **Kept** the vertical-slice definition, walking-skeleton conditional, sizing heuristics, breadth-first sequencing, and the `Grounded in:` requirement per task.

### Generate Tasks Prompt

**File**: `internal/plan/prompts/generate-tasks.md`
**Purpose**: Write `TASKS.md` (sections A–J) and spawn parallel subagents for the 15-section `TASK<N>.md` files. Full tier only.
**Key changes**: Removed the duplicate guardrail wall and the per-subagent Testing & Quality block. Every TASK<N>.md section MUST end with `Grounded in:` — uncited sections are deleted by the per-task critic that runs after subagents complete.

### Slim Task Prompt

**File**: `internal/plan/prompts/task-slim.md`
**Purpose**: Generate one `TASK<N>.md` in the slim 6-section format for tiny + small tiers.
**Sections** (always in this order): Outcome, Scope, Acceptance, Files likely touched, Verification, Grounded in (overall). Each section carries a `Grounded in:` footer. At tiny tier the prompt asks for 1–3 repo file citations; at small tier 3+.

### Slim TASKS.md Prompt

**File**: `internal/plan/prompts/tasks-md-slim.md`
**Purpose**: Index TASK1..3.md at small tier with the section-G heading required by the workflow runner.
**Escape hatch**: If In-scope expands beyond 3 vertical-slice tasks, the prompt writes `TIER_MISMATCH: ...` instead of a real index. The dispatcher detects that string and aborts with a recommendation to re-plan at full tier.

## Implementation Pattern

All templated prompts follow the same pattern:

1. Go file embeds markdown template using `//go:embed`
2. `promptData` struct holds template parameters (`TasksDir`, `BriefPath`, `ArtifactPath`, `TaskNum`, `HasPRD`)
3. `renderTemplate` parses + executes against the data
4. Result trimmed and returned as string

Non-templated prompts (LintAndTest, CodeReview, etc.) are returned as plain strings.

## Integration Points

- **Workflow Runner** — Uses workflow prompts (Implement through Commit) in sequence per task
- **Plan Command** — Uses planning prompts (Requirements, Brief Synthesis, Triage, Critic, PRD, Technology, Design, Analyze Tasks, Generate Tasks, slim Task, slim TASKS.md) per the tier dispatcher
- **Task Summary** — Called during iteration setup to generate brief description
- **Step Runner** — Executes prompts via LLM and returns results
