#!/usr/bin/env python3
"""Run lgtm token-usage eval against released binaries."""

from __future__ import annotations

import argparse
import json
import os
import random
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from statistics import median
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_DIR = Path(__file__).resolve().parent / "fixture"
DEFAULT_DATA_ROOT = Path.home() / "lgtm-token-eval-data"


@dataclass(frozen=True)
class Binary:
    version: str
    path: Path


def main() -> int:
    args = parse_args()
    binaries = parse_binaries(args.bin)
    if len(binaries) < 2:
        raise SystemExit("provide at least two --bin VERSION PATH entries")

    eval_id = args.eval_id or datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    data_dir = args.data_root.expanduser().resolve() / eval_id
    runs_dir = data_dir / "runs"
    runs_dir.mkdir(parents=True, exist_ok=True)

    plan = build_plan(binaries, args.trials, args.seed)
    results_path = data_dir / "results.jsonl"
    print(f"eval_id={eval_id}", flush=True)
    print(f"data_dir={data_dir}", flush=True)
    print(f"runs={len(plan)}", flush=True)

    results: list[dict[str, Any]] = []
    for index, binary in enumerate(plan, start=1):
        result = run_trial(
            binary=binary,
            index=index,
            total=len(plan),
            data_dir=data_dir,
            runs_dir=runs_dir,
            timeout=args.timeout,
        )
        results.append(result)
        append_jsonl(results_path, result)
        status = "ok" if result["success"] else "fail"
        total_tokens = result["usage"]["total_tokens"]
        print(
            f"{index:03d}/{len(plan):03d} {binary.version} {status} "
            f"tokens={total_tokens} wall={result['wall_seconds']:.1f}s",
            flush=True,
        )

    write_summary(data_dir / "summary.md", results)
    print(f"summary={data_dir / 'summary.md'}", flush=True)
    return 0 if all(result["success"] for result in results) else 1


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--bin",
        action="append",
        nargs=2,
        metavar=("VERSION", "PATH"),
        required=True,
        help="lgtm binary version and path; repeat for each version",
    )
    parser.add_argument("--trials", type=int, default=5)
    parser.add_argument("--seed", type=int, default=20260530)
    parser.add_argument("--timeout", type=int, default=1800)
    parser.add_argument("--eval-id")
    parser.add_argument("--data-root", type=Path, default=DEFAULT_DATA_ROOT)
    return parser.parse_args()


def parse_binaries(raw_bins: list[list[str]]) -> list[Binary]:
    binaries = []
    for version, path in raw_bins:
        binary_path = Path(path).expanduser().resolve()
        if not binary_path.is_file():
            raise SystemExit(f"missing binary for {version}: {binary_path}")
        if not os.access(binary_path, os.X_OK):
            raise SystemExit(f"binary is not executable for {version}: {binary_path}")
        binaries.append(Binary(version=version, path=binary_path))
    return binaries


def build_plan(binaries: list[Binary], trials: int, seed: int) -> list[Binary]:
    plan = [binary for _ in range(trials) for binary in binaries]
    rng = random.Random(seed)
    rng.shuffle(plan)
    return plan


def run_trial(
    *,
    binary: Binary,
    index: int,
    total: int,
    data_dir: Path,
    runs_dir: Path,
    timeout: int,
) -> dict[str, Any]:
    run_id = f"{index:03d}-{binary.version}"
    run_dir = runs_dir / run_id
    repo_dir = run_dir / "repo"
    logs_dir = run_dir / "logs"
    run_dir.mkdir(parents=True, exist_ok=True)
    logs_dir.mkdir(parents=True, exist_ok=True)

    copy_fixture(repo_dir)
    run(["git", "init", "-b", "main"], cwd=repo_dir, timeout=60)

    start = time.monotonic()
    lgtm = run(
        [
            str(binary.path),
            "run",
            "--root",
            str(repo_dir),
            "--start-phase",
            "1",
            "--end-phase",
            "1",
            "--sleep-seconds",
            "0",
            "--stream-mode",
            "raw",
            "--log-dir",
            str(logs_dir),
            "--run-stamp",
            run_id,
        ],
        cwd=REPO_ROOT,
        timeout=timeout,
    )
    wall_seconds = time.monotonic() - start

    write_text(run_dir / "stdout.txt", lgtm.stdout)
    write_text(run_dir / "stderr.txt", lgtm.stderr)

    validation = validate_repo(repo_dir, timeout=300)
    write_json(run_dir / "validation.json", validation)

    usage = collect_usage(logs_dir)
    result = {
        "run_id": run_id,
        "version": binary.version,
        "binary": str(binary.path),
        "index": index,
        "total_runs": total,
        "success": lgtm.returncode == 0 and validation["success"],
        "lgtm_exit": lgtm.returncode,
        "wall_seconds": round(wall_seconds, 3),
        "usage": usage,
        "validation_success": validation["success"],
        "validation": validation["summary"],
        "logs_dir": str(logs_dir),
        "stdout_path": str(run_dir / "stdout.txt"),
        "stderr_path": str(run_dir / "stderr.txt"),
    }
    write_json(run_dir / "metrics.json", result)

    shutil.rmtree(repo_dir, ignore_errors=True)
    return result


def copy_fixture(repo_dir: Path) -> None:
    if repo_dir.exists():
        shutil.rmtree(repo_dir)
    repo_dir.mkdir(parents=True)
    for name in ("PLAN.md", "AGENTS.md"):
        shutil.copy2(FIXTURE_DIR / name, repo_dir / name)


def validate_repo(repo_dir: Path, timeout: int) -> dict[str, Any]:
    checks = [
        {"name": "cargo_fmt", "cmd": ["cargo", "fmt", "--all", "--check"], "expect": None},
        {"name": "cargo_test", "cmd": ["cargo", "test", "--all"], "expect": None},
        {"name": "add", "cmd": ["cargo", "run", "--quiet", "--", "add", "2", "3"], "expect": "5"},
        {"name": "sub", "cmd": ["cargo", "run", "--quiet", "--", "sub", "9", "4"], "expect": "5"},
        {"name": "invalid", "cmd": ["cargo", "run", "--quiet", "--", "mul", "2", "3"], "expect_error": True},
    ]
    results = []
    for check in checks:
        completed = run(check["cmd"], cwd=repo_dir, timeout=timeout)
        stdout = completed.stdout.strip()
        stderr = completed.stderr.strip()
        ok = completed.returncode == 0
        if check.get("expect") is not None:
            ok = ok and stdout == check["expect"]
        if check.get("expect_error"):
            ok = completed.returncode != 0 and ("usage" in stderr.lower() or "error" in stderr.lower())
        results.append(
            {
                "name": check["name"],
                "ok": ok,
                "exit": completed.returncode,
                "stdout": stdout[-2000:],
                "stderr": stderr[-2000:],
            }
        )
    return {
        "success": all(result["ok"] for result in results),
        "summary": [{key: result[key] for key in ("name", "ok", "exit")} for result in results],
        "checks": results,
    }


def collect_usage(logs_dir: Path) -> dict[str, int]:
    explicit_turn_usage: list[dict[str, int]] = []
    thread_totals: dict[str, dict[str, int]] = {}
    log_files = 0
    for path in sorted(logs_dir.glob("*.jsonl")):
        log_files += 1
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            for payload in parse_log_payloads(line):
                usage = turn_completed_usage(payload)
                if usage is not None:
                    explicit_turn_usage.append(usage)

                update = thread_token_usage(payload)
                if update is not None:
                    thread_id, thread_usage = update
                    previous = thread_totals.get(thread_id)
                    if previous is None or thread_usage["total_tokens"] >= previous["total_tokens"]:
                        thread_totals[thread_id] = thread_usage

    if thread_totals:
        total = sum_usage(thread_totals.values())
        total["usage_objects"] = len(thread_totals)
    else:
        total = sum_usage(explicit_turn_usage)
        total["usage_objects"] = len(explicit_turn_usage)
    total["log_files"] = log_files
    return total


def parse_log_payloads(line: str) -> list[dict[str, Any]]:
    try:
        payload = json.loads(line)
    except json.JSONDecodeError:
        return []
    payloads = [payload] if isinstance(payload, dict) else []
    if isinstance(payload, dict) and isinstance(payload.get("line"), str):
        try:
            inner = json.loads(payload["line"])
        except json.JSONDecodeError:
            pass
        else:
            if isinstance(inner, dict):
                payloads.append(inner)
    return payloads


def turn_completed_usage(payload: dict[str, Any]) -> dict[str, int] | None:
    if payload.get("method") != "turn/completed":
        return None
    params = payload.get("params")
    if not isinstance(params, dict):
        return None
    turn = params.get("turn")
    usage = turn.get("usage") if isinstance(turn, dict) else None
    if not isinstance(usage, dict):
        usage = params.get("usage")
    if not isinstance(usage, dict):
        return None
    normalized = normalize_usage(usage)
    return normalized if any(normalized.values()) else None


def thread_token_usage(payload: dict[str, Any]) -> tuple[str, dict[str, int]] | None:
    if payload.get("method") != "thread/tokenUsage/updated":
        return None
    params = payload.get("params")
    if not isinstance(params, dict) or not isinstance(params.get("threadId"), str):
        return None
    token_usage = params.get("tokenUsage")
    if not isinstance(token_usage, dict):
        return None
    total = token_usage.get("total")
    if not isinstance(total, dict):
        return None
    normalized = normalize_usage(total)
    return params["threadId"], normalized


def normalize_usage(usage: dict[str, Any]) -> dict[str, int]:
    return {
        "input_tokens": int_value(usage, "input_tokens", "prompt_tokens", "inputTokens"),
        "cached_input_tokens": cached_tokens(usage),
        "output_tokens": int_value(usage, "output_tokens", "completion_tokens", "outputTokens"),
        "reasoning_tokens": reasoning_tokens(usage),
        "total_tokens": int_value(usage, "total_tokens", "totalTokens"),
    }


def sum_usage(usages: Any) -> dict[str, int]:
    total = {
        "input_tokens": 0,
        "cached_input_tokens": 0,
        "output_tokens": 0,
        "reasoning_tokens": 0,
        "total_tokens": 0,
    }
    for usage in usages:
        for key in total:
            total[key] += usage[key]
    return total


def cached_tokens(usage: dict[str, Any]) -> int:
    direct = int_value(usage, "cached_input_tokens", "cachedInputTokens")
    details = nested_int(usage, "input_tokens_details", "cached_tokens")
    prompt_details = nested_int(usage, "prompt_tokens_details", "cached_tokens")
    cache_read = int_value(usage, "input_tokens_cache_read")
    return direct + details + prompt_details + cache_read


def reasoning_tokens(usage: dict[str, Any]) -> int:
    direct = int_value(usage, "reasoning_tokens", "reasoningTokens", "reasoningOutputTokens")
    output_details = nested_int(usage, "output_tokens_details", "reasoning_tokens")
    completion_details = nested_int(usage, "completion_tokens_details", "reasoning_tokens")
    return direct + output_details + completion_details


def int_value(usage: dict[str, Any], *keys: str) -> int:
    for key in keys:
        value = usage.get(key)
        if isinstance(value, int):
            return value
    return 0


def nested_int(usage: dict[str, Any], parent: str, child: str) -> int:
    value = usage.get(parent)
    if isinstance(value, dict) and isinstance(value.get(child), int):
        return value[child]
    return 0


def run(cmd: list[str], *, cwd: Path, timeout: int) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            cmd,
            cwd=cwd,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        return subprocess.CompletedProcess(
            cmd,
            returncode=124,
            stdout=error.stdout or "",
            stderr=error.stderr or f"timed out after {timeout}s",
        )


def append_jsonl(path: Path, value: dict[str, Any]) -> None:
    with path.open("a", encoding="utf-8") as file:
        file.write(json.dumps(value, sort_keys=True) + "\n")


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_text(path: Path, value: str) -> None:
    path.write_text(value, encoding="utf-8", errors="replace")


def write_summary(path: Path, results: list[dict[str, Any]]) -> None:
    lines = [
        "# lgtm Token Usage Eval",
        "",
        f"Runs: {len(results)}",
        "",
        "| version | runs | success | median total | p75 total | median input | median cached | median output | median reasoning | median wall sec |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for version in sorted({result["version"] for result in results}):
        group = [result for result in results if result["version"] == version]
        successful = [result for result in group if result["success"]]
        usage_group = successful or group
        lines.append(
            "| {version} | {runs} | {success}/{runs} | {total} | {p75} | {input} | {cached} | {output} | {reasoning} | {wall} |".format(
                version=version,
                runs=len(group),
                success=len(successful),
                total=metric_median(usage_group, "total_tokens"),
                p75=metric_p75(usage_group, "total_tokens"),
                input=metric_median(usage_group, "input_tokens"),
                cached=metric_median(usage_group, "cached_input_tokens"),
                output=metric_median(usage_group, "output_tokens"),
                reasoning=metric_median(usage_group, "reasoning_tokens"),
                wall=round(median([result["wall_seconds"] for result in usage_group]), 1),
            )
        )

    versions = sorted({result["version"] for result in results})
    if len(versions) == 2:
        base, candidate = versions
        base_total = metric_median([r for r in results if r["version"] == base and r["success"]], "total_tokens")
        candidate_total = metric_median([r for r in results if r["version"] == candidate and r["success"]], "total_tokens")
        if base_total and candidate_total:
            delta = round((candidate_total - base_total) / base_total * 100, 1)
            lines.extend(["", f"Total-token median delta {candidate} vs {base}: {delta}%"])

    lines.extend(["", "Failed runs:", ""])
    failed = [result for result in results if not result["success"]]
    if failed:
        for result in failed:
            lines.append(f"- {result['run_id']}: lgtm_exit={result['lgtm_exit']} validation={result['validation_success']}")
    else:
        lines.append("- none")
    lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def metric_median(results: list[dict[str, Any]], key: str) -> int:
    values = [result["usage"][key] for result in results]
    return int(median(values)) if values else 0


def metric_p75(results: list[dict[str, Any]], key: str) -> int:
    values = sorted(result["usage"][key] for result in results)
    if not values:
        return 0
    index = int(round((len(values) - 1) * 0.75))
    return values[index]


if __name__ == "__main__":
    sys.exit(main())
