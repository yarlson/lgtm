# Infrastructure: Post-run Operations

Post-run operations execute after all tasks are completed. These steps automate git push and GitHub PR creation.

## Post-run Module

**Files**:

- `internal/postrun/postrun.go` — Post-run orchestration and CI fix loop
- `internal/postrun/git.go` — Git remote detection, push, branch tracking, and commit creation
- `internal/postrun/github.go` — GitHub API operations (PR creation, CI status checking, log fetching)
- `internal/postrun/workflow.go` — CI workflow detection
- `internal/postrun/prompts/pr.md` — LLM prompt template for PR title/body generation
- `internal/postrun/prompts/ci_fix.md` — LLM prompt template for CI failure diagnosis and fixing
- `internal/postrun/prompts/prompts.go` — Template rendering for both PR and CI fix prompts
- `internal/postrun/parse.go` — LLM output parsing for PR title/body

## Workflow

After all tasks complete, the workflow runner calls `postrun.Run()` with:

```go
postrun.Run(ctx, postrun.Config{
    Output:       output,
    Executor:     executor,       // LLM executor for PR generation
    RemoteURL:    remoteURL,      // Pre-detected remote URL (empty = no remote)
    IsGitHub:     isGitHub,       // Pre-detected GitHub flag
    PRDPath:      prdPath,        // PRD.md path for PR body context
    TasksDir:     tasksDir,       // Tasks directory
    RepoRoot:     repoRoot,       // Repository root for workflow detection (defaults to ".")
    PollInterval: pollInterval,   // CI status poll interval (defaults to 15s)
})
```

### Step Sequence

1. **Check for remote** — If no `origin` remote configured, skip push and exit cleanly
2. **Push to origin** — Run `git push origin HEAD` (never uses `--force`)
   - Displays progress message: "Pushing to origin..."
   - On success, displays completion with branch name and timing
   - On failure, returns error (workflow stops with error message)
3. **Check for GitHub** — If non-GitHub remote, skip GitHub-specific features and exit
4. **PR creation flow** (GitHub remotes only):
   - Get default branch via `gh repo view`
   - Skip PR creation if on default branch
   - Check if PR already exists via `gh pr view` (skip if exists)
   - Hand off to the LLM agent with a prompt containing the PRD, commit messages, and diff stat. The agent drafts the title and body and invokes `gh pr create` itself in a single step
   - Verify the PR was created via `gh pr view`; surface an error if the agent did not create one
   - Display PR URL and number
5. **CI status monitoring** (all remotes with GitHub or after PR creation):
   - Detect if CI workflows exist via `HasRelevantWorkflows()` (scans `.github/workflows/*.yml`)
   - If no relevant workflows: Display "No CI workflows found, done" and exit
   - If workflows exist: Poll CI status via `CheckStatus()` (uses `gh pr checks` or `gh run list`)
   - Display status updates when check status changes (silent between polls)
   - Format status as individual checks (≤5) or grouped summary (>5)
   - On all checks passed: Display "CI passed — PR ready for review" (or "CI passed" for default branch)
   - **On any check failed: Enter CI fix loop** (see below)

## CI Fix Loop

When a CI check fails, snap automatically attempts to fix it:

### Fix Attempt Sequence (up to 10 times)

1. **Fetch failure logs**:
   - Call `FailedRunID()` to get the most recent failed workflow run ID via `gh run list --status failure`
   - Call `FailureLogs(runID)` to fetch logs via `gh run view <id> --log-failed`
   - Logs are truncated to 50KB to prevent LLM context overflow
   - Logs are held in memory only — never written to disk (for secrets safety)

2. **Diagnose and fix via LLM**:
   - Render CI fix prompt with failure logs, failed check name, and attempt number
   - Call `Executor.Run()` with `model.Fast` to invoke Claude
   - LLM is instructed to diagnose root cause and apply minimal fix
   - LLM is explicitly prohibited from modifying CI workflow files (`.github/workflows/`)

3. **Commit and push fix**:
   - Stage all changes via `git add -A`
   - Create new commit with message `fix: resolve <check-name> CI failure` (never amend)
   - Push fix via `git push origin HEAD` (never uses `--force`)

4. **Re-poll CI**:
   - Resume polling from step 5 of the main workflow
   - If CI passes: Complete with "CI passed — PR ready for review"
   - If CI fails again: Increment attempt counter and repeat (max 10 attempts)

### Termination Conditions

- **Success**: Any attempt where CI passes after fix
- **Max retries exhausted**: After 10 failed fix attempts, display "CI still failing after 10 attempts" and return error
- **Log fetch failure**: If `FailedRunID()` or `FailureLogs()` fails, return error with "Failed to read CI logs" message
- **Commit/push failure**: If `CommitAll()` or `Push()` fails, return error and stop

### Constants

- `maxFixAttempts = 10` — Hard limit on fix attempts
- `maxLogSize = 50 * 1024` — Log truncation threshold (50KB)

## Git Remote Detection

**Function**: `DetectRemote()` in `internal/postrun/git.go`

Returns the URL for the `origin` remote:

- **Success**: Returns remote URL string
- **No remote**: Returns empty string and nil error (not treated as an error)
- **Error**: Returns error only for actual git failures (e.g., not in a git repository, permission errors)

Uses `git remote get-url origin` under the hood.

## GitHub Remote Detection

**Function**: `IsGitHubRemote(remoteURL string)` in `internal/postrun/git.go`

Returns true if the remote URL points to github.com. Handles:

- **HTTPS format**: `https://github.com/user/repo.git`
- **SSH shorthand**: `git@github.com:user/repo.git`
- **SSH protocol**: `ssh://git@github.com/user/repo.git`

Returns false for empty URL or non-GitHub remotes.

## Git Push

**Function**: `Push(ctx context.Context)` in `internal/postrun/git.go`

Pushes the current branch to `origin` using `git push origin HEAD`:

- Never uses `--force` flag (safe by design)
- Captures stderr output for error reporting
- Returns `PushError` type wrapping git error with stderr output
- `PushError.Error()` displays stderr if available, else underlying error

## Current Branch

**Function**: `CurrentBranch(ctx context.Context)` in `internal/postrun/git.go`

Returns the name of the current branch using `git branch --show-current`.

Returns empty string for detached HEAD state.

Used in completion messages to show which branch was pushed, and to determine if PR creation should proceed.

## Diff Stat

**Function**: `DiffStat(ctx context.Context, baseBranch string)` in `internal/postrun/git.go`

Returns diff statistics between the base branch and HEAD using `git diff <baseBranch>...HEAD --stat`.

Used as input to the PR generation prompt to give the LLM context about what changed.

## Commit Messages

**Function**: `CommitMessages(ctx context.Context, baseBranch string)` in `internal/postrun/git.go`

Returns the full commit message bodies for commits between the base branch and HEAD using `git log -n 30 <baseBranch>..HEAD --reverse --format=%B%n`.

- Oldest commit first (mirrors reviewer reading order).
- Capped at the last 30 commits to keep the prompt budget bounded on long-lived branches.
- Returns `""` with nil error when the range is empty (e.g., already on the base branch).

Used as the primary "what shipped" input to the PR generation prompt — the LLM derives the `What` bullets from these messages rather than from the diff itself, so it never has raw code to regurgitate into the title.

## Commit All

**Function**: `CommitAll(ctx context.Context, message string)` in `internal/postrun/git.go`

Stages all changes and creates a new commit with the given message:

- Runs `git add -A` to stage all changes
- Runs `git commit -m "<message>"` to create a new commit
- Never uses `--amend` flag (always creates a new commit)
- Returns error with message "nothing to commit" if there are no staged changes
- Returns error with git error details if the commit fails

Used by the CI fix loop to commit fixes after the LLM has modified files.

## GitHub Operations

**File**: `internal/postrun/github.go`

### DefaultBranch

Retrieves the repository's default branch using `gh repo view --json defaultBranchRef -q .defaultBranchRef.name`.

Used to determine if the current branch is the default branch (in which case PR creation is skipped).

### PRExists

Checks if a PR already exists for the current branch using `gh pr view --json state,url`.

- Returns `(true, url, nil)` if a PR exists
- Returns `(false, "", nil)` if no PR exists (exit code 1 is not treated as an error)
- Returns error only for actual gh failures

Used for PR deduplication — prevents creating duplicate PRs.

### CheckStatus

Retrieves CI check status using `gh pr checks` or `gh run list`.

- **For PR branches**: Uses `gh pr checks --json name,state,conclusion` to get individual check results
- **For default branch**: Uses `gh run list --branch <branch> --json name,status,conclusion --limit 1` to get the latest workflow run
- Returns a slice of `CheckResult` structs with normalized statuses: "passed", "failed", "running", "pending"
- Normalizes GitHub's various status values (SUCCESS, PENDING, FAILURE, etc.) to our standard statuses

### FailedRunID

Finds the ID of the most recent failed workflow run using `gh run list --status failure --limit 1 --json databaseId`.

- Returns the workflow run ID as a string
- Returns error with message "no failed runs found" if no failures exist
- Used by the CI fix loop to locate which workflow run to fetch logs from

### FailureLogs

Fetches the failure logs from a workflow run using `gh run view <runID> --log-failed`.

- Returns the full log content as a string
- Truncates logs to 50KB (`maxLogSize`) to prevent LLM context window overflow
- Appends `[log truncated — exceeded 50KB limit]` marker if truncation occurred
- Logs are held in memory only — never written to disk (for secrets safety)
- Used by the CI fix loop to provide failure context to the LLM

## CI Workflow Detection

**File**: `internal/postrun/workflow.go`

### HasRelevantWorkflows

Detects if the repository has GitHub Actions workflows triggered by `push` or `pull_request`:

- Scans `.github/workflows/*.yml` and `*.yaml` files
- Uses text-based scanning (not full YAML parsing) for conservative matching
- Matches trigger lines like `on: push`, `on: [push, pull_request]`, `on:\n  push:`, etc.
- Returns `true` if at least one workflow has a relevant trigger
- Returns `false` if `.github/workflows/` doesn't exist or is empty
- Skips unreadable files gracefully (returns false, not error)

Used to determine if CI monitoring should proceed after push/PR creation.

## PR Generation

**File**: `internal/postrun/prompts/pr.md` and `internal/postrun/prompts/prompts.go`

The PR prompt template (`pr.md`) is delivered to the LLM agent (Claude or Codex), which has shell access via the executor. The agent is instructed to draft the title and body and then run `gh pr create` itself in a single step — there is no Go-side parsing of the agent's output. The prompt enforces:

- **Title** — free prose, imperative mood, 50–72 characters (hard cap 72), no scope prefix, no code or identifiers, no quotation marks. Single line. Describe the user-visible or behavioural change, not the mechanism.
- **Body** — three blocks (or two when no PRD is available), each with a level-3 markdown heading, in this order:
  - `### Why` — one or two sentences anchored in the PRD's motivation; paraphrased, never quoted. Skipped entirely when no PRD is supplied.
  - `### What` — bulleted summary derived from the commit messages, with related commits merged and noisy subjects collapsed.
  - `### How to verify` — bulleted concrete checks, derived from PRD Requirements when present, otherwise from commits and diff stat.
- **Length budget** — body targets ~150 words, hard ceiling 250.
- **Anti-patterns** — the prompt explicitly forbids code snippets / file contents in title or body, pasting the diff stat, quoting the PRD verbatim, inventing requirements not in the inputs, and filler phrasing like "This PR…".
- **Execution** — agent runs `gh pr create --title "<title>" --body "<body>"` exactly once, with no extra flags. If `gh` rejects the title (for example "Title is too long"), the agent shortens the title to fit the 72-character cap and retries once. After the PR is created, the agent stops.

The `PR()` function renders this template with:

- `{{.PRDContent}}` — Full PRD.md text (used for motivation only; absent → `Why` block is omitted)
- `{{.CommitMessages}}` — Output of `CommitMessages()` (oldest first, last 30 commits)
- `{{.DiffStat}}` — Output of `DiffStat()` (file-level scope; never quoted in body)

After the agent returns, `createPRFlow` calls `PRExists()` once to confirm the PR was created and to capture its URL for the success message. There is no Go-side parsing of the agent's textual output — the source of truth is GitHub.

## CI Fix Generation

**File**: `internal/postrun/prompts/ci_fix.md` and `internal/postrun/prompts/prompts.go`

The CI fix prompt template (`ci_fix.md`) instructs the LLM to:

- Diagnose the root cause from CI failure logs
- Apply the minimal code fix to make the failing check pass
- Never modify CI workflow files (`.github/workflows/`)
- Never add skip/ignore directives to bypass checks
- Focus only on fixing the specific failing check

The `CIFix()` function renders this template with:

- `{{.FailureLogs}}` — Truncated CI failure logs (max 50KB)
- `{{.CheckName}}` — Name of the failing check
- `{{.AttemptNumber}}` — Current attempt number (1-based)
- `{{.MaxAttempts}}` — Maximum attempts allowed (10)

After LLM execution, the modified files are committed (new commit) and pushed (no force).

## Integration Points

**Pre-flight checks** (in `cmd/run.go`):

1. Detect remote via `postrun.DetectRemote()`
2. Check if GitHub via `postrun.IsGitHubRemote(remoteURL)`
3. If GitHub, validate `gh` CLI via `provider.ValidateGH()` (see [`provider.md`](../cli/provider.md))
4. Pass `remoteURL` and `isGitHub` flags to workflow runner

**Completion** (in `internal/workflow/runner.go`):

- After all tasks complete, call `postrun.Run()` with detected remote info
- Runner's `selectIdleTask()` invokes post-run when no more tasks found

## Configuration

Post-run behavior is automatic and requires no user configuration:

- If no remote: Push is skipped
- If non-GitHub remote: Push succeeds, GitHub features skipped
- If GitHub remote: Push succeeds, gh CLI pre-validated during startup

The PRD context is passed to the PR generation prompt to create meaningful PR descriptions that explain the _why_ behind changes, not just raw diff summaries.
