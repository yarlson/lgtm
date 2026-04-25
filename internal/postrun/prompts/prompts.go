package prompts

import (
	"bytes"
	_ "embed"
	"strings"
	"text/template"
)

//go:embed pr.md
var prTmpl string

//go:embed ci_fix.md
var ciFixTmpl string

// PRData holds template parameters for the PR generation prompt.
type PRData struct {
	PRDContent     string
	CommitMessages string
	DiffStat       string
}

// PR renders the PR generation prompt template with the given data.
func PR(data PRData) (string, error) {
	tmpl, err := template.New("pr").Parse(prTmpl)
	if err != nil {
		return "", err
	}
	var buf bytes.Buffer
	if err := tmpl.Execute(&buf, data); err != nil {
		return "", err
	}
	return strings.TrimSpace(buf.String()), nil
}

// CIFixData holds template parameters for the CI fix prompt.
type CIFixData struct {
	FailureLogs   string
	CheckName     string
	AttemptNumber int
	MaxAttempts   int
}

// CIFix renders the CI fix prompt template with the given data.
func CIFix(data CIFixData) (string, error) {
	tmpl, err := template.New("ci_fix").Parse(ciFixTmpl)
	if err != nil {
		return "", err
	}
	var buf bytes.Buffer
	if err := tmpl.Execute(&buf, data); err != nil {
		return "", err
	}
	return strings.TrimSpace(buf.String()), nil
}
