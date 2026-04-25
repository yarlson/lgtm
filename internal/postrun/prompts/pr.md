You are generating a GitHub pull request title and body. Use the inputs below.

## Output format

Line 1: the title.
Line 2: blank.
Lines 3+: the body in the structure defined under "Body structure".

Output only the title and body. No preamble, no code fences, no trailing notes.

## Title rules

- Free prose, imperative mood ("Add", "Fix", "Refactor"; not "Added", not "Adds").
- Target 50–72 characters. Hard cap 72.
- No scope prefix (no "feat:", no "[postrun]", no "PR:").
- No code, no file paths, no identifiers in backticks, no quotation marks.
- Describe the user-visible or behavioural change, not the mechanism.

## Body structure

The body has{{if .PRDContent}} three{{else}} two{{end}} blocks, in this exact order, each with a level-3 markdown heading:

{{if .PRDContent}}### Why

One or two sentences explaining the motivation, anchored in the PRD below. Do not quote the PRD; paraphrase. Do not restate the title.

{{end}}### What

Bulleted list. Each bullet summarises one logical change, derived from the commit messages. Merge related commits into one bullet. Do not list commit hashes. Collapse noisy commit subjects ("wip", "fix typo", etc.).

### How to verify

Bulleted list. Each bullet is a concrete check a reviewer can perform locally or in CI{{if .PRDContent}}, derived from the PRD's Requirements / Acceptance Criteria{{else}}, derived from the commits and diff stat{{end}}. Use imperative phrasing ("Run X", "Open Y, confirm Z").

## Length budget

Total body target: ~150 words. Hard ceiling: 250 words. Prefer terse bullets over prose.

## Anti-patterns (do not do these)

- Do not include code snippets, diffs, or file contents in the title or body.
- Do not paste the diff stat into the body.
- Do not quote the PRD verbatim. Use it only for motivation.
- Do not invent requirements that are not in the PRD or commits.
- Do not pad with filler ("This PR…", "In this change we…").
  {{if not .PRDContent}}- The PRD is not available for this branch. Skip the "Why" block entirely; do not fabricate motivation.
  {{end}}

## Inputs

{{if .PRDContent}}### PRD (motivation source — do not quote)

{{.PRDContent}}

{{end}}### Commit messages (oldest first)

{{if .CommitMessages}}{{.CommitMessages}}{{else}}(no commits in range){{end}}

### Diff stat (context only — do not paste)

{{if .DiffStat}}{{.DiffStat}}{{else}}(no diff){{end}}
