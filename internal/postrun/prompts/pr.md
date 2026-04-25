Your job: create a GitHub pull request for the current branch by running `gh pr create` with a title and body that follow the rules below. Use the inputs at the bottom to compose the title and body. Run `gh pr create` exactly once.

## Title rules

- Free prose, imperative mood ("Add", "Fix", "Refactor"; not "Added", not "Adds").
- Target 50–72 characters. Hard cap 72.
- One single line. No newlines, no markdown headings, no bullets, no quotation marks.
- No code, no file paths, no identifiers in backticks.
- No scope prefix ("feat:", "[postrun]", "PR:").
- Describe the user-visible or behavioural change, not the mechanism.

## Body rules

The body has{{if .PRDContent}} three{{else}} two{{end}} blocks, in this exact order, each with a level-3 markdown heading:

{{if .PRDContent}}### Why

One or two sentences explaining the motivation, anchored in the PRD below. Paraphrase; do not quote. Do not restate the title.

{{end}}### What

Bulleted list. Each bullet summarises one logical change, derived from the commit messages. Merge related commits into one bullet. Do not list commit hashes. Collapse noisy commit subjects ("wip", "fix typo", etc.).

### How to verify

Bulleted list. Each bullet is a concrete check a reviewer can perform locally or in CI{{if .PRDContent}}, derived from the PRD's Requirements / Acceptance Criteria{{else}}, derived from the commits and diff stat{{end}}. Use imperative phrasing ("Run X", "Open Y, confirm Z").

## Length budget

Body target ~150 words. Hard ceiling 250 words. Prefer terse bullets over prose.

## Anti-patterns (do not do these)

- Do not include code snippets, diffs, or file contents in the title or body.
- Do not paste the diff stat into the body.
- Do not quote the PRD verbatim. Use it only for motivation.
- Do not invent requirements that are not in the PRD or commits.
- Do not pad with filler ("This PR…", "In this change we…").
{{if not .PRDContent}}- The PRD is not available for this branch. Skip the "Why" block entirely; do not fabricate motivation.
{{end}}

## Execution

Run exactly one shell command: `gh pr create --title "<title>" --body "<body>"`. Do not pass `--draft`, `--base`, or any other flag. Do not run `gh pr create` more than once. If `gh pr create` rejects the title (for example "Title is too long"), shorten the title to fit the 72-character cap and retry once. After the PR is created, stop — do not output further commentary.

## Inputs

{{if .PRDContent}}### PRD (motivation source — do not quote)

{{.PRDContent}}

{{end}}### Commit messages (oldest first)

{{if .CommitMessages}}{{.CommitMessages}}{{else}}(no commits in range){{end}}

### Diff stat (context only — do not paste)

{{if .DiffStat}}{{.DiffStat}}{{else}}(no diff){{end}}
