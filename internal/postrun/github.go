package postrun

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os/exec"
	"strings"
)

// DefaultBranch returns the default branch name of the GitHub repository.
// Runs: gh repo view --json defaultBranchRef -q .defaultBranchRef.name.
func DefaultBranch(ctx context.Context) (string, error) {
	cmd := exec.CommandContext(ctx, "gh", "repo", "view", "--json", "defaultBranchRef", "-q", ".defaultBranchRef.name")
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return "", &GHError{Stderr: strings.TrimSpace(stderr.String()), Err: err}
	}
	return strings.TrimSpace(stdout.String()), nil
}

// prViewResult represents the JSON output from gh pr view.
type prViewResult struct {
	State string `json:"state"`
	URL   string `json:"url"`
}

// PRExists checks if a PR already exists for the current branch.
// Returns (exists, url, error). Exit code 1 from gh means no PR exists (not an error).
func PRExists(ctx context.Context) (exists bool, prURL string, err error) {
	cmd := exec.CommandContext(ctx, "gh", "pr", "view", "--json", "state,url")
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		// Exit code 1 = no PR for this branch — not an error
		if cmd.ProcessState != nil && cmd.ProcessState.ExitCode() == 1 {
			return false, "", nil
		}
		return false, "", &GHError{Stderr: strings.TrimSpace(stderr.String()), Err: err}
	}

	var result prViewResult
	if err := json.Unmarshal(stdout.Bytes(), &result); err != nil {
		return false, "", err
	}
	return true, result.URL, nil
}

// prCheckResult represents a single check from gh pr checks --json.
// Note: gh no longer accepts "conclusion" as a JSON field for pr checks; the
// state field carries the conclusion-style values (SUCCESS, FAILURE, ...).
type prCheckResult struct {
	Name  string `json:"name"`
	State string `json:"state"`
}

// runCheckResult represents a single run from gh run list --json.
type runCheckResult struct {
	Name       string `json:"name"`
	Status     string `json:"status"`
	Conclusion string `json:"conclusion"`
}

// CheckStatus returns the current CI check results.
// If hasPR is true, uses "gh pr checks --json"; otherwise uses "gh run list --json" scoped to the given branch.
func CheckStatus(ctx context.Context, hasPR bool, branch string) ([]CheckResult, error) {
	if hasPR {
		return checkStatusPR(ctx)
	}
	return checkStatusRun(ctx, branch)
}

func checkStatusPR(ctx context.Context) ([]CheckResult, error) {
	cmd := exec.CommandContext(ctx, "gh", "pr", "checks", "--json", "name,state")
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return nil, &GHError{Stderr: strings.TrimSpace(stderr.String()), Err: err}
	}

	var raw []prCheckResult
	if err := json.Unmarshal(stdout.Bytes(), &raw); err != nil {
		return nil, err
	}

	results := make([]CheckResult, len(raw))
	for i, r := range raw {
		results[i] = CheckResult{
			Name:   r.Name,
			Status: normalizePRCheckStatus(r.State),
		}
	}
	return results, nil
}

func checkStatusRun(ctx context.Context, branch string) ([]CheckResult, error) {
	cmd := exec.CommandContext(ctx, "gh", "run", "list", "--branch", branch, "--json", "name,status,conclusion", "--limit", "1")
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return nil, &GHError{Stderr: strings.TrimSpace(stderr.String()), Err: err}
	}

	var raw []runCheckResult
	if err := json.Unmarshal(stdout.Bytes(), &raw); err != nil {
		return nil, err
	}

	results := make([]CheckResult, len(raw))
	for i, r := range raw {
		results[i] = CheckResult{
			Name:       r.Name,
			Status:     normalizeRunStatus(r.Status, r.Conclusion),
			Conclusion: r.Conclusion,
		}
	}
	return results, nil
}

// normalizePRCheckStatus converts gh pr checks states to our standard statuses.
// gh pr checks states: PENDING, SUCCESS, FAILURE, ERROR, CANCELLED, etc.
func normalizePRCheckStatus(state string) string {
	switch strings.ToUpper(state) {
	case "SUCCESS":
		return "passed"
	case "FAILURE", "ERROR", "CANCELLED", "TIMED_OUT", "ACTION_REQUIRED", "STALE", "STARTUP_FAILURE":
		return "failed"
	default:
		return "pending"
	}
}

// normalizeRunStatus converts gh run list statuses to our standard statuses.
// gh run list statuses: completed, in_progress, queued, waiting, requested, pending.
func normalizeRunStatus(status, conclusion string) string {
	switch strings.ToLower(status) {
	case "completed":
		switch strings.ToLower(conclusion) {
		case "success":
			return "passed"
		default:
			return "failed"
		}
	case "in_progress":
		return "running"
	default:
		return "pending"
	}
}

const maxLogSize = 50 * 1024 // 50KB

// FailureLogs fetches the failed run logs via gh run view --log-failed.
// Truncates output to maxLogSize (50KB) to prevent context window overflow.
func FailureLogs(ctx context.Context, runID string) (string, error) {
	cmd := exec.CommandContext(ctx, "gh", "run", "view", runID, "--log-failed")
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return "", &GHError{Stderr: strings.TrimSpace(stderr.String()), Err: err}
	}
	return truncateLog(stdout.String()), nil
}

// truncateLog truncates log content to maxLogSize, appending a marker if truncated.
func truncateLog(content string) string {
	if len(content) <= maxLogSize {
		return content
	}
	return content[:maxLogSize] + "\n\n[log truncated — exceeded 50KB limit]"
}

// failedRunResult represents a single run from gh run list --json.
type failedRunResult struct {
	DatabaseID int `json:"databaseId"` //nolint:tagliatelle // GitHub API uses camelCase
}

// FailedRunID finds the ID of the most recent failed workflow run.
func FailedRunID(ctx context.Context) (string, error) {
	cmd := exec.CommandContext(ctx, "gh", "run", "list", "--status", "failure", "--limit", "1", "--json", "databaseId")
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return "", &GHError{Stderr: strings.TrimSpace(stderr.String()), Err: err}
	}

	var results []failedRunResult
	if err := json.Unmarshal(stdout.Bytes(), &results); err != nil {
		return "", err
	}
	if len(results) == 0 {
		return "", &GHError{Stderr: "no failed runs found", Err: nil}
	}

	return fmt.Sprintf("%d", results[0].DatabaseID), nil
}

// GHError wraps a gh CLI failure with stderr output.
type GHError struct {
	Stderr string
	Err    error
}

func (e *GHError) Error() string {
	if e.Stderr != "" {
		return e.Stderr
	}
	if e.Err != nil {
		return e.Err.Error()
	}
	return "unknown gh error"
}

func (e *GHError) Unwrap() error {
	return e.Err
}
