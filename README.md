<div align="center">

<img src="assets/banner.png" alt="lgtm workflow banner showing plan, PLAN.md, AGENTS.md, implementation, validation, and review" width="100%">

# lgtm

Plan the work. Execute one phase. Verify. Review. Repeat.

</div>

`lgtm` is a local Codex harness for repo-sized work that should not be done
as one giant prompt. It creates or refines a `PLAN.md`, then runs selected plan
phases through implementation, validation, and review passes.

Use it for migrations, cleanup, feature slices, test hardening, and docs drift
where Codex needs a repeatable local loop.

> [!WARNING]
> `lgtm` drives `codex app-server` with `danger-full-access` and approval
> policy `never`. In host mode, that runs inside the target repository. In
> Apple Container mode, the target repository is mounted into a Linux VM.

## Overview

- `lgtm plan [BRIEF]` starts an interactive planning session.
- After planning produces final artifacts, choose whether to implement the
  completed plan now or exit.
- `lgtm run` executes selected phases from `PLAN.md`.
- Each run phase gets implement, validate, and review passes.
- Prompts are anchored to `PLAN.md`, `AGENTS.md`, and the exact phase heading.
- `PLAN.md` is reloaded before each phase so earlier phases can update later
  phases.
- Pretty output uses active spinner rows while Codex is thinking or running
  tools, then replaces them with final evidence.
- App-server protocol logs are written to `.lgtm/logs/`.
- Managed skills are installed under `.agents/skills/lgtm-*`.

## Install

Homebrew is the simplest path on macOS and Linux:

```bash
brew install yarlson/tap/lgtm
```

Prebuilt archives are available on the GitHub Releases page:

[github.com/yarlson/lgtm/releases/latest](https://github.com/yarlson/lgtm/releases/latest)

Download the archive for your platform, verify it with the matching `.sha256`
file, unpack it, and place the `lgtm` binary somewhere on your `PATH`.

Or install from a local checkout:

```bash
cargo install --path .
```

For development, run from the repository:

```bash
cargo run -- --help
```

## Plan

Plan mode requires an interactive TTY. `PLAN.md` and `AGENTS.md` do not need to
exist before planning starts.

```bash
lgtm plan "split the migration into small reviewable phases"
```

Plan mode asks one question at a time. The inline composer supports normal
editing keys, cursor movement, paste, and multiline answers with `Ctrl+J`,
`Shift+Enter`, or `Alt+Enter`. Press `Ctrl+C` once to clear the current input;
press it twice quickly to quit.

Enter `/finish` to ask Codex to write the final `PLAN.md` from the current
session context, or `/quit` to exit without another Codex turn. If `AGENTS.md`
was missing at the start, plan mode keeps going until both `PLAN.md` and
`AGENTS.md` are complete.

After the final artifacts are complete, plan mode asks whether to implement now
or exit. Pressing Enter alone is not a default and re-prompts for an explicit
choice. Choosing implementation stops the planning app-server and starts the
normal run-mode pipeline for all detected phases from Phase 1, using the current
plan command's root, plan path, Codex binary, execution sandbox settings, log
directory, and run stamp.

## Run

For run mode, the target repository must contain:

- `PLAN.md`
- `AGENTS.md`
- a Git repository at the target root, or permission to initialize one

<img src="assets/lgtm-run.gif" alt="Terminal recording of lgtm run showing the startup banner and active phase status line" width="100%">

Run one phase:

```bash
lgtm run --root ../target-repo --start-phase 1 --end-phase 1 --sleep-seconds 0
```

Run from phase 2 through the detected final phase:

```bash
lgtm run --root ../target-repo --start-phase 2
```

Use raw protocol output:

```bash
lgtm run --stream-mode raw
```

## Apple Container Sandboxing

Apple Container mode runs `codex app-server` through Apple's `container` CLI
instead of starting it directly on the host.

Requirements:

- macOS 26 or newer on Apple silicon
- Apple's `container` CLI installed and available on `PATH`
- a Codex auth file at `~/.codex/auth.json`
- the default sandbox image, pulled from GHCR or built locally

The default image is published as `ghcr.io/yarlson/lgtm-codex:latest`.
Pull it before running in Apple Container mode if you want to make image setup
explicit:

```bash
container image pull ghcr.io/yarlson/lgtm-codex:latest
```

If the published image is unavailable, build the same default image locally
with Apple Container:

```bash
container build -t ghcr.io/yarlson/lgtm-codex:latest containers/codex
```

Or build it with Docker:

```bash
docker build -t ghcr.io/yarlson/lgtm-codex:latest containers/codex
```

Smoke check the image:

```bash
container run --rm ghcr.io/yarlson/lgtm-codex:latest codex --version
```

Run inside Apple Container:

```bash
lgtm run --execution-sandbox apple-container --root ../target-repo
```

Before launching `codex app-server`, lgtm fails fast if the host is not macOS 26
or newer on Apple silicon, the configured `container` executable cannot run,
the Codex auth file is missing, Apple Container services are stopped, or the
sandbox image is neither available locally nor pullable.

Common remediation commands:

```bash
container system start
container image pull ghcr.io/yarlson/lgtm-codex:latest
container build -t ghcr.io/yarlson/lgtm-codex:latest containers/codex
```

The container receives:

- the target repository mounted read-write at `/workspace`
- `.lgtm/sandbox/home` mounted at `/root` for sandbox-local tool state written
  below `HOME`
- `~/.codex/auth.json` copied into a temporary directory mounted at
  `/root/.codex` over the sandbox home, so Codex can write runtime config
  without touching the real host Codex directory or persisting Codex auth in
  `.lgtm`
- `.lgtm/sandbox/mise` mounted at `/mise` for mise-installed toolchains and cache
- `HOME=/root` and `CODEX_HOME=/root/.codex`
- `MISE_DATA_DIR=/mise`, `MISE_CONFIG_DIR=/mise`, and
  `MISE_CACHE_DIR=/mise/cache`
- `MISE_PIN=1`, so activated tool versions are written exactly

The default image is `ghcr.io/yarlson/lgtm-codex:latest`. Override it with
`--sandbox-image` or `LGTM_SANDBOX_IMAGE`.

The image includes `mise`. In Apple Container mode, lgtm adds sandbox-specific
instructions telling Codex to use `mise use -g -y <tool>@<version>` when a
project needs a missing interpreter, runtime, compiler, or package manager and
does not already declare a toolchain. This activates the tool through
`/mise/config.toml`, so later commands can run directly through `/mise/shims`
without repeated `mise exec` wrappers. Mise state stays under
`.lgtm/sandbox/mise`; tool installers that write below `HOME` stay under
`.lgtm/sandbox/home`. Both are ignored with the rest of lgtm's generated state.
Do not place secrets in the sandbox home; it is intentionally persisted between
Apple Container runs.
The image also adds `/mise/shims` during shell startup, because Codex tool calls
run through fresh login shells.

## Options

Run options:

| Option                | Environment              | Default                             | Description                         |
| --------------------- | ------------------------ | ----------------------------------- | ----------------------------------- |
| `--root`              | `ROOT_DIR`               | current directory                   | Target repository root              |
| `--plan-path`         | `PLAN_PATH`              | `PLAN.md`                           | Plan file path under the root       |
| `--agents-path`       | `REPO_AGENTS_PATH`       | `AGENTS.md`                         | Agent instruction path              |
| `--start-phase`       | `START_PHASE`            | `1`                                 | First phase to run                  |
| `--end-phase`         | `END_PHASE`              | detected                            | Last phase to run                   |
| `--sleep-seconds`     | `SLEEP_SECONDS`          | `10`                                | Delay between phases                |
| `--codex-bin`         | `CODEX_BIN`              | `codex`                             | Host Codex executable               |
| `--execution-sandbox` | `LGTM_EXECUTION_SANDBOX` | `host`                              | `host` or `apple-container`         |
| `--sandbox-image`     | `LGTM_SANDBOX_IMAGE`     | `ghcr.io/yarlson/lgtm-codex:latest` | Apple Container image               |
| `--container-bin`     | `CONTAINER_BIN`          | `container`                         | Apple Container executable          |
| `--codex-auth-path`   | `CODEX_AUTH_PATH`        | `~/.codex/auth.json`                | Codex auth file for Apple Container |
| `--stream-mode`       | `STREAM_MODE`            | `pretty`                            | `pretty` or `raw`                   |
| `--log-dir`           | `LOG_DIR`                | `.lgtm/logs`                        | Log directory                       |
| `--run-stamp`         | `RUN_STAMP`              | timestamp                           | Log filename prefix                 |

Plan options:

| Option                | Environment              | Default                             | Description                         |
| --------------------- | ------------------------ | ----------------------------------- | ----------------------------------- |
| `[BRIEF]`             |                          |                                     | Optional planning brief             |
| `--root`              | `ROOT_DIR`               | current directory                   | Target repository root              |
| `--plan-path`         | `PLAN_PATH`              | `PLAN.md`                           | Plan file path under the root       |
| `--codex-bin`         | `CODEX_BIN`              | `codex`                             | Host Codex executable               |
| `--execution-sandbox` | `LGTM_EXECUTION_SANDBOX` | `host`                              | `host` or `apple-container`         |
| `--sandbox-image`     | `LGTM_SANDBOX_IMAGE`     | `ghcr.io/yarlson/lgtm-codex:latest` | Apple Container image               |
| `--container-bin`     | `CONTAINER_BIN`          | `container`                         | Apple Container executable          |
| `--codex-auth-path`   | `CODEX_AUTH_PATH`        | `~/.codex/auth.json`                | Codex auth file for Apple Container |
| `--log-dir`           | `LOG_DIR`                | `.lgtm/logs`                        | Log directory                       |
| `--run-stamp`         | `RUN_STAMP`              | timestamp                           | Log filename prefix                 |

## Safety And Logs

Before planning or running, `lgtm` checks for unmanaged `lgtm-*` skills,
ensures the target root is a Git root, and installs bundled managed skills. If
Git is not initialized, it asks before running `git init` and `git branch -M
main`.

Logs are written as JSONL:

```text
.lgtm/logs/<run-stamp>-plan-001.jsonl
.lgtm/logs/<run-stamp>-phase-01-index.jsonl
.lgtm/logs/<run-stamp>-phase-01-implement.jsonl
.lgtm/logs/<run-stamp>-phase-01-validate.jsonl
.lgtm/logs/<run-stamp>-phase-01-review.jsonl
```

Each log line records app-server protocol direction and payload. Managed skills
and logs are ignored in target repositories through:

```gitignore
.agents/skills/lgtm-*
.lgtm/
```

## Development

```bash
make check
```

Equivalent commands:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --all-targets --all-features
```

Release packaging is defined in `.github/workflows/release.yml`. After merging
the release changes, create the `v0.14.0` tag. The workflow validates that the
tag matches `Cargo.toml`, builds platform archives, publishes the GitHub
Release, pushes the arm64 Apple Container sandbox image as
`ghcr.io/yarlson/lgtm-codex:0.14.0` and
`ghcr.io/yarlson/lgtm-codex:latest`, and can update the Homebrew formula
through `scripts/update-homebrew-formula.sh`.

Release notes for v0.14.0 should announce macOS Apple Container sandboxing
support, not generic Docker sandboxing. After the release workflow is green,
verify the GitHub Release assets, Homebrew formula update, and GHCR image tags,
then smoke the published image:

```bash
container run --rm ghcr.io/yarlson/lgtm-codex:latest codex --version
```

## License

[MIT](LICENSE)
