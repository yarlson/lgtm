package plan

import (
	"bytes"
	"context"
	"errors"
	"io"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/yarlson/snap/internal/model"
)

type criticMockExecutor struct {
	capturedPrompt string
	err            error
}

func (m *criticMockExecutor) Run(_ context.Context, _ io.Writer, _ model.Type, args ...string) error {
	if len(args) > 0 {
		m.capturedPrompt = args[0]
	}
	return m.err
}

func TestRunCritic_PassesBriefAndArtifactPaths(t *testing.T) {
	exec := &criticMockExecutor{}
	var buf bytes.Buffer

	runCritic(context.Background(), exec, &buf, "/work/BRIEF.md", "/work/PRD.md")

	assert.Contains(t, exec.capturedPrompt, "/work/BRIEF.md")
	assert.Contains(t, exec.capturedPrompt, "/work/PRD.md")
}

func TestRunCritic_NonFatalOnExecutorError(t *testing.T) {
	exec := &criticMockExecutor{err: errors.New("LLM timeout")}
	var buf bytes.Buffer

	runCritic(context.Background(), exec, &buf, "/work/BRIEF.md", "/work/PRD.md")

	assert.Contains(t, buf.String(), "critic skipped")
	assert.Contains(t, buf.String(), "PRD.md")
}

func TestBaseName(t *testing.T) {
	cases := []struct {
		in, want string
	}{
		{"/a/b/c.md", "c.md"},
		{"c.md", "c.md"},
		{"a\\b\\c.md", "c.md"},
		{"", ""},
	}
	for _, c := range cases {
		assert.Equal(t, c.want, baseName(c.in), "baseName(%q)", c.in)
	}
}

func TestRunCritic_PromptContainsRules(t *testing.T) {
	exec := &criticMockExecutor{}
	var buf bytes.Buffer

	runCritic(context.Background(), exec, &buf, "/b.md", "/a.md")
	for _, marker := range []string{"delete", "Grounded in", "Critic complete", "could", "consider"} {
		assert.Contains(t, exec.capturedPrompt, marker, "critic prompt missing %q", marker)
	}
	require.Empty(t, buf.String())
}
