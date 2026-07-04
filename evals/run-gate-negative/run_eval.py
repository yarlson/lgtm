#!/usr/bin/env python3
"""Evaluate deterministic lgtm run gate failures."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import stat
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT))

from evals.common.lgtm_logs import collect_usage
from evals.common.scoring import (
    read_gate_statuses,
    score_run_gate_state_diff,
    score_run_gate_trajectory,
)

DEFAULT_DATA_ROOT = Path.home() / "lgtm-run-gate-negative-eval-data"
DEFAULT_LGTM_BIN = REPO_ROOT / "target/debug/lgtm"
PROMPT_DIR = REPO_ROOT / "evals" / "prompts"
PASS_VERDICT = (
    'LGTM_VERDICT: {"schema_version":1,"status":"pass","summary":"passed",'
    '"checks":["fake check"],"fixes":[],"blockers":[],"out_of_scope":[]}'
)


@dataclass(frozen=True)
class GateCase:
    name: str
    validate_text: str
    review_text: str
    expect_success: bool
    expect_stderr: str
    expect_commit_log: bool
    gate_file: str | None
    gate_status: str | None
    implement_change: bool = True
    commit_behavior: str = "normal"


CASES = {
    "validate-block": GateCase(
        name="validate-block",
        validate_text=(
            'validation blocked\nLGTM_VERDICT: {"schema_version":1,"status":"block",'
            '"summary":"blocked","checks":[],"fixes":[],"blockers":["validation failed"],'
            '"out_of_scope":[]}'
        ),
        review_text=f"review passed\n{PASS_VERDICT}",
        expect_success=False,
        expect_stderr="Phase 1 validation blocked",
        expect_commit_log=False,
        gate_file="eval-phase-01-validate.json",
        gate_status="block",
    ),
    "validate-missing-verdict": GateCase(
        name="validate-missing-verdict",
        validate_text="validation passed without marker",
        review_text=f"review passed\n{PASS_VERDICT}",
        expect_success=False,
        expect_stderr="Phase 1 validation verdict is invalid",
        expect_commit_log=False,
        gate_file="eval-phase-01-validate.json",
        gate_status="invalid",
    ),
    "validate-malformed-verdict": GateCase(
        name="validate-malformed-verdict",
        validate_text='validation malformed\nLGTM_VERDICT: {"schema_version":1,"status":',
        review_text=f"review passed\n{PASS_VERDICT}",
        expect_success=False,
        expect_stderr="Phase 1 validation verdict is invalid",
        expect_commit_log=False,
        gate_file="eval-phase-01-validate.json",
        gate_status="invalid",
    ),
    "review-block": GateCase(
        name="review-block",
        validate_text=f"validation passed\n{PASS_VERDICT}",
        review_text=(
            'review blocked\nLGTM_VERDICT: {"schema_version":1,"status":"block",'
            '"summary":"blocked","checks":[],"fixes":[],"blockers":["review failed"],'
            '"out_of_scope":[]}'
        ),
        expect_success=False,
        expect_stderr="Phase 1 review blocked",
        expect_commit_log=False,
        gate_file="eval-phase-01-review.json",
        gate_status="block",
    ),
    "review-missing-verdict": GateCase(
        name="review-missing-verdict",
        validate_text=f"validation passed\n{PASS_VERDICT}",
        review_text="review passed without marker",
        expect_success=False,
        expect_stderr="Phase 1 review verdict is invalid",
        expect_commit_log=False,
        gate_file="eval-phase-01-review.json",
        gate_status="invalid",
    ),
    "review-malformed-verdict": GateCase(
        name="review-malformed-verdict",
        validate_text=f"validation passed\n{PASS_VERDICT}",
        review_text='review malformed\nLGTM_VERDICT: {"schema_version":1,"status":',
        expect_success=False,
        expect_stderr="Phase 1 review verdict is invalid",
        expect_commit_log=False,
        gate_file="eval-phase-01-review.json",
        gate_status="invalid",
    ),
    "commit-missing": GateCase(
        name="commit-missing",
        validate_text=f"validation passed\n{PASS_VERDICT}",
        review_text=f"review passed\n{PASS_VERDICT}",
        expect_success=False,
        expect_stderr="Phase 1 commit did not create a new git commit",
        expect_commit_log=True,
        gate_file="eval-phase-01-review.json",
        gate_status="pass",
        commit_behavior="skip",
    ),
    "generated-state-commit": GateCase(
        name="generated-state-commit",
        validate_text=f"validation passed\n{PASS_VERDICT}",
        review_text=f"review passed\n{PASS_VERDICT}",
        expect_success=False,
        expect_stderr="Phase 1 commit included generated lgtm state",
        expect_commit_log=True,
        gate_file="eval-phase-01-review.json",
        gate_status="pass",
        commit_behavior="generated-state",
    ),
    "pass-control": GateCase(
        name="pass-control",
        validate_text=f"validation passed\n{PASS_VERDICT}",
        review_text=f"review passed\n{PASS_VERDICT}",
        expect_success=True,
        expect_stderr="",
        expect_commit_log=True,
        gate_file="eval-phase-01-review.json",
        gate_status="pass",
    ),
}


def main() -> int:
    args = parse_args()
    if args.judge_prompts:
        require_judge_eval()
        validate_prompt_files()
        if args.dry_run:
            print("judge prompt dry-run passed", flush=True)
            return 0
        raise SystemExit("judge prompt execution is not implemented; use --dry-run")
    selected = resolve_cases(args.case, include_pass_control=args.include_pass_control)
    eval_id = args.eval_id or datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    data_dir = args.data_root.expanduser().resolve() / eval_id
    data_dir.mkdir(parents=True, exist_ok=True)

    results = []
    for iteration in range(1, args.iterations + 1):
        for case in selected:
            result = run_case(
                case=case,
                iteration=iteration,
                data_dir=data_dir,
                lgtm_bin=args.lgtm_bin,
                timeout=args.timeout,
                stream_mode=args.stream_mode,
                score_trajectory=not args.no_score_trajectory,
                score_state_diff=not args.no_score_state_diff,
            )
            results.append(result)
            append_jsonl(data_dir / "results.jsonl", result)
            print_result(result)

    write_summary(data_dir / "summary.md", results)
    print(f"summary={data_dir / 'summary.md'}", flush=True)
    return 0 if all(result["passed"] for result in results) else 1


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--case",
        action="append",
        default=[],
        help="case name; default: all negative cases",
    )
    parser.add_argument("--iterations", type=int, default=1)
    parser.add_argument("--timeout", type=int, default=60)
    parser.add_argument("--stream-mode", choices=["pretty", "raw"], default="pretty")
    parser.add_argument(
        "--lgtm-bin",
        type=Path,
        default=DEFAULT_LGTM_BIN,
        help="built lgtm binary used for eval runs",
    )
    parser.add_argument("--eval-id")
    parser.add_argument("--data-root", type=Path, default=DEFAULT_DATA_ROOT)
    parser.add_argument("--include-pass-control", action="store_true")
    parser.add_argument(
        "--no-score-trajectory",
        action="store_true",
        help="disable the default structural trajectory scorer",
    )
    parser.add_argument(
        "--no-score-state-diff",
        action="store_true",
        help="disable the default git state-diff scorer",
    )
    parser.add_argument("--score-trajectory", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--score-state-diff", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--judge-prompts", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args()


def require_judge_eval() -> None:
    if os.environ.get("LGTM_LIVE_EVAL_JUDGE") == "1":
        return
    raise SystemExit("judge prompt eval requires LGTM_LIVE_EVAL_JUDGE=1")


def validate_prompt_files() -> None:
    required = [
        "trajectory_judge.md",
        "state_diff_judge.md",
        "diff_vs_plan_judge.md",
        "guardrail_judge.md",
    ]
    missing = []
    empty = []
    for name in required:
        path = PROMPT_DIR / name
        if not path.is_file():
            missing.append(str(path))
        elif not path.read_text(encoding="utf-8").strip():
            empty.append(str(path))
    if missing or empty:
        details = []
        if missing:
            details.append(f"missing: {', '.join(missing)}")
        if empty:
            details.append(f"empty: {', '.join(empty)}")
        raise SystemExit("; ".join(details))


def resolve_cases(names: list[str], *, include_pass_control: bool) -> list[GateCase]:
    if names:
        selected = []
        for name in names:
            if name not in CASES:
                raise SystemExit(f"unknown case {name!r}; available: {', '.join(CASES)}")
            selected.append(CASES[name])
        if include_pass_control and CASES["pass-control"] not in selected:
            selected.append(CASES["pass-control"])
        return selected

    selected = [case for case in CASES.values() if case.name != "pass-control"]
    if include_pass_control:
        selected.append(CASES["pass-control"])
    return selected


def run_case(
    *,
    case: GateCase,
    iteration: int,
    data_dir: Path,
    lgtm_bin: Path,
    timeout: int,
    stream_mode: str,
    score_trajectory: bool,
    score_state_diff: bool,
) -> dict[str, Any]:
    if not lgtm_bin.is_file():
        raise SystemExit(
            f"lgtm binary not found at {lgtm_bin}; run `cargo build` or pass --lgtm-bin"
        )

    run_id = f"{iteration:02d}-{case.name}"
    run_dir = data_dir / "runs" / run_id
    repo_dir = run_dir / "repo"
    if run_dir.exists():
        shutil.rmtree(run_dir)
    run_dir.mkdir(parents=True)
    prepare_repo(repo_dir)
    fake_codex = write_fake_codex(run_dir, repo_dir, case)

    cmd = [
        str(lgtm_bin),
        "run",
        "--root",
        str(repo_dir),
        "--end-phase",
        "1",
        "--sleep-seconds",
        "0",
        "--codex-bin",
        str(fake_codex),
        "--run-stamp",
        "eval",
        "--stream-mode",
        stream_mode,
    ]
    try:
        completed = subprocess.run(
            cmd,
            text=True,
            capture_output=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        stdout = error.stdout if isinstance(error.stdout, str) else ""
        stderr = error.stderr if isinstance(error.stderr, str) else ""
        completed = subprocess.CompletedProcess(cmd, 124, stdout, stderr + "\ntimed out\n")

    (run_dir / "stdout.txt").write_text(completed.stdout, encoding="utf-8", errors="replace")
    (run_dir / "stderr.txt").write_text(completed.stderr, encoding="utf-8", errors="replace")
    gates_dir = repo_dir / ".lgtm/gates"
    if gates_dir.is_dir():
        shutil.copytree(gates_dir, run_dir / "gates")
    logs_dir = repo_dir / ".lgtm/logs"
    if logs_dir.is_dir():
        shutil.copytree(logs_dir, run_dir / "logs")

    gate_statuses = read_gate_statuses(gates_dir)
    checks = evaluate_case(case, repo_dir, completed, gate_statuses)
    optional_scores: dict[str, Any] = {}
    if score_trajectory:
        optional_scores["trajectory"] = score_run_gate_trajectory(
            exit_code=completed.returncode,
            expect_success=case.expect_success,
            stderr=completed.stderr,
            expect_stderr=case.expect_stderr,
            commit_log_exists=(repo_dir / ".lgtm/logs/eval-phase-01-commit.jsonl").exists(),
            expect_commit_log=case.expect_commit_log,
            gate_statuses=gate_statuses,
            gate_file=case.gate_file,
            gate_status=case.gate_status,
            logs_dir=logs_dir,
        )
    if score_state_diff:
        optional_scores["state_diff"] = score_run_gate_state_diff(
            repo_dir=repo_dir,
            exit_code=completed.returncode,
            stderr=completed.stderr,
            expect_success=case.expect_success,
            expect_commit_log=case.expect_commit_log,
        )
    logs_copy = run_dir / "logs"
    usage = collect_usage(logs_copy) if logs_copy.is_dir() else None
    optional_passed = all(score["passed"] for score in optional_scores.values())
    result = {
        "run_id": run_id,
        "case": case.name,
        "iteration": iteration,
        "exit_code": completed.returncode,
        "passed": all(checks.values()) and optional_passed,
        "checks": checks,
        "stdout_path": str(run_dir / "stdout.txt"),
        "stderr_path": str(run_dir / "stderr.txt"),
    }
    if optional_scores:
        result["scores"] = optional_scores
    if usage is not None:
        result["usage"] = usage
    gates_copy = run_dir / "gates"
    if gates_copy.is_dir():
        result["gates_path"] = str(gates_copy)
    if logs_copy.is_dir():
        result["logs_path"] = str(logs_copy)
    write_json(run_dir / "metrics.json", result)
    return result


def evaluate_case(
    case: GateCase,
    repo_dir: Path,
    completed: subprocess.CompletedProcess[str],
    gate_statuses: dict[str, str],
) -> dict[str, bool]:
    checks = {
        "exit_code": completed.returncode == 0
        if case.expect_success
        else completed.returncode != 0,
        "stderr": case.expect_stderr in completed.stderr,
        "commit_log": (repo_dir / ".lgtm/logs/eval-phase-01-commit.jsonl").exists()
        == case.expect_commit_log,
    }
    if case.gate_file and case.gate_status:
        checks["gate_artifact"] = gate_statuses.get(case.gate_file) == case.gate_status
    return checks


def prepare_repo(repo_dir: Path) -> None:
    repo_dir.mkdir(parents=True)
    (repo_dir / "PLAN.md").write_text(valid_plan(), encoding="utf-8")
    (repo_dir / "AGENTS.md").write_text("# Agents\n", encoding="utf-8")
    subprocess.run(["git", "init", "-b", "main"], cwd=repo_dir, check=True)
    subprocess.run(["git", "add", "PLAN.md", "AGENTS.md"], cwd=repo_dir, check=True)
    subprocess.run(
        [
            "git",
            "-c",
            "user.name=lgtm eval",
            "-c",
            "user.email=lgtm@example.com",
            "commit",
            "-m",
            "chore: baseline fixture",
        ],
        cwd=repo_dir,
        check=True,
        stdout=subprocess.DEVNULL,
    )


def valid_plan() -> str:
    return """# Plan

## Decisions

- Test run gate.

## Non-Goals

- None.

## Open Risks

- None.

## Loopholes To Close

- None.

## Phase 1 - Gate

Goal:
Exercise gate behavior.

Deliverables:
- Implement a changed file.

Dependencies:
- None.

Unresolved decisions:
- None.

Steps:
- Write a fixture file.

Validation:
- Check the fixture file.
"""


def write_fake_codex(run_dir: Path, repo_dir: Path, case: GateCase) -> Path:
    script = run_dir / "codex"
    validate_text = json.dumps(case.validate_text)
    review_text = json.dumps(case.review_text)
    implement_change = "1" if case.implement_change else "0"
    commit_behavior = case.commit_behavior
    repo = shell_quote(repo_dir)
    script.write_text(
        f"""#!/usr/bin/env sh
set -eu
repo={repo}
dir=$(dirname "$0")

read initialize
printf '%s\\n' '{{"id":1,"result":{{"userAgent":"fake","codexHome":"/tmp/codex"}}}}'
read initialized
read thread_start
printf '%s\\n' '{{"id":2,"result":{{"thread":{{"id":"thr-eval"}}}}}}'
while IFS= read -r turn_start; do
  turn_counter="$dir/turn-counter"
  if [ -f "$turn_counter" ]; then
    turn_n=$(cat "$turn_counter")
  else
    turn_n=0
  fi
  turn_n=$((turn_n + 1))
  printf '%s\\n' "$turn_n" >"$turn_counter"
  id=$(printf '%s\\n' "$turn_start" | sed -n 's/.*"id":\\([0-9][0-9]*\\).*/\\1/p')
  printf '{{"id":%s,"result":{{"turn":{{"id":"turn-eval","status":"inProgress","items":[]}}}}}}\\n' "$id"
  if [ "$turn_n" = 1 ]; then
    printf '%s\\n' '{{"method":"turn/completed","params":{{"threadId":"thr-eval","turn":{{"id":"turn-eval","status":"completed","items":[{{"type":"agentMessage","id":"msg-index","text":"{{\\"phases\\":[{{\\"id\\":1,\\"title\\":\\"Gate\\",\\"heading\\":\\"## Phase 1 - Gate\\"}}]}}","status":"completed"}}]}}}}}}'
  elif [ "$turn_n" = 2 ]; then
    if [ {json.dumps(implement_change)} = "1" ]; then
      printf '%s\\n' "changed" >"$repo/changed.txt"
    fi
    printf '%s\\n' '{{"method":"turn/completed","params":{{"threadId":"thr-eval","turn":{{"id":"turn-eval","status":"completed","items":[{{"type":"agentMessage","id":"msg-implement","text":"implemented","status":"completed"}}]}}}}}}'
  elif [ "$turn_n" = 3 ]; then
    printf '%s\\n' '{{"method":"turn/completed","params":{{"threadId":"thr-eval","turn":{{"id":"turn-eval","status":"completed","items":[{{"type":"agentMessage","id":"msg-validate","text":{validate_text},"status":"completed"}}]}}}}}}'
  elif [ "$turn_n" = 4 ]; then
    printf '%s\\n' '{{"method":"turn/completed","params":{{"threadId":"thr-eval","turn":{{"id":"turn-eval","status":"completed","items":[{{"type":"agentMessage","id":"msg-review","text":{review_text},"status":"completed"}}]}}}}}}'
  else
    if [ {json.dumps(commit_behavior)} = "skip" ]; then
      :
    elif [ {json.dumps(commit_behavior)} = "generated-state" ]; then
      mkdir -p "$repo/.lgtm"
      printf '%s\\n' "generated" >"$repo/.lgtm/forced-generated.txt"
      git -C "$repo" add -A
      git -C "$repo" add -f .lgtm/forced-generated.txt
      git -C "$repo" -c user.name='lgtm eval' -c user.email='lgtm@example.com' commit -m 'feat: commit generated state' >/dev/null
    elif [ -n "$(git -C "$repo" status --porcelain)" ]; then
      git -C "$repo" add -A
      git -C "$repo" -c user.name='lgtm eval' -c user.email='lgtm@example.com' commit -m 'feat: commit phase' >/dev/null
    fi
    printf '%s\\n' '{{"method":"turn/completed","params":{{"threadId":"thr-eval","turn":{{"id":"turn-eval","status":"completed","items":[{{"type":"agentMessage","id":"msg-commit","text":"committed","status":"completed"}}]}}}}}}'
  fi
done
""",
        encoding="utf-8",
    )
    script.chmod(script.stat().st_mode | stat.S_IXUSR)
    return script


def print_result(result: dict[str, Any]) -> None:
    status = "pass" if result["passed"] else "fail"
    failed = [name for name, passed in result["checks"].items() if not passed]
    for score_name, score in result.get("scores", {}).items():
        if not score["passed"]:
            failed.append(score_name)
    suffix = "" if not failed else f" failed={','.join(failed)}"
    print(f"{result['run_id']} {status}{suffix}", flush=True)


def write_summary(path: Path, results: list[dict[str, Any]]) -> None:
    lines = ["# lgtm-run-gate-negative Eval Summary", ""]
    for result in results:
        lines.extend(
            [
                f"## {result['run_id']}",
                "",
                f"- Status: {'pass' if result['passed'] else 'fail'}",
                f"- Exit code: {result['exit_code']}",
                f"- Stdout: `{result['stdout_path']}`",
                f"- Stderr: `{result['stderr_path']}`",
                "",
            ]
        )
        failed = [name for name, passed in result["checks"].items() if not passed]
        if failed:
            lines.append("Failed checks:")
            lines.extend(f"- {name}" for name in failed)
            lines.append("")
        for score_name, score in result.get("scores", {}).items():
            lines.extend(
                [
                    f"{score_name} score:",
                    f"- Status: {'pass' if score['passed'] else 'fail'}",
                    f"- Failures: {', '.join(score['failures']) if score['failures'] else 'none'}",
                    "",
                ]
            )
    path.write_text("\n".join(lines), encoding="utf-8")


def append_jsonl(path: Path, value: dict[str, Any]) -> None:
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(value, sort_keys=True) + "\n")


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def shell_quote(path: Path) -> str:
    return "'" + str(path).replace("'", "'\\''") + "'"


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit(130)
