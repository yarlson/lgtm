#!/usr/bin/env python3
"""Evaluate lgtm-plan-create prompt output quality.

The eval generates a real PLAN.md with Codex, scores the artifact, and stores
all run data outside the repo by default. It is intentionally about output
quality, not token usage or transport behavior.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
EVAL_ROOT = Path(__file__).resolve().parent
DEFAULT_DATA_ROOT = Path.home() / "lgtm-plan-create-eval-data"
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
FAMILY_PATTERNS = {
    "schema_parser_diagnostics": (
        "schema",
        "parser",
        "diagnostic",
        ".kargo.yml",
        "manifest",
    ),
    "policy_security_trust": (
        "policy",
        "authorization",
        "trust",
        "security",
        "allowlist",
    ),
    "persistence_indexes_migrations": (
        "mongo",
        "persistence",
        "index",
        "migration",
        "collection",
    ),
    "scheduler_state_machine": (
        "scheduler",
        "state machine",
        "dag state",
        "ready queue",
        "lease",
    ),
    "protocol_api_contracts": (
        "protocol",
        "api",
        "websocket",
        "rpc",
        "contract",
    ),
    "agent_runtime": (
        "agent",
        "worker",
        "runtime",
        "runner",
        "execution",
    ),
    "secrets_isolation_resources": (
        "secret",
        "isolation",
        "sandbox",
        "resource",
        "cap",
    ),
    "logs_artifacts_checks_audit_observability": (
        "log",
        "artifact",
        "check",
        "audit",
        "metric",
        "observability",
    ),
    "dashboard_operator_actions": (
        "dashboard",
        "operator",
        "cancel",
        "rerun",
        "retry",
    ),
    "shadow_fallback_rollout": (
        "shadow",
        "fallback",
        "rollout",
        "feature flag",
        "enable",
    ),
    "migration_cleanup_removal": (
        "jenkins removal",
        "remove jenkins",
        "cleanup",
        "legacy path",
        "migration",
    ),
    "end_to_end_readiness": (
        "end-to-end",
        "e2e",
        "smoke",
        "readiness",
        "gate",
    ),
}


@dataclass(frozen=True)
class EvalCase:
    name: str
    brief_path: Path


def main() -> int:
    args = parse_args()
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
                model=args.model,
                timeout=args.timeout,
                score_only=args.score_only,
                min_score=args.min_score,
            )
            results.append(result)
            append_jsonl(data_dir / "results.jsonl", result)
            print_result(result, min_score=args.min_score)

    write_summary(data_dir / "summary.md", results, args.min_score)
    print(f"summary={data_dir / 'summary.md'}", flush=True)
    passed = all(result["score"]["passed"] for result in results)
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
    parser.add_argument("--model")
    parser.add_argument("--eval-id")
    parser.add_argument("--data-root", type=Path, default=DEFAULT_DATA_ROOT)
    parser.add_argument(
        "--score-only",
        type=Path,
        help="score an existing PLAN.md instead of generating with Codex",
    )
    return parser.parse_args()


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
    model: str | None,
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
        prompt = build_generation_prompt(case.brief_path.read_text(encoding="utf-8"))
        (run_dir / "prompt.txt").write_text(prompt, encoding="utf-8")
        start = time.monotonic()
        completed = run_codex(
            repo_dir=repo_dir,
            prompt=prompt,
            codex_bin=codex_bin,
            model=model,
            timeout=timeout,
        )
        wall_seconds = time.monotonic() - start
        stdout = completed.stdout
        stderr = completed.stderr
        exit_code = completed.returncode
        plan_path = repo_dir / "PLAN.md"
        plan_text = plan_path.read_text(encoding="utf-8") if plan_path.is_file() else ""
        (run_dir / "stdout.txt").write_text(stdout, encoding="utf-8", errors="replace")
        (run_dir / "stderr.txt").write_text(stderr, encoding="utf-8", errors="replace")

    (run_dir / "PLAN.md").write_text(plan_text, encoding="utf-8")
    score = score_plan(plan_text, min_score=min_score)
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


def build_generation_prompt(brief: str) -> str:
    skill = (REPO_ROOT / "skills/lgtm-plan-create/SKILL.md").read_text(
        encoding="utf-8"
    )
    return f"""Use the following managed lgtm planning skill exactly.

<lgtm-plan-create-skill>
{skill}
</lgtm-plan-create-skill>

Target PLAN.md path: PLAN.md
Target AGENTS.md path: AGENTS.md

This is an eval of final plan quality. The user has already answered `/finish`.
Do not ask another question. Write the final PLAN.md now.

Important eval constraint:
- Treat this brief as broad platform/migration/architecture work.
- A plan with fewer than 12 phases is under-split unless the brief is explicitly narrow.
- Split broad phase families instead of merging schema, policy, persistence,
  scheduler, protocol, agent runtime, secrets/isolation, logs/artifacts/checks,
  dashboard, shadow rollout, migration cleanup, and readiness gates.

User brief:
{brief.strip()}
"""


def run_codex(
    *,
    repo_dir: Path,
    prompt: str,
    codex_bin: str,
    model: str | None,
    timeout: int,
) -> subprocess.CompletedProcess[str]:
    cmd = [
        codex_bin,
        "--sandbox",
        "danger-full-access",
        "-a",
        "never",
        "exec",
        "--cd",
        str(repo_dir),
        "--skip-git-repo-check",
        "--output-last-message",
        str(repo_dir / "last-message.txt"),
    ]
    if model:
        cmd.extend(["--model", model])
    cmd.append("-")
    return subprocess.run(
        cmd,
        input=prompt,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )


def score_plan(plan: str, *, min_score: int) -> dict[str, Any]:
    phases = parse_phases(plan)
    checks: dict[str, Any] = {}
    checks["has_plan_heading"] = plan.lstrip().startswith("# Plan")
    checks["required_top_level"] = [
        heading for heading in REQUIRED_TOP_LEVEL if heading in plan
    ]
    checks["phase_count"] = len(phases)
    checks["all_phase_labels"] = all(
        all(label in phase["body"] for label in REQUIRED_PHASE_LABELS)
        for phase in phases
    )
    checks["family_coverage"] = covered_families(plan)
    checks["weak_phrases"] = weak_phrase_hits(phases)
    checks["generic_validation_count"] = generic_validation_count(phases)

    score = 0
    score += 10 if checks["has_plan_heading"] else 0
    score += len(checks["required_top_level"]) * 4
    score += min(checks["phase_count"], 14) * 2
    score += 16 if checks["phase_count"] >= 12 else 0
    score += 14 if checks["all_phase_labels"] else 0
    score += min(len(checks["family_coverage"]), 12) * 2
    score -= len(checks["weak_phrases"]) * 4
    score -= checks["generic_validation_count"] * 3
    score = max(0, min(100, score))

    blockers = []
    if checks["phase_count"] < 12:
        blockers.append("broad plan has fewer than 12 phases")
    missing_top = sorted(set(REQUIRED_TOP_LEVEL) - set(checks["required_top_level"]))
    if missing_top:
        blockers.append(f"missing top-level sections: {', '.join(missing_top)}")
    if not checks["all_phase_labels"]:
        blockers.append("one or more phases miss required phase labels")
    if len(checks["family_coverage"]) < 10:
        blockers.append("covers fewer than 10 broad-work phase families")
    if checks["weak_phrases"]:
        blockers.append(f"contains weak phrases: {', '.join(checks['weak_phrases'])}")
    if checks["generic_validation_count"] > 0:
        blockers.append("contains generic validation blocks")

    return {
        "value": score,
        "min_score": min_score,
        "passed": score >= min_score and not blockers,
        "blockers": blockers,
        "checks": checks,
    }


def parse_phases(plan: str) -> list[dict[str, str]]:
    matches = list(re.finditer(r"^## Phase \d+ - .+$", plan, re.MULTILINE))
    phases = []
    for index, match in enumerate(matches):
        end = matches[index + 1].start() if index + 1 < len(matches) else len(plan)
        phases.append({"heading": match.group(0), "body": plan[match.end() : end]})
    return phases


def covered_families(plan: str) -> list[str]:
    lower = plan.lower()
    covered = []
    for family, patterns in FAMILY_PATTERNS.items():
        if any(pattern in lower for pattern in patterns):
            covered.append(family)
    return covered


def generic_validation_count(phases: list[dict[str, str]]) -> int:
    proof_words = (
        "test",
        "check",
        "smoke",
        "command",
        "assert",
        "confirm",
        "verify",
        "validate",
        "lint",
        "build",
        "migration",
        "fixture",
    )
    count = 0
    for phase in phases:
        validation = phase["body"].split("Validation:", 1)
        if len(validation) != 2:
            count += 1
            continue
        block = validation[1].strip().lower()
        if re.fullmatch(r"[-*\s]*(run tests|add tests|manual qa|verify it works)\.?", block):
            count += 1
        if not any(word in block for word in proof_words):
            count += 1
    return count


def weak_phrase_hits(phases: list[dict[str, str]]) -> list[str]:
    hits = []
    for phase in phases:
        text = "\n".join(
            [
                phase["heading"],
                labeled_block(phase["body"], "Goal:", "Deliverables:"),
                labeled_block(phase["body"], "Steps:", "Validation:"),
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


def labeled_block(body: str, start_label: str, end_label: str) -> str:
    if start_label not in body:
        return ""
    tail = body.split(start_label, 1)[1]
    if end_label not in tail:
        return tail
    return tail.split(end_label, 1)[0]


def print_result(result: dict[str, Any], *, min_score: int) -> None:
    score = result["score"]
    status = "pass" if score["passed"] else "fail"
    print(
        f"{result['run_id']} {status} score={score['value']}/{min_score} "
        f"phases={score['checks']['phase_count']} "
        f"families={len(score['checks']['family_coverage'])}",
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
                f"- Score: {score['value']}/{min_score}",
                f"- Phases: {score['checks']['phase_count']}",
                f"- Families: {len(score['checks']['family_coverage'])}",
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
