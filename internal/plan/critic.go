package plan

import (
	"context"
	"fmt"
	"io"

	"github.com/yarlson/snap/internal/model"
	"github.com/yarlson/snap/internal/ui"
	"github.com/yarlson/snap/internal/workflow"
)

// runCritic invokes the per-artifact critic on artifactPath against briefPath.
// Failures are non-fatal: the critic logs a warning to output and the planner
// continues. The critic's job is to delete uncited content from artifactPath
// in place via the LLM's Write tool.
func runCritic(ctx context.Context, exec workflow.Executor, output io.Writer, briefPath, artifactPath string) {
	prompt, err := RenderCriticPrompt(briefPath, artifactPath)
	if err != nil {
		fmt.Fprint(output, ui.Info(fmt.Sprintf("  critic skipped for %s: render prompt: %v", baseName(artifactPath), err)))
		return
	}

	if err := exec.Run(ctx, output, model.Fast, prompt); err != nil {
		fmt.Fprint(output, ui.Info(fmt.Sprintf("  critic skipped for %s: %v", baseName(artifactPath), err)))
	}
}

// baseName returns the trailing path component without depending on filepath
// (keeps the helper tiny and test-friendly).
func baseName(path string) string {
	for i := len(path) - 1; i >= 0; i-- {
		if path[i] == '/' || path[i] == '\\' {
			return path[i+1:]
		}
	}
	return path
}
