package plan

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"github.com/yarlson/tap"

	"github.com/yarlson/snap/internal/model"
	"github.com/yarlson/snap/internal/ui"
	"github.com/yarlson/snap/internal/workflow"
)

// briefFileName is the on-disk name of the structured brief produced by Phase 1.
const briefFileName = "BRIEF.md"

// planTierMarker is the on-disk filename next to .plan-started that records the
// triaged tier so resume can re-enter the right dispatcher branch.
const planTierMarker = ".plan-tier"

// openInEditor invokes the user's $EDITOR (fallback "vi") on the given path.
// Exposed as a package var so tests can replace it without spawning real
// editor processes.
var openInEditor = func(path string) error {
	editor := os.Getenv("EDITOR")
	if editor == "" {
		editor = "vi"
	}
	cmd := exec.Command(editor, path)
	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	return cmd.Run()
}

// Planner orchestrates the redesigned planning pipeline:
// Phase 1 chat → BRIEF.md synthesis → BRIEF review → triage → tier-conditional
// Phase 2 with per-artifact critic.
type Planner struct {
	executor          workflow.Executor
	sessionName       string
	tasksDir          string
	sessionDir        string
	output            io.Writer
	input             io.Reader
	interactive       bool         // when true, uses tap for interactive TTY input
	briefFile         string       // filename for display when --from is used (e.g. "brief.md")
	briefBody         string       // file content from --from
	resume            bool         // when true, first executor call uses -c to continue prior conversation
	afterFirstMessage func() error // called once after the first successful executor call
	firstMessageDone  bool

	// Test seam: lets tests force a tier without exercising the real triage
	// classifier or tap.Select. When non-empty, the user-confirmation prompt
	// is skipped.
	forcedTier *TriageResult
}

// PlannerOption configures a Planner.
type PlannerOption func(*Planner)

// WithOutput sets the output writer.
func WithOutput(w io.Writer) PlannerOption {
	return func(p *Planner) { p.output = w }
}

// WithInput sets the input reader for Phase 1.
func WithInput(r io.Reader) PlannerOption {
	return func(p *Planner) { p.input = r }
}

// WithResume sets whether to resume a previous planning conversation.
func WithResume(resume bool) PlannerOption {
	return func(p *Planner) { p.resume = resume }
}

// WithAfterFirstMessage sets a callback that fires once after the first successful executor call.
func WithAfterFirstMessage(fn func() error) PlannerOption {
	return func(p *Planner) { p.afterFirstMessage = fn }
}

// WithInteractive enables interactive input via tap during Phase 1 + brief review.
func WithInteractive(interactive bool) PlannerOption {
	return func(p *Planner) { p.interactive = interactive }
}

// WithBrief sets brief content provided via --from, skipping the Phase 1 chat.
func WithBrief(filename, content string) PlannerOption {
	return func(p *Planner) {
		p.briefFile = filename
		p.briefBody = content
	}
}

// WithSessionDir sets the session directory (parent of tasksDir) where the
// .plan-tier marker is written. Defaults to filepath.Dir(tasksDir).
func WithSessionDir(dir string) PlannerOption {
	return func(p *Planner) { p.sessionDir = dir }
}

// WithForcedTier injects a triage result (test-only seam). When set, Triage()
// is bypassed and the user-confirmation prompt is skipped.
func WithForcedTier(tr TriageResult) PlannerOption {
	return func(p *Planner) {
		t := tr
		p.forcedTier = &t
	}
}

// NewPlanner creates a new Planner with the given options.
func NewPlanner(executor workflow.Executor, sessionName, tasksDir string, opts ...PlannerOption) *Planner {
	p := &Planner{
		executor:    executor,
		sessionName: sessionName,
		tasksDir:    tasksDir,
		sessionDir:  filepath.Dir(tasksDir),
		output:      os.Stdout,
		input:       os.Stdin,
	}
	for _, opt := range opts {
		opt(p)
	}
	return p
}

// onFirstMessage fires the afterFirstMessage callback once after the first successful executor call.
func (p *Planner) onFirstMessage() error {
	if p.firstMessageDone || p.afterFirstMessage == nil {
		return nil
	}
	p.firstMessageDone = true
	return p.afterFirstMessage()
}

// briefPath returns the absolute path to the session's BRIEF.md.
func (p *Planner) briefPath() string {
	return filepath.Join(p.tasksDir, briefFileName)
}

// artifactPath returns the absolute path to a named artifact in tasksDir.
func (p *Planner) artifactPath(name string) string {
	return filepath.Join(p.tasksDir, name)
}

// artifactExists reports whether an artifact already exists on disk.
func (p *Planner) artifactExists(name string) bool {
	_, err := os.Stat(p.artifactPath(name))
	return err == nil
}

// Run orchestrates the full planning pipeline.
func (p *Planner) Run(ctx context.Context) error {
	switch {
	case p.briefBody != "":
		fmt.Fprint(p.output, ui.Step(fmt.Sprintf("Planning session '%s' — using %s as input", p.sessionName, p.briefFile)))
	case p.resume:
		fmt.Fprint(p.output, ui.Step(fmt.Sprintf("Resuming planning for session '%s'", p.sessionName)))
	default:
		fmt.Fprint(p.output, ui.Step(fmt.Sprintf("Planning session '%s'", p.sessionName)))
	}

	// Step 1: ensure BRIEF.md exists (Phase 1 chat OR --from copy OR resume detect).
	briefPath, err := p.ensureBrief(ctx)
	if err != nil {
		if ctx.Err() != nil || errors.Is(err, context.Canceled) {
			fmt.Fprintln(p.output, ui.Interrupted("Planning aborted"))
		}
		return err
	}

	// Step 2: review/edit BRIEF.md.
	if err := p.reviewBrief(ctx, briefPath); err != nil {
		if ctx.Err() != nil || errors.Is(err, context.Canceled) {
			fmt.Fprintln(p.output, ui.Interrupted("Planning aborted"))
		}
		return err
	}

	// Step 3: triage (or load tier from marker on resume).
	tr, err := p.resolveTier(ctx, briefPath)
	if err != nil {
		if ctx.Err() != nil || errors.Is(err, context.Canceled) {
			fmt.Fprintln(p.output, ui.Interrupted("Planning aborted"))
		}
		return err
	}

	// Step 4: dispatch per tier.
	return p.generateDocuments(ctx, briefPath, tr)
}

// ensureBrief produces BRIEF.md by one of three paths:
//   - --from mode: copy the input content to BRIEF.md.
//   - resume: BRIEF.md should already be on disk; verify.
//   - fresh: run Phase 1 chat then ask the model to synthesize BRIEF.md.
func (p *Planner) ensureBrief(ctx context.Context) (string, error) {
	briefPath := p.briefPath()

	// --from mode: write the file directly, no LLM call.
	if p.briefBody != "" {
		if err := os.MkdirAll(filepath.Dir(briefPath), 0o755); err != nil {
			return "", fmt.Errorf("create tasks dir: %w", err)
		}
		if err := os.WriteFile(briefPath, []byte(p.briefBody), 0o600); err != nil {
			return "", fmt.Errorf("write brief: %w", err)
		}
		return briefPath, nil
	}

	// Resume: BRIEF.md already exists → skip Phase 1.
	if p.resume {
		if _, err := os.Stat(briefPath); err == nil {
			return briefPath, nil
		}
		fmt.Fprint(p.output, ui.Info("Resume requested but BRIEF.md is missing — running Phase 1 from scratch."))
	}

	// Fresh: Phase 1 chat → BRIEF.md synthesis call.
	if err := p.gatherRequirements(ctx, briefPath); err != nil {
		return "", err
	}
	if err := p.synthesizeBrief(ctx, briefPath); err != nil {
		return "", err
	}
	return briefPath, nil
}

// gatherRequirements runs the interactive Phase 1 chat loop until /done.
func (p *Planner) gatherRequirements(ctx context.Context, briefPath string) error {
	fmt.Fprint(p.output, ui.Step("Gathering requirements — type /done when ready"))

	prompt, err := RenderRequirementsPrompt(briefPath)
	if err != nil {
		return fmt.Errorf("requirements prompt failed: %w", err)
	}
	var initArgs []string
	if p.resume {
		initArgs = append(initArgs, "-c")
	}
	initArgs = append(initArgs, prompt)
	if err := p.executor.Run(ctx, p.output, model.Thinking, initArgs...); err != nil {
		return fmt.Errorf("requirements prompt failed: %w", err)
	}

	if err := p.onFirstMessage(); err != nil {
		return err
	}

	if p.interactive {
		return p.gatherRequirementsInteractive(ctx)
	}
	return p.gatherRequirementsScanner(ctx)
}

func (p *Planner) gatherRequirementsInteractive(ctx context.Context) error {
	for {
		if ctx.Err() != nil {
			return ctx.Err()
		}

		fmt.Fprint(p.output, "\n")

		result := tap.Textarea(ctx, tap.TextareaOptions{
			Message:     "Your response",
			Placeholder: "Describe your requirements, or /done to finish",
			Validate: func(s string) error {
				if strings.TrimSpace(s) == "" {
					return errors.New("enter a message, or /done to finish")
				}
				return nil
			},
		})

		if ctx.Err() != nil {
			return ctx.Err()
		}
		if result == "" {
			return context.Canceled
		}

		result = strings.TrimSpace(result)
		if strings.EqualFold(result, "/done") {
			return nil
		}

		if err := p.executor.Run(ctx, p.output, model.Thinking, "-c", result); err != nil {
			return fmt.Errorf("chat message failed: %w", err)
		}
	}
}

func (p *Planner) gatherRequirementsScanner(ctx context.Context) error {
	scanner := bufio.NewScanner(p.input)
	for {
		if ctx.Err() != nil {
			return ctx.Err()
		}

		fmt.Fprint(p.output, "\nsnap plan> ")

		if !scanner.Scan() {
			break
		}

		line := strings.TrimSpace(scanner.Text())
		if strings.EqualFold(line, "/done") {
			break
		}
		if line == "" {
			continue
		}

		if err := p.executor.Run(ctx, p.output, model.Thinking, "-c", line); err != nil {
			return fmt.Errorf("chat message failed: %w", err)
		}
	}

	if err := scanner.Err(); err != nil {
		return fmt.Errorf("input read error: %w", err)
	}

	return nil
}

// synthesizeBrief asks Claude to write the structured BRIEF.md from chat history,
// then verifies the file actually appeared on disk. The agent occasionally treats
// the synthesis call as a chat turn and asks clarifying questions instead of
// writing — without verification, the planner would happily proceed against a
// non-existent file. We fail loudly instead.
func (p *Planner) synthesizeBrief(ctx context.Context, briefPath string) error {
	prompt, err := RenderBriefSynthesisPrompt(briefPath)
	if err != nil {
		return fmt.Errorf("render brief synthesis prompt: %w", err)
	}

	fmt.Fprint(p.output, ui.Step("Synthesizing BRIEF.md..."))
	start := time.Now()
	if err := p.executor.Run(ctx, p.output, model.Fast, "-c", prompt); err != nil {
		return fmt.Errorf("brief synthesis failed: %w", err)
	}
	if _, statErr := os.Stat(briefPath); statErr != nil {
		fmt.Fprintln(p.output, ui.StepFailed("BRIEF.md synthesis", time.Since(start)))
		return fmt.Errorf("synthesis call returned success but %s was not written: %w", briefPath, statErr)
	}
	fmt.Fprintln(p.output, ui.StepComplete("BRIEF.md written", time.Since(start)))
	return nil
}

// reviewBrief offers the user a chance to inspect and edit BRIEF.md before triage.
// Skipped when non-interactive (piped tests, CI) or when --from supplied the brief
// (the user already controls that file directly).
func (p *Planner) reviewBrief(ctx context.Context, briefPath string) error {
	if !p.interactive || p.briefBody != "" {
		return nil
	}

	for {
		if ctx.Err() != nil {
			return ctx.Err()
		}

		choice := tap.Select(ctx, tap.SelectOptions[string]{
			Message: fmt.Sprintf("Brief written to %s.", briefPath),
			Options: []tap.SelectOption[string]{
				{Value: "continue", Label: "Continue with this brief"},
				{Value: "edit", Label: "Open in $EDITOR (or vi)"},
				{Value: "abort", Label: "Abort"},
			},
		})

		if ctx.Err() != nil {
			return ctx.Err()
		}

		switch choice {
		case "continue":
			return nil
		case "edit":
			if err := openInEditor(briefPath); err != nil {
				fmt.Fprint(p.output, ui.Error(fmt.Sprintf("editor failed: %v", err)))
			}
			continue
		case "abort", "":
			return context.Canceled
		}
	}
}

// resolveTier returns the tier for this run. On resume with a stored marker,
// it re-uses the marker; otherwise it runs the LLM classifier and prompts the
// user to confirm.
func (p *Planner) resolveTier(ctx context.Context, briefPath string) (TriageResult, error) {
	if p.forcedTier != nil {
		if err := p.writeTierMarker(*p.forcedTier); err != nil {
			fmt.Fprint(p.output, ui.Info(fmt.Sprintf("could not write tier marker: %v", err)))
		}
		return *p.forcedTier, nil
	}

	// Resume: try to read existing marker.
	if p.resume {
		if tr, ok := p.readTierMarker(); ok {
			fmt.Fprint(p.output, ui.Info(fmt.Sprintf("Resuming with tier: %s.", tr.Tier)))
			return tr, nil
		}
	}

	// Fresh: classify, then confirm.
	fmt.Fprint(p.output, ui.Step("Triaging brief..."))
	tr, err := Triage(ctx, p.executor, briefPath, p.output)
	if err != nil {
		return TriageResult{}, fmt.Errorf("triage: %w", err)
	}

	if p.interactive {
		tr = p.confirmTier(ctx, tr)
		if ctx.Err() != nil {
			return TriageResult{}, ctx.Err()
		}
	}
	if err := p.writeTierMarker(tr); err != nil {
		fmt.Fprint(p.output, ui.Info(fmt.Sprintf("could not write tier marker: %v", err)))
	}
	return tr, nil
}

func (p *Planner) confirmTier(ctx context.Context, suggested TriageResult) TriageResult {
	options := []tap.SelectOption[Tier]{
		{Value: TierTiny, Label: "Tiny:  one focused change → TASK1.md only"},
		{Value: TierSmall, Label: "Small: 2–4 vertical slices → PRD-lite + TASKS.md + TASK1–3.md"},
		{Value: TierFull, Label: "Full:  multi-module → PRD + (TECH) + (DESIGN) + TASKS.md + TASK1..N.md"},
	}
	// Move the suggested tier to the head of the list (default selection).
	sorted := make([]tap.SelectOption[Tier], 0, len(options))
	for _, o := range options {
		if o.Value == suggested.Tier {
			sorted = append(sorted, o)
		}
	}
	for _, o := range options {
		if o.Value != suggested.Tier {
			sorted = append(sorted, o)
		}
	}

	rationale := suggested.Rationale
	if rationale == "" {
		rationale = "no rationale supplied"
	}

	choice := tap.Select(ctx, tap.SelectOptions[Tier]{
		Message: fmt.Sprintf("Triage suggests %s — %s. Confirm or override:", suggested.Tier, rationale),
		Options: sorted,
	})
	if ctx.Err() != nil || choice == "" {
		return suggested
	}
	if choice == suggested.Tier {
		return suggested
	}
	// User overrode the tier. Reset flags to match the new tier:
	// at non-full tiers, flags don't matter; at full tier, default to true so
	// the user gets the broadest artifact set unless they edit BRIEF.md.
	res := TriageResult{Tier: choice, Rationale: "user override"}
	if choice == TierFull {
		res.HasArchitecture = true
		res.HasUI = true
	}
	return res
}

func (p *Planner) writeTierMarker(tr TriageResult) error {
	if p.sessionDir == "" {
		return nil
	}
	line := fmt.Sprintf("%s|%t|%t\n", tr.Tier, tr.HasArchitecture, tr.HasUI)
	return os.WriteFile(filepath.Join(p.sessionDir, planTierMarker), []byte(line), 0o600)
}

func (p *Planner) readTierMarker() (TriageResult, bool) {
	if p.sessionDir == "" {
		return TriageResult{}, false
	}
	data, err := os.ReadFile(filepath.Join(p.sessionDir, planTierMarker))
	if err != nil {
		return TriageResult{}, false
	}
	parts := strings.Split(strings.TrimSpace(string(data)), "|")
	if len(parts) != 3 {
		return TriageResult{}, false
	}
	tier := Tier(parts[0])
	if tier != TierTiny && tier != TierSmall && tier != TierFull {
		return TriageResult{}, false
	}
	return TriageResult{
		Tier:            tier,
		HasArchitecture: parts[1] == "true",
		HasUI:           parts[2] == "true",
	}, true
}

// generateDocuments dispatches to the per-tier pipeline.
func (p *Planner) generateDocuments(ctx context.Context, briefPath string, tr TriageResult) error {
	fmt.Fprint(p.output, ui.Step("Generating planning documents..."))

	switch tr.Tier {
	case TierTiny:
		return p.generateTiny(ctx, briefPath)
	case TierSmall:
		return p.generateSmall(ctx, briefPath)
	case TierFull:
		return p.generateFull(ctx, briefPath, tr)
	default:
		return fmt.Errorf("unknown tier: %s", tr.Tier)
	}
}

// runWriterStep renders + executes the writer for one artifact, then runs the critic.
// If runCritic is false, the critic is skipped (e.g. for TASKS.md).
// continueChat controls whether to pass -c (used only by analyze→generate at full tier).
func (p *Planner) runWriterStep(
	ctx context.Context,
	stepIdx, totalSteps int,
	label, artifactName, briefPath string,
	render func() (string, error),
	continueChat bool,
	runCriticAfter bool,
) error {
	if ctx.Err() != nil {
		return ctx.Err()
	}

	fmt.Fprint(p.output, ui.StepNumbered(stepIdx, totalSteps, label))
	start := time.Now()

	if artifactName != "" && p.artifactExists(artifactName) {
		fmt.Fprintln(p.output, ui.StepComplete(fmt.Sprintf("%s already exists, skipping", artifactName), 0))
	} else {
		prompt, err := render()
		if err != nil {
			return fmt.Errorf("render %s: %w", label, err)
		}
		var args []string
		if continueChat {
			args = append(args, "-c")
		}
		args = append(args, prompt)
		if err := p.executor.Run(ctx, p.output, model.Thinking, args...); err != nil {
			fmt.Fprintln(p.output, ui.StepFailed(label, time.Since(start)))
			if ctx.Err() != nil {
				return ctx.Err()
			}
			return fmt.Errorf("step %d/%d %q failed: %w", stepIdx, totalSteps, label, err)
		}
		if err := p.onFirstMessage(); err != nil {
			return err
		}
		// Verify the artifact actually appeared. The agent occasionally claims
		// success while having asked a clarifying question instead of writing.
		if artifactName != "" && !p.artifactExists(artifactName) {
			fmt.Fprintln(p.output, ui.StepFailed(label, time.Since(start)))
			return fmt.Errorf("step %d/%d %q reported success but %s was not written", stepIdx, totalSteps, label, artifactName)
		}
		fmt.Fprintln(p.output, ui.StepComplete(label, time.Since(start)))
	}

	if runCriticAfter && artifactName != "" {
		runCritic(ctx, p.executor, p.output, briefPath, p.artifactPath(artifactName))
	}
	return nil
}

// generateTiny: TASK1.md + critic.
func (p *Planner) generateTiny(ctx context.Context, briefPath string) error {
	totalSteps := 1
	if err := p.runWriterStep(
		ctx, 1, totalSteps, "Generate TASK1.md",
		"TASK1.md", briefPath,
		func() (string, error) { return RenderSlimTaskPrompt(p.tasksDir, briefPath, 1, false) },
		false, true,
	); err != nil {
		return err
	}
	fmt.Fprintln(p.output)
	fmt.Fprintln(p.output, ui.Complete("Planning complete"))
	return nil
}

// generateSmall: PRD-lite + slim TASKS.md + per-task slim files, each with critic.
func (p *Planner) generateSmall(ctx context.Context, briefPath string) error {
	totalSteps := 3 // PRD, TASKS.md, per-task

	// Step 1: PRD-lite.
	if err := p.runWriterStep(
		ctx, 1, totalSteps, "Generate PRD",
		"PRD.md", briefPath,
		func() (string, error) { return RenderPRDPrompt(p.tasksDir, briefPath) },
		false, true,
	); err != nil {
		return err
	}

	// Step 2: TASKS.md slim. No critic (structural index, no claims to ground).
	if err := p.runWriterStep(
		ctx, 2, totalSteps, "Generate TASKS.md",
		"TASKS.md", briefPath,
		func() (string, error) { return RenderSlimTasksMdPrompt(p.tasksDir, briefPath) },
		false, false,
	); err != nil {
		return err
	}

	// Detect TIER_MISMATCH escape hatch.
	if data, err := os.ReadFile(p.artifactPath("TASKS.md")); err == nil {
		if strings.Contains(string(data), "TIER_MISMATCH") {
			fmt.Fprint(p.output, ui.Info("Brief expanded beyond 3 tasks — re-run with full tier (snap plan again, choose 'full')."))
			return fmt.Errorf("tier mismatch: brief requires the full tier")
		}
	}

	// Step 3: count tasks from TASKS.md, then write each slim TASK<N>.md.
	taskCount := countTasksInTasksMd(p.artifactPath("TASKS.md"))
	if taskCount == 0 {
		taskCount = 1
	}
	if taskCount > 3 {
		taskCount = 3
	}

	for i := 1; i <= taskCount; i++ {
		idx := i
		artifactName := fmt.Sprintf("TASK%d.md", idx)
		if err := p.runWriterStep(
			ctx, 3, totalSteps, fmt.Sprintf("Generate %s", artifactName),
			artifactName, briefPath,
			func() (string, error) { return RenderSlimTaskPrompt(p.tasksDir, briefPath, idx, true) },
			false, true,
		); err != nil {
			return err
		}
	}

	fmt.Fprintln(p.output)
	fmt.Fprintln(p.output, ui.Complete("Planning complete"))
	return nil
}

// generateFull: PRD + (TECH) + (DESIGN) + analyze + generate-tasks (15-section
// TASK<N>.md via subagents). Critic runs after each non-conversational artifact.
func (p *Planner) generateFull(ctx context.Context, briefPath string, tr TriageResult) error {
	type fullStep struct {
		label    string
		artifact string
		render   func() (string, error)
		runCrit  bool
		// Only the final generate-tasks step uses -c (analyze runs before it,
		// in conversation; generate-tasks reads the analyzed list).
		continueChat bool
	}

	steps := []fullStep{
		{
			label: "Generate PRD", artifact: "PRD.md",
			render:  func() (string, error) { return RenderPRDPrompt(p.tasksDir, briefPath) },
			runCrit: true,
		},
	}
	if tr.HasArchitecture {
		steps = append(steps, fullStep{
			label: "Generate TECHNOLOGY.md", artifact: "TECHNOLOGY.md",
			render:  func() (string, error) { return RenderTechnologyPrompt(p.tasksDir, briefPath) },
			runCrit: true,
		})
	}
	if tr.HasUI {
		steps = append(steps, fullStep{
			label: "Generate DESIGN.md", artifact: "DESIGN.md",
			render:  func() (string, error) { return RenderDesignPrompt(p.tasksDir, briefPath) },
			runCrit: true,
		})
	}
	steps = append(steps,
		fullStep{
			label: "Analyze tasks", artifact: "", // in-conversation, no file written
			render:  func() (string, error) { return RenderAnalyzeTasksPrompt(p.tasksDir, briefPath) },
			runCrit: false,
		},
		fullStep{
			label: "Generate tasks", artifact: "TASKS.md",
			render:       func() (string, error) { return RenderGenerateTasksPrompt(p.tasksDir, briefPath) },
			runCrit:      false, // critic skipped on TASKS.md (structural index)
			continueChat: true,
		},
	)

	total := len(steps)
	for i, s := range steps {
		if err := p.runWriterStep(ctx, i+1, total, s.label, s.artifact, briefPath, s.render, s.continueChat, s.runCrit); err != nil {
			return err
		}
	}

	// Per-task critic batch over TASK<N>.md files written by generate-tasks subagents.
	taskFiles := listTaskFiles(p.tasksDir)
	if len(taskFiles) > 0 {
		fmt.Fprint(p.output, ui.Info(fmt.Sprintf("Running critic over %d task file(s)...", len(taskFiles))))
		var criticTasks []parallelTask
		for _, name := range taskFiles {
			fname := name
			prompt, err := RenderCriticPrompt(briefPath, p.artifactPath(fname))
			if err != nil {
				fmt.Fprint(p.output, ui.Info(fmt.Sprintf("  critic skipped for %s: render prompt: %v", fname, err)))
				continue
			}
			criticTasks = append(criticTasks, parallelTask{
				name:      "critic " + fname,
				modelType: model.Fast,
				args:      []string{prompt},
			})
		}
		// runParallel returns results; critic failures are non-fatal.
		_ = runParallel(ctx, p.executor, criticTasks, 0)
	}

	fmt.Fprintln(p.output)
	fmt.Fprintln(p.output, ui.Complete("Planning complete"))
	return nil
}

// countTasksInTasksMd counts rows in the section G table of TASKS.md (lines that
// look like "| 1 | TASK1.md |" etc.). Returns 0 if the file can't be read.
func countTasksInTasksMd(path string) int {
	data, err := os.ReadFile(path)
	if err != nil {
		return 0
	}
	count := 0
	for _, line := range strings.Split(string(data), "\n") {
		trimmed := strings.TrimSpace(line)
		if !strings.HasPrefix(trimmed, "|") {
			continue
		}
		// Skip header, separator, and rows whose first cell isn't numeric.
		fields := strings.Split(trimmed, "|")
		if len(fields) < 3 {
			continue
		}
		first := strings.TrimSpace(fields[1])
		if first == "" {
			continue
		}
		if first[0] < '0' || first[0] > '9' {
			continue
		}
		count++
	}
	return count
}

// listTaskFiles returns the names of TASK<N>.md files in tasksDir, in numeric order.
func listTaskFiles(tasksDir string) []string {
	entries, err := os.ReadDir(tasksDir)
	if err != nil {
		return nil
	}
	var names []string
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		name := e.Name()
		if strings.HasPrefix(name, "TASK") && strings.HasSuffix(name, ".md") && name != "TASKS.md" {
			names = append(names, name)
		}
	}
	return names
}
