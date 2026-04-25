package cmd

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	planpkg "github.com/yarlson/snap/internal/plan"
)

// mockPlanProvider creates a mock claude script that accepts any args and exits 0.
// It outputs a minimal stream-json line so the parser has something to process,
// and — critically — fakes the LLM's Write tool by creating the artifact files
// the planner expects after each step. The planner verifies file existence
// after every writer call, so the mock must actually produce them on disk.
//
// MOCK_TASKS_DIR env var, when set, points at the session's tasks directory.
// The script inspects the prompt (last argv) to decide which artifact to write.
func mockPlanProvider(t *testing.T) string {
	t.Helper()
	mockBinDir := t.TempDir()
	script := `#!/bin/sh
PROMPT="${@: -1}"

if [ -n "$MOCK_TASKS_DIR" ]; then
  mkdir -p "$MOCK_TASKS_DIR"

  case "$PROMPT" in
    *"Synthesize the conversation above"*)
      cat > "$MOCK_TASKS_DIR/BRIEF.md" << 'BRIEFEOF'
## Problem
(none)
## Users
(none)
## In scope
(none)
## Non-goals
(none)
## Success criteria
(none)
## Constraints
(none)
## Open questions
(none)
BRIEFEOF
      ;;
    *"Write a PRD for the work"*)
      printf '# PRD\n' > "$MOCK_TASKS_DIR/PRD.md"
      ;;
    *"Map the product requirements into an engineering plan"*)
      printf '# TECHNOLOGY\n' > "$MOCK_TASKS_DIR/TECHNOLOGY.md"
      ;;
    *"Translate the product requirements into a design"*)
      printf '# DESIGN\n' > "$MOCK_TASKS_DIR/DESIGN.md"
      ;;
    *"exactly six sections"*)
      # Slim TASK<N>.md prompt — extract N from the prompt.
      N=$(printf '%s' "$PROMPT" | grep -oE 'TASK[0-9]+\.md' | head -1 | grep -oE '[0-9]+')
      [ -z "$N" ] && N=1
      printf '# TASK '"$N"'\n' > "$MOCK_TASKS_DIR/TASK${N}.md"
      ;;
    *"Cap at 3 tasks"*)
      cat > "$MOCK_TASKS_DIR/TASKS.md" << 'TASKSEOF'
## G. Task list

| # | File | Outcome | Grounded in |
| - | ---- | ------- | ----------- |
| 1 | TASK1.md | outcome | BRIEF.md#x; src/x |
TASKSEOF
      ;;
    *"Write TASKS.md and generate individual"*)
      cat > "$MOCK_TASKS_DIR/TASKS.md" << 'TASKSEOF'
# TASKS

## G. Task List

| # | File | Name | Epic | Outcome | Risk | Size |
|---|------|------|------|---------|------|------|
| 0 | TASK0.md | Task zero | E1 | Works | Low | S |
| 1 | TASK1.md | Task one | E2 | Works | Low | S |

## H. Dependencies
TASKSEOF
      ;;
  esac
fi

echo '{"type":"assistant","message":{"content":[{"type":"text","text":"OK"}]}}'
exit 0
`
	mockClaude := filepath.Join(mockBinDir, "claude")
	require.NoError(t, os.WriteFile(mockClaude, []byte(script), 0o755)) //nolint:gosec // G306: test mock
	return mockBinDir + ":/usr/bin:/bin"
}

// CUJ-1: Fresh-start planning — snap plan on a project with no sessions.
func TestE2E_PlanFreshProject(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping E2E test in short mode")
	}

	binPath := buildSnap(t)
	projectDir := t.TempDir()
	ctx := context.Background()

	mockPath := mockPlanProvider(t)

	// The auto-created "default" session will have tasks at this path.
	tasksDir := filepath.Join(projectDir, ".snap", "sessions", "default", "tasks")

	// Run snap plan on a fresh project with no sessions — should auto-create "default".
	plan := exec.CommandContext(ctx, binPath, "plan")
	plan.Dir = projectDir
	plan.Env = append(os.Environ(), "PATH="+mockPath, "MOCK_TASKS_DIR="+tasksDir)
	plan.Stdin = strings.NewReader("/done\n")

	output, planErr := plan.CombinedOutput()
	require.NoError(t, planErr, "snap plan (fresh project) failed: %s", output)

	outputStr := string(output)

	// Auto-creation should be silent — no "created" message in output.
	assert.NotContains(t, outputStr, "created")

	// Planning should proceed with the "default" session.
	assert.Contains(t, outputStr, "Planning session 'default'")
	assert.Contains(t, outputStr, "Planning complete")

	// The "default" session directory should exist on disk.
	defaultSessionDir := filepath.Join(projectDir, ".snap", "sessions", "default")
	info, err := os.Stat(defaultSessionDir)
	require.NoError(t, err)
	assert.True(t, info.IsDir())

	// The tasks directory should exist.
	info, err = os.Stat(tasksDir)
	require.NoError(t, err)
	assert.True(t, info.IsDir())
}

// CUJ-2: Plan and Implement — plan portion (interactive).
func TestE2E_CUJ2_PlanInteractive(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping E2E test in short mode")
	}

	binPath := buildSnap(t)
	projectDir := t.TempDir()
	ctx := context.Background()

	// Step 1: Create session.
	create := exec.CommandContext(ctx, binPath, "new", "auth")
	create.Dir = projectDir
	out, err := create.CombinedOutput()
	require.NoError(t, err, "snap new failed: %s", out)

	tasksDir := filepath.Join(projectDir, ".snap", "sessions", "auth", "tasks")

	// Step 2: Run plan with piped input.
	mockPath := mockPlanProvider(t)

	plan := exec.CommandContext(ctx, binPath, "plan", "auth")
	plan.Dir = projectDir
	plan.Env = append(os.Environ(), "PATH="+mockPath, "MOCK_TASKS_DIR="+tasksDir)
	plan.Stdin = strings.NewReader("Add auth feature\n/done\n")

	output, planErr := plan.CombinedOutput()
	require.NoError(t, planErr, "snap plan failed: %s", output)

	outputStr := string(output)

	// New pipeline: BRIEF synthesis + triage + (full-tier writers).
	// Mock claude returns garbled output, so triage falls back to TierFull with
	// both flags → 5 writer steps (PRD + TECH + DESIGN + analyze + generate).
	assert.Contains(t, outputStr, "Triaging brief...")
	assert.Contains(t, outputStr, "Step 1/5")
	assert.Contains(t, outputStr, "Step 5/5")

	// Assert planning complete message.
	assert.Contains(t, outputStr, "Planning complete")

	// Assert .plan-started marker was written.
	markerPath := filepath.Join(projectDir, ".snap", "sessions", "auth", ".plan-started")
	_, err = os.Stat(markerPath)
	assert.NoError(t, err, ".plan-started marker should exist")

	// Assert .plan-tier marker was written (triage result persisted).
	tierPath := filepath.Join(projectDir, ".snap", "sessions", "auth", ".plan-tier")
	_, err = os.Stat(tierPath)
	assert.NoError(t, err, ".plan-tier marker should exist")
}

// CUJ-2: Plan and Implement — with file (--from).
func TestE2E_CUJ2_PlanWithFile(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping E2E test in short mode")
	}

	binPath := buildSnap(t)
	projectDir := t.TempDir()
	ctx := context.Background()

	// Step 1: Create session.
	create := exec.CommandContext(ctx, binPath, "new", "auth")
	create.Dir = projectDir
	out, err := create.CombinedOutput()
	require.NoError(t, err, "snap new failed: %s", out)

	tasksDir := filepath.Join(projectDir, ".snap", "sessions", "auth", "tasks")

	// Step 2: Create brief.md.
	briefPath := filepath.Join(projectDir, "brief.md")
	require.NoError(t, os.WriteFile(briefPath, []byte("I want OAuth2 authentication"), 0o600))

	// Step 3: Run plan with --from (no stdin needed).
	mockPath := mockPlanProvider(t)

	plan := exec.CommandContext(ctx, binPath, "plan", "auth", "--from", "brief.md")
	plan.Dir = projectDir
	plan.Env = append(os.Environ(), "PATH="+mockPath, "MOCK_TASKS_DIR="+tasksDir)

	output, planErr := plan.CombinedOutput()
	require.NoError(t, planErr, "snap plan --from failed: %s", output)

	outputStr := string(output)

	// Assert --from header.
	assert.Contains(t, outputStr, "using brief.md as input")

	// New pipeline: triage runs even with --from. Fallback is full + flags → 5 steps.
	assert.Contains(t, outputStr, "Triaging brief...")
	assert.Contains(t, outputStr, "Step 1/5")
	assert.Contains(t, outputStr, "Step 5/5")

	// Assert planning complete.
	assert.Contains(t, outputStr, "Planning complete")

	// BRIEF.md was created from --from input.
	writtenBrief := filepath.Join(projectDir, ".snap", "sessions", "auth", "tasks", "BRIEF.md")
	_, err = os.Stat(writtenBrief)
	assert.NoError(t, err, "BRIEF.md should be written from --from content")
}

// Test: snap plan with nonexistent session.
func TestE2E_PlanNonexistentSession(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping E2E test in short mode")
	}

	binPath := buildSnap(t)
	projectDir := t.TempDir()
	ctx := context.Background()

	mockPath := mockPlanProvider(t)

	plan := exec.CommandContext(ctx, binPath, "plan", "nonexistent")
	plan.Dir = projectDir
	plan.Env = append(os.Environ(), "PATH="+mockPath)

	output, planErr := plan.CombinedOutput()
	require.Error(t, planErr)

	outputStr := string(output)
	assert.Contains(t, outputStr, "not found")
	assert.Contains(t, outputStr, "snap new nonexistent")
}

// Test: snap plan --from with nonexistent file.
func TestE2E_PlanFromNonexistentFile(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping E2E test in short mode")
	}

	binPath := buildSnap(t)
	projectDir := t.TempDir()
	ctx := context.Background()

	// Create session.
	create := exec.CommandContext(ctx, binPath, "new", "auth")
	create.Dir = projectDir
	out, err := create.CombinedOutput()
	require.NoError(t, err, "snap new failed: %s", out)

	mockPath := mockPlanProvider(t)

	plan := exec.CommandContext(ctx, binPath, "plan", "auth", "--from", "nonexistent.md")
	plan.Dir = projectDir
	plan.Env = append(os.Environ(), "PATH="+mockPath)

	output, planErr := plan.CombinedOutput()
	require.Error(t, planErr)

	outputStr := string(output)
	assert.Contains(t, outputStr, "failed to read input file")
}

// Test: snap plan auto-detects single session.
func TestE2E_PlanAutoDetectSingleSession(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping E2E test in short mode")
	}

	binPath := buildSnap(t)
	projectDir := t.TempDir()
	ctx := context.Background()

	// Create one session.
	create := exec.CommandContext(ctx, binPath, "new", "auth")
	create.Dir = projectDir
	out, err := create.CombinedOutput()
	require.NoError(t, err, "snap new failed: %s", out)

	tasksDir := filepath.Join(projectDir, ".snap", "sessions", "auth", "tasks")
	mockPath := mockPlanProvider(t)

	// Run plan without session name — should auto-detect.
	plan := exec.CommandContext(ctx, binPath, "plan")
	plan.Dir = projectDir
	plan.Env = append(os.Environ(), "PATH="+mockPath, "MOCK_TASKS_DIR="+tasksDir)
	plan.Stdin = strings.NewReader("/done\n")

	output, planErr := plan.CombinedOutput()
	require.NoError(t, planErr, "snap plan (auto-detect) failed: %s", output)

	outputStr := string(output)
	assert.Contains(t, outputStr, "Planning session 'auth'")
	assert.Contains(t, outputStr, "Planning complete")
}

// Test: file listing printed after plan completion.
// This test uses an empty session (no artifacts) so the conflict guard is not triggered.
func TestE2E_PlanFileListing(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping E2E test in short mode")
	}

	binPath := buildSnap(t)
	projectDir := t.TempDir()
	ctx := context.Background()

	// Step 1: Create session (empty — no conflict guard trigger).
	create := exec.CommandContext(ctx, binPath, "new", "auth")
	create.Dir = projectDir
	out, err := create.CombinedOutput()
	require.NoError(t, err, "snap new failed: %s", out)

	tasksDir := filepath.Join(projectDir, ".snap", "sessions", "auth", "tasks")

	// Step 2: Run plan (mock provider outputs nothing, so no files generated).
	mockPath := mockPlanProvider(t)

	plan := exec.CommandContext(ctx, binPath, "plan", "auth")
	plan.Dir = projectDir
	plan.Env = append(os.Environ(), "PATH="+mockPath, "MOCK_TASKS_DIR="+tasksDir)
	plan.Stdin = strings.NewReader("/done\n")

	output, planErr := plan.CombinedOutput()
	require.NoError(t, planErr, "snap plan failed: %s", output)

	outputStr := string(output)

	// Planning should proceed without conflict prompt.
	assert.Contains(t, outputStr, "Planning session 'auth'")
	assert.Contains(t, outputStr, "Planning complete")

	// Assert run instruction is printed after plan completion.
	assert.Contains(t, outputStr, "Run: snap run auth")
}

// Test: snap plan on non-empty session with piped input returns conflict error.
func TestE2E_PlanConflictNonTTY(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping E2E test in short mode")
	}

	binPath := buildSnap(t)
	projectDir := t.TempDir()
	ctx := context.Background()

	// Step 1: Create session.
	create := exec.CommandContext(ctx, binPath, "new", "auth")
	create.Dir = projectDir
	out, err := create.CombinedOutput()
	require.NoError(t, err, "snap new failed: %s", out)

	// Step 2: Place a task file to trigger conflict guard.
	tasksDir := filepath.Join(projectDir, ".snap", "sessions", "auth", "tasks")
	require.NoError(t, os.WriteFile(filepath.Join(tasksDir, "TASK1.md"), []byte("# Task 1\n"), 0o600))

	// Step 3: Run plan with piped input (non-TTY).
	mockPath := mockPlanProvider(t)

	plan := exec.CommandContext(ctx, binPath, "plan", "auth")
	plan.Dir = projectDir
	plan.Env = append(os.Environ(), "PATH="+mockPath)
	plan.Stdin = strings.NewReader("/done\n")

	output, planErr := plan.CombinedOutput()
	require.Error(t, planErr, "snap plan should fail with conflict error")

	outputStr := string(output)
	assert.Contains(t, outputStr, "already has planning artifacts")
	assert.Contains(t, outputStr, "snap delete auth")
	assert.Contains(t, outputStr, "snap new")
	assert.Contains(t, outputStr, "snap plan")
}

// CUJ-1: Plan CLI Feature with UI Contract — verifies planning prompts contain UI task sections.
func TestPlanE2E_UIContract(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping E2E test in short mode")
	}

	binPath := buildSnap(t)
	projectDir := t.TempDir()
	ctx := context.Background()

	mockPath := mockPlanProvider(t)
	tasksDir := filepath.Join(projectDir, ".snap", "sessions", "default", "tasks")

	// Run snap plan — should complete without error.
	plan := exec.CommandContext(ctx, binPath, "plan")
	plan.Dir = projectDir
	plan.Env = append(os.Environ(), "PATH="+mockPath, "MOCK_TASKS_DIR="+tasksDir)
	plan.Stdin = strings.NewReader("/done\n")

	output, planErr := plan.CombinedOutput()
	require.NoError(t, planErr, "snap plan failed: %s", output)
	assert.Contains(t, string(output), "Planning complete")

	// Verify analyze-tasks prompt now grounds tasks against BRIEF + repo, not via
	// the legacy 6-anti-pattern rubric (which was stripped).
	analyzePrompt, err := renderAnalyzeTasksForTest(tasksDir)
	require.NoError(t, err)
	assert.Contains(t, analyzePrompt, "vertical slice")
	assert.Contains(t, analyzePrompt, "Grounded in:")
	assert.NotContains(t, analyzePrompt, "UI-Undefined Task")
	assert.NotContains(t, analyzePrompt, "Context Alignment Check")

	// Verify generate-tasks prompt still emits the 15-section TASK<N>.md format
	// and now demands Grounded-in footers per section.
	generatePrompt, err := renderGenerateTasksForTest(tasksDir)
	require.NoError(t, err)
	assert.Contains(t, generatePrompt, "user-facing: yes/no")
	assert.Contains(t, generatePrompt, "DESIGN.md state matrix")
	assert.Contains(t, generatePrompt, "DESIGN.md contract rules")
	assert.Contains(t, generatePrompt, "Grounded in:")
}

// renderAnalyzeTasksForTest calls the plan package's render function for E2E verification.
func renderAnalyzeTasksForTest(tasksDir string) (string, error) {
	return planpkg.RenderAnalyzeTasksPrompt(tasksDir, tasksDir+"/BRIEF.md")
}

// renderGenerateTasksForTest calls the plan package's render function for E2E verification.
func renderGenerateTasksForTest(tasksDir string) (string, error) {
	return planpkg.RenderGenerateTasksPrompt(tasksDir, tasksDir+"/BRIEF.md")
}

// Test: snap plan with multiple sessions and no name shows error.
func TestE2E_PlanMultipleSessionsError(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping E2E test in short mode")
	}

	binPath := buildSnap(t)
	projectDir := t.TempDir()
	ctx := context.Background()

	// Create two sessions.
	for _, name := range []string{"auth", "api"} {
		create := exec.CommandContext(ctx, binPath, "new", name)
		create.Dir = projectDir
		out, err := create.CombinedOutput()
		require.NoError(t, err, "snap new %s failed: %s", name, out)
	}

	mockPath := mockPlanProvider(t)

	plan := exec.CommandContext(ctx, binPath, "plan")
	plan.Dir = projectDir
	plan.Env = append(os.Environ(), "PATH="+mockPath)

	output, planErr := plan.CombinedOutput()
	require.Error(t, planErr)

	outputStr := string(output)
	assert.Contains(t, outputStr, "multiple sessions found")
	assert.Contains(t, outputStr, "auth")
	assert.Contains(t, outputStr, "api")
	assert.Contains(t, outputStr, "snap plan <name>")
}
