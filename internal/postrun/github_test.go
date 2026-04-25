package postrun

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// mockGH creates a fake gh script in a temp directory and prepends it to PATH.
// The script writes the given stdout to stdout.
func mockGH(t *testing.T, stdout string) {
	t.Helper()
	binDir := t.TempDir()

	script := "#!/bin/sh\n"
	if stdout != "" {
		script += "printf '%s' '" + stdout + "'\n"
	}

	ghPath := filepath.Join(binDir, "gh")
	require.NoError(t, os.WriteFile(ghPath, []byte(script), 0o755)) //nolint:gosec // test script needs execute permission

	origPath := os.Getenv("PATH")
	t.Setenv("PATH", binDir+string(os.PathListSeparator)+origPath)
}

// mockGHScript creates a fake gh script with custom shell content.
func mockGHScript(t *testing.T, script string) {
	t.Helper()
	binDir := t.TempDir()

	ghPath := filepath.Join(binDir, "gh")
	require.NoError(t, os.WriteFile(ghPath, []byte("#!/bin/sh\n"+script), 0o755)) //nolint:gosec // test script needs execute permission

	origPath := os.Getenv("PATH")
	t.Setenv("PATH", binDir+string(os.PathListSeparator)+origPath)
}

func TestDefaultBranch(t *testing.T) {
	mockGH(t, "main")

	branch, err := DefaultBranch(context.Background())
	require.NoError(t, err)
	assert.Equal(t, "main", branch)
}

func TestDefaultBranch_DevelopBranch(t *testing.T) {
	mockGH(t, "develop")

	branch, err := DefaultBranch(context.Background())
	require.NoError(t, err)
	assert.Equal(t, "develop", branch)
}

func TestPRExists_NoPR(t *testing.T) {
	mockGHScript(t, "exit 1\n")

	exists, url, err := PRExists(context.Background())
	require.NoError(t, err)
	assert.False(t, exists)
	assert.Empty(t, url)
}

func TestPRExists_HasPR(t *testing.T) {
	mockGH(t, `{"state":"OPEN","url":"https://github.com/user/repo/pull/42"}`)

	exists, url, err := PRExists(context.Background())
	require.NoError(t, err)
	assert.True(t, exists)
	assert.Equal(t, "https://github.com/user/repo/pull/42", url)
}

func TestCheckStatus_AllGreen(t *testing.T) {
	mockGHScript(t, `
printf '%s' '[{"name":"lint","state":"SUCCESS","conclusion":"success"},{"name":"test","state":"SUCCESS","conclusion":"success"}]'
`)

	checks, err := CheckStatus(context.Background(), true, "main")
	require.NoError(t, err)
	require.Len(t, checks, 2)
	assert.Equal(t, "lint", checks[0].Name)
	assert.Equal(t, "passed", checks[0].Status)
	assert.Equal(t, "test", checks[1].Name)
	assert.Equal(t, "passed", checks[1].Status)
}

func TestCheckStatus_Mixed(t *testing.T) {
	mockGHScript(t, `
printf '%s' '[{"name":"lint","state":"SUCCESS","conclusion":"success"},{"name":"test","state":"FAILURE","conclusion":"failure"},{"name":"build","state":"PENDING","conclusion":""}]'
`)

	checks, err := CheckStatus(context.Background(), true, "main")
	require.NoError(t, err)
	require.Len(t, checks, 3)
	assert.Equal(t, "passed", checks[0].Status)
	assert.Equal(t, "failed", checks[1].Status)
	assert.Equal(t, "pending", checks[2].Status)
}

func TestCheckStatus_NoPR(t *testing.T) {
	// When no PR, CheckStatus uses gh run list with --json
	mockGHScript(t, `
printf '%s' '[{"name":"CI","status":"completed","conclusion":"success"}]'
`)

	checks, err := CheckStatus(context.Background(), false, "main")
	require.NoError(t, err)
	require.Len(t, checks, 1)
	assert.Equal(t, "CI", checks[0].Name)
	assert.Equal(t, "passed", checks[0].Status)
}

func TestCheckStatus_Empty(t *testing.T) {
	mockGHScript(t, `printf '%s' '[]'`)

	checks, err := CheckStatus(context.Background(), true, "main")
	require.NoError(t, err)
	assert.Empty(t, checks)
}

func TestTruncateLog_UnderLimit(t *testing.T) {
	content := strings.Repeat("a", 10*1024) // 10KB
	result := truncateLog(content)
	assert.Equal(t, content, result)
}

func TestTruncateLog_OverLimit(t *testing.T) {
	content := strings.Repeat("a", 100*1024) // 100KB
	result := truncateLog(content)
	assert.Equal(t, maxLogSize, len(result[:maxLogSize]))
	assert.Len(t, result, maxLogSize+len("\n\n[log truncated — exceeded 50KB limit]"))
	assert.Contains(t, result, "[log truncated")
}

func TestTruncateLog_ExactLimit(t *testing.T) {
	content := strings.Repeat("a", maxLogSize) // exactly 50KB
	result := truncateLog(content)
	assert.Equal(t, content, result)
}

func TestFailureLogs(t *testing.T) {
	mockGHScript(t, `printf '%s' 'Error: lint failed on line 42'`)

	logs, err := FailureLogs(context.Background(), "12345")
	require.NoError(t, err)
	assert.Equal(t, "Error: lint failed on line 42", logs)
}

func TestFailureLogs_Truncated(t *testing.T) {
	// Create a gh mock that outputs >50KB
	binDir := t.TempDir()
	// Generate a large output via dd
	script := "#!/bin/sh\ndd if=/dev/zero bs=1024 count=60 2>/dev/null | tr '\\0' 'x'\n"
	ghPath := filepath.Join(binDir, "gh")
	require.NoError(t, os.WriteFile(ghPath, []byte(script), 0o755)) //nolint:gosec // test script

	origPath := os.Getenv("PATH")
	t.Setenv("PATH", binDir+string(os.PathListSeparator)+origPath)

	logs, err := FailureLogs(context.Background(), "12345")
	require.NoError(t, err)
	assert.Contains(t, logs, "[log truncated")
}

func TestFailedRunID(t *testing.T) {
	mockGH(t, `[{"databaseId":98765}]`)

	id, err := FailedRunID(context.Background())
	require.NoError(t, err)
	assert.Equal(t, "98765", id)
}

func TestFailedRunID_NoRuns(t *testing.T) {
	mockGH(t, `[]`)

	_, err := FailedRunID(context.Background())
	require.Error(t, err)
	assert.Contains(t, err.Error(), "no failed runs found")
}
