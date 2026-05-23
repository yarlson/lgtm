# AGENTS.md

## Project Overview

`lgtm` is a Rust 2024 CLI and library crate that runs a repository-local
`PLAN.md` through Codex-backed implementation, validation, and review passes.
The binary wraps `codex exec`, installs bundled `lgtm-*` skills into the target
repository, stores raw JSONL logs under `.codex-log/`, and renders a compact
terminal transcript.

This is a single-crate repository, not a monorepo. The package and binary name
are both `lgtm`, even though the checkout directory may still be named
`snap-rs`.

## Setup Commands

- Install Rust stable with Cargo, rustfmt, and clippy available.
- Install dependencies and build the crate:

```bash
cargo build --all-targets --all-features
```

- Run the CLI from the repo without installing:

```bash
cargo run -- --help
```

- Install the local binary:

```bash
cargo install --path .
```

## Development Workflow

- Main entrypoint: `src/main.rs` calls `lgtm::run()`.
- CLI parsing and environment variables live in `src/cli.rs`.
- Phase orchestration and Codex process management live in `src/codex.rs`.
- `PLAN.md` parsing lives in `src/plan.rs`.
- Prompt construction lives in `src/prompt.rs`.
- Managed skill installation lives in `src/skills.rs`.
- Git preflight behavior lives in `src/git.rs`.
- Codex JSONL event parsing lives in `src/events.rs`.
- Transcript rendering lives in `src/render.rs` and `src/render/*`.
- Terminal/color behavior lives in `src/terminal.rs`.
- Bundled Codex skills live in `skills/lgtm-*/SKILL.md` and are embedded into
  the binary at compile time.

Keep changes phase-scoped and surgical. Do not broaden `lgtm` into branch,
PR, CI, or remote release automation unless the task explicitly asks for that.
The local harness intentionally does not commit, push, open PRs, or manage CI.

## Testing and Verification

Run the full local check before finishing meaningful code changes:

```bash
make check
```

`make check` runs:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --all-targets --all-features
```

Useful focused commands:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --test loop
cargo test <test_name>
```

Tests are split between inline module tests under `src/` and integration tests
under `tests/`. The integration tests use temporary Git repositories and fake
`codex` executables; preserve that pattern for orchestration behavior so tests
exercise the real binary without invoking the real Codex CLI.

When changing prompt text, managed skills, or rendered output, update the
expected integration-test assertions in `tests/loop.rs` only to match intended
user-visible behavior.

## Code Style

- Follow idiomatic Rust 2024 and the existing module boundaries.
- Keep business rules out of rendering and terminal glue.
- Prefer explicit structs/enums for event and payload shapes over ad hoc JSON or
  string parsing when the data has structure.
- Keep errors typed through `src/error.rs`; include actionable context in error
  messages.
- Avoid large, cross-cutting refactors unless they are directly required by the
  task.
- Do not add dependencies for small standard-library tasks.
- Format with `cargo fmt`; clippy warnings are denied.

## Bundled Skill Rules

- Source skill files are `skills/lgtm-*/SKILL.md`.
- Installed target-repo skills are generated under `.agents/skills/lgtm-*`;
  do not treat those generated copies as source.
- Managed skills must keep exact frontmatter identifying the skill name and
  `managed-by: lgtm`; `lgtm` refuses to overwrite project-owned skills that do
  not match the managed-skill contract.
- If a skill rename or new bundled skill is required, update both the skill file
  tree and the Rust embedding/installation path in `src/skills.rs`.

## Runtime and Security Notes

`lgtm` runs:

```text
codex exec -C <root> --dangerously-bypass-approvals-and-sandbox --json -
```

inside the target repository. Treat this as fully autonomous local filesystem
and command execution. Preserve the existing safety messaging in README/help
text when changing invocation behavior.

Do not log secrets, tokens, or environment dumps. Raw Codex logs belong in
`.codex-log/`, which is ignored by this repository and added to target
repositories by preflight.

## Build and Release

- Debug build:

```bash
make build
```

- Release build:

```bash
make release
```

GitHub Releases are produced by `.github/workflows/release.yml` on `v*` tags.
The tag version must match `Cargo.toml`. CI validates formatting, clippy, and
tests with `--locked`, builds platform archives, publishes GitHub Release
assets, and updates `Formula/lgtm.rb` in `yarlson/homebrew-tap` using the
`HOMEBREW_TAP_TOKEN` secret.

If only the root package version in `Cargo.lock` is stale after a version bump,
prefer:

```bash
cargo update --offline -p lgtm
```

## Pull Request Guidelines

- Keep diffs narrow and tied to the requested behavior.
- Include or update tests for behavior changes.
- Run `make check` before handing off code changes.
- For documentation-only changes, still verify the documented commands against
  the current `Makefile`, `Cargo.toml`, README, and workflow files.
