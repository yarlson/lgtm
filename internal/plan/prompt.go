package plan

import (
	"bytes"
	"embed"
	"strings"
	"text/template"
)

//go:embed prompts/*.md
var promptFS embed.FS

// promptData holds template parameters for plan prompt rendering.
type promptData struct {
	TasksDir     string
	BriefPath    string
	ArtifactPath string
	TaskNum      int
	HasPRD       bool
}

// RenderRequirementsPrompt returns the Phase 1 requirements-gathering prompt.
// briefPath is where Claude will write BRIEF.md when the user types /done.
func RenderRequirementsPrompt(briefPath string) (string, error) {
	return renderTemplate("prompts/requirements.md", promptData{BriefPath: briefPath})
}

// RenderBriefSynthesisPrompt renders the BRIEF.md synthesis prompt sent after /done.
func RenderBriefSynthesisPrompt(briefPath string) (string, error) {
	return renderTemplate("prompts/brief.md", promptData{BriefPath: briefPath})
}

// RenderTriagePrompt renders the triage classification prompt.
func RenderTriagePrompt(briefPath string) (string, error) {
	return renderTemplate("prompts/triage.md", promptData{BriefPath: briefPath})
}

// RenderCriticPrompt renders the per-artifact critic prompt.
func RenderCriticPrompt(briefPath, artifactPath string) (string, error) {
	return renderTemplate("prompts/critic.md", promptData{BriefPath: briefPath, ArtifactPath: artifactPath})
}

// RenderPRDPrompt renders the PRD generation prompt.
func RenderPRDPrompt(tasksDir, briefPath string) (string, error) {
	return renderTemplate("prompts/prd.md", promptData{TasksDir: tasksDir, BriefPath: briefPath})
}

// RenderTechnologyPrompt renders the technology plan generation prompt.
func RenderTechnologyPrompt(tasksDir, briefPath string) (string, error) {
	return renderTemplate("prompts/technology.md", promptData{TasksDir: tasksDir, BriefPath: briefPath})
}

// RenderDesignPrompt renders the design spec generation prompt.
func RenderDesignPrompt(tasksDir, briefPath string) (string, error) {
	return renderTemplate("prompts/design.md", promptData{TasksDir: tasksDir, BriefPath: briefPath})
}

// RenderAnalyzeTasksPrompt renders the task analysis prompt.
func RenderAnalyzeTasksPrompt(tasksDir, briefPath string) (string, error) {
	return renderTemplate("prompts/analyze-tasks.md", promptData{TasksDir: tasksDir, BriefPath: briefPath})
}

// RenderGenerateTasksPrompt renders the task generation prompt (TASKS.md + TASK<N>.md subagents).
func RenderGenerateTasksPrompt(tasksDir, briefPath string) (string, error) {
	return renderTemplate("prompts/generate-tasks.md", promptData{TasksDir: tasksDir, BriefPath: briefPath})
}

// RenderSlimTaskPrompt renders the slim 6-section TASK<N>.md prompt for tiny + small tiers.
// hasPRD is true at small tier (PRD-lite exists), false at tiny tier.
func RenderSlimTaskPrompt(tasksDir, briefPath string, taskNum int, hasPRD bool) (string, error) {
	return renderTemplate("prompts/task-slim.md", promptData{
		TasksDir:  tasksDir,
		BriefPath: briefPath,
		TaskNum:   taskNum,
		HasPRD:    hasPRD,
	})
}

// RenderSlimTasksMdPrompt renders the slim TASKS.md index prompt for small tier.
func RenderSlimTasksMdPrompt(tasksDir, briefPath string) (string, error) {
	return renderTemplate("prompts/tasks-md-slim.md", promptData{TasksDir: tasksDir, BriefPath: briefPath})
}

func renderTemplate(name string, data promptData) (string, error) {
	content, err := promptFS.ReadFile(name)
	if err != nil {
		return "", err
	}

	tmpl, err := template.New(name).Parse(string(content))
	if err != nil {
		return "", err
	}

	var buf bytes.Buffer
	if err := tmpl.Execute(&buf, data); err != nil {
		return "", err
	}

	return strings.TrimSpace(buf.String()), nil
}
