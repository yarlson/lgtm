#!/usr/bin/env python3
"""Evaluate lgtm planning prompt output quality.

The eval generates a real PLAN.md with Codex, scores the artifact, and stores
all run data outside the repo by default. It is intentionally about output
quality, not token usage or transport behavior.
"""

from __future__ import annotations

import argparse
import json
import os
import pty
import re
import select
import signal
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

from evals.common.lgtm_logs import parse_log_payloads

EVAL_ROOT = Path(__file__).resolve().parent
DEFAULT_DATA_ROOT = Path.home() / "lgtm-plan-create-eval-data"
DEFAULT_LGTM_BIN = REPO_ROOT / "target/debug/lgtm"
CASE_WORKSTREAMS = {
    "kargo-job-system": {
        "manifest_schema": (
            ".kargo.yml",
            "manifest",
            "schema",
            "parser",
            "diagnostic",
        ),
        "legacy_compatibility": (
            "jenkins",
            "legacy",
            "unsupported",
            "compatibility",
            "migration diagnostic",
        ),
        "policy_security": (
            "policy",
            "authorization",
            "allowlist",
            "trust",
            "secret",
        ),
        "persistence_model": (
            "mongo",
            "collection",
            "index",
            "persistence",
            "retention",
        ),
        "scheduler_state_machine": (
            "scheduler",
            "state machine",
            "lease",
            "ready",
            "retry",
            "cancel",
        ),
        "agent_protocol": (
            "protocol",
            "websocket",
            "rpc",
            "dispatch",
            "heartbeat",
            "ack",
        ),
        "agent_runtime": (
            "agent",
            "runner",
            "runtime",
            "workspace",
            "sandbox",
            "execution",
        ),
        "logs_artifacts_checks": (
            "log",
            "artifact",
            "check",
            "blob",
            "status",
        ),
        "dashboard_api": (
            "dashboard",
            "api",
            "operator",
            "rerun",
            "cancel",
            "diagnostic",
        ),
        "shadow_rollout": (
            "shadow",
            "fallback",
            "rollout",
            "feature flag",
            "enable",
        ),
        "jenkins_removal": (
            "jenkins removal",
            "remove jenkins",
            "cutover",
            "migration",
            "cleanup",
        ),
        "end_to_end_readiness": (
            "end-to-end",
            "e2e",
            "smoke",
            "readiness",
            "gate",
        ),
    }
}
GENERIC_PHASE_TITLES = (
    "backend",
    "frontend",
    "ui",
    "tests",
    "testing",
    "rollout",
    "cleanup",
    "docs",
    "documentation",
    "observability",
    "integration",
)
REQUIRED_TOP_LEVEL = (
    "## Decisions",
    "## Non-Goals",
    "## Open Risks",
    "## Loopholes To Close",
)
REQUIRED_PHASE_LABELS = (
    "Goal:",
    "Deliverables:",
    "Dependencies:",
    "Unresolved decisions:",
    "Steps:",
    "Validation:",
)
WEAK_PHRASES = (
    "build backend",
    "add ui",
    "wire everything",
    "add tests",
    "roll out",
    "clean up",
    "make robust",
    "polish",
    "verify it works",
    "manual qa",
)
DEFERRED_DETAIL_PHRASES = (
    "details can be handled during implementation",
    "details later",
    "tbd",
    "to be determined",
    "figure out later",
)
VALIDATION_EVIDENCE_WORDS = (
    "unit",
    "integration",
    "fixture",
    "golden",
    "parser",
    "schema",
    "repository",
    "migration",
    "api",
    "contract",
    "websocket",
    "protocol",
    "dashboard",
    "component",
    "mock",
    "smoke",
    "e2e",
    "end-to-end",
    "lint",
    "build",
    "command",
    "metric",
    "log",
    "assert",
    "check",
)


@dataclass(frozen=True)
class EvalCase:
    name: str
    brief_path: Path


@dataclass(frozen=True)
class Phase:
    number: int
    title: str
    heading: str
    body: str
    goal: str
    deliverables: list[str]
    dependencies: list[str]
    unresolved_decisions: list[str]
    steps: list[str]
    validation: list[str]


@dataclass(frozen=True)
class PlanRunResult:
    completed: subprocess.CompletedProcess[str]
    pty_state: dict[str, Any]


def main() -> int:
    args = parse_args()
    if args.expect_fail and args.score_only is None:
        raise SystemExit("--expect-fail is only supported with --score-only")
    if args.score_only is None:
        require_live_eval()
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
                score_only=args.score_only,
                min_score=args.min_score,
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
        help="brief case name from evals/plan-create/briefs; default: all",
    )
    parser.add_argument("--iterations", type=int, default=1)
    parser.add_argument("--min-score", type=int, default=85)
    parser.add_argument("--timeout", type=int, default=900)
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
        "live plan-create eval requires LGTM_LIVE_EVAL=1; "
        "use --score-only for deterministic controls"
    )


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
    score_only: Path | None,
    min_score: int,
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
    else:
        prepare_repo(repo_dir)
        brief = case.brief_path.read_text(encoding="utf-8")
        (run_dir / "brief.md").write_text(brief, encoding="utf-8")
        start = time.monotonic()
        plan_run = run_lgtm_plan_pty(
            repo_dir=repo_dir,
            codex_bin=codex_bin,
            lgtm_bin=lgtm_bin,
            brief=brief,
            run_id=run_id,
            timeout=timeout,
        )
        wall_seconds = time.monotonic() - start
        completed = plan_run.completed
        stdout = completed.stdout
        stderr = completed.stderr
        exit_code = completed.returncode
        plan_path = repo_dir / "PLAN.md"
        plan_text = plan_path.read_text(encoding="utf-8") if plan_path.is_file() else ""
        logs_dir = repo_dir / ".lgtm/logs"
        if logs_dir.is_dir():
            shutil.copytree(logs_dir, run_dir / "logs")
        pty_state = plan_run.pty_state

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
    logs_copy = run_dir / "logs"
    if logs_copy.is_dir():
        result["logs_path"] = str(logs_copy)
    if not score_only:
        result["pty_state"] = pty_state
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


def run_lgtm_plan_pty(
    *,
    repo_dir: Path,
    codex_bin: str,
    lgtm_bin: Path,
    brief: str,
    run_id: str,
    timeout: int,
) -> PlanRunResult:
    if not lgtm_bin.is_file():
        raise SystemExit(
            f"lgtm binary not found at {lgtm_bin}; run `cargo build` or pass --lgtm-bin"
        )

    cmd = [
        str(lgtm_bin),
        "plan",
        brief.strip(),
        "--root",
        str(repo_dir),
        "--codex-bin",
        codex_bin,
        "--log-dir",
        ".lgtm/logs",
        "--run-stamp",
        run_id,
    ]

    master_fd, slave_fd = pty.openpty()
    started_at = time.monotonic()
    process = subprocess.Popen(
        cmd,
        stdin=slave_fd,
        stdout=slave_fd,
        stderr=slave_fd,
        close_fds=True,
        start_new_session=True,
    )
    os.close(slave_fd)

    transcript = bytearray()
    timed_out = False
    pty_state: dict[str, Any] = {
        "first_turn_completed": False,
        "sent_finish": False,
        "second_turn_completed": False,
        "saw_exit_prompt": False,
        "sent_exit": False,
        "timed_out": False,
        "timeout_reason": "",
    }

    try:
        while True:
            if time.monotonic() - started_at > timeout:
                timed_out = True
                pty_state["timed_out"] = True
                pty_state["timeout_reason"] = "wall_timeout"
                terminate_process_group(process)
                break

            ready, _, _ = select.select([master_fd], [], [], 0.1)
            if master_fd in ready:
                try:
                    chunk = os.read(master_fd, 4096)
                except OSError:
                    chunk = b""
                if chunk:
                    transcript.extend(chunk)

            plain = strip_ansi(transcript.decode("utf-8", errors="replace"))
            pty_state["first_turn_completed"] = bool(
                pty_state["first_turn_completed"]
                or plan_turn_completed(repo_dir, run_id, 1)
            )
            pty_state["second_turn_completed"] = bool(
                pty_state["second_turn_completed"]
                or plan_turn_completed(repo_dir, run_id, 2)
            )
            pty_state["saw_exit_prompt"] = bool(
                pty_state["saw_exit_prompt"] or "Implement now or exit? [i/e]" in plain
            )
            if not pty_state["sent_exit"] and (
                pty_state["saw_exit_prompt"] or pty_state["second_turn_completed"]
            ):
                os.write(master_fd, b"e\r")
                pty_state["sent_exit"] = True
            elif not pty_state["sent_finish"] and pty_state["first_turn_completed"]:
                os.write(master_fd, b"/finish\r")
                pty_state["sent_finish"] = True

            if process.poll() is not None:
                while True:
                    ready, _, _ = select.select([master_fd], [], [], 0)
                    if master_fd not in ready:
                        break
                    try:
                        chunk = os.read(master_fd, 4096)
                    except OSError:
                        break
                    if not chunk:
                        break
                    transcript.extend(chunk)
                break
    finally:
        os.close(master_fd)

    return_code = 124 if timed_out else process.wait()
    stdout = transcript.decode("utf-8", errors="replace")
    stderr = "timed out waiting for lgtm plan\n" if timed_out else ""
    return PlanRunResult(
        completed=subprocess.CompletedProcess(cmd, return_code, stdout, stderr),
        pty_state=pty_state,
    )


def terminate_process_group(process: subprocess.Popen[Any]) -> None:
    if process.poll() is not None:
        return
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    try:
        process.wait(timeout=5)
        return
    except subprocess.TimeoutExpired:
        pass
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        return
    process.wait(timeout=5)


def strip_ansi(text: str) -> str:
    return re.sub(r"\x1b\[[0-9;?]*[ -/]*[@-~]", "", text)


def plan_turn_completed(repo_dir: Path, run_id: str, turn_number: int) -> bool:
    log_path = repo_dir / ".lgtm" / "logs" / f"{run_id}-plan-{turn_number:03}.jsonl"
    if not log_path.is_file():
        return False
    try:
        lines = log_path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return False
    for line in lines:
        for payload in parse_log_payloads(line):
            if payload.get("method") == "turn/completed":
                return True
    return False


def score_plan(plan: str, *, case_name: str, min_score: int) -> dict[str, Any]:
    phases = parse_phases(plan)
    required_workstreams = CASE_WORKSTREAMS.get(case_name, {})
    phase_quality = [score_phase(phase) for phase in phases]
    domain_matches = match_domains(phases, phase_quality, required_workstreams)
    whole_plan_domains = {
        domain
        for domain, mentioned in whole_plan_domain_mentions(plan, required_workstreams).items()
        if mentioned
    }
    missing_domains = sorted(set(required_workstreams) - set(domain_matches))
    keyword_only_domains = sorted(set(whole_plan_domains) - set(domain_matches))
    generic_phase_titles = [
        phase.heading
        for phase in phases
        if normalized_title(phase.title) in GENERIC_PHASE_TITLES
    ]
    generic_validation_phases = [
        phase.heading
        for phase, quality in zip(phases, phase_quality)
        if not quality["validation_specific"]
    ]
    weak_phrases = weak_phrase_hits(phases)
    overloaded_phases = overloaded_phase_headings(phases, required_workstreams)
    fake_dependencies = fake_dependency_ratio(phases)
    top_section_issues = top_section_issues_for(plan)

    checks: dict[str, Any] = {}
    checks["has_plan_heading"] = plan.lstrip().startswith("# Plan")
    checks["required_top_level"] = [
        heading for heading in REQUIRED_TOP_LEVEL if heading in plan
    ]
    checks["phase_count"] = len(phases)
    checks["sequential_phase_numbers"] = [phase.number for phase in phases] == list(
        range(1, len(phases) + 1)
    )
    checks["all_phase_labels"] = all(quality["has_required_labels"] for quality in phase_quality)
    checks["domain_coverage"] = sorted(domain_matches)
    checks["missing_domains"] = missing_domains
    checks["keyword_only_domains"] = keyword_only_domains
    checks["generic_phase_titles"] = generic_phase_titles
    checks["weak_phrases"] = weak_phrases
    checks["generic_validation_phases"] = generic_validation_phases
    checks["overloaded_phases"] = overloaded_phases
    checks["fake_dependency_ratio"] = fake_dependencies
    checks["top_section_issues"] = top_section_issues

    structure_score = 0
    structure_score += 4 if checks["has_plan_heading"] else 0
    structure_score += len(checks["required_top_level"])
    structure_score += 4 if checks["all_phase_labels"] else 0
    structure_score += 3 if checks["sequential_phase_numbers"] else 0

    top_score = max(0, 10 - len(top_section_issues) * 3)
    coverage_score = (
        25
        if not required_workstreams
        else round((len(domain_matches) / len(required_workstreams)) * 25)
    )
    specific_ratio = ratio(
        sum(1 for quality in phase_quality if quality["specific_contract"]),
        len(phase_quality),
    )
    specificity_score = round(specific_ratio * 20)
    validation_ratio = ratio(
        sum(1 for quality in phase_quality if quality["validation_specific"]),
        len(phase_quality),
    )
    validation_score = round(validation_ratio * 15)
    decomposition_score = max(
        0,
        15
        - len(generic_phase_titles) * 3
        - len(overloaded_phases) * 4
        - (6 if fake_dependencies >= 0.75 and len(phases) >= 4 else 0),
    )

    score = (
        structure_score
        + top_score
        + coverage_score
        + specificity_score
        + validation_score
        + decomposition_score
    )
    score -= len(weak_phrases) * 2
    score = max(0, min(100, score))

    blockers = []
    missing_top = sorted(set(REQUIRED_TOP_LEVEL) - set(checks["required_top_level"]))
    if missing_top:
        blockers.append(f"missing top-level sections: {', '.join(missing_top)}")
    if not checks["all_phase_labels"]:
        blockers.append("one or more phases miss required phase labels")
    if missing_domains:
        blockers.append(f"missing concrete workstreams: {', '.join(missing_domains)}")
    if keyword_only_domains:
        blockers.append(f"keyword-only workstreams: {', '.join(keyword_only_domains)}")
    if generic_phase_titles:
        blockers.append(f"generic umbrella phase titles: {len(generic_phase_titles)}")
    if len(generic_phase_titles) > max(1, len(phases) // 4):
        blockers.append("more than 25% of phases are generic umbrella phases")
    if generic_validation_phases:
        blockers.append(f"generic validation phases: {len(generic_validation_phases)}")
    if top_section_issues:
        blockers.append(f"weak top-level sections: {', '.join(top_section_issues)}")
    if overloaded_phases:
        blockers.append(f"overloaded phases: {len(overloaded_phases)}")
    if fake_dependencies >= 0.75 and len(phases) >= 4:
        blockers.append("dependencies do not show a real implementation order")
    if weak_phrases:
        blockers.append(f"contains weak phrases: {', '.join(weak_phrases)}")

    return {
        "value": score,
        "min_score": min_score,
        "passed": score >= min_score and not blockers,
        "blockers": blockers,
        "checks": checks,
    }


def parse_phases(plan: str) -> list[Phase]:
    matches = list(re.finditer(r"^## Phase \d+ - .+$", plan, re.MULTILINE))
    phases = []
    for index, match in enumerate(matches):
        end = matches[index + 1].start() if index + 1 < len(matches) else len(plan)
        heading = match.group(0)
        number_text, title = heading.removeprefix("## Phase ").split(" - ", 1)
        body = plan[match.end() : end]
        phases.append(
            Phase(
                number=int(number_text),
                title=title.strip(),
                heading=heading,
                body=body,
                goal=extract_labeled_text(body, "Goal:"),
                deliverables=extract_labeled_list(body, "Deliverables:"),
                dependencies=extract_labeled_list(body, "Dependencies:"),
                unresolved_decisions=extract_labeled_list(body, "Unresolved decisions:"),
                steps=extract_labeled_list(body, "Steps:"),
                validation=extract_labeled_list(body, "Validation:"),
            )
        )
    return phases


def score_phase(phase: Phase) -> dict[str, bool]:
    has_required_labels = all(label in phase.body for label in REQUIRED_PHASE_LABELS)
    has_contract_items = bool(phase.goal.strip()) and bool(phase.deliverables) and bool(phase.steps)
    concrete_items = [
        item
        for item in [phase.goal, *phase.deliverables, *phase.steps]
        if item_is_concrete(item)
    ]
    validation_specific = validation_is_specific(phase.validation)
    return {
        "has_required_labels": has_required_labels,
        "specific_contract": has_contract_items
        and len(concrete_items) >= 2
        and normalized_title(phase.title) not in GENERIC_PHASE_TITLES,
        "validation_specific": validation_specific,
    }


def extract_labeled_text(body: str, label: str) -> str:
    block = labeled_block(body, label)
    lines = [line.strip() for line in block.splitlines() if line.strip()]
    if not lines:
        return ""
    if lines[0].startswith(("-", "*")):
        return normalize_item(lines[0])
    return lines[0]


def extract_labeled_list(body: str, label: str) -> list[str]:
    block = labeled_block(body, label)
    items = []
    for line in block.splitlines():
        stripped = line.strip()
        if stripped.startswith(("-", "*")):
            items.append(normalize_item(stripped))
    if items:
        return items
    return [line.strip() for line in block.splitlines() if line.strip()]


def labeled_block(body: str, start_label: str) -> str:
    if start_label not in body:
        return ""
    tail = body.split(start_label, 1)[1]
    next_positions = [
        position
        for label in REQUIRED_PHASE_LABELS
        if label != start_label and (position := tail.find(label)) >= 0
    ]
    if not next_positions:
        return tail
    return tail[: min(next_positions)]


def normalize_item(line: str) -> str:
    return re.sub(r"^[-*\s]+", "", line).strip().rstrip(".")


def normalized_title(title: str) -> str:
    return re.sub(r"\s+", " ", title.strip().lower())


def item_is_concrete(item: str) -> bool:
    normalized = normalize_item(item).lower()
    if not normalized or any(weak_phrase_line_match(normalized, phrase) for phrase in WEAK_PHRASES):
        return False
    return len(re.findall(r"[a-z0-9_.-]+", normalized)) >= 4


def validation_is_specific(items: list[str]) -> bool:
    if not items:
        return False
    for item in items:
        normalized = normalize_item(item).lower()
        if weak_phrase_line_match(normalized, "verify it works"):
            return False
        if normalized in {"run tests", "add tests", "manual qa"}:
            return False
    return any(
        any(word in normalize_item(item).lower() for word in VALIDATION_EVIDENCE_WORDS)
        for item in items
    )


def match_domains(
    phases: list[Phase],
    phase_quality: list[dict[str, bool]],
    required_workstreams: dict[str, tuple[str, ...]],
) -> dict[str, list[str]]:
    matches: dict[str, list[str]] = {}
    for phase, quality in zip(phases, phase_quality):
        if not quality["specific_contract"] or not quality["validation_specific"]:
            continue
        text = phase_semantic_text(phase)
        for domain, patterns in required_workstreams.items():
            if any(pattern in text for pattern in patterns):
                matches.setdefault(domain, []).append(phase.heading)
    return matches


def whole_plan_domain_mentions(
    plan: str, required_workstreams: dict[str, tuple[str, ...]]
) -> dict[str, bool]:
    lower = plan.lower()
    return {
        domain: any(pattern in lower for pattern in patterns)
        for domain, patterns in required_workstreams.items()
    }


def phase_semantic_text(phase: Phase) -> str:
    return "\n".join(
        [
            phase.title,
            phase.goal,
            *phase.deliverables,
            *phase.steps,
            *phase.validation,
        ]
    ).lower()


def overloaded_phase_headings(
    phases: list[Phase], required_workstreams: dict[str, tuple[str, ...]]
) -> list[str]:
    overloaded = []
    for phase in phases:
        title = normalized_title(phase.title)
        if title not in GENERIC_PHASE_TITLES and "integration" not in title:
            continue
        text = phase_semantic_text(phase)
        domains = [
            domain
            for domain, patterns in required_workstreams.items()
            if any(pattern in text for pattern in patterns)
        ]
        if len(domains) >= 4 and "readiness" not in title:
            overloaded.append(phase.heading)
    return overloaded


def fake_dependency_ratio(phases: list[Phase]) -> float:
    if not phases:
        return 1.0
    fake = 0
    for phase in phases:
        deps = [normalize_item(dep).lower() for dep in phase.dependencies]
        if not deps or all(dep in {"none", "phase 1", "phase 1."} for dep in deps):
            fake += 1
    return fake / len(phases)


def top_section_issues_for(plan: str) -> list[str]:
    issues = []
    decisions = top_section(plan, "## Decisions")
    non_goals = top_section(plan, "## Non-Goals")
    risks = top_section(plan, "## Open Risks")
    loopholes = top_section(plan, "## Loopholes To Close")
    if len(bullet_items(decisions)) < 3:
        issues.append("thin decisions")
    if "jenkins" not in non_goals.lower() and "compatibility" not in non_goals.lower():
        issues.append("missing Jenkins compatibility non-goal")
    risk_words = [
        "migration",
        "compatibility",
        "runtime",
        "rollout",
        "agent",
        "jenkins",
        "scheduler",
        "dashboard",
        "secret",
        "side effect",
    ]
    if not any(word in risks.lower() for word in risk_words):
        issues.append("thin risks")
    if any(phrase in loopholes.lower() for phrase in DEFERRED_DETAIL_PHRASES):
        issues.append("loopholes defer details")
    return issues


def top_section(plan: str, heading: str) -> str:
    marker = plan.find(heading)
    if marker < 0:
        return ""
    next_heading = re.search(r"^## ", plan[marker + len(heading) :], re.MULTILINE)
    if not next_heading:
        return plan[marker + len(heading) :]
    end = marker + len(heading) + next_heading.start()
    return plan[marker + len(heading) : end]


def bullet_items(text: str) -> list[str]:
    return [
        normalize_item(line)
        for line in text.splitlines()
        if line.strip().startswith(("-", "*"))
    ]


def ratio(numerator: int, denominator: int) -> float:
    if denominator == 0:
        return 0.0
    return numerator / denominator


def weak_phrase_hits(phases: list[Phase]) -> list[str]:
    hits = []
    for phase in phases:
        text = "\n".join(
            [
                phase.heading,
                phase.goal,
                *phase.deliverables,
                *phase.steps,
                *phase.validation,
            ]
        ).lower()
        for line in text.splitlines():
            normalized = re.sub(r"^[-*\s]*(goal:\s*)?", "", line).strip()
            normalized = normalized.removesuffix(".")
            for phrase in WEAK_PHRASES:
                if weak_phrase_line_match(normalized, phrase) and phrase not in hits:
                    hits.append(phrase)
    return hits


def weak_phrase_line_match(line: str, phrase: str) -> bool:
    if line == phrase:
        return True
    if phrase == "wire everything" and line == "wire everything together":
        return True
    return False


def print_result(result: dict[str, Any], *, min_score: int) -> None:
    score = result["score"]
    status = "pass" if result.get("expectation_passed", score["passed"]) else "fail"
    score_status = "scorer-pass" if score["passed"] else "scorer-fail"
    expectation = " expected-fail" if result.get("expected_failure") else ""
    print(
        f"{result['run_id']} {status}{expectation} {score_status} "
        f"score={score['value']}/{min_score} "
        f"phases={score['checks']['phase_count']} "
        f"domains={len(score['checks']['domain_coverage'])}",
        flush=True,
    )
    for blocker in score["blockers"]:
        print(f"  blocker: {blocker}", flush=True)


def write_summary(path: Path, results: list[dict[str, Any]], min_score: int) -> None:
    lines = ["# lgtm-plan-create Eval Summary", ""]
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


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit(130)
