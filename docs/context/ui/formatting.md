# UI: Text Formatting & Styling

## Overview

Package `internal/ui` provides styled text output functions for CLI display, including headers, status messages, error handling, task duration tracking, and bounded tool-output viewports.

## Core Functions

### Structure Functions

- **Header(text, description)** — Major section header with "▶" prefix and primary color; optional description line below in dim styling (max width truncated with "…" if needed)
- **Step(text)** — Step marker with "▶" prefix and secondary color
- **StepNumbered(current, total, text)** — Step with counter (e.g., "Step 2/9: Task name")

### Status Functions

- **Success(text)** — Green checkmark (✓) with success color and dim text
- **Error(text)** — Red X mark (✗) with error color and dim text
- **ErrorWithDetails(message, details)** — Error with multi-line details in tree format
- **DimError(text)** — Error message in dimmed red (combination of dim style and error color)
- **Complete(text)** — Sparkle (✨) with success color and bold text
- **CompleteWithDuration(text, elapsed)** — Completion message with right-aligned duration in dim styling
- **CompleteBoxed(taskName, filesChanged, linesAdded, testsPassing)** — Boxed completion summary with task name, file/line changes, and test status
- **StepComplete(text, elapsed)** — Step completion with right-aligned duration
- **StepFailed(text, elapsed)** — Step failure with right-aligned duration

### Data Formatting Functions

- **KeyValue(key, value)** — Key-value pair with bold key, normal value, colon-separated, newline-terminated
- **TaskDone(text)** — Completed checklist item with `[x]` marker in success color + bold, text dimmed, 2-space indent
- **TaskPending(text)** — Pending checklist item with `[ ]` marker, entire line dimmed, 2-space indent
- **TaskActive(text, suffix)** — In-progress checklist item with `[~]` marker in secondary color + bold, text normal, optional dimmed suffix in parentheses, 2-space indent

### Text Styling Functions

- **Info(text)** — Informational text with dim styling, newline-terminated (used for secondary information like "Next steps:", setup instructions)
- **Tool(text)** — Tool/provider reference text with specific styling
- **FormatToolOutput(text)** — Formats tool output; renders plain text and trims output over 10 lines down to the most recent 10 visible lines
- **FormatToolError(text)** — Same viewport behavior as `FormatToolOutput`, but uses error styling for failed tool output
- **Separator()** — Visual separator line (e.g., for dividing sections)

### Viewport Utilities

- **NewLineViewport(maxLines)** — Creates a virtual line buffer that retains only the most recent lines from streamed text
- **NewStreamViewport(writer, maxLines)** — Creates a live viewport that can redraw plain lines in place on a TTY while streamed tool output grows
- **LineViewport.Append(text)** — Adds streamed text while preserving trailing partial lines across chunks
- **LineViewport.VisibleLines()** — Returns the currently visible slice of the virtual buffer
- **LineViewport.TotalLines()** — Returns how many logical lines have been observed
- **LineViewport.Overflowed()** — Reports whether older lines were trimmed from view
- **StreamViewport.Append(text, isError)** — Updates the live viewport with a streamed chunk
- **StreamViewport.FinalText()** — Returns the current visible text snapshot for non-TTY rendering

### Duration Functions

- **FormatDuration(d)** — Converts `time.Duration` to human-readable string
  - Input: `time.Duration` (negative values treated as 0)
  - Output: Format rules:
    - `<60s` → "45s"
    - `1m–59m` → "2m 34s"
    - `≥60m` → "1h 12m"
  - Zero components omitted, sub-second durations show "0s"

### Execution Control Functions

- **Interrupted(text)** — Message shown when execution interrupted (warning symbol with bold styling)
- **InterruptedWithContext(text, currentStep, totalSteps)** — Interrupted message with step context and resume instructions

### Writer Utilities

- **NewSwitchWriter(underlying, opts...)** — Wraps an `io.Writer` to conditionally transform output
  - Accepts underlying writer (e.g., `os.Stdout`)
  - Supports functional options (WithLFToCRLF, etc.)
  - Used to apply line-ending conversion or other transformations based on output context
- **WithLFToCRLF()** — Option for NewSwitchWriter that converts LF (`\n`) to CRLF (`\r\n`) in all writes
  - Useful for Windows terminal compatibility when output is piped to terminal
  - Only applied when explicitly requested via NewSwitchWriter option

### Startup & Summary Functions

- **FormatStartupSummary(tasksDir, provider, taskCount, doneCount, action)** — Plain-text startup summary (no ANSI codes)
  - Format: `snap: <tasksDir> | <provider> | <N> tasks (<M> done) | <action>`
  - Example: `snap: docs/tasks/ | claude | 3 tasks (1 done) | starting TASK2`
  - Example: `snap: docs/tasks/ | claude | 3 tasks (1 done) | resuming TASK2 from step 5`
  - Uses singular "task" when taskCount == 1
  - Displayed at startup before workflow begins

### Utility Functions

- **StripColors(text)** — Removes ANSI color codes from string
- **VerticalSpace(lines)** — Returns newline string of specified line count (0 returns empty string)
- **ResolveColor(colorType)** — Returns ANSI escape code for color token (respects NO_COLOR and TTY)
- **ResolveStyle(styleToken)** — Returns ANSI escape code for style token (bold/dim/normal)
- **ResetColorMode()** — Re-evaluates color mode based on NO_COLOR environment variable (for testing)

## Styling System

### Color Control

Colors are automatically managed through `ResolveColor()` and `ResolveStyle()` functions which respect:

- **NO_COLOR environment variable** — Set to any non-empty value to disable all ANSI color codes
- **TTY detection** — Automatically disables colors when output is piped or redirected (non-TTY)
- **Runtime evaluation** — Colors resolved at call time, not build time, enabling dynamic mode changes

### Colors

- **ColorPrimary** — Major sections/headers (teal #00af87)
- **ColorSecondary** — Steps/subsections (sky blue #5fafff)
- **ColorTertiary** — Supporting text (medium gray #8a8a8a)
- **ColorSuccess** — Completed actions (forest green #00af5f)
- **ColorError** — Failures (warm red #d75f5f)
- **ColorWarning** — Warnings/interrupts (amber #ffaf00)
- **ColorInfo** — Neutral information (soft cyan #5fd7d7)
- **ColorTool** — Tool invocations (lavender #af87ff)
- **ColorCelebrate** — Completions (lime #87ff87)
- **ColorDim** — Low-priority output (charcoal #585858)

### Styles (Weights)

- **WeightBold** — Emphasize text (headers, success/error markers)
- **WeightDim** — Reduce emphasis (secondary info, durations)
- **WeightNormal** — Regular text, reset codes

### Spacing Constants

- **BoxWidth** — Width of bordered containers
- **BoxPaddingLeft/Right** — Horizontal padding inside boxes
- **ToolOutputMaxLines** — Maximum number of visible lines retained in the tool-output viewport
- **SpaceXS, SpaceSM, SpaceMD** — Vertical spacing (newline counts)
- **SeparatorWidth** — Width for right-aligned content (duration alignment)
- **IndentResult** — Left indent for result lines

## Integration Points

- **Runner** (`internal/workflow/runner.go`) — Uses `CompleteWithDuration()` for task completion; calls `FormatStartupSummary()` at startup to show workflow state; uses `Info()` for snapshot messages and `DimError()` for queued prompt failures
- **Step execution** — Uses `Step()`, `StepNumbered()` for progress display; `StepComplete()`, `StepFailed()` for outcomes
- **Error handling** — Uses `Error()`, `ErrorWithDetails()`, `DimError()` for failure output
- **Plan command** (`cmd/plan.go`) — Uses `Step()` for phase headers in planner; `Info()` for completion messages and file listings
- **Planner** (`internal/plan/planner.go`) — Uses `Step()` for "Gathering requirements" and "Generating planning documents" messages; `Interrupted()` for abort messages; `Info()` for file preservation messages and progress output
- **Run command** (`cmd/run.go`) — Uses `Info()` for warning messages (PRD not found) and "No state file exists" output
- **New command** (`cmd/new.go`) — Uses `Info()` to format instructional text for "Next steps" output
- **List command** (`cmd/list.go`) — Uses `Info()` for empty state messages; directly applies `ResolveStyle()` styling codes (bold/dim/reset) to table output for visual hierarchy
- **Status command** (`cmd/status.go`) — Uses `KeyValue()` for session metadata display, `Info()` for section headers and summary messages, `TaskDone()`/`TaskActive()`/`TaskPending()` for task state display
- **Interrupts** — Uses `Interrupted()`, `InterruptedWithContext()` to display execution interruption messages
- **Claude/Codex executors** (`internal/claude/executor.go`, `internal/codex/executor.go`) — Route multiline tool results through the 10-line viewport to prevent large command output from bloating the main transcript; Codex also consumes streamed command-output delta events when available
