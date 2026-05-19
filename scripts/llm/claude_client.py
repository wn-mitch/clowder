"""Headless `claude` CLI wrapper for presenter-layer LLM calls.

Routes through the user's `claude` CLI in `--print --output-format json`
mode. Auth comes from the user's existing subscription (OAuth /
keychain) — **not** `ANTHROPIC_API_KEY`. We deliberately do NOT pass
`--bare` because `--bare` forces API-key auth and rejects OAuth /
keychain reads, breaking subscription billing.

The single entry point `call_haiku_json` returns `(parsed_json | None,
meta)` and **never raises** — every failure mode collapses to
`parsed_json = None` with a `meta.status` discriminator. Callers treat
None as "no enrichment available" and fall back to their deterministic
path.

Worst-case wall: ~16s (single retry on timeout). Typical: 2-3s.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import time
from pathlib import Path
from typing import Any


# Hard cap on system-prompt size we'll inline as a CLI arg. macOS argv
# limit is ~256KB but we keep well clear; bigger prompts should be
# refactored or chunked.
_MAX_SYSTEM_PROMPT_CHARS = 32_000

# Bytes of stderr to retain for debugging when a call fails non-zero.
_STDERR_TAIL_BYTES = 1_024


def call_haiku_json(
    *,
    user_payload: dict | str,
    system_prompt_path: Path,
    schema_path: Path,
    timeout_secs: float = 20.0,
    model: str = "claude-haiku-4-5",
) -> tuple[dict | None, dict]:
    """Invoke `claude` headlessly and parse a JSON object from its stdout.

    The model is *asked* (in the system prompt) to honor the schema at
    `schema_path`. We don't pass the schema to the CLI — `--json-schema`
    has not stabilized across versions, so we validate in Python after
    parsing.

    Returns `(parsed | None, meta)` where:
      meta = {
        "status": "ok" | "timeout" | "nonzero" | "malformed"
                | "schema_violation" | "disabled" | "error",
        "elapsed_ms": int,
        "stderr_tail": str,   # bytes from claude stderr (last ~1KB)
        "model": str,         # echoed model id
      }

    NEVER raises. Caller treats `parsed is None` as "no enrichment".
    """
    started = time.monotonic()
    meta: dict[str, Any] = {
        "status": "error",
        "elapsed_ms": 0,
        "stderr_tail": "",
        "model": model,
    }

    if shutil.which("claude") is None:
        meta["status"] = "disabled"
        meta["elapsed_ms"] = int((time.monotonic() - started) * 1000)
        return None, meta

    try:
        system_prompt = system_prompt_path.read_text(encoding="utf-8")
    except OSError as e:
        meta["status"] = "error"
        meta["stderr_tail"] = f"prompt read failed: {e}"
        meta["elapsed_ms"] = int((time.monotonic() - started) * 1000)
        return None, meta

    if len(system_prompt) > _MAX_SYSTEM_PROMPT_CHARS:
        meta["status"] = "error"
        meta["stderr_tail"] = (
            f"system prompt too large: {len(system_prompt)} chars "
            f"(cap {_MAX_SYSTEM_PROMPT_CHARS})"
        )
        meta["elapsed_ms"] = int((time.monotonic() - started) * 1000)
        return None, meta

    payload_text = (
        user_payload if isinstance(user_payload, str)
        else json.dumps(user_payload, default=str)
    )

    # Load the schema once; if it's missing we fall back to "object with
    # any shape" — better to ship suggestions than to refuse.
    try:
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        schema = None

    parsed, run_meta = _run_once(
        payload_text=payload_text,
        system_prompt=system_prompt,
        model=model,
        timeout_secs=timeout_secs,
    )

    # Single retry on timeout only — nonzero is auth/config and won't
    # fix itself; malformed is the model's fault and likely repeats.
    if run_meta["status"] == "timeout":
        parsed, run_meta = _run_once(
            payload_text=payload_text,
            system_prompt=system_prompt,
            model=model,
            timeout_secs=timeout_secs,
        )

    meta.update(run_meta)
    meta["elapsed_ms"] = int((time.monotonic() - started) * 1000)

    if parsed is None:
        return None, meta

    if schema is not None and not _matches_schema(parsed, schema):
        meta["status"] = "schema_violation"
        return None, meta

    return parsed, meta


# ── internals ───────────────────────────────────────────────────────────────


def _run_once(
    *,
    payload_text: str,
    system_prompt: str,
    model: str,
    timeout_secs: float,
) -> tuple[dict | None, dict]:
    """One shot at the CLI. Returns (parsed_or_none, partial_meta).

    No `--bare` — that flag forces ANTHROPIC_API_KEY auth and disables
    OAuth/keychain reads, breaking subscription billing. The trade-off
    is ~1-2s of extra startup latency (hooks/LSP/plugin sync load) per
    call.
    """
    cmd = [
        "claude",
        "--print",
        "--no-session-persistence",
        "--model", model,
        "--output-format", "json",
        "--input-format", "text",
        "--system-prompt", system_prompt,
    ]
    try:
        proc = subprocess.run(
            cmd,
            input=payload_text,
            capture_output=True,
            text=True,
            timeout=timeout_secs,
        )
    except subprocess.TimeoutExpired as e:
        return None, {
            "status": "timeout",
            "stderr_tail": _tail(getattr(e, "stderr", "") or ""),
        }
    except (OSError, ValueError) as e:
        return None, {
            "status": "error",
            "stderr_tail": str(e)[:_STDERR_TAIL_BYTES],
        }

    if proc.returncode != 0:
        # The CLI itself failed (e.g., auth misconfigured, network
        # broken, model alias unknown). Surface the result text if the
        # CLI managed to write a JSON envelope before exiting.
        tail = _tail(proc.stderr)
        outer = _parse_json_from_text(proc.stdout)
        if isinstance(outer, dict):
            result_text = outer.get("result")
            if isinstance(result_text, str) and result_text.strip():
                tail = (tail + "\n--- result ---\n" + result_text)[
                    -_STDERR_TAIL_BYTES:
                ]
        return None, {"status": "nonzero", "stderr_tail": tail}

    # `claude --output-format json` returns an envelope like
    # {"result": "<assistant text>", "is_error": false, ...}. Some CLI
    # versions print preamble lines before the JSON; tolerate by
    # scanning for the first `{`.
    outer = _parse_json_from_text(proc.stdout)
    if outer is None:
        return None, {
            "status": "malformed",
            "stderr_tail": _tail(proc.stderr),
        }

    # The CLI can exit 0 but flag `is_error: true` for content-policy
    # rejections, auth issues that mid-flight surface as JSON, etc.
    # Treat that as nonzero — the result text is usually an error
    # message, not the structured output we asked for.
    if outer.get("is_error") is True:
        result_text = outer.get("result")
        tail = (
            f"is_error=true: {result_text!s}"[-_STDERR_TAIL_BYTES:]
            if isinstance(result_text, str) else "is_error=true"
        )
        return None, {"status": "nonzero", "stderr_tail": tail}

    inner_text = _extract_assistant_text(outer)
    if inner_text is None:
        return None, {
            "status": "malformed",
            "stderr_tail": _tail(proc.stderr),
        }

    inner = _parse_json_from_text(inner_text)
    if inner is None:
        return None, {
            "status": "malformed",
            "stderr_tail": _tail(proc.stderr),
        }

    return inner, {"status": "ok", "stderr_tail": ""}


def _parse_json_from_text(text: str) -> dict | None:
    """Best-effort JSON-object parser tolerating preamble + trailing junk.

    Strategy: try `json.loads` first. On failure, scan for the first
    `{` and use `JSONDecoder.raw_decode` which parses one JSON value
    and reports the offset where it ends — letting us ignore trailing
    markdown fences (```), prose tails, or anything else the model
    appended after its JSON object.

    Returns `None` if no parse succeeds or if the parsed value isn't a
    dict.
    """
    if not text:
        return None
    try:
        val = json.loads(text)
        return val if isinstance(val, dict) else None
    except json.JSONDecodeError:
        pass
    decoder = json.JSONDecoder()
    brace = text.find("{")
    while brace >= 0:
        try:
            val, _end = decoder.raw_decode(text[brace:])
        except json.JSONDecodeError:
            brace = text.find("{", brace + 1)
            continue
        return val if isinstance(val, dict) else None
    return None


def _extract_assistant_text(outer: dict) -> str | None:
    """Pull the assistant's textual reply out of the CLI's JSON envelope.

    `claude --output-format json` varies slightly across versions; try
    the documented `result` key first, then a few common fallbacks.
    """
    for key in ("result", "text", "content", "message"):
        val = outer.get(key)
        if isinstance(val, str) and val.strip():
            return val
    # Some versions wrap content in a list of blocks.
    for key in ("content", "messages"):
        val = outer.get(key)
        if isinstance(val, list):
            for block in val:
                if isinstance(block, dict) and isinstance(block.get("text"), str):
                    return block["text"]
    return None


def _tail(s: str) -> str:
    if not s:
        return ""
    if len(s) <= _STDERR_TAIL_BYTES:
        return s
    return s[-_STDERR_TAIL_BYTES:]


def _matches_schema(value: Any, schema: dict) -> bool:
    """Minimal schema check — type + required keys + nested arrays.

    We don't ship a full JSON-Schema validator; the schema is
    authored-and-reviewed-with the code, so a structural sniff is
    enough to catch the model returning the wrong shape.
    """
    typ = schema.get("type")
    if typ == "object":
        if not isinstance(value, dict):
            return False
        for req in schema.get("required", []):
            if req not in value:
                return False
        props = schema.get("properties", {})
        for k, v in value.items():
            sub = props.get(k)
            if sub is not None and not _matches_schema(v, sub):
                return False
        return True
    if typ == "array":
        if not isinstance(value, list):
            return False
        max_items = schema.get("maxItems")
        if max_items is not None and len(value) > max_items:
            return False
        item_schema = schema.get("items")
        if item_schema is not None:
            return all(_matches_schema(item, item_schema) for item in value)
        return True
    if typ == "string":
        if not isinstance(value, str):
            return False
        max_len = schema.get("maxLength")
        if max_len is not None and len(value) > max_len:
            return False
        return True
    if typ == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    if typ == "number":
        return isinstance(value, (int, float)) and not isinstance(value, bool)
    if typ == "boolean":
        return isinstance(value, bool)
    if typ == "null":
        return value is None
    return True


def enrichment_enabled_via_env() -> bool:
    """True iff env says enrichment should fire.

    `LOGQ_ENRICH=1` opts in. `LOGQ_ENRICH=0` opts out (overrides
    presence-implies-on if a caller wants to gate ad-hoc). When
    `CLAUDE_SESSION_ID` is set we're nested inside a parent
    `claude` session (e.g., a polecat-child invocation of `just q`);
    skip to avoid double-billing and re-entrancy weirdness.
    """
    if os.environ.get("CLAUDE_SESSION_ID"):
        return False
    val = os.environ.get("LOGQ_ENRICH", "").strip().lower()
    return val in {"1", "true", "yes", "on"}
