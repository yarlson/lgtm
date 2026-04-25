package ui

import (
	"fmt"
	"io"
	"os"
	"strings"

	"golang.org/x/term"
)

const (
	// ToolOutputMaxLines is the visible height of the virtual tool-output buffer.
	ToolOutputMaxLines = 10
)

// LineViewport keeps only the most recent lines from a streamed text source.
// It preserves a trailing partial line so callers can feed chunks incrementally.
type LineViewport struct {
	maxLines   int
	lines      []string
	totalLines int
	pending    string
}

// StreamViewport renders a plain scrolling viewport for streamed tool output.
type StreamViewport struct {
	writer        io.Writer
	viewport      *LineViewport
	renderedLines int
	isTTY         bool
}

// NewLineViewport creates a viewport with a fixed visible line capacity.
func NewLineViewport(maxLines int) *LineViewport {
	if maxLines <= 0 {
		maxLines = 1
	}

	return &LineViewport{
		maxLines: maxLines,
		lines:    make([]string, 0, maxLines),
	}
}

// NewStreamViewport creates a streaming viewport for tool output.
func NewStreamViewport(w io.Writer, maxLines int) *StreamViewport {
	isTTY := false
	if f, ok := w.(*os.File); ok {
		isTTY = term.IsTerminal(int(f.Fd()))
	}

	return &StreamViewport{
		writer:   w,
		viewport: NewLineViewport(maxLines),
		isTTY:    isTTY,
	}
}

// Append adds streamed text to the viewport and retains only the newest lines.
func (v *LineViewport) Append(text string) {
	if text == "" {
		return
	}

	text = StripColors(strings.ReplaceAll(text, "\r\n", "\n"))
	chunks := strings.Split(v.pending+text, "\n")

	for _, line := range chunks[:len(chunks)-1] {
		v.push(line)
	}

	v.pending = chunks[len(chunks)-1]
}

// VisibleLines returns the currently visible lines, including a trailing partial line.
func (v *LineViewport) VisibleLines() []string {
	lines := append([]string(nil), v.lines...)
	if v.pending != "" {
		lines = append(lines, v.pending)
	}
	if len(lines) <= v.maxLines {
		return lines
	}
	return append([]string(nil), lines[len(lines)-v.maxLines:]...)
}

// TotalLines returns the total number of logical lines observed so far.
func (v *LineViewport) TotalLines() int {
	if v.pending != "" {
		return v.totalLines + 1
	}
	return v.totalLines
}

// Overflowed reports whether the viewport had to discard older lines.
func (v *LineViewport) Overflowed() bool {
	return v.TotalLines() > v.maxLines
}

// Append adds streamed text and updates the live viewport when attached to a TTY.
func (v *StreamViewport) Append(text string, isError bool) error {
	if text == "" {
		return nil
	}

	v.viewport.Append(text)
	if !v.isTTY {
		return nil
	}

	return v.render(isError)
}

// FinalText returns the currently visible text joined with newlines.
func (v *StreamViewport) FinalText() string {
	return strings.Join(v.viewport.VisibleLines(), "\n")
}

// HasOutput reports whether the viewport currently contains visible content.
func (v *StreamViewport) HasOutput() bool {
	return len(v.viewport.VisibleLines()) > 0
}

// IsTTY reports whether this viewport is attached to a terminal.
func (v *StreamViewport) IsTTY() bool {
	return v.isTTY
}

// Reset clears the viewport state.
func (v *StreamViewport) Reset() {
	v.viewport = NewLineViewport(v.viewport.maxLines)
	v.renderedLines = 0
}

// FormatToolOutput formats tool output and trims it to the visible viewport.
func FormatToolOutput(text string) string {
	return formatViewportOutput(text, false)
}

// FormatToolError formats failed tool output with the same viewport behavior.
func FormatToolError(text string) string {
	return formatViewportOutput(text, true)
}

func (v *LineViewport) push(line string) {
	v.totalLines++
	v.lines = append(v.lines, line)
	if len(v.lines) > v.maxLines {
		v.lines = v.lines[len(v.lines)-v.maxLines:]
	}
}

func formatViewportOutput(text string, isError bool) string {
	text = StripColors(strings.ReplaceAll(text, "\r\n", "\n"))
	text = strings.TrimSuffix(text, "\n")
	if text == "" {
		return ""
	}

	viewport := NewLineViewport(ToolOutputMaxLines)
	viewport.Append(text)
	if isError {
		return renderToolOutputLines(viewport.VisibleLines(), true)
	}

	return renderToolOutputLines(viewport.VisibleLines(), false)
}

func (v *StreamViewport) render(isError bool) error {
	lines := v.viewport.VisibleLines()
	clearCount := v.renderedLines
	if len(lines) > clearCount {
		clearCount = len(lines)
	}

	if v.renderedLines > 0 {
		if _, err := fmt.Fprintf(v.writer, "\x1b[%dA", v.renderedLines); err != nil {
			return err
		}
	}

	for i := 0; i < clearCount; i++ {
		if _, err := io.WriteString(v.writer, "\r\x1b[2K"); err != nil {
			return err
		}
		if i < len(lines) {
			if _, err := io.WriteString(v.writer, styledToolOutputLine(lines[i], isError)); err != nil {
				return err
			}
		}
		if i < clearCount-1 {
			if _, err := io.WriteString(v.writer, "\n"); err != nil {
				return err
			}
		}
	}

	if clearCount > 0 {
		if _, err := io.WriteString(v.writer, "\n"); err != nil {
			return err
		}
	}

	v.renderedLines = len(lines)
	return nil
}

func renderToolOutputLines(lines []string, isError bool) string {
	if len(lines) == 0 {
		return ""
	}

	var builder strings.Builder
	for i, line := range lines {
		builder.WriteString(styledToolOutputLine(line, isError))
		if i < len(lines)-1 {
			builder.WriteByte('\n')
		}
	}
	builder.WriteByte('\n')

	return builder.String()
}

func styledToolOutputLine(line string, isError bool) string {
	sanitized := StripColors(line)
	resetCode := ResolveStyle(WeightNormal)

	if isError {
		return ResolveStyle(WeightDim) + ResolveColor(ColorError) + sanitized + resetCode
	}

	return ResolveStyle(WeightDim) + sanitized + resetCode
}
