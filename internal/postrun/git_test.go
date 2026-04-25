package postrun

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestIsGitHubRemote(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected bool
	}{
		{name: "HTTPS with .git", input: "https://github.com/user/repo.git", expected: true},
		{name: "HTTPS without .git", input: "https://github.com/user/repo", expected: true},
		{name: "SSH", input: "git@github.com:user/repo.git", expected: true},
		{name: "SSH protocol", input: "ssh://git@github.com/user/repo", expected: true},
		{name: "GitLab HTTPS", input: "https://gitlab.com/user/repo.git", expected: false},
		{name: "Bitbucket SSH", input: "git@bitbucket.org:user/repo.git", expected: false},
		{name: "GitHub Enterprise", input: "https://github.example.com/user/repo", expected: false},
		{name: "empty", input: "", expected: false},
		{name: "not a URL", input: "not-a-url", expected: false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			assert.Equal(t, tt.expected, IsGitHubRemote(tt.input))
		})
	}
}

// gitCmd runs a git command in the given directory.
func gitCmd(t *testing.T, dir string, args ...string) {
	t.Helper()
	ctx := context.Background()
	cmd := exec.CommandContext(ctx, "git", args...)
	cmd.Dir = dir
	cmd.Env = append(os.Environ(),
		"GIT_AUTHOR_NAME=test",
		"GIT_AUTHOR_EMAIL=test@test.com",
		"GIT_COMMITTER_NAME=test",
		"GIT_COMMITTER_EMAIL=test@test.com",
	)
	out, err := cmd.CombinedOutput()
	require.NoError(t, err, "git %v failed: %s", args, out)
}

// initGitRepo creates a git repo in a temp directory with an initial commit.
func initGitRepo(t *testing.T) string {
	t.Helper()
	dir := t.TempDir()
	gitCmd(t, dir, "init", "-b", "main")
	gitCmd(t, dir, "config", "user.email", "test@test.com")
	gitCmd(t, dir, "config", "user.name", "test")
	require.NoError(t, os.WriteFile(filepath.Join(dir, "README.md"), []byte("# Test"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "initial")
	return dir
}

// initBareRemote creates a bare git repo and adds it as "origin" to the work repo.
func initBareRemote(t *testing.T, workDir string) string {
	t.Helper()
	bareDir := t.TempDir()
	gitCmd(t, bareDir, "init", "--bare", "-b", "main")
	gitCmd(t, workDir, "remote", "add", "origin", bareDir)
	return bareDir
}

// chdir changes to the given directory and returns a cleanup function.
func chdir(t *testing.T, dir string) {
	t.Helper()
	oldDir, err := os.Getwd()
	require.NoError(t, err)
	require.NoError(t, os.Chdir(dir))
	t.Cleanup(func() {
		//nolint:errcheck // Best-effort restore in test cleanup
		os.Chdir(oldDir)
	})
}

func TestDetectRemote_WithRemote(t *testing.T) {
	dir := initGitRepo(t)
	bareDir := initBareRemote(t, dir)
	chdir(t, dir)

	remote, err := DetectRemote()
	require.NoError(t, err)
	assert.Equal(t, bareDir, remote)
}

func TestDetectRemote_NoRemote(t *testing.T) {
	dir := initGitRepo(t)
	chdir(t, dir)

	remote, err := DetectRemote()
	require.NoError(t, err)
	assert.Empty(t, remote)
}

func TestCurrentBranch(t *testing.T) {
	dir := initGitRepo(t)
	chdir(t, dir)

	branch, err := CurrentBranch(context.Background())
	require.NoError(t, err)
	assert.Equal(t, "main", branch)
}

func TestPush_Success(t *testing.T) {
	dir := initGitRepo(t)
	bareDir := initBareRemote(t, dir)
	chdir(t, dir)

	err := Push(context.Background())
	require.NoError(t, err)

	// Verify commit exists in bare repo
	ctx := context.Background()
	cmd := exec.CommandContext(ctx, "git", "log", "--oneline")
	cmd.Dir = bareDir
	out, err := cmd.CombinedOutput()
	require.NoError(t, err)
	assert.Contains(t, string(out), "initial")
}

func TestCommitAll(t *testing.T) {
	dir := initGitRepo(t)
	chdir(t, dir)

	// Create uncommitted changes
	require.NoError(t, os.WriteFile(filepath.Join(dir, "newfile.txt"), []byte("hello"), 0o600))

	err := CommitAll(context.Background(), "add new file")
	require.NoError(t, err)

	// Verify new commit exists
	cmd := exec.CommandContext(context.Background(), "git", "log", "--oneline")
	cmd.Dir = dir
	out, err := cmd.CombinedOutput()
	require.NoError(t, err)
	assert.Contains(t, string(out), "add new file")
}

func TestCommitAll_NothingToCommit(t *testing.T) {
	dir := initGitRepo(t)
	chdir(t, dir)

	err := CommitAll(context.Background(), "empty commit")
	require.Error(t, err)
	assert.Contains(t, err.Error(), "nothing to commit")
}

func TestCommitMessages_HappyPath(t *testing.T) {
	dir := initGitRepo(t)
	gitCmd(t, dir, "checkout", "-b", "feature")

	require.NoError(t, os.WriteFile(filepath.Join(dir, "first.txt"), []byte("first"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "Add login form")

	require.NoError(t, os.WriteFile(filepath.Join(dir, "second.txt"), []byte("second"), 0o600))
	gitCmd(t, dir, "add", ".")
	gitCmd(t, dir, "commit", "-m", "Wire login to API\n\nUses POST /session.")

	chdir(t, dir)

	out, err := CommitMessages(context.Background(), "main")
	require.NoError(t, err)
	assert.Contains(t, out, "Add login form")
	assert.Contains(t, out, "Wire login to API")
	assert.Contains(t, out, "Uses POST /session.")
	assert.Less(t, strings.Index(out, "Add login form"), strings.Index(out, "Wire login to API"),
		"oldest commit should appear first (--reverse)")
}

func TestCommitMessages_EmptyRange(t *testing.T) {
	dir := initGitRepo(t)
	chdir(t, dir)

	out, err := CommitMessages(context.Background(), "main")
	require.NoError(t, err)
	assert.Empty(t, out)
}
