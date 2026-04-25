package ui_test

import (
	"fmt"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"

	"github.com/yarlson/snap/internal/ui"
)

func TestLineViewport_AppendKeepsLastLinesAcrossChunks(t *testing.T) {
	t.Parallel()

	viewport := ui.NewLineViewport(3)
	viewport.Append("one\ntwo\nthr")
	viewport.Append("ee\nfour\nfive")

	assert.Equal(t, []string{"three", "four", "five"}, viewport.VisibleLines())
	assert.Equal(t, 5, viewport.TotalLines())
	assert.True(t, viewport.Overflowed())
}

func TestFormatToolOutput_UsesViewportForLongOutput(t *testing.T) {
	t.Parallel()

	lines := make([]string, 12)
	for i := range lines {
		lines[i] = fmt.Sprintf("line %d", i+1)
	}

	rendered := ui.StripColors(ui.FormatToolOutput(strings.Join(lines, "\n")))

	assert.Contains(t, rendered, "Tool output (last 10/12 lines)")
	assert.NotContains(t, rendered, "│ line 1 ")
	assert.NotContains(t, rendered, "│ line 2 ")
	for _, line := range lines[2:] {
		assert.Contains(t, rendered, line)
	}
}

func TestFormatToolOutput_LeavesShortOutputUnboxed(t *testing.T) {
	t.Parallel()

	rendered := ui.StripColors(ui.FormatToolOutput("alpha\nbeta"))

	assert.Equal(t, "alpha\nbeta\n", rendered)
}
