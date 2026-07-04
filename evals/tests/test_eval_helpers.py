from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

from evals.common.lgtm_logs import collect_usage
from evals.common.scoring import observed_phase_pass_order

REPO_ROOT = Path(__file__).resolve().parents[2]


def load_plan_create_runner():
    path = REPO_ROOT / "evals" / "plan-create" / "run_eval.py"
    spec = importlib.util.spec_from_file_location("plan_create_run_eval", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class EvalHelperTests(unittest.TestCase):
    def test_collect_usage_reads_nested_app_server_payloads(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            logs_dir = Path(temp)
            payload = {
                "method": "turn/completed",
                "params": {
                    "turn": {
                        "usage": {
                            "input_tokens": 11,
                            "input_tokens_details": {"cached_tokens": 7},
                            "output_tokens": 5,
                            "output_tokens_details": {"reasoning_tokens": 2},
                            "total_tokens": 16,
                        }
                    }
                },
            }
            wrapped = {"direction": "in", "line": json.dumps(payload)}
            (logs_dir / "eval-phase-01-validate.jsonl").write_text(
                json.dumps(wrapped) + "\n",
                encoding="utf-8",
            )

            usage = collect_usage(logs_dir)

            self.assertEqual(usage["log_files"], 1)
            self.assertEqual(usage["usage_objects"], 1)
            self.assertEqual(usage["input_tokens"], 11)
            self.assertEqual(usage["cached_input_tokens"], 7)
            self.assertEqual(usage["output_tokens"], 5)
            self.assertEqual(usage["reasoning_tokens"], 2)
            self.assertEqual(usage["total_tokens"], 16)

    def test_trajectory_order_requires_completed_payloads(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            logs_dir = Path(temp)
            completed = {
                "direction": "in",
                "line": json.dumps({"method": "turn/completed", "params": {}}),
            }
            (logs_dir / "eval-phase-01-index.jsonl").write_text(
                json.dumps(completed) + "\n",
                encoding="utf-8",
            )
            (logs_dir / "eval-phase-01-commit.jsonl").write_text(
                "turn/completed\n",
                encoding="utf-8",
            )

            self.assertEqual(observed_phase_pass_order(logs_dir), ["index"])

    def test_plan_pty_completion_requires_structural_payload(self) -> None:
        plan_create = load_plan_create_runner()
        with tempfile.TemporaryDirectory() as temp:
            repo_dir = Path(temp)
            logs_dir = repo_dir / ".lgtm" / "logs"
            logs_dir.mkdir(parents=True)
            payload = {
                "direction": "in",
                "line": json.dumps({"method": "turn/completed", "params": {}}),
            }
            (logs_dir / "run-plan-001.jsonl").write_text(
                json.dumps(payload) + "\n",
                encoding="utf-8",
            )
            (logs_dir / "run-plan-002.jsonl").write_text(
                "turn/completed\n",
                encoding="utf-8",
            )

            self.assertTrue(plan_create.plan_turn_completed(repo_dir, "run", 1))
            self.assertFalse(plan_create.plan_turn_completed(repo_dir, "run", 2))


if __name__ == "__main__":
    unittest.main()
