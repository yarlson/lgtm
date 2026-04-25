package plan

import (
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

const (
	testBriefPath = ".snap/sessions/auth/tasks/BRIEF.md"
	testTasksDir  = ".snap/sessions/auth/tasks"
)

func TestRenderRequirementsPrompt(t *testing.T) {
	prompt, err := RenderRequirementsPrompt(testBriefPath)
	require.NoError(t, err)
	assert.Contains(t, prompt, "## Context")
	assert.Contains(t, prompt, "CLAUDE.md")
	assert.Contains(t, prompt, "docs/context/")
	assert.Contains(t, prompt, "## Process")
	assert.Contains(t, prompt, "/done")

	// Scope drift prevention.
	assert.Contains(t, prompt, "## Scope Lock")
	assert.Contains(t, prompt, "Do NOT suggest adjacent features")

	// Final Step writes BRIEF.md.
	assert.Contains(t, prompt, "## Final Step: Write BRIEF.md")
	assert.Contains(t, prompt, testBriefPath)
	assert.Contains(t, prompt, "Problem")
	assert.Contains(t, prompt, "Users")
	assert.Contains(t, prompt, "In scope")
	assert.Contains(t, prompt, "Non-goals")
	assert.Contains(t, prompt, "Success criteria")
	assert.Contains(t, prompt, "Constraints")
	assert.Contains(t, prompt, "Open questions")
	assert.Contains(t, prompt, "BRIEF.md written")
}

func TestRenderBriefSynthesisPrompt(t *testing.T) {
	prompt, err := RenderBriefSynthesisPrompt(testBriefPath)
	require.NoError(t, err)
	assert.Contains(t, prompt, testBriefPath)
	for _, section := range []string{"Problem", "Users", "In scope", "Non-goals", "Success criteria", "Constraints", "Open questions"} {
		assert.Contains(t, prompt, section, "expected section %q in brief synthesis prompt", section)
	}
	assert.Contains(t, prompt, "(none)")
	assert.Contains(t, prompt, "BRIEF.md written")
}

func TestRenderTriagePrompt(t *testing.T) {
	prompt, err := RenderTriagePrompt(testBriefPath)
	require.NoError(t, err)
	assert.Contains(t, prompt, testBriefPath)
	assert.Contains(t, prompt, "tiny")
	assert.Contains(t, prompt, "small")
	assert.Contains(t, prompt, "full")
	assert.Contains(t, prompt, "has_architecture")
	assert.Contains(t, prompt, "has_ui")
	assert.Contains(t, prompt, "rationale")
	assert.Contains(t, prompt, "one line of JSON")
}

func TestRenderCriticPrompt(t *testing.T) {
	artifactPath := testTasksDir + "/PRD.md"
	prompt, err := RenderCriticPrompt(testBriefPath, artifactPath)
	require.NoError(t, err)
	assert.Contains(t, prompt, testBriefPath)
	assert.Contains(t, prompt, artifactPath)
	assert.Contains(t, prompt, "delete")
	assert.Contains(t, prompt, "Grounded in")
	// Forbidden patterns must be enumerated.
	for _, forbidden := range []string{"could", "consider", "future", "stretch", "Optional"} {
		assert.Contains(t, prompt, forbidden, "critic prompt missing forbidden pattern %q", forbidden)
	}
	assert.Contains(t, prompt, "Critic complete")
}

func TestRenderPRDPrompt(t *testing.T) {
	result, err := RenderPRDPrompt(testTasksDir, testBriefPath)
	require.NoError(t, err)

	assert.Contains(t, result, testBriefPath)
	assert.Contains(t, result, testTasksDir+"/PRD.md")
	assert.Contains(t, result, "## Repo Evidence")
	assert.Contains(t, result, "Grounded in:")
	// Drift license must be gone.
	assert.NotContains(t, result, "make a decision and list it as an assumption")
	assert.NotContains(t, result, "Do NOT turn assumptions into requirements")
	// Forbidden words list.
	assert.Contains(t, result, "consider")
	assert.Contains(t, result, "future")
	assert.Contains(t, result, "PRD.md written")
}

func TestRenderTechnologyPrompt(t *testing.T) {
	result, err := RenderTechnologyPrompt(testTasksDir, testBriefPath)
	require.NoError(t, err)

	assert.Contains(t, result, testBriefPath)
	assert.Contains(t, result, testTasksDir+"/PRD.md")
	assert.Contains(t, result, testTasksDir+"/TECHNOLOGY.md")
	assert.Contains(t, result, "## Repo Evidence")
	assert.Contains(t, result, "Grounded in:")
	// Embedded 30-line testing philosophy block must be gone.
	assert.NotContains(t, result, "Three layers, distinct purposes")
	assert.NotContains(t, result, "What \"outside\" means per surface")
	assert.Contains(t, result, "TECHNOLOGY.md written")
}

func TestRenderDesignPrompt(t *testing.T) {
	result, err := RenderDesignPrompt(testTasksDir, testBriefPath)
	require.NoError(t, err)

	assert.Contains(t, result, testBriefPath)
	assert.Contains(t, result, testTasksDir+"/PRD.md")
	assert.Contains(t, result, testTasksDir+"/DESIGN.md")
	assert.Contains(t, result, "## Repo Evidence")
	assert.Contains(t, result, "Grounded in:")

	// Contract Rules + State Matrix preserved.
	assert.Contains(t, result, "Contract")
	assert.Contains(t, result, "MUST")
	assert.Contains(t, result, "MUST NOT")
	assert.Contains(t, result, "State Matrix")
	assert.Contains(t, result, "30 rules")
	assert.Contains(t, result, "DESIGN.md written")
}

func TestRenderAnalyzeTasksPrompt(t *testing.T) {
	result, err := RenderAnalyzeTasksPrompt(testTasksDir, testBriefPath)
	require.NoError(t, err)

	assert.Contains(t, result, testBriefPath)
	assert.Contains(t, result, testTasksDir+"/PRD.md")
	assert.Contains(t, result, testTasksDir+"/TECHNOLOGY.md")
	assert.Contains(t, result, "vertical slice")
	assert.Contains(t, result, "Walking Skeleton")
	assert.Contains(t, result, "Scope (In) bullets")
	assert.Contains(t, result, "Acceptance criteria")
	assert.Contains(t, result, "Grounded in:")

	// Stripped: 6 anti-patterns, traceability gate, context alignment.
	assert.NotContains(t, result, "Horizontal Slice")
	assert.NotContains(t, result, "UI-Undefined Task")
	assert.NotContains(t, result, "Traceability Gate")
	assert.NotContains(t, result, "Context Alignment Check")
	assert.NotContains(t, result, "## Conflict Resolution")
	assert.NotContains(t, result, "6 anti-patterns")
}

func TestRenderGenerateTasksPrompt(t *testing.T) {
	result, err := RenderGenerateTasksPrompt(testTasksDir, testBriefPath)
	require.NoError(t, err)

	assert.Contains(t, result, testTasksDir+"/TASKS.md")
	assert.Contains(t, result, testBriefPath)

	// TASKS.md A–J sections preserved.
	for _, section := range []string{"A. ", "B. ", "C. ", "D. ", "E. ", "F. ", "G. ", "H. ", "I. ", "J. "} {
		assert.Contains(t, result, section, "should contain section %s", section)
	}

	// 15-section TASK format preserved.
	assert.Contains(t, result, "0. Task Type and Placement")
	assert.Contains(t, result, "14. Follow-ups Unlocked")

	// Subagent dispatch + Grounded-in footer requirement.
	assert.Contains(t, result, "Agent tool")
	assert.Contains(t, result, "subagent")
	assert.Contains(t, result, "Grounded in:")
}

func TestRenderSlimTaskPrompt_Tiny(t *testing.T) {
	result, err := RenderSlimTaskPrompt(testTasksDir, testBriefPath, 1, false)
	require.NoError(t, err)

	assert.Contains(t, result, testBriefPath)
	assert.Contains(t, result, testTasksDir+"/TASK1.md")
	// At tiny: 1–3 repo files required.
	assert.Contains(t, result, "1–3 file")
	assert.NotContains(t, result, "PRD.md")

	// 6 sections.
	assert.Contains(t, result, "1. Outcome")
	assert.Contains(t, result, "2. Scope")
	assert.Contains(t, result, "3. Acceptance")
	assert.Contains(t, result, "4. Files likely touched")
	assert.Contains(t, result, "5. Verification")
	assert.Contains(t, result, "6. Grounded in")
	assert.Contains(t, result, "TASK1.md written")
}

func TestRenderSlimTaskPrompt_Small(t *testing.T) {
	result, err := RenderSlimTaskPrompt(testTasksDir, testBriefPath, 2, true)
	require.NoError(t, err)

	assert.Contains(t, result, testBriefPath)
	assert.Contains(t, result, testTasksDir+"/PRD.md")
	assert.Contains(t, result, testTasksDir+"/TASK2.md")
	// Small: 3+ repo file paths required.
	assert.Contains(t, result, "3+ repo file paths")
	assert.Contains(t, result, "TASK2.md written")
}

func TestRenderSlimTasksMdPrompt(t *testing.T) {
	result, err := RenderSlimTasksMdPrompt(testTasksDir, testBriefPath)
	require.NoError(t, err)

	assert.Contains(t, result, testTasksDir+"/TASKS.md")
	assert.Contains(t, result, testBriefPath)
	// Section G heading required for snap run compatibility.
	assert.Contains(t, result, "## G. Task list")
	// Cap at 3 tasks.
	assert.Contains(t, result, "Cap at 3 tasks")
	// TIER_MISMATCH escape hatch.
	assert.Contains(t, result, "TIER_MISMATCH")
	assert.Contains(t, result, "TASKS.md written")
}

func TestAllPrompts_HaveBriefPath(t *testing.T) {
	tests := []struct {
		name   string
		render func() (string, error)
	}{
		{"Requirements", func() (string, error) { return RenderRequirementsPrompt(testBriefPath) }},
		{"BriefSynthesis", func() (string, error) { return RenderBriefSynthesisPrompt(testBriefPath) }},
		{"Triage", func() (string, error) { return RenderTriagePrompt(testBriefPath) }},
		{"Critic", func() (string, error) { return RenderCriticPrompt(testBriefPath, testTasksDir+"/PRD.md") }},
		{"PRD", func() (string, error) { return RenderPRDPrompt(testTasksDir, testBriefPath) }},
		{"Technology", func() (string, error) { return RenderTechnologyPrompt(testTasksDir, testBriefPath) }},
		{"Design", func() (string, error) { return RenderDesignPrompt(testTasksDir, testBriefPath) }},
		{"AnalyzeTasks", func() (string, error) { return RenderAnalyzeTasksPrompt(testTasksDir, testBriefPath) }},
		{"GenerateTasks", func() (string, error) { return RenderGenerateTasksPrompt(testTasksDir, testBriefPath) }},
		{"SlimTaskTiny", func() (string, error) { return RenderSlimTaskPrompt(testTasksDir, testBriefPath, 1, false) }},
		{"SlimTaskSmall", func() (string, error) { return RenderSlimTaskPrompt(testTasksDir, testBriefPath, 1, true) }},
		{"SlimTasksMd", func() (string, error) { return RenderSlimTasksMdPrompt(testTasksDir, testBriefPath) }},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := tt.render()
			require.NoError(t, err)
			assert.Contains(t, result, testBriefPath, "%s prompt must reference brief path", tt.name)
		})
	}
}

func TestNoPrincipleseamble_AllPrompts(t *testing.T) {
	// principles.md is deleted; no rendered prompt should include KISS/DRY/SOLID/YAGNI preamble.
	prompts := []func() (string, error){
		func() (string, error) { return RenderPRDPrompt(testTasksDir, testBriefPath) },
		func() (string, error) { return RenderTechnologyPrompt(testTasksDir, testBriefPath) },
		func() (string, error) { return RenderDesignPrompt(testTasksDir, testBriefPath) },
		func() (string, error) { return RenderAnalyzeTasksPrompt(testTasksDir, testBriefPath) },
		func() (string, error) { return RenderGenerateTasksPrompt(testTasksDir, testBriefPath) },
	}

	for _, render := range prompts {
		result, err := render()
		require.NoError(t, err)
		// "Engineering Principles" was the heading of principles.md.
		assert.NotContains(t, result, "Engineering Principles")
		assert.False(t, strings.Contains(result, "KISS") && strings.Contains(result, "DRY") && strings.Contains(result, "SOLID") && strings.Contains(result, "YAGNI"),
			"prompt still contains KISS/DRY/SOLID/YAGNI preamble")
	}
}
