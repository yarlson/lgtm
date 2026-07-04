"""Deterministic scorers shared by lgtm eval runners."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

from evals.common.lgtm_logs import parse_log_payloads


def read_gate_statuses(gates_dir: Path) -> dict[str, str]:
    statuses: dict[str, str] = {}
    if not gates_dir.is_dir():
        return statuses
    for path in sorted(gates_dir.glob("*.json")):
        try:
            gate = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            statuses[path.name] = "unreadable"
            continue
        status = gate.get("status")
        statuses[path.name] = status if isinstance(status, str) else "missing"
    return statuses


def committed_paths(repo_dir: Path) -> list[str]:
    completed = subprocess.run(
        ["git", "-C", str(repo_dir), "show", "--format=", "--name-only", "HEAD"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        return []
    return [line.strip() for line in completed.stdout.splitlines() if line.strip()]


def git_status_paths(repo_dir: Path) -> list[str]:
    completed = subprocess.run(
        ["git", "-C", str(repo_dir), "status", "--porcelain"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        return []
    return [line[3:] for line in completed.stdout.splitlines() if len(line) > 3]


def score_run_gate_trajectory(
    *,
    exit_code: int,
    expect_success: bool,
    stderr: str,
    expect_stderr: str,
    commit_log_exists: bool,
    expect_commit_log: bool,
    gate_statuses: dict[str, str],
    gate_file: str | None,
    gate_status: str | None,
    logs_dir: Path,
) -> dict[str, Any]:
    pass_order = observed_phase_pass_order(logs_dir)
    expected_order = expected_phase_pass_order(
        expect_commit_log=expect_commit_log,
        gate_file=gate_file,
        gate_status=gate_status,
    )
    checks = {
        "terminal_status": exit_code == 0 if expect_success else exit_code != 0,
        "expected_stderr": not expect_stderr or expect_stderr in stderr,
        "commit_pass_reached": commit_log_exists == expect_commit_log,
        "expected_gate_status": True,
        "blocking_gate_stopped_before_commit": True,
        "pass_order": pass_order == expected_order,
    }
    if gate_file and gate_status:
        checks["expected_gate_status"] = gate_statuses.get(gate_file) == gate_status
        if gate_status in {"block", "invalid"}:
            checks["blocking_gate_stopped_before_commit"] = not commit_log_exists

    failures = [name for name, passed in checks.items() if not passed]
    return {
        "passed": not failures,
        "checks": checks,
        "failures": failures,
        "pass_order": pass_order,
        "expected_pass_order": expected_order,
    }


def score_run_gate_state_diff(
    *,
    repo_dir: Path,
    exit_code: int,
    stderr: str,
    expect_success: bool,
    expect_commit_log: bool,
) -> dict[str, Any]:
    paths = committed_paths(repo_dir)
    status_paths = git_status_paths(repo_dir)
    generated_paths = [path for path in paths if path.startswith(".lgtm/")]
    checks = {
        "generated_state_rejected": not generated_paths
        or (exit_code != 0 and "generated lgtm state" in stderr),
        "blocked_or_invalid_did_not_commit_phase": True,
        "success_committed_changed_file": True,
        "success_left_clean_worktree": True,
    }
    if not expect_commit_log:
        checks["blocked_or_invalid_did_not_commit_phase"] = "changed.txt" not in paths
    if expect_success:
        checks["success_committed_changed_file"] = "changed.txt" in paths
        checks["success_left_clean_worktree"] = not status_paths

    failures = [name for name, passed in checks.items() if not passed]
    return {
        "passed": not failures,
        "checks": checks,
        "failures": failures,
        "committed_paths": paths,
        "generated_paths": generated_paths,
        "status_paths": status_paths,
    }


def observed_phase_pass_order(logs_dir: Path) -> list[str]:
    present = set()
    if not logs_dir.is_dir():
        return []
    for path in sorted(logs_dir.glob("*.jsonl")):
        name = path.name
        marker = "-phase-01-"
        if marker not in name or not name.endswith(".jsonl"):
            continue
        if not log_has_completed_turn(path):
            continue
        action = name.split(marker, 1)[1].removesuffix(".jsonl")
        present.add(action)
    return [action for action in ["index", "implement", "validate", "review", "commit"] if action in present]


def log_has_completed_turn(path: Path) -> bool:
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return False
    for line in lines:
        for payload in parse_log_payloads(line):
            if payload.get("method") == "turn/completed":
                return True
    return False


def expected_phase_pass_order(
    *,
    expect_commit_log: bool,
    gate_file: str | None,
    gate_status: str | None,
) -> list[str]:
    if expect_commit_log:
        return ["index", "implement", "validate", "review", "commit"]
    if gate_status in {"block", "invalid"} and gate_file:
        if "validate" in gate_file:
            return ["index", "implement", "validate"]
        if "review" in gate_file:
            return ["index", "implement", "validate", "review"]
    return ["index", "implement", "validate", "review"]
