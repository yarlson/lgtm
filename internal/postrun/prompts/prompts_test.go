package prompts_test

import (
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/yarlson/snap/internal/postrun/prompts"
)

func TestPR_RendersTemplate(t *testing.T) {
	data := prompts.PRData{
		PRDContent:     "## Summary\nAdd user authentication to the app.",
		CommitMessages: "Wire OAuth login\n\nAdd token refresh helper",
		DiffStat:       " 5 files changed, 120 insertions(+), 10 deletions(-)",
	}

	result, err := prompts.PR(data)
	require.NoError(t, err)

	assert.NotEmpty(t, result)
	assert.Contains(t, result, "authentication")
	assert.Contains(t, result, "5 files changed")
	assert.Contains(t, result, "Wire OAuth login")
	assert.Contains(t, result, "### Why")
	assert.Contains(t, result, "### What")
	assert.Contains(t, result, "### How to verify")
	assert.Contains(t, result, "50–72")
	assert.Contains(t, result, "~150 words")
	assert.Contains(t, result, "Anti-patterns")
	assert.Contains(t, result, "gh pr create")
	assert.Equal(t, strings.TrimSpace(result), result)
}

func TestPR_EmptyInputs(t *testing.T) {
	data := prompts.PRData{}

	result, err := prompts.PR(data)
	require.NoError(t, err)
	assert.NotEmpty(t, result)
	assert.NotContains(t, result, "### Why")
	assert.Contains(t, result, "### What")
	assert.Contains(t, result, "### How to verify")
	assert.Contains(t, result, "PRD is not available")
	assert.Contains(t, result, "(no commits in range)")
	assert.Contains(t, result, "(no diff)")
}

func TestPR_NoPRDButCommits(t *testing.T) {
	data := prompts.PRData{
		CommitMessages: "Add foo handler",
		DiffStat:       " 1 file changed",
	}

	result, err := prompts.PR(data)
	require.NoError(t, err)

	assert.NotContains(t, result, "### Why")
	assert.Contains(t, result, "### What")
	assert.Contains(t, result, "Add foo handler")
	assert.Contains(t, result, "PRD is not available")
}

func TestPR_WithPRDNoCommits(t *testing.T) {
	data := prompts.PRData{
		PRDContent: "## Goal\nThing",
	}

	result, err := prompts.PR(data)
	require.NoError(t, err)

	assert.Contains(t, result, "### Why")
	assert.Contains(t, result, "(no commits in range)")
	assert.Contains(t, result, "(no diff)")
}

func TestCIFix_RendersTemplate(t *testing.T) {
	data := prompts.CIFixData{
		FailureLogs:   "Error: undefined variable 'foo' at main.go:10",
		CheckName:     "lint",
		AttemptNumber: 2,
		MaxAttempts:   10,
	}

	result, err := prompts.CIFix(data)
	require.NoError(t, err)

	assert.NotEmpty(t, result)
	assert.Contains(t, result, "lint")
	assert.Contains(t, result, "undefined variable")
	assert.Contains(t, result, "attempt 2 of 10")
	assert.Equal(t, strings.TrimSpace(result), result)
}

func TestCIFix_EmptyLogs(t *testing.T) {
	data := prompts.CIFixData{
		FailureLogs:   "",
		CheckName:     "test",
		AttemptNumber: 1,
		MaxAttempts:   10,
	}

	result, err := prompts.CIFix(data)
	require.NoError(t, err)
	assert.NotEmpty(t, result)
	assert.Contains(t, result, "test")
}
