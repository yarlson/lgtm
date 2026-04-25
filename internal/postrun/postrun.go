package postrun

import (
	"context"
	"fmt"
	"io"
	"os"
	"regexp"
	"strings"
	"time"

	"github.com/yarlson/snap/internal/model"
	"github.com/yarlson/snap/internal/postrun/prompts"
	"github.com/yarlson/snap/internal/ui"
)

// CheckResult represents the status of a single CI check.
type CheckResult struct {
	Name       string // Check/job name
	Status     string // "pending", "running", "passed", "failed"
	Conclusion string // Raw GitHub conclusion field
}

var prNumberRe = regexp.MustCompile(`/pull/(\d+)`)

// Executor runs an LLM call. Matches the workflow.Executor interface.
type Executor interface {
	Run(ctx context.Context, w io.Writer, mt model.Type, args ...string) error
}

// Config holds configuration for the post-run step.
type Config struct {
	Output       io.Writer
	Executor     Executor      // LLM executor for PR generation (nil = skip LLM, use default title)
	RemoteURL    string        // Pre-detected remote URL (empty = no remote)
	IsGitHub     bool          // Pre-detected GitHub flag
	PRDPath      string        // Path to PRD.md for PR body context
	TasksDir     string        // Tasks directory
	RepoRoot     string        // Repository root path for workflow detection (defaults to ".")
	PollInterval time.Duration // CI poll interval (defaults to 15s)
}

const (
	defaultPollInterval = 15 * time.Second
	maxFixAttempts      = 10
)

// Run executes the post-run step: push to remote, create PR if on GitHub, monitor CI.
func Run(ctx context.Context, cfg Config) error {
	if cfg.RemoteURL == "" {
		fmt.Fprint(cfg.Output, ui.Info("No remote configured, skipping push"))
		return nil
	}

	// Push to origin
	fmt.Fprint(cfg.Output, ui.Step("Pushing to origin..."))
	pushStart := time.Now()

	if err := Push(ctx); err != nil {
		return fmt.Errorf("push failed: %w", err)
	}

	branch, err := CurrentBranch(ctx)
	if err != nil {
		branch = "unknown"
	}
	fmt.Fprint(cfg.Output, ui.StepComplete(fmt.Sprintf("Pushed to origin/%s", branch), time.Since(pushStart)))

	if !cfg.IsGitHub {
		fmt.Fprint(cfg.Output, ui.Info("Non-GitHub remote, skipping PR and CI"))
		return nil
	}

	// PR creation flow — returns whether a PR exists for the branch
	hasPR, err := createPRFlow(ctx, cfg, branch)
	if err != nil {
		return err
	}

	// CI monitoring
	return monitorCI(ctx, cfg, hasPR, branch)
}

func createPRFlow(ctx context.Context, cfg Config, currentBranch string) (hasPR bool, err error) {
	// Detached HEAD — skip PR creation silently
	if currentBranch == "" || currentBranch == "unknown" {
		return false, nil
	}

	// Get default branch
	defaultBranch, err := DefaultBranch(ctx)
	if err != nil {
		return false, fmt.Errorf("failed to detect default branch: %w", err)
	}

	// On default branch — skip PR creation
	if currentBranch == defaultBranch {
		fmt.Fprint(cfg.Output, ui.Info("On default branch, skipping PR creation"))
		return false, nil
	}

	// Check if PR already exists
	exists, existingURL, err := PRExists(ctx)
	if err != nil {
		return false, fmt.Errorf("failed to check for existing PR: %w", err)
	}
	if exists {
		fmt.Fprint(cfg.Output, ui.Info(fmt.Sprintf("PR already exists: %s", existingURL)))
		return true, nil
	}

	// Generate PR title and body via LLM
	fmt.Fprint(cfg.Output, ui.Step("Creating pull request..."))
	prStart := time.Now()

	title, body := generatePR(ctx, cfg, defaultBranch)

	// Create PR
	prURL, err := CreatePR(ctx, title, body)
	if err != nil {
		fmt.Fprint(cfg.Output, ui.Error(fmt.Sprintf("PR creation failed: %s", err)))
		return false, fmt.Errorf("PR creation failed: %w", err)
	}

	// Extract PR number from URL
	prNumber := extractPRNumber(prURL)
	fmt.Fprint(cfg.Output, ui.StepComplete(fmt.Sprintf("PR %s created: %s", prNumber, prURL), time.Since(prStart)))

	return true, nil
}

func generatePR(ctx context.Context, cfg Config, defaultBranch string) (title, body string) {
	if cfg.Executor == nil {
		return "Update", ""
	}

	// Read PRD content
	var prdContent string
	if cfg.PRDPath != "" {
		data, readErr := os.ReadFile(cfg.PRDPath)
		if readErr == nil {
			prdContent = string(data)
		}
	}

	// Get diff stat and commit messages (best-effort, ignore errors).
	diffStat, _ := DiffStat(ctx, defaultBranch)             //nolint:errcheck // best-effort diff stat
	commitMessages, _ := CommitMessages(ctx, defaultBranch) //nolint:errcheck // best-effort commit log

	// Render prompt
	prompt, err := prompts.PR(prompts.PRData{
		PRDContent:     prdContent,
		CommitMessages: commitMessages,
		DiffStat:       diffStat,
	})
	if err != nil {
		return "Update", ""
	}

	// Call LLM
	var buf strings.Builder
	if err := cfg.Executor.Run(ctx, &buf, model.Fast, prompt); err != nil {
		return "Update", ""
	}

	// Parse LLM output
	title, body, parseErr := parsePROutput(buf.String())
	if parseErr != nil {
		return "Update", ""
	}

	return title, body
}

// monitorCI detects relevant CI workflows and polls check status until completion.
// On CI failure, it enters a fix loop: fetch logs, LLM fix, commit, push, re-poll.
func monitorCI(ctx context.Context, cfg Config, hasPR bool, branch string) error {
	repoRoot := cfg.RepoRoot
	if repoRoot == "" {
		repoRoot = "."
	}

	hasWorkflows, err := HasRelevantWorkflows(repoRoot)
	if err != nil {
		return fmt.Errorf("failed to detect CI workflows: %w", err)
	}
	if !hasWorkflows {
		fmt.Fprint(cfg.Output, ui.Info("No CI workflows found, done"))
		return nil
	}

	fmt.Fprint(cfg.Output, ui.Step("Waiting for CI checks..."))

	pollInterval := cfg.PollInterval
	if pollInterval == 0 {
		pollInterval = defaultPollInterval
	}

	attempt := 0
	var prev []CheckResult

	for {
		if ctx.Err() != nil {
			return nil //nolint:nilerr // context cancellation is a clean exit, not an error
		}

		checks, err := CheckStatus(ctx, hasPR, branch)
		if err != nil {
			if ctx.Err() != nil {
				return nil //nolint:nilerr // context cancellation is a clean exit, not an error
			}
			return fmt.Errorf("failed to get CI status: %w", err)
		}

		if checksChanged(prev, checks) {
			fmt.Fprint(cfg.Output, ui.Info("  "+formatCheckStatus(checks)))
		}
		prev = checks

		if len(checks) > 0 && allCompleted(checks) {
			if anyFailed(checks) {
				attempt++
				if attempt > maxFixAttempts {
					fmt.Fprint(cfg.Output, ui.Error(fmt.Sprintf("CI still failing after %d attempts", maxFixAttempts)))
					return fmt.Errorf("CI still failing after %d attempts: %s", maxFixAttempts, failedCheckNames(checks))
				}

				checkName := firstFailedName(checks)
				if err := fixCI(ctx, cfg, checkName, attempt); err != nil {
					return err
				}

				// Reset prev so we re-print status on next poll
				prev = nil
				fmt.Fprint(cfg.Output, ui.Step("Waiting for CI checks..."))

				// Wait before polling again to give CI time to pick up the new push
				select {
				case <-ctx.Done():
					return nil
				case <-time.After(pollInterval):
				}
				continue
			}

			if hasPR {
				fmt.Fprint(cfg.Output, ui.Complete("CI passed — PR ready for review"))
			} else {
				fmt.Fprint(cfg.Output, ui.Complete("CI passed"))
			}
			return nil
		}

		select {
		case <-ctx.Done():
			return nil
		case <-time.After(pollInterval):
		}
	}
}

// fixCI performs a single CI fix attempt: fetch logs, call LLM, commit, push.
func fixCI(ctx context.Context, cfg Config, checkName string, attempt int) error {
	fmt.Fprint(cfg.Output, ui.Info(fmt.Sprintf("CI failed — %s (attempt %d/%d)", checkName, attempt, maxFixAttempts)))

	// Fetch failed run ID
	runID, err := FailedRunID(ctx)
	if err != nil {
		fmt.Fprint(cfg.Output, ui.Error(fmt.Sprintf("Failed to read CI logs: %s", err)))
		return fmt.Errorf("failed to get failed run ID: %w", err)
	}

	// Fetch failure logs (in-memory only, never written to disk)
	logs, err := FailureLogs(ctx, runID)
	if err != nil {
		fmt.Fprint(cfg.Output, ui.Error(fmt.Sprintf("Failed to read CI logs: %s", err)))
		return fmt.Errorf("failed to fetch CI logs: %w", err)
	}

	// Render CI fix prompt
	prompt, err := prompts.CIFix(prompts.CIFixData{
		FailureLogs:   logs,
		CheckName:     checkName,
		AttemptNumber: attempt,
		MaxAttempts:   maxFixAttempts,
	})
	if err != nil {
		return fmt.Errorf("failed to render CI fix prompt: %w", err)
	}

	// Call LLM to diagnose and fix
	if cfg.Executor == nil {
		return fmt.Errorf("no executor configured for CI fix")
	}
	if err := cfg.Executor.Run(ctx, cfg.Output, model.Fast, prompt); err != nil {
		return fmt.Errorf("CI fix LLM call failed: %w", err)
	}

	// Commit the fix (new commit, never amend)
	commitMsg := fmt.Sprintf("fix: resolve %s CI failure", checkName)
	if err := CommitAll(ctx, commitMsg); err != nil {
		return fmt.Errorf("failed to commit CI fix: %w", err)
	}

	// Push the fix
	if err := Push(ctx); err != nil {
		return fmt.Errorf("failed to push CI fix: %w", err)
	}

	fmt.Fprint(cfg.Output, ui.Step("Fix pushed, waiting for CI..."))
	return nil
}

// firstFailedName returns the name of the first failed check.
func firstFailedName(checks []CheckResult) string {
	for _, c := range checks {
		if c.Status == "failed" {
			return c.Name
		}
	}
	return "unknown"
}

// failedCheckNames returns a comma-separated list of failed check names.
func failedCheckNames(checks []CheckResult) string {
	var names []string
	for _, c := range checks {
		if c.Status == "failed" {
			names = append(names, c.Name)
		}
	}
	return strings.Join(names, ", ")
}

// extractPRNumber extracts "#N" from a GitHub PR URL like "https://github.com/user/repo/pull/42".
func extractPRNumber(url string) string {
	matches := prNumberRe.FindStringSubmatch(url)
	if len(matches) >= 2 {
		return "#" + matches[1]
	}
	return ""
}

// formatCheckStatus formats check results for terminal display.
// ≤5 checks: "lint: passed, test: running".
// >5 checks: "3 passed, 1 running, 2 pending".
func formatCheckStatus(checks []CheckResult) string {
	if len(checks) <= 5 {
		parts := make([]string, len(checks))
		for i, c := range checks {
			parts[i] = c.Name + ": " + c.Status
		}
		return strings.Join(parts, ", ")
	}

	counts := map[string]int{}
	for _, c := range checks {
		counts[c.Status]++
	}

	var parts []string
	for _, status := range []string{"passed", "failed", "running", "pending"} {
		if n := counts[status]; n > 0 {
			parts = append(parts, fmt.Sprintf("%d %s", n, status))
		}
	}
	return strings.Join(parts, ", ")
}

// checksChanged returns true if any check's status changed between polls.
func checksChanged(prev, curr []CheckResult) bool {
	if len(prev) != len(curr) {
		return true
	}
	prevMap := make(map[string]string, len(prev))
	for _, c := range prev {
		prevMap[c.Name] = c.Status
	}
	for _, c := range curr {
		if prevMap[c.Name] != c.Status {
			return true
		}
	}
	return false
}

// allCompleted returns true if no checks are "pending" or "running".
func allCompleted(checks []CheckResult) bool {
	for _, c := range checks {
		if c.Status == "pending" || c.Status == "running" {
			return false
		}
	}
	return true
}

// anyFailed returns true if any check has status "failed".
func anyFailed(checks []CheckResult) bool {
	for _, c := range checks {
		if c.Status == "failed" {
			return true
		}
	}
	return false
}
