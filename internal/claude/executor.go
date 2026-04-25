package claude

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"os/exec"
	"strings"

	"github.com/yarlson/snap/internal/model"
	"github.com/yarlson/snap/internal/ui"
)

// Executor runs the claude CLI and streams its output.
type Executor struct{}

// NewExecutor creates a new claude CLI executor.
func NewExecutor() *Executor {
	return &Executor{}
}

// resolveModel maps an abstract model type to a Claude-specific model name.
func resolveModel(mt model.Type) string {
	switch mt {
	case model.Fast:
		return "haiku"
	case model.Thinking:
		return "opus"
	default:
		return ""
	}
}

// Run executes the claude CLI with the given arguments and streams parsed output to the writer.
// The model parameter is resolved to a Claude-specific model name and passed via --model flag.
func (e *Executor) Run(ctx context.Context, w io.Writer, mt model.Type, args ...string) error {
	// Add required flags for stream-json output
	fullArgs := []string{
		"--dangerously-skip-permissions",
		"--print",
		"--output-format=stream-json",
		"--include-partial-messages",
		"--verbose",
	}
	if resolved := resolveModel(mt); resolved != "" {
		fullArgs = append(fullArgs, "--model", resolved)
	}
	fullArgs = append(fullArgs, args...)

	cmd := exec.CommandContext(ctx, "claude", fullArgs...)

	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return fmt.Errorf("failed to create stdout pipe: %w", err)
	}

	stderr, err := cmd.StderrPipe()
	if err != nil {
		return fmt.Errorf("failed to create stderr pipe: %w", err)
	}

	if err := cmd.Start(); err != nil {
		return fmt.Errorf("failed to start claude command: %w", err)
	}

	// Parse and stream output in real-time
	parser := NewStreamParser(w)
	parseErr := parser.Parse(stdout)

	// Read stderr
	var stderrBytes []byte
	if stderrData, err := io.ReadAll(stderr); err == nil {
		stderrBytes = stderrData
	}

	// Wait for command to complete
	if err := cmd.Wait(); err != nil {
		if len(stderrBytes) > 0 {
			return fmt.Errorf("claude command failed: %w (stderr: %s)", err, string(stderrBytes))
		}
		return fmt.Errorf("claude command failed: %w", err)
	}

	return parseErr
}

// StreamParser parses stream-json output and writes formatted output in real-time.
type StreamParser struct {
	writer           io.Writer
	markdownRenderer *ui.MarkdownRenderer
	// Track tool uses to suppress noisy Read results
	toolUses map[string]string // tool_use_id -> tool_name
	// Track if last output was a tool result (for spacing)
	lastWasToolResult bool
}

// NewStreamParser creates a new stream parser that writes to the given writer.
func NewStreamParser(w io.Writer) *StreamParser {
	return &StreamParser{
		writer:           w,
		markdownRenderer: ui.NewMarkdownRenderer(),
		toolUses:         make(map[string]string),
	}
}

// Parse reads stream-json input line by line and writes formatted output immediately.
func (p *StreamParser) Parse(r io.Reader) error {
	scanner := bufio.NewScanner(r)
	// Increase buffer size to handle large JSON lines (e.g., when reading large files)
	const maxCapacity = 10 * 1024 * 1024 // 10MB
	buf := make([]byte, maxCapacity)
	scanner.Buffer(buf, maxCapacity)

	for scanner.Scan() {
		line := scanner.Text()
		if line == "" {
			continue
		}

		var msg StreamMessage
		if err := json.Unmarshal([]byte(line), &msg); err != nil {
			// Skip malformed lines
			continue
		}

		switch msg.Type {
		case "assistant":
			for _, content := range msg.Message.Content {
				switch content.Type {
				case "text":
					if content.Text != "" {
						// Render markdown for assistant text responses
						rendered, err := p.markdownRenderer.Render(content.Text)
						if err != nil {
							// Fall back to raw text on error
							rendered = content.Text
						}
						if _, err := fmt.Fprint(p.writer, rendered); err != nil {
							return err
						}
						// Text output resets the tool result tracking
						p.lastWasToolResult = false
					}
				case "tool_use":
					// Add spacing before tool use if last output was a tool result (SpaceXS per design system)
					if p.lastWasToolResult {
						if _, err := fmt.Fprint(p.writer, ui.VerticalSpace(ui.SpaceXS)); err != nil {
							return err
						}
					}

					// Track tool use for result filtering
					if content.ID != "" && content.Name != "" {
						p.toolUses[content.ID] = content.Name
					}
					if output := formatToolUse(content); output != "" {
						if _, err := fmt.Fprint(p.writer, output); err != nil {
							return err
						}
					}
					// Tool use resets the flag
					p.lastWasToolResult = false
				}
			}
		case "user":
			for _, content := range msg.Message.Content {
				if content.Type != "tool_result" {
					continue
				}

				// Suppress Read tool results (too noisy)
				toolName := p.toolUses[content.ToolUseID]
				if toolName == "Read" {
					continue
				}

				output := ""
				if toolName == "TodoWrite" {
					output = formatTodoToolResult(content, msg.ToolUseResult)
				}
				if output == "" {
					output = formatToolResult(content)
				}
				if output != "" {
					if _, err := fmt.Fprint(p.writer, output); err != nil {
						return err
					}
					// Mark that we just output a tool result
					p.lastWasToolResult = true
				}
			}
		}

		// Flush output if writer is stdout/stderr
		if f, ok := p.writer.(*os.File); ok {
			//nolint:errcheck // Sync errors on stdout/stderr are not critical
			f.Sync()
		}
	}

	if err := scanner.Err(); err != nil {
		return fmt.Errorf("error reading stream: %w", err)
	}

	return nil
}

// StreamMessage represents a message in the stream-json format.
type StreamMessage struct {
	Type          string          `json:"type"`
	ToolUseResult json.RawMessage `json:"tool_use_result,omitempty"`
	Message       struct {
		Content []ContentBlock `json:"content"`
	} `json:"message"`
}

// ContentBlock represents a content block in a message.
type ContentBlock struct {
	Type      string         `json:"type"`
	Text      string         `json:"text,omitempty"`
	ID        string         `json:"id,omitempty"`
	Name      string         `json:"name,omitempty"`
	Input     map[string]any `json:"input,omitempty"`
	Content   string         `json:"content,omitempty"`
	IsError   bool           `json:"is_error,omitempty"`
	ToolUseID string         `json:"tool_use_id,omitempty"`
}

func formatToolUse(content ContentBlock) string {
	if content.Name == "TodoWrite" {
		return formatTodoToolUse(content)
	}

	var parts []string
	parts = append(parts, content.Name)

	// Extract key parameters to show
	if filePath, ok := content.Input["file_path"].(string); ok {
		parts = append(parts, fmt.Sprintf("file_path=%s", filePath))
	}
	if pattern, ok := content.Input["pattern"].(string); ok {
		parts = append(parts, fmt.Sprintf("pattern=%s", pattern))
	}
	if command, ok := content.Input["command"].(string); ok {
		// Show abbreviated command if it's long
		if len(command) > 50 {
			command = command[:47] + "..."
		}
		parts = append(parts, fmt.Sprintf("command=%s", command))
	}

	return ui.Tool(strings.Join(parts, " ")) + "\n"
}

type TodoItem struct {
	Content string `json:"content"`
	Status  string `json:"status"`
	//nolint:tagliatelle // Claude stream-json uses camelCase field names.
	ActiveForm string `json:"activeForm,omitempty"`
}

type TodoToolUseResult struct {
	//nolint:tagliatelle // Claude stream-json uses camelCase field names.
	OldTodos []TodoItem `json:"oldTodos"`
	//nolint:tagliatelle // Claude stream-json uses camelCase field names.
	NewTodos []TodoItem `json:"newTodos"`
}

func formatTodoToolUse(content ContentBlock) string {
	todos := extractTodosFromInput(content.Input)
	header := "TodoWrite"
	if len(todos) > 0 {
		header = fmt.Sprintf("TodoWrite (%d todos)", len(todos))
	}

	return ui.Tool(header) + "\n"
}

func formatTodoToolResult(content ContentBlock, raw json.RawMessage) string {
	if content.IsError {
		return formatToolResult(content)
	}
	if len(raw) == 0 {
		return ""
	}

	var result TodoToolUseResult
	if err := json.Unmarshal(raw, &result); err != nil {
		return ""
	}
	if len(result.NewTodos) == 0 {
		return ""
	}

	summary := fmt.Sprintf("TodoWrite updated %d todos", len(result.NewTodos))
	if len(result.OldTodos) == 0 {
		summary = fmt.Sprintf("TodoWrite initialized %d todos", len(result.NewTodos))
	}

	var builder strings.Builder
	builder.WriteString(ui.Info(summary))
	builder.WriteString(formatTodoList(result.NewTodos))
	return builder.String()
}

func extractTodosFromInput(input map[string]any) []TodoItem {
	if len(input) == 0 {
		return nil
	}

	rawTodos, ok := input["todos"].([]any)
	if !ok {
		return nil
	}

	todos := make([]TodoItem, 0, len(rawTodos))
	for _, raw := range rawTodos {
		obj, ok := raw.(map[string]any)
		if !ok {
			continue
		}

		var todo TodoItem
		if v, ok := obj["content"].(string); ok {
			todo.Content = v
		}
		if v, ok := obj["status"].(string); ok {
			todo.Status = v
		}
		if v, ok := obj["activeForm"].(string); ok {
			todo.ActiveForm = v
		}
		if todo.Content == "" && todo.Status == "" {
			continue
		}

		todos = append(todos, todo)
	}

	return todos
}

func formatTodoList(todos []TodoItem) string {
	if len(todos) == 0 {
		return ""
	}

	resetCode := ui.ResolveStyle(ui.WeightNormal)

	var builder strings.Builder
	for _, todo := range todos {
		status := ui.StripColors(todo.Status)
		content := todo.Content
		if content == "" {
			content = "(no content)"
		}
		content = ui.StripColors(content)

		colorCode := todoStatusColor(status)
		fmt.Fprintf(&builder, "   %s%s %s%s\n", colorCode, todoStatusMarker(status), content, resetCode)
	}

	return builder.String()
}

func todoStatusMarker(status string) string {
	switch status {
	case "completed":
		return "[x]"
	case "in_progress":
		return "[~]"
	case "pending":
		return "[ ]"
	default:
		return "[-]"
	}
}

func todoStatusColor(status string) string {
	switch status {
	case "completed":
		return ui.ResolveColor(ui.ColorSuccess)
	case "in_progress":
		return ui.ResolveColor(ui.ColorWarning)
	case "pending":
		return ui.ResolveStyle(ui.WeightDim)
	default:
		return ui.ResolveStyle(ui.WeightDim)
	}
}

func formatToolResult(content ContentBlock) string {
	if content.IsError {
		return ui.FormatToolError(content.Content)
	}
	return ui.FormatToolOutput(content.Content)
}
