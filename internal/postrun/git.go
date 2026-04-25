package postrun

import (
	"bytes"
	"context"
	"fmt"
	"net/url"
	"os/exec"
	"strings"
)

// DetectRemote returns the URL for the "origin" remote.
// Returns empty string and nil error if no remote named "origin" exists.
func DetectRemote() (string, error) {
	ctx := context.Background()
	cmd := exec.CommandContext(ctx, "git", "remote", "get-url", "origin")
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		stderrStr := stderr.String()
		// No remote named "origin" or not in a git repo — not an error
		if strings.Contains(stderrStr, "No such remote") || strings.Contains(stderrStr, "not a git repository") {
			return "", nil
		}
		return "", err
	}
	return strings.TrimSpace(stdout.String()), nil
}

// IsGitHubRemote returns true if the remote URL points to github.com.
// Handles HTTPS, SSH (git@github.com:...), and SSH protocol (ssh://git@github.com/...) formats.
func IsGitHubRemote(remoteURL string) bool {
	if remoteURL == "" {
		return false
	}

	// Handle SSH shorthand: git@github.com:user/repo.git
	if strings.HasPrefix(remoteURL, "git@") {
		hostPart := strings.TrimPrefix(remoteURL, "git@")
		colonIdx := strings.Index(hostPart, ":")
		if colonIdx < 0 {
			return false
		}
		host := hostPart[:colonIdx]
		return host == "github.com"
	}

	// Parse as URL (handles https:// and ssh:// schemes)
	u, err := url.Parse(remoteURL)
	if err != nil || u.Host == "" {
		return false
	}

	host := u.Hostname()
	return host == "github.com"
}

// Push pushes the current branch to origin. Never uses --force.
func Push(ctx context.Context) error {
	cmd := exec.CommandContext(ctx, "git", "push", "origin", "HEAD")
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return &PushError{Stderr: strings.TrimSpace(stderr.String()), Err: err}
	}
	return nil
}

// PushError wraps a git push failure with stderr output.
type PushError struct {
	Stderr string
	Err    error
}

func (e *PushError) Error() string {
	if e.Stderr != "" {
		return e.Stderr
	}
	return e.Err.Error()
}

func (e *PushError) Unwrap() error {
	return e.Err
}

// CurrentBranch returns the name of the current branch.
// Returns empty string for detached HEAD.
func CurrentBranch(ctx context.Context) (string, error) {
	cmd := exec.CommandContext(ctx, "git", "branch", "--show-current")
	var stdout bytes.Buffer
	cmd.Stdout = &stdout
	if err := cmd.Run(); err != nil {
		return "", err
	}
	return strings.TrimSpace(stdout.String()), nil
}

// CommitAll stages all changes and creates a new commit. Never amends.
func CommitAll(ctx context.Context, message string) error {
	addCmd := exec.CommandContext(ctx, "git", "add", "-A")
	var addStderr bytes.Buffer
	addCmd.Stderr = &addStderr
	if err := addCmd.Run(); err != nil {
		return fmt.Errorf("git add failed: %s", strings.TrimSpace(addStderr.String()))
	}

	commitCmd := exec.CommandContext(ctx, "git", "commit", "-m", message)
	var commitStdout, commitStderr bytes.Buffer
	commitCmd.Stdout = &commitStdout
	commitCmd.Stderr = &commitStderr
	if err := commitCmd.Run(); err != nil {
		combined := strings.TrimSpace(commitStdout.String() + "\n" + commitStderr.String())
		// "nothing to commit" is not a crash — return a descriptive error
		if strings.Contains(combined, "nothing to commit") {
			return fmt.Errorf("nothing to commit")
		}
		return fmt.Errorf("git commit failed: %s", combined)
	}
	return nil
}

// DiffStat returns the diff stat between the given base branch and HEAD.
func DiffStat(ctx context.Context, baseBranch string) (string, error) {
	cmd := exec.CommandContext(ctx, "git", "diff", baseBranch+"...HEAD", "--stat") //nolint:gosec // baseBranch comes from gh CLI output, not user input
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return "", err
	}
	return strings.TrimSpace(stdout.String()), nil
}

// CommitMessages returns full commit message bodies for commits between
// baseBranch and HEAD, oldest first, separated by a blank line. Capped at the
// last 30 commits so very long branches do not blow the prompt budget.
// Returns "" with nil error when the range is empty.
func CommitMessages(ctx context.Context, baseBranch string) (string, error) {
	cmd := exec.CommandContext(ctx, "git", "log", "-n", "30", baseBranch+"..HEAD", "--reverse", "--format=%B%n") //nolint:gosec // baseBranch comes from gh CLI output, not user input
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return "", err
	}
	return strings.TrimSpace(stdout.String()), nil
}
