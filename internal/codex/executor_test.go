package codex_test

import (
	"bytes"
	"context"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/yarlson/snap/internal/codex"
	"github.com/yarlson/snap/internal/model"
	"github.com/yarlson/snap/internal/ui"
)

func TestBuildCommandArgs(t *testing.T) {
	tests := []struct {
		name     string
		input    []string
		expected []string
	}{
		{
			name:     "fresh exec",
			input:    []string{"do work"},
			expected: []string{"exec", "--json", "--dangerously-bypass-approvals-and-sandbox", "do work"},
		},
		{
			name:     "resume with context flag",
			input:    []string{"-c", "continue work"},
			expected: []string{"exec", "resume", "--last", "--json", "--dangerously-bypass-approvals-and-sandbox", "continue work"},
		},
		{
			name:     "context flag in the middle",
			input:    []string{"--skip-git-repo-check", "-c", "continue work"},
			expected: []string{"exec", "resume", "--last", "--json", "--dangerously-bypass-approvals-and-sandbox", "--skip-git-repo-check", "continue work"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			assert.Equal(t, tt.expected, codex.BuildCommandArgs(tt.input...))
		})
	}
}

func TestEventParser_Parse(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		contains []string
	}{
		{
			name: "agent message and command success",
			input: strings.Join([]string{
				`{"type":"item.started","item":{"id":"1","type":"command_execution","command":"/bin/zsh -lc pwd","status":"in_progress"}}`,
				`{"type":"item.completed","item":{"id":"1","type":"command_execution","command":"/bin/zsh -lc pwd","aggregated_output":"/Users/test\n","exit_code":0,"status":"completed"}}`,
				`{"type":"item.completed","item":{"id":"2","type":"agent_message","text":"Done"}}`,
			}, "\n"),
			contains: []string{"🔧 Shell", "pwd", "/Users/test", "Done"},
		},
		{
			name: "command failure",
			input: `{"type":"item.started","item":{"id":"1","type":"command_execution","command":"/bin/zsh -lc exit 7","status":"in_progress"}}` + "\n" +
				`{"type":"item.completed","item":{"id":"1","type":"command_execution","command":"/bin/zsh -lc exit 7","aggregated_output":"","exit_code":7,"status":"failed"}}`,
			contains: []string{"Shell", "exit 7", "Command failed (exit code 7)"},
		},
		{
			name: "long command output is trimmed to viewport",
			input: `{"type":"item.started","item":{"id":"1","type":"command_execution","command":"/bin/zsh -lc seq 12","status":"in_progress"}}` + "\n" +
				`{"type":"item.completed","item":{"id":"1","type":"command_execution","command":"/bin/zsh -lc seq 12","aggregated_output":"line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\nline 11\nline 12\n","exit_code":0,"status":"completed"}}`,
			contains: []string{"Tool output (last 10/12 lines)", "line 12"},
		},
		{
			name:     "skips malformed lines",
			input:    `not-json` + "\n" + `{"type":"item.completed","item":{"id":"2","type":"agent_message","text":"ok"}}`,
			contains: []string{"ok"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var out bytes.Buffer
			parser := codex.NewEventParser(&out)
			err := parser.Parse(strings.NewReader(tt.input))
			require.NoError(t, err)

			rendered := ui.StripColors(out.String())
			for _, fragment := range tt.contains {
				assert.Contains(t, rendered, fragment)
			}

			if tt.name == "long command output is trimmed to viewport" {
				assert.NotContains(t, rendered, "│ line 1 ")
				assert.NotContains(t, rendered, "│ line 2 ")
			}
		})
	}
}

func TestExecutor_Metadata(t *testing.T) {
	executor := codex.NewExecutor()
	assert.Equal(t, "codex", executor.ProviderName())

	err := executor.Run(context.Background(), &bytes.Buffer{}, model.Fast, "Reply with exactly hi")
	_ = err // Runtime execution depends on local codex auth/setup; interface is exercised.
}
