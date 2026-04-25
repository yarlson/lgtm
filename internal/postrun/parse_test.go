package postrun

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestParsePROutput(t *testing.T) {
	tests := []struct {
		name      string
		input     string
		wantTitle string
		wantBody  string
		wantErr   bool
	}{
		{
			name:      "title and body",
			input:     "Add auth\n\nImplements user login with OAuth2 support.",
			wantTitle: "Add auth",
			wantBody:  "Implements user login with OAuth2 support.",
		},
		{
			name:      "title only",
			input:     "Title only",
			wantTitle: "Title only",
			wantBody:  "",
		},
		{
			name:    "empty input",
			input:   "",
			wantErr: true,
		},
		{
			name:    "whitespace only",
			input:   "   \n\n  ",
			wantErr: true,
		},
		{
			name:      "title with multiple blank lines before body",
			input:     "Fix bug\n\n\nMultiple blank lines before body.",
			wantTitle: "Fix bug",
			wantBody:  "Multiple blank lines before body.",
		},
		{
			name:      "multiline body",
			input:     "Add feature\n\nFirst paragraph.\n\nSecond paragraph.",
			wantTitle: "Add feature",
			wantBody:  "First paragraph.\n\nSecond paragraph.",
		},
		{
			name:      "title with trailing whitespace",
			input:     "  Add feature  \n\nBody text.",
			wantTitle: "Add feature",
			wantBody:  "Body text.",
		},
		{
			name:      "three-block body with markdown headings",
			input:     "Add user login\n\n### Why\n\nUsers need to authenticate.\n\n### What\n\n- Add login form\n- Wire to API\n\n### How to verify\n\n- Run tests\n- Open /login",
			wantTitle: "Add user login",
			wantBody:  "### Why\n\nUsers need to authenticate.\n\n### What\n\n- Add login form\n- Wire to API\n\n### How to verify\n\n- Run tests\n- Open /login",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			title, body, err := parsePROutput(tt.input)
			if tt.wantErr {
				require.Error(t, err)
				return
			}
			require.NoError(t, err)
			assert.Equal(t, tt.wantTitle, title)
			assert.Equal(t, tt.wantBody, body)
		})
	}
}
