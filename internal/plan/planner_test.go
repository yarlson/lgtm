package plan

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/yarlson/snap/internal/model"
)

// recordingExecutor records every Run call and returns canned output. If a
// fileWriter is set, it runs after each call to simulate the LLM's Write tool
// creating artifact files on disk.
type recordingExecutor struct {
	mu             sync.Mutex
	calls          []executorCall
	cannedOutput   string
	fileWriter     func(prompt string)
	failOnContains map[string]error
}

type executorCall struct {
	modelType model.Type
	prompt    string
}

func (m *recordingExecutor) Run(_ context.Context, w io.Writer, mt model.Type, args ...string) error {
	prompt := args[len(args)-1]
	m.mu.Lock()
	m.calls = append(m.calls, executorCall{modelType: mt, prompt: prompt})
	m.mu.Unlock()

	for substr, err := range m.failOnContains {
		if strings.Contains(prompt, substr) {
			return err
		}
	}
	out := m.cannedOutput
	if out == "" {
		out = "LLM response\n"
	}
	if _, err := fmt.Fprint(w, out); err != nil {
		return err
	}
	if m.fileWriter != nil {
		m.fileWriter(prompt)
	}
	return nil
}

func (m *recordingExecutor) snapshot() []executorCall {
	m.mu.Lock()
	defer m.mu.Unlock()
	out := make([]executorCall, len(m.calls))
	copy(out, m.calls)
	return out
}

// promptsContaining returns the prompts whose text matches the substring.
func promptsContaining(calls []executorCall, substr string) []executorCall {
	var out []executorCall
	for _, c := range calls {
		if strings.Contains(c.prompt, substr) {
			out = append(out, c)
		}
	}
	return out
}

// newSessionDirs creates a temp session dir and returns the (sessionDir, tasksDir) pair.
func newSessionDirs(t *testing.T) (sessionDir, tasksDir string) {
	t.Helper()
	root := t.TempDir()
	sessionDir = filepath.Join(root, "session")
	tasksDir = filepath.Join(sessionDir, "tasks")
	require.NoError(t, os.MkdirAll(tasksDir, 0o755))
	return sessionDir, tasksDir
}

// fileWriterForArtifacts returns a func that writes empty placeholder files
// based on unambiguous markers in each prompt. Simulates the LLM's Write tool
// so artifactExists checks behave correctly.
func fileWriterForArtifacts(tasksDir, briefPath string) func(prompt string) {
	write := func(name, body string) {
		_ = os.WriteFile(filepath.Join(tasksDir, name), []byte(body), 0o600) //nolint:errcheck // best-effort test fixture
	}

	return func(prompt string) {
		switch {
		// BRIEF synthesis (prompts/brief.md).
		case strings.Contains(prompt, "Synthesize the conversation above"):
			_ = os.WriteFile(briefPath, []byte("# BRIEF\n## Problem\n(none)\n"), 0o600) //nolint:errcheck // best-effort test fixture

		// Slim TASK<N>.md (prompts/task-slim.md). Contains "exactly six sections".
		case strings.Contains(prompt, "exactly six sections"):
			for n := 1; n <= 9; n++ {
				marker := fmt.Sprintf("TASK%d.md`", n)
				if strings.Contains(prompt, marker) {
					write(fmt.Sprintf("TASK%d.md", n), "# Task")
					return
				}
			}

		// PRD writer (prompts/prd.md). "Write a PRD" only appears in this prompt.
		case strings.Contains(prompt, "Write a PRD for the work"):
			write("PRD.md", "# PRD")

		// Technology writer.
		case strings.Contains(prompt, "Map the product requirements into an engineering plan"):
			write("TECHNOLOGY.md", "# Technology")

		// Design writer.
		case strings.Contains(prompt, "Translate the product requirements into a design"):
			write("DESIGN.md", "# Design")

		// Slim TASKS.md index (prompts/tasks-md-slim.md).
		case strings.Contains(prompt, "Cap at 3 tasks"):
			write("TASKS.md", "## G. Task list\n\n| 1 | TASK1.md | outcome | BRIEF#x |\n| 2 | TASK2.md | outcome | BRIEF#y |\n")

		// Full-tier generate-tasks (prompts/generate-tasks.md). Writes TASKS.md
		// + spawns subagents — simulate by writing TASKS.md and one TASK1.md.
		case strings.Contains(prompt, "Write TASKS.md and generate individual"):
			write("TASKS.md", "## G. Task List\n\n| 1 | TASK1.md | outcome | BRIEF#x |\n")
			write("TASK1.md", "# Task 1")
		}
	}
}

// --- Phase 1 chat loop tests ---

func TestPlanner_Phase1_UserMessageThenDone(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	briefPath := filepath.Join(tasksDir, "BRIEF.md")

	exec := &recordingExecutor{fileWriter: fileWriterForArtifacts(tasksDir, briefPath)}
	var out bytes.Buffer

	p := NewPlanner(exec, "auth", tasksDir,
		WithOutput(&out),
		WithInput(strings.NewReader("I want OAuth2 auth\n/done\n")),
		WithSessionDir(sessionDir),
		WithForcedTier(TriageResult{Tier: TierTiny}),
	)

	err := p.Run(context.Background())
	require.NoError(t, err)

	calls := exec.snapshot()
	require.GreaterOrEqual(t, len(calls), 2)
	assert.NotContains(t, calls[0].prompt, "I want OAuth2")
	// User message gets sent with -c (last arg in args list).
	userMsgs := promptsContaining(calls, "I want OAuth2 auth")
	assert.Len(t, userMsgs, 1)

	output := out.String()
	assert.Contains(t, output, "Gathering requirements")
}

func TestPlanner_Phase1_DoneImmediately_TriggersBriefSynth(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	briefPath := filepath.Join(tasksDir, "BRIEF.md")

	exec := &recordingExecutor{fileWriter: fileWriterForArtifacts(tasksDir, briefPath)}
	var out bytes.Buffer

	p := NewPlanner(exec, "auth", tasksDir,
		WithOutput(&out),
		WithInput(strings.NewReader("/done\n")),
		WithSessionDir(sessionDir),
		WithForcedTier(TriageResult{Tier: TierTiny}),
	)

	require.NoError(t, p.Run(context.Background()))

	// Brief synthesis prompt fires after /done.
	briefSynthCalls := promptsContaining(exec.snapshot(), "Synthesize the conversation")
	assert.Len(t, briefSynthCalls, 1)
	assert.FileExists(t, briefPath)
}

// --- --from mode ---

func TestPlanner_FromBrief_SkipsPhase1ButTriages(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	briefPath := filepath.Join(tasksDir, "BRIEF.md")

	exec := &recordingExecutor{fileWriter: fileWriterForArtifacts(tasksDir, briefPath)}
	var out bytes.Buffer

	p := NewPlanner(exec, "auth", tasksDir,
		WithOutput(&out),
		WithBrief("brief.md", "# from-file\nuser-supplied"),
		WithSessionDir(sessionDir),
		WithForcedTier(TriageResult{Tier: TierTiny}),
	)

	require.NoError(t, p.Run(context.Background()))

	// BRIEF.md was written from --from content (no Phase 1 chat, no synth call).
	body, err := os.ReadFile(briefPath)
	require.NoError(t, err)
	assert.Contains(t, string(body), "user-supplied")

	// No requirements prompt was sent.
	assert.Empty(t, promptsContaining(exec.snapshot(), "Gather requirements"))
	assert.Empty(t, promptsContaining(exec.snapshot(), "Synthesize the conversation"))
}

// --- Tier dispatcher ---

func TestPlanner_Tiny_OnlyTASK1(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	briefPath := filepath.Join(tasksDir, "BRIEF.md")
	exec := &recordingExecutor{fileWriter: fileWriterForArtifacts(tasksDir, briefPath)}
	var out bytes.Buffer

	p := NewPlanner(exec, "s", tasksDir,
		WithOutput(&out),
		WithBrief("brief.md", "tiny"),
		WithSessionDir(sessionDir),
		WithForcedTier(TriageResult{Tier: TierTiny}),
	)
	require.NoError(t, p.Run(context.Background()))

	// Exactly one writer + one critic for TASK1.md.
	calls := exec.snapshot()
	taskWriters := promptsContaining(calls, "exactly six sections")
	assert.Len(t, taskWriters, 1)
	criticCalls := promptsContaining(calls, "strict reviewer")
	assert.Len(t, criticCalls, 1)
	assert.FileExists(t, filepath.Join(tasksDir, "TASK1.md"))
	assert.NoFileExists(t, filepath.Join(tasksDir, "PRD.md"))
	assert.NoFileExists(t, filepath.Join(tasksDir, "TECHNOLOGY.md"))
	assert.NoFileExists(t, filepath.Join(tasksDir, "DESIGN.md"))
}

func TestPlanner_Small_PRDPlusTasksMdPlusSlimTasks(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	briefPath := filepath.Join(tasksDir, "BRIEF.md")
	exec := &recordingExecutor{fileWriter: fileWriterForArtifacts(tasksDir, briefPath)}
	var out bytes.Buffer

	p := NewPlanner(exec, "s", tasksDir,
		WithOutput(&out),
		WithBrief("brief.md", "small"),
		WithSessionDir(sessionDir),
		WithForcedTier(TriageResult{Tier: TierSmall}),
	)
	require.NoError(t, p.Run(context.Background()))

	calls := exec.snapshot()
	// PRD writer fires exactly once.
	assert.Len(t, promptsContaining(calls, "Write a PRD for the work"), 1)
	// Slim TASKS.md writer fires exactly once.
	assert.Len(t, promptsContaining(calls, "Cap at 3 tasks"), 1)
	// PRD + each TASK gets a critic. TASKS.md does not.
	assert.GreaterOrEqual(t, len(promptsContaining(calls, "strict reviewer")), 3)
	assert.FileExists(t, filepath.Join(tasksDir, "PRD.md"))
	assert.FileExists(t, filepath.Join(tasksDir, "TASKS.md"))
	assert.FileExists(t, filepath.Join(tasksDir, "TASK1.md"))
	assert.FileExists(t, filepath.Join(tasksDir, "TASK2.md"))
}

func TestPlanner_Full_NoFlags_SkipsTechAndDesign(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	briefPath := filepath.Join(tasksDir, "BRIEF.md")
	exec := &recordingExecutor{fileWriter: fileWriterForArtifacts(tasksDir, briefPath)}
	var out bytes.Buffer

	p := NewPlanner(exec, "s", tasksDir,
		WithOutput(&out),
		WithBrief("brief.md", "full"),
		WithSessionDir(sessionDir),
		WithForcedTier(TriageResult{Tier: TierFull, HasArchitecture: false, HasUI: false}),
	)
	require.NoError(t, p.Run(context.Background()))

	assert.FileExists(t, filepath.Join(tasksDir, "PRD.md"))
	assert.NoFileExists(t, filepath.Join(tasksDir, "TECHNOLOGY.md"))
	assert.NoFileExists(t, filepath.Join(tasksDir, "DESIGN.md"))
	assert.FileExists(t, filepath.Join(tasksDir, "TASKS.md"))
}

func TestPlanner_Full_AllFlags_GeneratesEverything(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	briefPath := filepath.Join(tasksDir, "BRIEF.md")
	exec := &recordingExecutor{fileWriter: fileWriterForArtifacts(tasksDir, briefPath)}
	var out bytes.Buffer

	p := NewPlanner(exec, "s", tasksDir,
		WithOutput(&out),
		WithBrief("brief.md", "full"),
		WithSessionDir(sessionDir),
		WithForcedTier(TriageResult{Tier: TierFull, HasArchitecture: true, HasUI: true}),
	)
	require.NoError(t, p.Run(context.Background()))

	for _, name := range []string{"PRD.md", "TECHNOLOGY.md", "DESIGN.md", "TASKS.md"} {
		assert.FileExists(t, filepath.Join(tasksDir, name))
	}
}

// --- Resume ---

func TestPlanner_Resume_LoadsTierMarker(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	briefPath := filepath.Join(tasksDir, "BRIEF.md")

	// Pre-existing brief and tier marker.
	require.NoError(t, os.WriteFile(briefPath, []byte("# brief"), 0o600))
	require.NoError(t, os.WriteFile(filepath.Join(sessionDir, planTierMarker), []byte("tiny|false|false\n"), 0o600))

	exec := &recordingExecutor{fileWriter: fileWriterForArtifacts(tasksDir, briefPath)}
	var out bytes.Buffer

	p := NewPlanner(exec, "s", tasksDir,
		WithOutput(&out),
		WithSessionDir(sessionDir),
		WithResume(true),
	)
	require.NoError(t, p.Run(context.Background()))

	// Tier was tiny: only TASK1.md should appear (no triage call to executor).
	assert.FileExists(t, filepath.Join(tasksDir, "TASK1.md"))
	// No triage-classifier prompt was sent (resume short-circuits it).
	assert.Empty(t, promptsContaining(exec.snapshot(), "Classify the work"))
}

func TestPlanner_Resume_SkipsExistingArtifacts(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	briefPath := filepath.Join(tasksDir, "BRIEF.md")
	require.NoError(t, os.WriteFile(briefPath, []byte("# brief"), 0o600))
	require.NoError(t, os.WriteFile(filepath.Join(sessionDir, planTierMarker), []byte("tiny|false|false\n"), 0o600))
	// TASK1.md already on disk.
	require.NoError(t, os.WriteFile(filepath.Join(tasksDir, "TASK1.md"), []byte("# existing"), 0o600))

	exec := &recordingExecutor{fileWriter: fileWriterForArtifacts(tasksDir, briefPath)}
	var out bytes.Buffer

	p := NewPlanner(exec, "s", tasksDir,
		WithOutput(&out),
		WithSessionDir(sessionDir),
		WithResume(true),
	)
	require.NoError(t, p.Run(context.Background()))

	// No writer call — only critic on existing file.
	calls := exec.snapshot()
	assert.Empty(t, promptsContaining(calls, "exactly six sections"), "writer must be skipped when artifact exists")
	assert.Len(t, promptsContaining(calls, "strict reviewer"), 1, "critic still runs on existing file")
}

// --- Tier marker writes ---

func TestPlanner_WritesTierMarkerAfterTriage(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	briefPath := filepath.Join(tasksDir, "BRIEF.md")
	exec := &recordingExecutor{fileWriter: fileWriterForArtifacts(tasksDir, briefPath)}
	var out bytes.Buffer

	p := NewPlanner(exec, "s", tasksDir,
		WithOutput(&out),
		WithBrief("brief.md", "x"),
		WithSessionDir(sessionDir),
		WithForcedTier(TriageResult{Tier: TierSmall, HasArchitecture: true, HasUI: false}),
	)
	require.NoError(t, p.Run(context.Background()))

	data, err := os.ReadFile(filepath.Join(sessionDir, planTierMarker))
	require.NoError(t, err)
	assert.Equal(t, "small|true|false\n", string(data))
}

// --- TIER_MISMATCH detection ---

func TestPlanner_Small_TierMismatchAborts(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	briefPath := filepath.Join(tasksDir, "BRIEF.md")
	mismatchWriter := func(prompt string) {
		if strings.Contains(prompt, "Synthesize the conversation") {
			_ = os.WriteFile(briefPath, []byte("# brief"), 0o600) //nolint:errcheck // best-effort test fixture
			return
		}
		if strings.Contains(prompt, "PRD.md`") && strings.Contains(prompt, "Write a PRD") {
			_ = os.WriteFile(filepath.Join(tasksDir, "PRD.md"), []byte("# PRD"), 0o600) //nolint:errcheck // best-effort test fixture
			return
		}
		if strings.Contains(prompt, "section G") {
			// Slim TASKS.md emits the TIER_MISMATCH escape hatch.
			_ = os.WriteFile(filepath.Join(tasksDir, "TASKS.md"), //nolint:errcheck // best-effort test fixture
				[]byte("TIER_MISMATCH: this work needs the full tier.\n"), 0o600)
		}
	}

	exec := &recordingExecutor{fileWriter: mismatchWriter}
	var out bytes.Buffer

	p := NewPlanner(exec, "s", tasksDir,
		WithOutput(&out),
		WithBrief("brief.md", "x"),
		WithSessionDir(sessionDir),
		WithForcedTier(TriageResult{Tier: TierSmall}),
	)
	err := p.Run(context.Background())
	require.Error(t, err)
	assert.Contains(t, err.Error(), "tier mismatch")
}

// --- Phase 1 cancellation ---

func TestPlanner_Phase1_ContextCancel(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	exec := &recordingExecutor{}
	var out bytes.Buffer

	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	p := NewPlanner(exec, "s", tasksDir,
		WithOutput(&out),
		WithInput(strings.NewReader("/done\n")),
		WithSessionDir(sessionDir),
		WithForcedTier(TriageResult{Tier: TierTiny}),
	)
	err := p.Run(ctx)
	require.Error(t, err)
	assert.Contains(t, out.String(), "Planning aborted")
}

// --- onFirstMessage callback ---

func TestPlanner_OnFirstMessage_FiresOnceAfterFirstCall(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	briefPath := filepath.Join(tasksDir, "BRIEF.md")
	exec := &recordingExecutor{fileWriter: fileWriterForArtifacts(tasksDir, briefPath)}
	var out bytes.Buffer
	calls := 0

	p := NewPlanner(exec, "s", tasksDir,
		WithOutput(&out),
		WithInput(strings.NewReader("/done\n")),
		WithSessionDir(sessionDir),
		WithForcedTier(TriageResult{Tier: TierTiny}),
		WithAfterFirstMessage(func() error {
			calls++
			return nil
		}),
	)
	require.NoError(t, p.Run(context.Background()))
	assert.Equal(t, 1, calls, "afterFirstMessage must fire exactly once")
}

func TestPlanner_OnFirstMessage_PropagatesError(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	briefPath := filepath.Join(tasksDir, "BRIEF.md")
	exec := &recordingExecutor{fileWriter: fileWriterForArtifacts(tasksDir, briefPath)}
	var out bytes.Buffer

	p := NewPlanner(exec, "s", tasksDir,
		WithOutput(&out),
		WithInput(strings.NewReader("/done\n")),
		WithSessionDir(sessionDir),
		WithForcedTier(TriageResult{Tier: TierTiny}),
		WithAfterFirstMessage(func() error { return errors.New("marker write failed") }),
	)
	err := p.Run(context.Background())
	require.Error(t, err)
	assert.Contains(t, err.Error(), "marker write failed")
}

// --- Verify-or-fail when LLM claims success but doesn't write ---

func TestPlanner_BriefSynth_FailsWhenFileNotWritten(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)

	// Executor reports success but never writes BRIEF.md (simulates the agent
	// asking a clarifying question instead of using the Write tool).
	exec := &recordingExecutor{}
	var out bytes.Buffer

	p := NewPlanner(exec, "auth", tasksDir,
		WithOutput(&out),
		WithInput(strings.NewReader("/done\n")),
		WithSessionDir(sessionDir),
		WithForcedTier(TriageResult{Tier: TierTiny}),
	)

	err := p.Run(context.Background())
	require.Error(t, err)
	assert.Contains(t, err.Error(), "BRIEF.md")
	assert.Contains(t, err.Error(), "not written")
}

func TestPlanner_TinyWriter_FailsWhenTaskFileNotWritten(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	briefPath := filepath.Join(tasksDir, "BRIEF.md")
	require.NoError(t, os.WriteFile(briefPath, []byte("# brief"), 0o600))

	// fileWriter writes BRIEF on synthesis call but ignores the slim TASK call.
	briefOnlyWriter := func(prompt string) {
		if strings.Contains(prompt, "Synthesize the conversation above") {
			_ = os.WriteFile(briefPath, []byte("# brief"), 0o600) //nolint:errcheck // test fixture
		}
	}
	exec := &recordingExecutor{fileWriter: briefOnlyWriter}
	var out bytes.Buffer

	p := NewPlanner(exec, "auth", tasksDir,
		WithOutput(&out),
		WithBrief("brief.md", "tiny brief"),
		WithSessionDir(sessionDir),
		WithForcedTier(TriageResult{Tier: TierTiny}),
	)

	err := p.Run(context.Background())
	require.Error(t, err)
	assert.Contains(t, err.Error(), "TASK1.md")
	assert.Contains(t, err.Error(), "not written")
}

// --- Critic non-fatal ---

func TestPlanner_CriticFailureIsNonFatal(t *testing.T) {
	sessionDir, tasksDir := newSessionDirs(t)
	briefPath := filepath.Join(tasksDir, "BRIEF.md")
	exec := &recordingExecutor{
		fileWriter:     fileWriterForArtifacts(tasksDir, briefPath),
		failOnContains: map[string]error{"strict reviewer": errors.New("haiku unavailable")},
	}
	var out bytes.Buffer

	p := NewPlanner(exec, "s", tasksDir,
		WithOutput(&out),
		WithBrief("brief.md", "x"),
		WithSessionDir(sessionDir),
		WithForcedTier(TriageResult{Tier: TierTiny}),
	)
	require.NoError(t, p.Run(context.Background()), "critic failure must not abort planning")
	assert.FileExists(t, filepath.Join(tasksDir, "TASK1.md"))
	assert.Contains(t, out.String(), "critic skipped")
}

// --- Helper-level tests ---

func TestCountTasksInTasksMd(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "TASKS.md")
	require.NoError(t, os.WriteFile(path, []byte(`
## G. Task list

| # | File     | Outcome | Grounded |
| - | -------- | ------- | -------- |
| 1 | TASK1.md | a       | x        |
| 2 | TASK2.md | b       | y        |
| 3 | TASK3.md | c       | z        |
`), 0o600))
	assert.Equal(t, 3, countTasksInTasksMd(path))
}

func TestListTaskFiles(t *testing.T) {
	dir := t.TempDir()
	for _, name := range []string{"TASK1.md", "TASK2.md", "TASKS.md", "PRD.md", "scratch.txt"} {
		require.NoError(t, os.WriteFile(filepath.Join(dir, name), []byte(""), 0o600))
	}
	got := listTaskFiles(dir)
	assert.ElementsMatch(t, []string{"TASK1.md", "TASK2.md"}, got)
}
