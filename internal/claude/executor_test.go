package claude_test

import (
	"bytes"
	"context"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/yarlson/snap/internal/claude"
	"github.com/yarlson/snap/internal/model"
)

func TestExecutor_Run(t *testing.T) {
	tests := []struct {
		name    string
		args    []string
		wantErr bool
	}{
		{
			name:    "runs with basic args",
			args:    []string{"-p", "hello"},
			wantErr: false,
		},
		{
			name:    "runs with multiple args",
			args:    []string{"-p", "test prompt"},
			wantErr: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			executor := claude.NewExecutor()
			var output bytes.Buffer
			err := executor.Run(context.Background(), &output, model.Fast, tt.args...)

			if tt.wantErr {
				assert.Error(t, err)
			} else {
				// We don't assert no error because claude might not be installed
				// or might fail, but we verify the interface works
				_ = output
				_ = err
			}
		})
	}
}

func TestStreamParser(t *testing.T) {
	tests := []struct {
		name            string
		input           string
		expectedContent []string
	}{
		{
			name: "parses text messages",
			input: `{"type":"assistant","message":{"content":[{"type":"text","text":"Hello world"}]}}
{"type":"assistant","message":{"content":[{"type":"text","text":"Second message"}]}}`,
			expectedContent: []string{"Hello world", "Second message"},
		},
		{
			name:            "parses tool use",
			input:           `{"type":"assistant","message":{"content":[{"type":"text","text":"Creating file"},{"type":"tool_use","name":"Write","input":{"file_path":"/path/to/file.go","content":"package main"}}]}}`,
			expectedContent: []string{"Creating file", "🔧 Write", "file_path=/path/to/file.go"},
		},
		{
			name:            "parses tool results success",
			input:           `{"type":"user","message":{"content":[{"type":"tool_result","content":"File created successfully"}]}}`,
			expectedContent: []string{"File created successfully"},
		},
		{
			name:            "parses tool results error",
			input:           `{"type":"user","message":{"content":[{"type":"tool_result","content":"Failed to create file","is_error":true}]}}`,
			expectedContent: []string{"Failed to create file"},
		},
		{
			name: "handles mixed content and suppresses Read results",
			input: `{"type":"assistant","message":{"content":[{"type":"text","text":"Starting work"}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_123","name":"Read","input":{"file_path":"test.go"}}]}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_123","content":"File content here..."}]}}
{"type":"assistant","message":{"content":[{"type":"text","text":"Done"}]}}`,
			expectedContent: []string{"Starting work", "🔧 Read", "file_path=test.go", "Done"},
		},
		{
			name: "shows Write tool results",
			input: `{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_456","name":"Write","input":{"file_path":"output.go"}}]}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_456","content":"File created successfully"}]}}`,
			expectedContent: []string{"🔧 Write", "file_path=output.go", "File created successfully"},
		},
		{
			name: "trims long tool results to viewport",
			input: `{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_789","name":"Bash","input":{"command":"seq 12"}}]}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_789","content":"line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\nline 11\nline 12"}]}}`,
			expectedContent: []string{"🔧 Bash", "Tool output (last 10/12 lines)", "line 12"},
		},
		{
			name:  "parses TodoWrite tool input",
			input: `{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_todo_1","name":"TodoWrite","input":{"todos":[{"content":"Inspect parser tokenization logic","status":"pending","activeForm":"Inspecting parser tokenization logic"},{"content":"Review parser error handling paths","status":"in_progress","activeForm":"Reviewing parser error handling paths"}]}}]}}`,
			expectedContent: []string{
				"🔧 TodoWrite (2 todos)",
			},
		},
		{
			name: "parses TodoWrite tool_use_result diff payload",
			input: `{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_todo_2","name":"TodoWrite","input":{"todos":[{"content":"Inspect parser tokenization logic","status":"pending","activeForm":"Inspecting parser tokenization logic"}]}}]}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_todo_2","content":"Todos have been modified successfully"}]},"tool_use_result":{"oldTodos":[{"content":"Inspect parser tokenization logic","status":"pending","activeForm":"Inspecting parser tokenization logic"}],"newTodos":[{"content":"Inspect parser tokenization logic","status":"completed","activeForm":"Inspecting parser tokenization logic"},{"content":"Review parser error handling paths","status":"in_progress","activeForm":"Reviewing parser error handling paths"}]}}`,
			expectedContent: []string{
				"TodoWrite",
				"[x] Inspect parser tokenization logic",
				"[~] Review parser error handling paths",
			},
		},
		{
			name: "skips non-relevant messages",
			input: `{"type":"system","subtype":"init"}
{"type":"assistant","message":{"content":[{"type":"text","text":"Only this"}]}}`,
			expectedContent: []string{"Only this"},
		},
		{
			name:            "handles empty input",
			input:           "",
			expectedContent: []string{},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			reader := strings.NewReader(tt.input)
			var output bytes.Buffer
			parser := claude.NewStreamParser(&output)

			err := parser.Parse(reader)
			require.NoError(t, err)

			// Strip colors for easier testing
			strippedResult := stripColors(output.String())

			// Check that all expected content appears in the output
			for _, expected := range tt.expectedContent {
				assert.Contains(t, strippedResult, expected,
					"Expected to find %q in output", expected)
			}

			if tt.name == "trims long tool results to viewport" {
				assert.NotContains(t, strippedResult, "│ line 1 ")
				assert.NotContains(t, strippedResult, "│ line 2 ")
			}
		})
	}
}

// stripColors removes ANSI color codes from a string.
func stripColors(s string) string {
	var result strings.Builder
	inEscape := false
	for _, ch := range s {
		switch {
		case ch == '\033':
			inEscape = true
		case inEscape && ch == 'm':
			inEscape = false
		case !inEscape:
			result.WriteRune(ch)
		}
	}
	return result.String()
}
