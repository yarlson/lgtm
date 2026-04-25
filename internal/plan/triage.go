package plan

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"strings"

	"github.com/yarlson/snap/internal/model"
	"github.com/yarlson/snap/internal/ui"
	"github.com/yarlson/snap/internal/workflow"
)

// Tier names the triage classification.
type Tier string

const (
	TierTiny  Tier = "tiny"
	TierSmall Tier = "small"
	TierFull  Tier = "full"
)

// TriageResult is the parsed output of the triage classifier.
type TriageResult struct {
	Tier            Tier
	HasArchitecture bool
	HasUI           bool
	Rationale       string
}

// triageJSON mirrors the JSON line emitted by the triage prompt.
type triageJSON struct {
	Tier            string `json:"tier"`
	HasArchitecture bool   `json:"has_architecture"`
	HasUI           bool   `json:"has_ui"`
	Rationale       string `json:"rationale"`
}

// Triage classifies the work described in briefPath into a tier with two flags.
// On parse failure, returns TierFull with both flags true so the user can
// override via the tap.Select prompt without losing access to all artifacts.
func Triage(ctx context.Context, exec workflow.Executor, briefPath string, output io.Writer) (TriageResult, error) {
	prompt, err := RenderTriagePrompt(briefPath)
	if err != nil {
		return TriageResult{}, fmt.Errorf("render triage prompt: %w", err)
	}

	// Capture executor output to a buffer alongside the user-visible writer.
	var buf bytes.Buffer
	mw := io.MultiWriter(output, &buf)

	if err := exec.Run(ctx, mw, model.Fast, prompt); err != nil {
		return TriageResult{}, fmt.Errorf("triage executor: %w", err)
	}

	res, ok := parseTriageOutput(buf.String())
	if !ok {
		fmt.Fprint(output, ui.Info("Triage classifier output unparseable — defaulting to full tier."))
		return TriageResult{Tier: TierFull, HasArchitecture: true, HasUI: true, Rationale: "default fallback"}, nil
	}
	return res, nil
}

// parseTriageOutput finds the first JSON object on a single line of the captured
// output and decodes it. ANSI escapes are stripped first because the claude
// executor renders markdown.
func parseTriageOutput(raw string) (TriageResult, bool) {
	stripped := ui.StripColors(raw)
	for _, line := range strings.Split(stripped, "\n") {
		line = strings.TrimSpace(line)
		start := strings.Index(line, "{")
		end := strings.LastIndex(line, "}")
		if start < 0 || end <= start {
			continue
		}
		candidate := line[start : end+1]
		var parsed triageJSON
		if err := json.Unmarshal([]byte(candidate), &parsed); err != nil {
			continue
		}
		tier := Tier(strings.ToLower(parsed.Tier))
		if tier != TierTiny && tier != TierSmall && tier != TierFull {
			continue
		}
		return TriageResult{
			Tier:            tier,
			HasArchitecture: parsed.HasArchitecture,
			HasUI:           parsed.HasUI,
			Rationale:       strings.TrimSpace(parsed.Rationale),
		}, true
	}
	return TriageResult{}, false
}
