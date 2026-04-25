package plan

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/yarlson/snap/internal/model"
)

// triageMockExecutor writes a fixed string to the writer when invoked.
type triageMockExecutor struct {
	output string
	err    error
}

func (m *triageMockExecutor) Run(_ context.Context, w io.Writer, _ model.Type, _ ...string) error {
	if m.err != nil {
		return m.err
	}
	_, err := fmt.Fprint(w, m.output)
	return err
}

func TestParseTriageOutput_Tiny(t *testing.T) {
	raw := "Some preamble.\n{\"tier\":\"tiny\",\"has_architecture\":false,\"has_ui\":false,\"rationale\":\"single flag rename\"}\n"
	res, ok := parseTriageOutput(raw)
	require.True(t, ok)
	assert.Equal(t, TierTiny, res.Tier)
	assert.False(t, res.HasArchitecture)
	assert.False(t, res.HasUI)
	assert.Equal(t, "single flag rename", res.Rationale)
}

func TestParseTriageOutput_Small(t *testing.T) {
	raw := "{\"tier\":\"small\",\"has_architecture\":false,\"has_ui\":true,\"rationale\":\"new subcommand\"}"
	res, ok := parseTriageOutput(raw)
	require.True(t, ok)
	assert.Equal(t, TierSmall, res.Tier)
}

func TestParseTriageOutput_Full(t *testing.T) {
	raw := "{\"tier\":\"full\",\"has_architecture\":true,\"has_ui\":true,\"rationale\":\"new product\"}"
	res, ok := parseTriageOutput(raw)
	require.True(t, ok)
	assert.Equal(t, TierFull, res.Tier)
	assert.True(t, res.HasArchitecture)
	assert.True(t, res.HasUI)
}

func TestParseTriageOutput_HandlesAnsiEscapes(t *testing.T) {
	// Simulate ANSI-coloured rendered markdown wrapping the JSON line.
	raw := "\x1b[36m{\"tier\":\"tiny\",\"has_architecture\":false,\"has_ui\":false,\"rationale\":\"x\"}\x1b[0m"
	res, ok := parseTriageOutput(raw)
	require.True(t, ok)
	assert.Equal(t, TierTiny, res.Tier)
}

func TestParseTriageOutput_GarbledFails(t *testing.T) {
	for _, raw := range []string{
		"",
		"no JSON here",
		"{not valid json}",
		"{\"tier\":\"unknown\"}",
		"{\"tier\":\"tiny\"", // missing close brace
	} {
		_, ok := parseTriageOutput(raw)
		assert.False(t, ok, "expected parse failure for %q", raw)
	}
}

func TestTriage_HappyPath(t *testing.T) {
	exec := &triageMockExecutor{output: `{"tier":"small","has_architecture":false,"has_ui":true,"rationale":"new feature"}`}
	var buf bytes.Buffer

	res, err := Triage(context.Background(), exec, "/tmp/BRIEF.md", &buf)
	require.NoError(t, err)
	assert.Equal(t, TierSmall, res.Tier)
	assert.True(t, res.HasUI)
	assert.False(t, res.HasArchitecture)
	assert.Equal(t, "new feature", res.Rationale)
}

func TestTriage_GarbledFallsBackToFull(t *testing.T) {
	exec := &triageMockExecutor{output: "I cannot do this task today."}
	var buf bytes.Buffer

	res, err := Triage(context.Background(), exec, "/tmp/BRIEF.md", &buf)
	require.NoError(t, err)
	assert.Equal(t, TierFull, res.Tier)
	assert.True(t, res.HasArchitecture)
	assert.True(t, res.HasUI)
	assert.Contains(t, buf.String(), "unparseable")
}

func TestTriage_ExecutorErrorPropagates(t *testing.T) {
	exec := &triageMockExecutor{err: errors.New("network down")}
	var buf bytes.Buffer

	_, err := Triage(context.Background(), exec, "/tmp/BRIEF.md", &buf)
	require.Error(t, err)
	assert.Contains(t, err.Error(), "network down")
}
