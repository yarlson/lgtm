package codex

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

const (
	contextFlag       = "-c"
	maxScannerBufSize = 10 * 1024 * 1024
)

// Executor runs the codex CLI and streams parsed output.
type Executor struct{}

// NewExecutor creates a new codex CLI executor.
func NewExecutor() *Executor {
	return &Executor{}
}

// ProviderName returns the provider identifier.
func (e *Executor) ProviderName() string {
	return "codex"
}

// resolveModel maps an abstract model type to a Codex-specific model name.
func resolveModel(mt model.Type) string {
	switch mt {
	case model.Fast:
		return "gpt-5.3-codex-spark"
	case model.Thinking:
		return "gpt-5.3-codex"
	default:
		return ""
	}
}

// Run executes codex with JSONL output and streams parsed output in real-time.
// The model parameter is resolved to a Codex-specific model name and passed via --model flag.
func (e *Executor) Run(ctx context.Context, w io.Writer, mt model.Type, args ...string) error {
	cmdArgs := BuildCommandArgs(args...)
	if resolved := resolveModel(mt); resolved != "" {
		cmdArgs = append(cmdArgs, "--model", resolved)
	}
	cmd := exec.CommandContext(ctx, "codex", cmdArgs...)

	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return fmt.Errorf("failed to create stdout pipe: %w", err)
	}

	stderr, err := cmd.StderrPipe()
	if err != nil {
		return fmt.Errorf("failed to create stderr pipe: %w", err)
	}

	if err := cmd.Start(); err != nil {
		return fmt.Errorf("failed to start codex command: %w", err)
	}

	parser := NewEventParser(w)
	parseErr := parser.Parse(stdout)

	stderrBytes, readErr := io.ReadAll(stderr)
	if readErr != nil {
		stderrBytes = nil
	}

	if err := cmd.Wait(); err != nil {
		if len(stderrBytes) > 0 {
			return fmt.Errorf("codex command failed: %w (stderr: %s)", err, strings.TrimSpace(string(stderrBytes)))
		}
		return fmt.Errorf("codex command failed: %w", err)
	}

	return parseErr
}

// BuildCommandArgs converts workflow args into codex CLI arguments.
// The "-c" flag means "continue context" and is mapped to `exec resume --last`.
func BuildCommandArgs(args ...string) []string {
	resume := false
	passthrough := make([]string, 0, len(args))
	for _, arg := range args {
		if arg == contextFlag {
			resume = true
			continue
		}
		passthrough = append(passthrough, arg)
	}

	base := make([]string, 0, 3+len(passthrough))
	base = append(base, "exec", "--json", "--dangerously-bypass-approvals-and-sandbox")
	if resume {
		base = make([]string, 0, 5+len(passthrough))
		base = append(base, "exec", "resume", "--last", "--json", "--dangerously-bypass-approvals-and-sandbox")
	}

	return append(base, passthrough...)
}

// EventParser parses codex JSONL events and writes formatted output.
type EventParser struct {
	writer           io.Writer
	markdownRenderer *ui.MarkdownRenderer
}

// NewEventParser creates a parser that writes to w.
func NewEventParser(w io.Writer) *EventParser {
	return &EventParser{
		writer:           w,
		markdownRenderer: ui.NewMarkdownRenderer(),
	}
}

// Parse reads codex JSONL events and writes formatted output in real-time.
func (p *EventParser) Parse(r io.Reader) error {
	scanner := bufio.NewScanner(r)
	buf := make([]byte, maxScannerBufSize)
	scanner.Buffer(buf, maxScannerBufSize)

	for scanner.Scan() {
		line := scanner.Text()
		if line == "" {
			continue
		}

		var event streamEvent
		if err := json.Unmarshal([]byte(line), &event); err != nil {
			continue
		}

		switch event.Type {
		case "item.started":
			if err := p.handleItemStarted(event.Item); err != nil {
				return err
			}
		case "item.completed":
			if err := p.handleItemCompleted(event.Item); err != nil {
				return err
			}
		}

		if f, ok := p.writer.(*os.File); ok {
			//nolint:errcheck // Best-effort flush for streaming UX.
			f.Sync()
		}
	}

	if err := scanner.Err(); err != nil {
		return fmt.Errorf("error reading codex stream: %w", err)
	}

	return nil
}

func (p *EventParser) handleItemStarted(item streamItem) error {
	if item.Type != "command_execution" {
		return nil
	}
	if out := formatCommandStart(item.Command); out != "" {
		if _, err := fmt.Fprint(p.writer, out); err != nil {
			return err
		}
	}
	return nil
}

func (p *EventParser) handleItemCompleted(item streamItem) error {
	switch item.Type {
	case "agent_message":
		if item.Text == "" {
			return nil
		}
		rendered, err := p.markdownRenderer.Render(item.Text)
		if err != nil {
			rendered = item.Text
		}
		if _, err := fmt.Fprint(p.writer, rendered); err != nil {
			return err
		}
	case "command_execution":
		if out := formatCommandResult(item); out != "" {
			if _, err := fmt.Fprint(p.writer, out); err != nil {
				return err
			}
		}
	}
	return nil
}

func formatCommandStart(command string) string {
	if command == "" {
		return ""
	}
	const maxLen = 80
	trimmed := command
	if len(trimmed) > maxLen {
		trimmed = trimmed[:maxLen-3] + "..."
	}
	return ui.Tool("Shell command="+trimmed) + "\n"
}

func formatCommandResult(item streamItem) string {
	output := strings.TrimSuffix(item.AggregatedOutput, "\n")
	failed := strings.EqualFold(item.Status, "failed") || (item.ExitCode != nil && *item.ExitCode != 0)

	if failed {
		if output != "" {
			return ui.FormatToolError(output)
		}
		exitCode := "unknown"
		if item.ExitCode != nil {
			exitCode = fmt.Sprintf("%d", *item.ExitCode)
		}
		return ui.DimError(fmt.Sprintf("Command failed (exit code %s)", exitCode)) + "\n"
	}

	if output == "" {
		return ""
	}

	return ui.FormatToolOutput(output)
}

type streamEvent struct {
	Type string     `json:"type"`
	Item streamItem `json:"item"`
}

type streamItem struct {
	ID               string `json:"id"`
	Type             string `json:"type"`
	Text             string `json:"text"`
	Command          string `json:"command"`
	AggregatedOutput string `json:"aggregated_output"`
	Status           string `json:"status"`
	ExitCode         *int   `json:"exit_code"`
}
