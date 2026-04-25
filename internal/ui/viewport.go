package ui

import (
	"fmt"
	"strings"
	"unicode/utf8"
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

// FormatToolOutput formats tool output, boxing and trimming it when it exceeds
// the virtual viewport height.
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
	if !viewport.Overflowed() {
		if isError {
			return DimError(text) + "\n"
		}
		return Info(text)
	}

	title := fmt.Sprintf("Tool output (last %d/%d lines)", len(viewport.VisibleLines()), viewport.TotalLines())
	color := ColorTool
	if isError {
		color = ColorError
	}

	return renderViewportBox(title, viewport.VisibleLines(), color)
}

func renderViewportBox(title string, lines []string, color ColorToken) string {
	title = fitViewportTitle(title)
	colorCode := ResolveColor(color)
	borderStyle := ResolveStyle(WeightBold)
	textStyle := ResolveStyle(WeightDim)
	resetCode := ResolveStyle(WeightNormal)

	var builder strings.Builder
	builder.WriteString(boxTopBorder(title, borderStyle, colorCode, resetCode))
	for _, line := range lines {
		builder.WriteString(viewportLine(line, colorCode, textStyle, resetCode))
	}
	builder.WriteString(boxBottomBorder(borderStyle, colorCode, resetCode))
	builder.WriteByte('\n')

	return builder.String()
}

func fitViewportTitle(title string) string {
	const maxTitleRunes = BoxWidth - 8
	if utf8.RuneCountInString(title) <= maxTitleRunes {
		return title
	}
	runes := []rune(title)
	return string(runes[:maxTitleRunes-1]) + "…"
}

func viewportLine(text, borderColor, textStyle, resetCode string) string {
	text = fitText(StripColors(text))
	padding := boxContentWidth - utf8.RuneCountInString(text)
	if padding < 0 {
		padding = 0
	}

	return fmt.Sprintf("%s│ %s%s%s%s │%s\n",
		borderColor,
		textStyle,
		text,
		resetCode,
		strings.Repeat(" ", padding),
		resetCode,
	)
}
