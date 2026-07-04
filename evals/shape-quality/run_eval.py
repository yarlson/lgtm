#!/usr/bin/env python3
"""Evaluate lgtm shape output quality.

The eval executes the real shape workflow when requested, but reuses the
plan-create scorer so both planning paths are judged against the same artifact
quality bar.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT))

from evals.common.lgtm_logs import collect_usage

EVAL_ROOT = Path(__file__).resolve().parent
DEFAULT_DATA_ROOT = Path.home() / "lgtm-shape-quality-eval-data"
DEFAULT_LGTM_BIN = REPO_ROOT / "target/debug/lgtm"


@dataclass(frozen=True)
class EvalCase:
    name: str
    brief_path: Path


def main() -> int:
    args = parse_args()
    if args.expect_fail and args.score_only is None:
        raise SystemExit("--expect-fail is only supported with --score-only")
    if args.score_only is None:
        require_live_eval()
    plan_create = load_plan_create_eval()
    cases = resolve_cases(args.case)
    eval_id = args.eval_id or datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    data_dir = args.data_root.expanduser().resolve() / eval_id
    data_dir.mkdir(parents=True, exist_ok=True)

    results = []
    for iteration in range(1, args.iterations + 1):
        for case in cases:
            result = run_case(
                case=case,
                iteration=iteration,
                data_dir=data_dir,
                codex_bin=args.codex_bin,
                lgtm_bin=args.lgtm_bin,
                timeout=args.timeout,
                max_rounds=args.max_rounds,
                score_only=args.score_only,
                min_score=args.min_score,
                score_plan=plan_create.score_plan,
            )
            result["expected_failure"] = args.expect_fail
            result["expectation_passed"] = (
                not result["score"]["passed"] if args.expect_fail else result["score"]["passed"]
            )
            results.append(result)
            append_jsonl(data_dir / "results.jsonl", result)
            print_result(result, min_score=args.min_score)

    write_summary(data_dir / "summary.md", results, args.min_score)
    print(f"summary={data_dir / 'summary.md'}", flush=True)
    passed = all(result["score"]["passed"] for result in results)
    if args.expect_fail:
        passed = all(not result["score"]["passed"] for result in results)
    return 0 if passed else 1


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--case",
        action="append",
        default=[],
        help="brief case name from evals/shape-quality/briefs; default: all",
    )
    parser.add_argument("--iterations", type=int, default=1)
    parser.add_argument("--min-score", type=int, default=85)
    parser.add_argument("--timeout", type=int, default=1200)
    parser.add_argument("--max-rounds", type=int, default=200)
    parser.add_argument("--codex-bin", default="codex")
    parser.add_argument(
        "--lgtm-bin",
        type=Path,
        default=DEFAULT_LGTM_BIN,
        help="built lgtm binary used for generated eval runs",
    )
    parser.add_argument("--eval-id")
    parser.add_argument("--data-root", type=Path, default=DEFAULT_DATA_ROOT)
    parser.add_argument(
        "--score-only",
        type=Path,
        help="score an existing PLAN.md instead of generating with Codex",
    )
    parser.add_argument(
        "--expect-fail",
        action="store_true",
        help="invert success for weak score-only controls that should fail",
    )
    return parser.parse_args()


def require_live_eval() -> None:
    if os.environ.get("LGTM_LIVE_EVAL") == "1":
        return
    raise SystemExit(
        "live shape-quality eval requires LGTM_LIVE_EVAL=1; "
        "use --score-only for deterministic controls"
    )


def load_plan_create_eval() -> Any:
    module_path = REPO_ROOT / "evals" / "plan-create" / "run_eval.py"
    spec = importlib.util.spec_from_file_location("lgtm_plan_create_eval", module_path)
    if spec is None or spec.loader is None:
        raise SystemExit(f"failed to load scorer from {module_path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def resolve_cases(names: list[str]) -> list[EvalCase]:
    brief_dir = EVAL_ROOT / "briefs"
    paths = sorted(brief_dir.glob("*.md"))
    cases = {path.stem: EvalCase(path.stem, path) for path in paths}
    if not names:
        return list(cases.values())
    selected = []
    for name in names:
        if name not in cases:
            raise SystemExit(f"unknown case {name!r}; available: {', '.join(cases)}")
        selected.append(cases[name])
    return selected


def run_case(
    *,
    case: EvalCase,
    iteration: int,
    data_dir: Path,
    codex_bin: str,
    lgtm_bin: Path,
    timeout: int,
    max_rounds: int,
    score_only: Path | None,
    min_score: int,
    score_plan: Any,
) -> dict[str, Any]:
    run_id = f"{iteration:02d}-{case.name}"
    run_dir = data_dir / "runs" / run_id
    repo_dir = run_dir / "repo"
    run_dir.mkdir(parents=True, exist_ok=True)

    if score_only:
        plan_text = score_only.read_text(encoding="utf-8")
        stdout = ""
        stderr = ""
        exit_code = 0
        wall_seconds = 0.0
        usage = None
    else:
        prepare_repo(repo_dir)
        brief = case.brief_path.read_text(encoding="utf-8")
        brief_path = run_dir / "brief.md"
        brief_path.write_text(brief, encoding="utf-8")
        start = time.monotonic()
        completed = run_lgtm_shape(
            repo_dir=repo_dir,
            codex_bin=codex_bin,
            lgtm_bin=lgtm_bin,
            brief_path=brief_path,
            run_id=run_id,
            timeout=timeout,
            max_rounds=max_rounds,
        )
        wall_seconds = time.monotonic() - start
        stdout = completed.stdout
        stderr = completed.stderr
        exit_code = completed.returncode
        plan_path = repo_dir / "PLAN.md"
        plan_text = plan_path.read_text(encoding="utf-8") if plan_path.is_file() else ""
        logs_dir = repo_dir / ".lgtm/logs"
        if logs_dir.is_dir():
            shutil.copytree(logs_dir, run_dir / "logs")
        logs_copy = run_dir / "logs"
        usage = collect_usage(logs_copy) if logs_copy.is_dir() else None

    (run_dir / "stdout.txt").write_text(stdout, encoding="utf-8", errors="replace")
    (run_dir / "stderr.txt").write_text(stderr, encoding="utf-8", errors="replace")
    (run_dir / "PLAN.md").write_text(plan_text, encoding="utf-8")
    score = score_plan(plan_text, case_name=case.name, min_score=min_score)
    result = {
        "run_id": run_id,
        "case": case.name,
        "iteration": iteration,
        "success": exit_code == 0,
        "exit_code": exit_code,
        "wall_seconds": round(wall_seconds, 3),
        "score": score,
        "plan_path": str(run_dir / "PLAN.md"),
        "stdout_path": str(run_dir / "stdout.txt"),
        "stderr_path": str(run_dir / "stderr.txt"),
    }
    if usage is not None:
        result["usage"] = usage
    logs_copy = run_dir / "logs"
    if logs_copy.is_dir():
        result["logs_path"] = str(logs_copy)
    write_json(run_dir / "metrics.json", result)
    return result


def prepare_repo(repo_dir: Path) -> None:
    if repo_dir.exists():
        shutil.rmtree(repo_dir)
    repo_dir.mkdir(parents=True)
    (repo_dir / "AGENTS.md").write_text(
        "# AGENTS.md\n\n"
        "- Create planning artifacts only.\n"
        "- Do not implement code, commit, push, or manage CI.\n",
        encoding="utf-8",
    )
    subprocess.run(["git", "init", "-b", "main"], cwd=repo_dir, check=True)


def run_lgtm_shape(
    *,
    repo_dir: Path,
    codex_bin: str,
    lgtm_bin: Path,
    brief_path: Path,
    run_id: str,
    timeout: int,
    max_rounds: int,
) -> subprocess.CompletedProcess[str]:
    if not lgtm_bin.is_file():
        raise SystemExit(
            f"lgtm binary not found at {lgtm_bin}; run `cargo build` or pass --lgtm-bin"
        )

    cmd = [
        str(lgtm_bin),
        "shape",
        "--brief-file",
        str(brief_path),
        "--root",
        str(repo_dir),
        "--codex-bin",
        codex_bin,
        "--log-dir",
        ".lgtm/logs",
        "--run-stamp",
        run_id,
        "--max-rounds",
        str(max_rounds),
    ]
    try:
        return subprocess.run(
            cmd,
            text=True,
            capture_output=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        stdout = error.stdout if isinstance(error.stdout, str) else ""
        stderr = error.stderr if isinstance(error.stderr, str) else ""
        stderr += "\ntimed out waiting for lgtm shape\n"
        return subprocess.CompletedProcess(cmd, 124, stdout, stderr)


def print_result(result: dict[str, Any], *, min_score: int) -> None:
    score = result["score"]
    status = "pass" if result.get("expectation_passed", score["passed"]) else "fail"
    score_status = "scorer-pass" if score["passed"] else "scorer-fail"
    expectation = " expected-fail" if result.get("expected_failure") else ""
    print(
        f"{result['run_id']} {status}{expectation} {score_status} "
        f"score={score['value']}/{min_score} "
        f"phases={score['checks']['phase_count']} "
        f"domains={len(score['checks']['domain_coverage'])}"
        f"{usage_summary(result)}",
        flush=True,
    )
    for blocker in score["blockers"]:
        print(f"  blocker: {blocker}", flush=True)


def write_summary(path: Path, results: list[dict[str, Any]], min_score: int) -> None:
    lines = ["# lgtm-shape-quality Eval Summary", ""]
    for result in results:
        score = result["score"]
        lines.extend(
            [
                f"## {result['run_id']}",
                "",
                f"- Status: {'pass' if score['passed'] else 'fail'}",
                f"- Expectation status: {'pass' if result.get('expectation_passed', score['passed']) else 'fail'}",
                f"- Score: {score['value']}/{min_score}",
                f"- Phases: {score['checks']['phase_count']}",
                f"- Domains: {len(score['checks']['domain_coverage'])}",
                f"- Plan: `{result['plan_path']}`",
                "",
            ]
        )
        usage = result.get("usage")
        if isinstance(usage, dict):
            lines.extend(
                [
                    f"- Total tokens: {usage.get('total_tokens', 0)}",
                    f"- Usage objects: {usage.get('usage_objects', 0)}",
                    "",
                ]
            )
        if score["blockers"]:
            lines.append("Blockers:")
            lines.extend(f"- {blocker}" for blocker in score["blockers"])
            lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def append_jsonl(path: Path, value: dict[str, Any]) -> None:
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(value, sort_keys=True) + "\n")


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def usage_summary(result: dict[str, Any]) -> str:
    usage = result.get("usage")
    if not isinstance(usage, dict):
        return ""
    return f" tokens={usage.get('total_tokens', 0)}"


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit(130)
