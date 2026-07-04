"""Shared parsers for lgtm app-server log artifacts."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any


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
