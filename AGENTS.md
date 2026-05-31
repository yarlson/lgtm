# AGENTS.md

## Project Overview

`lgtm` is Rust 2024 CLI crate. Drives Codex via repo-local planning and phase execution loops. Uses `codex app-server`, installs bundled `lgtm-*` skills into target repos, writes app-server protocol logs under `.codex-log/`, renders compact terminal transcript with spinner rows for long turns.

Single-crate repo, not monorepo.

## Setup Commands

- Install Rust stable + Cargo, rustfmt, clippy.
- Build crate:

```bash
cargo build --all-targets --all-features
```

- Run CLI from repo without installing:

```bash
cargo run -- --help
```

- Install local binary:

```bash
cargo install --path .
```

## Development Workflow

- Entrypoint: `src/main.rs`.
- CLI parsing + env vars: `src/cli.rs`.
- Run-mode orchestration: `src/commands/run.rs`.
- Plan-mode orchestration: `src/commands/plan.rs`.
- Codex app-server protocol: `src/app_server/`.
- Phase index parsing: `src/phase_index.rs`.
- Prompt construction: `src/prompt.rs`.
- Managed skill install: `src/skills.rs`.
- Git preflight: `src/git.rs`.
- Output rendering: `src/output/`.
- Inline plan-mode answer editing: `src/composer.rs`.
- Bundled Codex skills: `skills/lgtm-*/SKILL.md`, embedded into binary at compile time.

Keep changes phase-scoped, surgical. Do not broaden `lgtm` into branch, PR, CI, or remote workflow automation unless task asks. Local harness intentionally does not commit, push, open PRs, or manage CI during normal `plan`/`run`.

## Testing and Verification

Run full local check before finishing meaningful code changes:

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

Focused commands:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --test run_app_server
cargo test --test plan_app_server
cargo test <test_name>
```

Tests split: inline module tests under `src/`, integration tests under `tests/`. Integration tests use temp Git repos + fake `codex` executables; preserve pattern for orchestration behavior so tests exercise real binary without real Codex CLI.

When changing prompt text, managed skills, or rendered output, update tests only to match intended user-visible behavior.

## Code Style

- Idiomatic Rust 2024, existing module boundaries.
- Keep business rules out of rendering/terminal glue.
- Prefer explicit structs/enums for protocol/payload shapes over ad hoc JSON or string parsing when data has structure.
- Actionable context in error messages.
- Avoid large cross-cutting refactors unless task requires.
- No deps for small standard-library tasks.
- Format with `cargo fmt`; clippy warnings denied.

## Bundled Skill Rules

- Source skill files: `skills/lgtm-*/SKILL.md`.
- Installed target-repo skills generated under `.agents/skills/lgtm-*`; not source.
- Managed skills keep exact frontmatter with skill name + `managed-by: lgtm`; `lgtm` refuses to overwrite project-owned skills not matching managed-skill contract.
- For skill rename or new bundled skill, update both skill file tree and Rust embedding/install path in `src/skills.rs`.

## Runtime and Security Notes

`lgtm` starts `codex app-server`, creates turns with `danger-full-access` and approval policy `never` inside target repo. Treat as fully autonomous local filesystem + command execution. Preserve existing safety messaging in README/help when changing invocation behavior.

No logging secrets, tokens, or env dumps. App-server protocol logs go in `.codex-log/`, ignored by this repo, added to target repos by preflight.

## Build and Release

- Debug build:

```bash
make build
```

- Release build:

```bash
make release
```

GitHub Releases produced by `.github/workflows/release.yml` on `v*` tags. Tag version must match `Cargo.toml`. CI validates formatting, clippy, tests with `--locked`, builds platform archives, publishes GitHub Release assets, can update `Formula/lgtm.rb` in `yarlson/homebrew-tap` via `HOMEBREW_TAP_TOKEN` secret.

Release ritual:

1. Bump minor package version in `Cargo.toml`.
2. Sync `Cargo.lock`:

```bash
cargo update --offline -p lgtm
```

3. Commit all release changes.
4. Create git tag `vX.Y.Z`, where `X.Y.Z` is the bumped `Cargo.toml` version.
5. Push commit + tag.
6. Wait for release CI.

If only root package version in `Cargo.lock` stale after bump, prefer:

```bash
cargo update --offline -p lgtm
```

## Pull Request Guidelines

- Keep diffs narrow, tied to requested behavior.
- Include/update tests for behavior changes.
- Run `make check` before handing off code changes.
- For doc-only changes, still verify documented commands against current `Makefile`, `Cargo.toml`, README, workflow files.