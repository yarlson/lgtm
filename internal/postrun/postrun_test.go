package postrun

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/yarlson/snap/internal/model"
)

// mockExecutor is a test double for the LLM executor.
// On PR-creation prompts, it simulates the real agent by invoking `gh pr create`
// (which is mocked via PATH). On other prompts, it streams the configured output.
type mockExecutor struct {
	output string
	err    error
}

func (m *mockExecutor) Run(ctx context.Context, w io.Writer, _ model.Type, args ...string) error {
	if m.err != nil {
		return m.err
	}
	if isPRPrompt(args) {
		return runMockGHCreate(ctx, "Add feature", "Body")
	}
	_, err := fmt.Fprint(w, m.output)
	return err
}

// capturingExecutor captures the prompt sent to the executor and, on PR prompts,
// invokes the mocked `gh pr create` so the post-LLM PRExists check succeeds.
type capturingExecutor struct {
	capturedPrompt string
	output         string
}

func (m *capturingExecutor) Run(ctx context.Context, w io.Writer, _ model.Type, args ...string) error {
	if len(args) > 0 {
		m.capturedPrompt = args[0]
	}
	if isPRPrompt(args) {
		return runMockGHCreate(ctx, "Add feature", "Body")
	}
	_, err := fmt.Fprint(w, m.output)
	return err
}

// isPRPrompt returns true if the executor was called with the PR-creation prompt.
func isPRPrompt(args []string) bool {
	return len(args) > 0 && strings.Contains(args[0], "GitHub pull request")
}

// runMockGHCreate invokes the mocked `gh pr create`. The gh mock script is
// expected on PATH and is responsible for writing the marker that flips
// `gh pr view` to "PR exists". Title is parameterised so tests can vary it; body
// is fixed because no test asserts on it.
//
//nolint:unparam // body is parameterised for clarity even though tests use "Body"
func runMockGHCreate(ctx context.Context, title, body string) error {
	cmd := exec.CommandContext(ctx, "gh", "pr", "create", "--title", title, "--body", body)
	return cmd.Run()
}

// mockGHMulti creates a gh script that handles repo view, pr view, and pr create subcommands.
// defaultBranch: returned by "gh repo view".
// prViewJSON: returned by "gh pr view" before any "pr create" call (empty string = exit 1).
// prCreateURL: returned by "gh pr create" (empty string = exit 1 with error). After a successful
// "pr create", "pr view" flips to returning a JSON document with state OPEN and that URL — so
// the post-LLM PRExists check sees the freshly-created PR.
//
//nolint:unparam // defaultBranch is parameterized for readability even though tests use "main"
func mockGHMulti(t *testing.T, defaultBranch, prViewJSON, prCreateURL string) {
	t.Helper()
	writeGHScript(t, ghScriptOptions{defaultBranch: defaultBranch, prViewJSON: prViewJSON, prCreateURL: prCreateURL})
}

// ghScriptOptions configures the gh mock script written by writeGHScript.
type ghScriptOptions struct {
	defaultBranch   string
	prViewJSON      string
	prCreateURL     string
	checksJSON      string   // single static "pr checks" response
	checksResponses []string // ordered "pr checks" responses (counter-driven)
	failedRunID     string   // "run list --status failure" response
	failedLogs      string   // "run view <id> --log-failed" response
}

// writeGHScript emits a stateful gh mock script. "pr create" writes a marker file;
// subsequent "pr view" calls return a JSON document built from prCreateURL so the
// post-LLM PRExists check sees the newly-created PR.
func writeGHScript(t *testing.T, opts ghScriptOptions) {
	t.Helper()
	binDir := t.TempDir()
	markerFile := filepath.Join(binDir, "pr_marker")
	counterFile := filepath.Join(binDir, "checks_counter")
	if len(opts.checksResponses) > 0 {
		require.NoError(t, os.WriteFile(counterFile, []byte("0"), 0o600))
	}

	var script strings.Builder
	script.WriteString("#!/bin/sh\n")
	script.WriteString("case \"$1 $2\" in\n")

	// gh repo view
	if opts.defaultBranch != "" {
		script.WriteString("  \"repo view\")\n")
		fmt.Fprintf(&script, "    printf '%%s' '%s'\n", opts.defaultBranch)
		script.WriteString("    ;;\n")
	}

	// gh pr view — flips after "pr create" writes the marker
	script.WriteString("  \"pr view\")\n")
	if opts.prCreateURL != "" {
		fmt.Fprintf(&script, "    if [ -f %s ]; then\n", markerFile)
		fmt.Fprintf(&script, "      printf '%%s' '{\"state\":\"OPEN\",\"url\":\"%s\"}'\n", opts.prCreateURL)
		script.WriteString("      exit 0\n")
		script.WriteString("    fi\n")
	}
	if opts.prViewJSON == "" {
		script.WriteString("    exit 1\n")
	} else {
		fmt.Fprintf(&script, "    printf '%%s' '%s'\n", opts.prViewJSON)
	}
	script.WriteString("    ;;\n")

	// gh pr create — writes marker on success
	script.WriteString("  \"pr create\")\n")
	if opts.prCreateURL == "" {
		script.WriteString("    echo 'creation failed' >&2\n")
		script.WriteString("    exit 1\n")
	} else {
		fmt.Fprintf(&script, "    : > %s\n", markerFile)
		fmt.Fprintf(&script, "    printf '%%s' '%s'\n", opts.prCreateURL)
	}
	script.WriteString("    ;;\n")

	// gh pr checks — single static response
	if opts.checksJSON != "" {
		script.WriteString("  \"pr checks\")\n")
		fmt.Fprintf(&script, "    printf '%%s' '%s'\n", opts.checksJSON)
		script.WriteString("    ;;\n")
	}

	// gh pr checks — counter-driven sequence
	if len(opts.checksResponses) > 0 {
		script.WriteString("  \"pr checks\")\n")
		fmt.Fprintf(&script, "    COUNT=$(cat %s)\n", counterFile)
		script.WriteString("    COUNT=$((COUNT + 1))\n")
		fmt.Fprintf(&script, "    printf '%%s' \"$COUNT\" > %s\n", counterFile)
		script.WriteString("    case $COUNT in\n")
		for i, resp := range opts.checksResponses {
			fmt.Fprintf(&script, "      %d)\n", i+1)
			fmt.Fprintf(&script, "        printf '%%s' '%s'\n", resp)
			script.WriteString("        ;;\n")
		}
		script.WriteString("      *)\n")
		fmt.Fprintf(&script, "        printf '%%s' '%s'\n", opts.checksResponses[len(opts.checksResponses)-1])
		script.WriteString("        ;;\n")
		script.WriteString("    esac\n")
		script.WriteString("    ;;\n")
	}

	// gh run list (FailedRunID)
	if opts.failedRunID != "" {
		script.WriteString("  \"run list\")\n")
		fmt.Fprintf(&script, "    printf '%%s' '[{\"databaseId\":%s}]'\n", opts.failedRunID)
		script.WriteString("    ;;\n")
	}

	// gh run view (FailureLogs)
	if opts.failedLogs != "" {
		script.WriteString("  \"run view\")\n")
		fmt.Fprintf(&script, "    printf '%%s' '%s'\n", opts.failedLogs)
		script.WriteString("    ;;\n")
	}

	script.WriteString("  *)\n")
	script.WriteString("    echo \"unexpected gh call: $*\" >&2\n")
	script.WriteString("    exit 99\n")
	script.WriteString("    ;;\n")
	script.WriteString("esac\n")

	ghPath := filepath.Join(binDir, "gh")
	require.NoError(t, os.WriteFile(ghPath, []byte(script.String()), 0o755)) //nolint:gosec // test script needs execute permission

	origPath := os.Getenv("PATH")
	t.Setenv("PATH", binDir+string(os.PathListSeparator)+origPath)
}

func TestRun_NoRemote(t *testing.T) {
	dir := initGitRepo(t)
	chdir(t, dir)

	var buf bytes.Buffer
	cfg := Config{
		Output:    &buf,
		RemoteURL: "",
	}

	err := Run(context.Background(), cfg)
	require.NoError(t, err)
	assert.Contains(t, buf.String(), "No remote configured, skipping push")
}

func TestRun_PushToBareSelfRemote(t *testing.T) {
	dir := initGitRepo(t)
	bareDir := initBareRemote(t, dir)
	chdir(t, dir)

	var buf bytes.Buffer
	cfg := Config{
		Output:    &buf,
		RemoteURL: bareDir,
		IsGitHub:  false,
	}

	err := Run(context.Background(), cfg)
	require.NoError(t, err)

	output := buf.String()
	assert.Contains(t, output, "Pushed to origin/")
	assert.Contains(t, output, "Non-GitHub remote, skipping PR and CI")

	// Verify commit exists in bare repo
	gitOut := gitOutput(t, bareDir, "log", "--oneline")
	assert.Contains(t, gitOut, "initial")
}

func TestRun_PushRejected(t *testing.T) {
	dir := initGitRepo(t)
	bareDir := initBareRemote(t, dir)

	// Push initial commit to bare repo
	gitCmd(t, dir, "push", "origin", "HEAD")

	// Create a divergent commit on bare repo by cloning and pushing from another work tree
	cloneDir := t.TempDir()
	gitCmd(t, cloneDir, "clone", bareDir, ".")
	gitCmd(t, cloneDir, "config", "user.email", "test@test.com")
	gitCmd(t, cloneDir, "config", "user.name", "test")
	require.NoError(t, os.WriteFile(filepath.Join(cloneDir, "diverge.txt"), []byte("diverge"), 0o600))
	gitCmd(t, cloneDir, "add", ".")
	gitCmd(t, cloneDir, "commit", "-m", "divergent")
	gitCmd(t, cloneDir, "push", "origin", "HEAD")

	// Now create a local commit that diverges from the bare repo
	require.NoError(t, os.WriteFile(filepath.Join(dir, "local.txt"), []byte("local"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "local divergent")

	chdir(t, dir)

	var buf bytes.Buffer
	cfg := Config{
		Output:    &buf,
		RemoteURL: bareDir,
		IsGitHub:  false,
	}

	err := Run(context.Background(), cfg)
	require.Error(t, err)
	assert.Contains(t, err.Error(), "push failed")
}

func TestRun_NonGitHubRemote(t *testing.T) {
	dir := initGitRepo(t)
	bareDir := initBareRemote(t, dir)
	chdir(t, dir)

	var buf bytes.Buffer
	cfg := Config{
		Output:    &buf,
		RemoteURL: bareDir,
		IsGitHub:  false,
	}

	err := Run(context.Background(), cfg)
	require.NoError(t, err)

	output := buf.String()
	assert.Contains(t, output, "Pushed to origin/")
	assert.Contains(t, output, "Non-GitHub remote, skipping PR and CI")
}

func TestRun_PRCreation_HappyPath(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)

	// Create feature branch with a commit
	gitCmd(t, dir, "checkout", "-b", "feature-x")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "feature.txt"), []byte("new feature"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add feature")
	chdir(t, dir)

	// Mock gh: default branch "main", no existing PR, PR create returns URL
	mockGHMulti(t, "main", "", "https://github.com/user/repo/pull/42")

	var buf bytes.Buffer
	cfg := Config{
		Output:    &buf,
		RemoteURL: "https://github.com/user/repo.git",
		IsGitHub:  true,
		Executor:  &mockExecutor{output: "Add new feature\n\nImplements the feature for better UX."},
	}

	err := Run(context.Background(), cfg)
	require.NoError(t, err)

	output := buf.String()
	assert.Contains(t, output, "Creating pull request...")
	assert.Contains(t, output, "PR #42 created: https://github.com/user/repo/pull/42")
}

func TestRun_PRSkip_DefaultBranch(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)
	chdir(t, dir)

	// Current branch is "main", default branch is "main"
	mockGHMulti(t, "main", "", "")

	var buf bytes.Buffer
	cfg := Config{
		Output:    &buf,
		RemoteURL: "https://github.com/user/repo.git",
		IsGitHub:  true,
		Executor:  &mockExecutor{output: "Title\n\nBody"},
	}

	err := Run(context.Background(), cfg)
	require.NoError(t, err)

	output := buf.String()
	assert.Contains(t, output, "On default branch, skipping PR creation")
	assert.NotContains(t, output, "Creating pull request...")
}

func TestRun_PRSkip_Existing(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)

	// Create feature branch
	gitCmd(t, dir, "checkout", "-b", "feature-y")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "feature.txt"), []byte("feature"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add feature")
	chdir(t, dir)

	// Mock gh: default branch "main", PR exists
	mockGHMulti(t, "main",
		`{"state":"OPEN","url":"https://github.com/user/repo/pull/10"}`,
		"")

	var buf bytes.Buffer
	cfg := Config{
		Output:    &buf,
		RemoteURL: "https://github.com/user/repo.git",
		IsGitHub:  true,
		Executor:  &mockExecutor{output: "Title\n\nBody"},
	}

	err := Run(context.Background(), cfg)
	require.NoError(t, err)

	output := buf.String()
	assert.Contains(t, output, "PR already exists: https://github.com/user/repo/pull/10")
	assert.NotContains(t, output, "Creating pull request...")
}

func TestRun_PRSkip_DetachedHead(t *testing.T) {
	// Test createPRFlow directly with empty branch (simulates detached HEAD).
	// Detached HEAD causes git push to fail, so we test the PR skip logic directly.
	mockGHMulti(t, "main", "", "")

	var buf bytes.Buffer
	cfg := Config{
		Output:   &buf,
		IsGitHub: true,
		Executor: &mockExecutor{output: "Title\n\nBody"},
	}

	// Empty branch = detached HEAD
	_, err := createPRFlow(context.Background(), cfg, "")
	require.NoError(t, err)

	output := buf.String()
	assert.NotContains(t, output, "Creating pull request...")
	assert.NotContains(t, output, "On default branch")
}

func TestRun_PRCreation_UsesPRDContext(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)

	// Create feature branch with a commit
	gitCmd(t, dir, "checkout", "-b", "feature-prd")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "feature.txt"), []byte("new feature"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add feature")

	// Write a PRD file with identifiable content
	prdPath := filepath.Join(dir, "PRD.md")
	require.NoError(t, os.WriteFile(prdPath, []byte("## Goal\nImplement offline-first sync engine for mobile clients."), 0o600))

	chdir(t, dir)

	// Mock gh: default branch "main", no existing PR, PR create returns URL
	mockGHMulti(t, "main", "", "https://github.com/user/repo/pull/99")

	executor := &capturingExecutor{output: "Add sync engine\n\nImplements offline-first sync for mobile."}

	var buf bytes.Buffer
	cfg := Config{
		Output:    &buf,
		RemoteURL: "https://github.com/user/repo.git",
		IsGitHub:  true,
		Executor:  executor,
		PRDPath:   prdPath,
	}

	err := Run(context.Background(), cfg)
	require.NoError(t, err)

	// Verify PRD content was included in the prompt sent to the executor
	assert.Contains(t, executor.capturedPrompt, "offline-first sync engine")
	assert.Contains(t, executor.capturedPrompt, "mobile clients")
}

func TestRun_PRCreation_PassesCommitsAndAntiPatterns(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)

	gitCmd(t, dir, "checkout", "-b", "feature-commits")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "a.txt"), []byte("a"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "Implement parser")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "b.txt"), []byte("b"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "Add parser tests")

	chdir(t, dir)

	mockGHMulti(t, "main", "", "https://github.com/user/repo/pull/1")

	executor := &capturingExecutor{}

	var buf bytes.Buffer
	cfg := Config{
		Output:    &buf,
		RemoteURL: "https://github.com/user/repo.git",
		IsGitHub:  true,
		Executor:  executor,
	}

	err := Run(context.Background(), cfg)
	require.NoError(t, err)

	assert.Contains(t, executor.capturedPrompt, "Implement parser")
	assert.Contains(t, executor.capturedPrompt, "Add parser tests")
	assert.Contains(t, executor.capturedPrompt, "Anti-patterns")
	assert.Contains(t, executor.capturedPrompt, "Do not quote the PRD")
	assert.Contains(t, executor.capturedPrompt, "50–72")
	assert.Contains(t, executor.capturedPrompt, "gh pr create")
}

func TestRun_PRCreation_Failed(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)

	// Create feature branch
	gitCmd(t, dir, "checkout", "-b", "feature-z")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "feature.txt"), []byte("feature"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add feature")
	chdir(t, dir)

	// Mock gh: default branch "main", no existing PR, PR create fails
	mockGHMulti(t, "main", "", "")

	var buf bytes.Buffer
	cfg := Config{
		Output:    &buf,
		RemoteURL: "https://github.com/user/repo.git",
		IsGitHub:  true,
		Executor:  &mockExecutor{output: "Add feature\n\nBody text."},
	}

	err := Run(context.Background(), cfg)
	require.Error(t, err)
	assert.Contains(t, err.Error(), "PR creation failed")

	output := buf.String()
	assert.Contains(t, output, "PR creation failed")
}

func TestRun_PRCreation_LLMFails(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)

	// Create feature branch
	gitCmd(t, dir, "checkout", "-b", "feature-llm-fail")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "feature.txt"), []byte("feature"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add feature")
	chdir(t, dir)

	// Mock gh: default branch "main", no existing PR, PR create would succeed if invoked
	mockGHMulti(t, "main", "", "https://github.com/user/repo/pull/77")

	var buf bytes.Buffer
	cfg := Config{
		Output:    &buf,
		RemoteURL: "https://github.com/user/repo.git",
		IsGitHub:  true,
		Executor:  &mockExecutor{err: fmt.Errorf("LLM timeout")},
	}

	// Agent failure surfaces directly: there is no fallback because the agent owns
	// drafting and creating the PR in a single step.
	err := Run(context.Background(), cfg)
	require.Error(t, err)
	assert.Contains(t, err.Error(), "PR creation failed")
	assert.Contains(t, err.Error(), "LLM timeout")
}

// gitOutput runs a git command in a directory and returns combined output.
func gitOutput(t *testing.T, dir string, args ...string) string {
	t.Helper()
	ctx := context.Background()
	cmd := exec.CommandContext(ctx, "git", args...)
	cmd.Dir = dir
	out, err := cmd.CombinedOutput()
	require.NoError(t, err, "git %v failed: %s", args, out)
	return string(out)
}

func TestFormatCheckStatus_FewChecks(t *testing.T) {
	checks := []CheckResult{
		{Name: "lint", Status: "passed"},
		{Name: "test", Status: "running"},
		{Name: "build", Status: "pending"},
	}
	result := formatCheckStatus(checks)
	assert.Equal(t, "lint: passed, test: running, build: pending", result)
}

func TestFormatCheckStatus_ManyChecks(t *testing.T) {
	checks := []CheckResult{
		{Name: "lint", Status: "passed"},
		{Name: "test", Status: "passed"},
		{Name: "build", Status: "passed"},
		{Name: "e2e", Status: "running"},
		{Name: "deploy", Status: "pending"},
		{Name: "security", Status: "pending"},
	}
	result := formatCheckStatus(checks)
	assert.Equal(t, "3 passed, 1 running, 2 pending", result)
}

func TestFormatCheckStatus_ManyChecks_AllPassed(t *testing.T) {
	checks := make([]CheckResult, 6)
	for i := range checks {
		checks[i] = CheckResult{Name: fmt.Sprintf("check%d", i), Status: "passed"}
	}
	result := formatCheckStatus(checks)
	assert.Equal(t, "6 passed", result)
}

func TestChecksChanged(t *testing.T) {
	prev := []CheckResult{
		{Name: "lint", Status: "running"},
		{Name: "test", Status: "pending"},
	}
	curr := []CheckResult{
		{Name: "lint", Status: "passed"},
		{Name: "test", Status: "running"},
	}
	assert.True(t, checksChanged(prev, curr))
}

func TestChecksChanged_Identical(t *testing.T) {
	checks := []CheckResult{
		{Name: "lint", Status: "running"},
		{Name: "test", Status: "pending"},
	}
	assert.False(t, checksChanged(checks, checks))
}

func TestChecksChanged_DifferentLength(t *testing.T) {
	prev := []CheckResult{{Name: "lint", Status: "running"}}
	curr := []CheckResult{
		{Name: "lint", Status: "running"},
		{Name: "test", Status: "pending"},
	}
	assert.True(t, checksChanged(prev, curr))
}

func TestChecksChanged_NilPrev(t *testing.T) {
	curr := []CheckResult{{Name: "lint", Status: "running"}}
	assert.True(t, checksChanged(nil, curr))
}

func TestAllCompleted(t *testing.T) {
	assert.True(t, allCompleted([]CheckResult{
		{Name: "lint", Status: "passed"},
		{Name: "test", Status: "failed"},
	}))
	assert.False(t, allCompleted([]CheckResult{
		{Name: "lint", Status: "passed"},
		{Name: "test", Status: "running"},
	}))
	assert.False(t, allCompleted([]CheckResult{
		{Name: "lint", Status: "passed"},
		{Name: "test", Status: "pending"},
	}))
}

func TestAnyFailed(t *testing.T) {
	assert.True(t, anyFailed([]CheckResult{
		{Name: "lint", Status: "passed"},
		{Name: "test", Status: "failed"},
	}))
	assert.False(t, anyFailed([]CheckResult{
		{Name: "lint", Status: "passed"},
		{Name: "test", Status: "passed"},
	}))
}

// addWorkflowFile creates a .github/workflows/ci.yml with a push trigger in the given repo root.
func addWorkflowFile(t *testing.T, repoRoot string) {
	t.Helper()
	wfDir := filepath.Join(repoRoot, ".github", "workflows")
	require.NoError(t, os.MkdirAll(wfDir, 0o755))
	content := "on: push\njobs:\n  test:\n    runs-on: ubuntu-latest\n"
	require.NoError(t, os.WriteFile(filepath.Join(wfDir, "ci.yml"), []byte(content), 0o600))
}

// mockGHWithCI is a thin wrapper around writeGHScript for tests that need a single static
// "pr checks" response.
func mockGHWithCI(t *testing.T, defaultBranch, prViewJSON, prCreateURL, checksJSON string) {
	t.Helper()
	writeGHScript(t, ghScriptOptions{
		defaultBranch: defaultBranch,
		prViewJSON:    prViewJSON,
		prCreateURL:   prCreateURL,
		checksJSON:    checksJSON,
	})
}

// mockGHWithCIStateful is a thin wrapper around writeGHScript for tests that need the
// "pr checks" response to change on successive calls.
func mockGHWithCIStateful(t *testing.T, defaultBranch, prViewJSON, prCreateURL string, checksResponses []string) {
	t.Helper()
	writeGHScript(t, ghScriptOptions{
		defaultBranch:   defaultBranch,
		prViewJSON:      prViewJSON,
		prCreateURL:     prCreateURL,
		checksResponses: checksResponses,
	})
}

func TestRun_CI_AllGreen(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)

	// Create feature branch with a commit
	gitCmd(t, dir, "checkout", "-b", "feature-ci")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "feature.txt"), []byte("new feature"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add feature")

	// Add workflow file with push trigger
	addWorkflowFile(t, dir)
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add workflows")

	chdir(t, dir)

	mockGHWithCI(t, "main", "", "https://github.com/user/repo/pull/50",
		`[{"name":"lint","state":"SUCCESS","conclusion":"success"},{"name":"test","state":"SUCCESS","conclusion":"success"}]`)

	var buf bytes.Buffer
	cfg := Config{
		Output:       &buf,
		RemoteURL:    "https://github.com/user/repo.git",
		IsGitHub:     true,
		Executor:     &mockExecutor{output: "Add feature\n\nImplements the feature."},
		RepoRoot:     dir,
		PollInterval: time.Millisecond,
	}

	err := Run(context.Background(), cfg)
	require.NoError(t, err)

	output := buf.String()
	assert.Contains(t, output, "Waiting for CI checks...")
	assert.Contains(t, output, "CI passed — PR ready for review")
}

func TestRun_CI_PendingThenGreen(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)

	// Create feature branch with a commit
	gitCmd(t, dir, "checkout", "-b", "feature-ci-pending")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "feature.txt"), []byte("new feature"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add feature")

	// Add workflow file with push trigger
	addWorkflowFile(t, dir)
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add workflows")

	chdir(t, dir)

	mockGHWithCIStateful(t, "main", "", "https://github.com/user/repo/pull/51",
		[]string{
			// First poll: pending
			`[{"name":"lint","state":"SUCCESS","conclusion":"success"},{"name":"test","state":"PENDING","conclusion":""}]`,
			// Second poll: all green
			`[{"name":"lint","state":"SUCCESS","conclusion":"success"},{"name":"test","state":"SUCCESS","conclusion":"success"}]`,
		})

	var buf bytes.Buffer
	cfg := Config{
		Output:       &buf,
		RemoteURL:    "https://github.com/user/repo.git",
		IsGitHub:     true,
		Executor:     &mockExecutor{output: "Add feature\n\nImplements the feature."},
		RepoRoot:     dir,
		PollInterval: time.Millisecond,
	}

	err := Run(context.Background(), cfg)
	require.NoError(t, err)

	output := buf.String()
	assert.Contains(t, output, "Waiting for CI checks...")
	// First poll prints status with pending check
	assert.Contains(t, output, "lint: passed, test: pending")
	// Second poll shows changed status and final success
	assert.Contains(t, output, "lint: passed, test: passed")
	assert.Contains(t, output, "CI passed — PR ready for review")
}

func TestRun_NoWorkflows(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)

	// Create feature branch — no .github/workflows/
	gitCmd(t, dir, "checkout", "-b", "feature-no-ci")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "feature.txt"), []byte("new feature"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add feature")

	chdir(t, dir)

	mockGHMulti(t, "main", "", "https://github.com/user/repo/pull/52")

	var buf bytes.Buffer
	cfg := Config{
		Output:    &buf,
		RemoteURL: "https://github.com/user/repo.git",
		IsGitHub:  true,
		Executor:  &mockExecutor{output: "Add feature\n\nBody."},
		RepoRoot:  dir,
	}

	err := Run(context.Background(), cfg)
	require.NoError(t, err)

	output := buf.String()
	assert.Contains(t, output, "No CI workflows found, done")
	assert.NotContains(t, output, "Waiting for CI checks...")
}

// mockGHWithCIFix is a thin wrapper around writeGHScript for tests that exercise the CI fix loop
// (counter-driven "pr checks" plus "run list" / "run view").
//
//nolint:unparam // defaultBranch is parameterized for readability even though tests use "main"
func mockGHWithCIFix(t *testing.T, defaultBranch, prViewJSON, prCreateURL string, checksResponses []string, failedRunID, failedLogs string) {
	t.Helper()
	writeGHScript(t, ghScriptOptions{
		defaultBranch:   defaultBranch,
		prViewJSON:      prViewJSON,
		prCreateURL:     prCreateURL,
		checksResponses: checksResponses,
		failedRunID:     failedRunID,
		failedLogs:      failedLogs,
	})
}

// fixLoopExecutor simulates the LLM agent during the CI fix loop. On the first call (PR-creation
// prompt) it invokes the mocked `gh pr create` so the post-LLM PRExists check sees the new PR.
// On subsequent calls (CI fix prompts) it writes a file so git has something to commit.
type fixLoopExecutor struct {
	dir     string
	callNum int
}

func (m *fixLoopExecutor) Run(ctx context.Context, _ io.Writer, _ model.Type, args ...string) error {
	m.callNum++
	if isPRPrompt(args) {
		return runMockGHCreate(ctx, "Fix lint", "Body")
	}
	fileName := filepath.Join(m.dir, fmt.Sprintf("fix-%d.txt", m.callNum))
	return os.WriteFile(fileName, []byte(fmt.Sprintf("fix %d", m.callNum)), 0o600)
}

func TestRun_CIFix_SuccessOnSecondAttempt(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)

	// Create feature branch
	gitCmd(t, dir, "checkout", "-b", "feature-ci-fix")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "feature.txt"), []byte("new feature"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add feature")

	addWorkflowFile(t, dir)
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add workflows")

	chdir(t, dir)

	// First poll: lint fails. Second poll (after fix): all pass.
	mockGHWithCIFix(t, "main", "", "https://github.com/user/repo/pull/60",
		[]string{
			`[{"name":"lint","state":"FAILURE","conclusion":"failure"},{"name":"test","state":"SUCCESS","conclusion":"success"}]`,
			`[{"name":"lint","state":"SUCCESS","conclusion":"success"},{"name":"test","state":"SUCCESS","conclusion":"success"}]`,
		},
		"12345", "Error: unused variable on line 10",
	)

	executor := &fixLoopExecutor{dir: dir}

	var buf bytes.Buffer
	cfg := Config{
		Output:       &buf,
		RemoteURL:    "https://github.com/user/repo.git",
		IsGitHub:     true,
		Executor:     executor,
		RepoRoot:     dir,
		PollInterval: time.Millisecond,
	}

	err := Run(context.Background(), cfg)
	require.NoError(t, err)

	output := buf.String()
	assert.Contains(t, output, "CI failed — lint (attempt 1/10)")
	assert.Contains(t, output, "Fix pushed, waiting for CI...")
	assert.Contains(t, output, "CI passed — PR ready for review")

	// Verify a fix commit exists
	logOutput := gitOutput(t, dir, "log", "--oneline")
	assert.Contains(t, logOutput, "fix: resolve lint CI failure")
}

func TestRun_CIFix_MaxRetriesExhausted(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)

	gitCmd(t, dir, "checkout", "-b", "feature-ci-max")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "feature.txt"), []byte("new feature"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add feature")

	addWorkflowFile(t, dir)
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add workflows")

	chdir(t, dir)

	// Always return failure
	alwaysFailing := `[{"name":"lint","state":"FAILURE","conclusion":"failure"}]`
	responses := make([]string, 11) // 11 responses (initial + 10 retries)
	for i := range responses {
		responses[i] = alwaysFailing
	}

	mockGHWithCIFix(t, "main", "", "https://github.com/user/repo/pull/61",
		responses, "12345", "Error: persistent issue",
	)

	executor := &fixLoopExecutor{dir: dir}

	var buf bytes.Buffer
	cfg := Config{
		Output:       &buf,
		RemoteURL:    "https://github.com/user/repo.git",
		IsGitHub:     true,
		Executor:     executor,
		RepoRoot:     dir,
		PollInterval: time.Millisecond,
	}

	err := Run(context.Background(), cfg)
	require.Error(t, err)
	assert.Contains(t, err.Error(), "CI still failing after 10 attempts")

	output := buf.String()
	assert.Contains(t, output, "CI still failing after 10 attempts")

	// Verify 10 fix commits exist
	logOutput := gitOutput(t, dir, "log", "--oneline")
	assert.Equal(t, 10, strings.Count(logOutput, "fix: resolve"))
}

func TestRun_CIFix_MultipleFailing(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)

	gitCmd(t, dir, "checkout", "-b", "feature-ci-multi")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "feature.txt"), []byte("new feature"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add feature")

	addWorkflowFile(t, dir)
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add workflows")

	chdir(t, dir)

	// Multiple checks failing, then all pass
	mockGHWithCIFix(t, "main", "", "https://github.com/user/repo/pull/62",
		[]string{
			`[{"name":"lint","state":"FAILURE","conclusion":"failure"},{"name":"test","state":"FAILURE","conclusion":"failure"}]`,
			`[{"name":"lint","state":"SUCCESS","conclusion":"success"},{"name":"test","state":"SUCCESS","conclusion":"success"}]`,
		},
		"12345", "Multiple errors",
	)

	executor := &fixLoopExecutor{dir: dir}

	var buf bytes.Buffer
	cfg := Config{
		Output:       &buf,
		RemoteURL:    "https://github.com/user/repo.git",
		IsGitHub:     true,
		Executor:     executor,
		RepoRoot:     dir,
		PollInterval: time.Millisecond,
	}

	err := Run(context.Background(), cfg)
	require.NoError(t, err)

	output := buf.String()
	// Should target the first failure (lint)
	assert.Contains(t, output, "CI failed — lint (attempt 1/10)")
	assert.Contains(t, output, "CI passed — PR ready for review")
}

func TestRun_CIFix_LogFetchFailed(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)

	gitCmd(t, dir, "checkout", "-b", "feature-ci-logfail")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "feature.txt"), []byte("new feature"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add feature")

	addWorkflowFile(t, dir)
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add workflows")

	chdir(t, dir)

	// Mock where run view fails. pr view flips after pr create writes the marker so
	// the post-LLM PRExists check sees the new PR and the flow proceeds to CI monitoring.
	binDir := t.TempDir()
	markerFile := filepath.Join(binDir, "pr_marker")

	script := fmt.Sprintf(`#!/bin/sh
case "$1 $2" in
  "repo view")
    printf '%%s' 'main'
    ;;
  "pr view")
    if [ -f %s ]; then
      printf '%%s' '{"state":"OPEN","url":"https://github.com/user/repo/pull/63"}'
      exit 0
    fi
    exit 1
    ;;
  "pr create")
    : > %s
    printf '%%s' 'https://github.com/user/repo/pull/63'
    ;;
  "pr checks")
    printf '%%s' '[{"name":"lint","state":"FAILURE","conclusion":"failure"}]'
    ;;
  "run list")
    printf '%%s' '[{"databaseId":12345}]'
    ;;
  "run view")
    echo 'failed to fetch logs' >&2
    exit 1
    ;;
  *)
    echo "unexpected gh call: $*" >&2
    exit 99
    ;;
esac
`, markerFile, markerFile)
	ghPath := filepath.Join(binDir, "gh")
	require.NoError(t, os.WriteFile(ghPath, []byte(script), 0o755)) //nolint:gosec // test script
	origPath := os.Getenv("PATH")
	t.Setenv("PATH", binDir+string(os.PathListSeparator)+origPath)

	var buf bytes.Buffer
	cfg := Config{
		Output:       &buf,
		RemoteURL:    "https://github.com/user/repo.git",
		IsGitHub:     true,
		Executor:     &mockExecutor{output: "Fix\n\nBody."},
		RepoRoot:     dir,
		PollInterval: time.Millisecond,
	}

	err := Run(context.Background(), cfg)
	require.Error(t, err)
	assert.Contains(t, buf.String(), "Failed to read CI logs")
}

func TestRun_CIFix_PushAfterFixFailed(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)

	gitCmd(t, dir, "checkout", "-b", "feature-ci-pushfail")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "feature.txt"), []byte("new feature"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add feature")

	addWorkflowFile(t, dir)
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add workflows")

	chdir(t, dir)

	// CI fails, fix succeeds, but then we remove the remote to make push fail
	mockGHWithCIFix(t, "main", "", "https://github.com/user/repo/pull/64",
		[]string{
			`[{"name":"lint","state":"FAILURE","conclusion":"failure"}]`,
		},
		"12345", "Error: lint issue",
	)

	// Use an executor that removes the remote after creating the fix file
	executor := &pushFailExecutor{dir: dir}

	var buf bytes.Buffer
	cfg := Config{
		Output:       &buf,
		RemoteURL:    "https://github.com/user/repo.git",
		IsGitHub:     true,
		Executor:     executor,
		RepoRoot:     dir,
		PollInterval: time.Millisecond,
	}

	err := Run(context.Background(), cfg)
	require.Error(t, err)
	assert.Contains(t, err.Error(), "failed to push CI fix")
}

// pushFailExecutor creates a fix file and then sabotages the git remote.
// On the PR-creation prompt it invokes the mocked `gh pr create` so the post-LLM
// PRExists check sees the new PR.
type pushFailExecutor struct {
	dir     string
	callNum int
}

func (m *pushFailExecutor) Run(ctx context.Context, _ io.Writer, _ model.Type, args ...string) error {
	m.callNum++
	if isPRPrompt(args) {
		return runMockGHCreate(ctx, "Fix", "Body")
	}
	// Create a fix file
	fileName := filepath.Join(m.dir, fmt.Sprintf("fix-%d.txt", m.callNum))
	if err := os.WriteFile(fileName, []byte("fix"), 0o600); err != nil {
		return err
	}
	// Sabotage the remote so push fails
	cmd := exec.CommandContext(ctx, "git", "remote", "set-url", "origin", "/nonexistent/path")
	cmd.Dir = m.dir
	_ = cmd.Run() //nolint:errcheck // best-effort sabotage for test
	return nil
}

func TestRun_CIFix_ContextCancelled(t *testing.T) {
	dir := t.TempDir()
	addWorkflowFile(t, dir)

	// Mock gh: CI fails, run list/view work, but context gets cancelled during fix
	binDir := t.TempDir()
	script := `#!/bin/sh
case "$1 $2" in
  "pr checks")
    printf '%s' '[{"name":"lint","state":"FAILURE","conclusion":"failure"}]'
    ;;
  "run list")
    printf '%s' '[{"databaseId":12345}]'
    ;;
  "run view")
    sleep 10
    ;;
  *)
    echo "unexpected gh call: $*" >&2
    exit 99
    ;;
esac
`
	ghPath := filepath.Join(binDir, "gh")
	require.NoError(t, os.WriteFile(ghPath, []byte(script), 0o755)) //nolint:gosec // test script
	origPath := os.Getenv("PATH")
	t.Setenv("PATH", binDir+string(os.PathListSeparator)+origPath)

	ctx, cancel := context.WithCancel(context.Background())

	var buf bytes.Buffer
	cfg := Config{
		Output:       &buf,
		Executor:     &mockExecutor{output: "fix"},
		RepoRoot:     dir,
		PollInterval: time.Millisecond,
	}

	go func() {
		time.Sleep(100 * time.Millisecond)
		cancel()
	}()

	err := monitorCI(ctx, cfg, true, "main")
	// Context cancellation during fix should not panic
	// It may return an error from the gh command being killed, which is acceptable
	_ = err
}

func TestRun_CIFix_LogsNotWrittenToDisk(t *testing.T) {
	dir := initGitRepo(t)
	initBareRemote(t, dir)

	gitCmd(t, dir, "checkout", "-b", "feature-ci-nologs")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "feature.txt"), []byte("new feature"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add feature")

	addWorkflowFile(t, dir)
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "add workflows")

	chdir(t, dir)

	// Use a distinctive sentinel in the log content that we can search for on disk
	const sentinel = "UNIQUE_LOG_SENTINEL_AC7_VERIFICATION_12345"

	mockGHWithCIFix(t, "main", "", "https://github.com/user/repo/pull/70",
		[]string{
			`[{"name":"lint","state":"FAILURE","conclusion":"failure"}]`,
			`[{"name":"lint","state":"SUCCESS","conclusion":"success"}]`,
		},
		"12345", sentinel,
	)

	executor := &fixLoopExecutor{dir: dir}

	var buf bytes.Buffer
	cfg := Config{
		Output:       &buf,
		RemoteURL:    "https://github.com/user/repo.git",
		IsGitHub:     true,
		Executor:     executor,
		RepoRoot:     dir,
		PollInterval: time.Millisecond,
	}

	err := Run(context.Background(), cfg)
	require.NoError(t, err)

	// Walk the repo directory and verify no file contains the log sentinel.
	// CI failure logs must only be held in memory.
	rootFS := os.DirFS(dir)
	err = fs.WalkDir(rootFS, ".", func(path string, d fs.DirEntry, walkErr error) error {
		if walkErr != nil || d.IsDir() {
			return walkErr
		}
		data, readErr := fs.ReadFile(rootFS, path)
		if readErr != nil {
			return readErr
		}
		if strings.Contains(string(data), sentinel) {
			t.Errorf("CI failure log content found on disk at %s", path)
		}
		return nil
	})
	require.NoError(t, err)
}

func TestRun_CI_ContextCancelled(t *testing.T) {
	dir := t.TempDir()
	addWorkflowFile(t, dir)

	// Mock gh that always returns pending
	mockGHScript(t, `printf '%s' '[{"name":"lint","state":"PENDING","conclusion":""},{"name":"test","state":"PENDING","conclusion":""}]'`)

	ctx, cancel := context.WithCancel(context.Background())

	var buf bytes.Buffer
	cfg := Config{
		Output:       &buf,
		RepoRoot:     dir,
		PollInterval: time.Millisecond,
	}

	// Cancel context shortly after starting
	go func() {
		time.Sleep(50 * time.Millisecond)
		cancel()
	}()

	// Test monitorCI directly to avoid push timing issues
	err := monitorCI(ctx, cfg, true, "main")
	// Context cancellation should not return an error
	assert.NoError(t, err)
}
