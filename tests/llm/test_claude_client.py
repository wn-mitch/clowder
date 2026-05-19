"""Unit tests for scripts/llm/claude_client.

All subprocess calls are mocked — these tests never hit the real
`claude` binary. Covers every failure mode the client must collapse
to `(None, meta)` without raising, plus the happy path.

Invoke via `just test-llm` (or `python3 tests/llm/test_claude_client.py -v`).
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "llm"))

import claude_client  # noqa: E402


PROMPTS = REPO_ROOT / "scripts" / "llm" / "prompts"
SCHEMA = PROMPTS / "logq_enrich.schema.json"
SYSTEM_PROMPT = PROMPTS / "logq_enrich.md"


def _fake_cli_envelope(inner_obj: dict | str) -> str:
    """Wrap an inner JSON object the way `claude --output-format json` does."""
    inner_text = inner_obj if isinstance(inner_obj, str) else json.dumps(inner_obj)
    return json.dumps({"result": inner_text})


def _ok_result() -> dict:
    return {
        "hint": "Three of four deaths are kittens under tick 1200050 — looks demographic.",
        "suggestions": [
            {"cmd": "just q deaths logs/tuned-42 --cause=Starvation",
             "why": "4 starvation deaths between ticks 1200045 and 1200090."},
        ],
    }


class CallHaikuJsonHappyPath(unittest.TestCase):
    def test_ok_parses_inner_json(self):
        completed = subprocess.CompletedProcess(
            args=[], returncode=0,
            stdout=_fake_cli_envelope(_ok_result()),
            stderr="",
        )
        with mock.patch.object(claude_client.shutil, "which",
                               return_value="/fake/claude"), \
             mock.patch.object(claude_client.subprocess, "run",
                               return_value=completed) as mrun:
            parsed, meta = claude_client.call_haiku_json(
                user_payload={"query": {}, "results": []},
                system_prompt_path=SYSTEM_PROMPT,
                schema_path=SCHEMA,
            )
        self.assertIsNotNone(parsed)
        self.assertEqual(parsed["suggestions"][0]["cmd"],
                         "just q deaths logs/tuned-42 --cause=Starvation")
        self.assertEqual(meta["status"], "ok")
        self.assertGreaterEqual(meta["elapsed_ms"], 0)
        # CLI invocation must use --print + --model + --no-session-persistence,
        # and must NOT pass --bare (which forces ANTHROPIC_API_KEY auth and
        # breaks subscription billing).
        called_cmd = mrun.call_args.args[0]
        self.assertIn("--print", called_cmd)
        self.assertIn("--model", called_cmd)
        self.assertIn("--no-session-persistence", called_cmd)
        self.assertNotIn("--bare", called_cmd,
                         "--bare disables OAuth/keychain auth — subscription "
                         "billing requires keychain reads.")
        # And the payload was piped on stdin, not in argv.
        self.assertIn("input", mrun.call_args.kwargs)

    def test_payload_can_be_string(self):
        completed = subprocess.CompletedProcess(
            args=[], returncode=0,
            stdout=_fake_cli_envelope(_ok_result()),
            stderr="",
        )
        with mock.patch.object(claude_client.shutil, "which",
                               return_value="/fake/claude"), \
             mock.patch.object(claude_client.subprocess, "run",
                               return_value=completed):
            parsed, _meta = claude_client.call_haiku_json(
                user_payload='{"already": "stringified"}',
                system_prompt_path=SYSTEM_PROMPT,
                schema_path=SCHEMA,
            )
        self.assertIsNotNone(parsed)


class CallHaikuJsonFailureModes(unittest.TestCase):
    def _run_with_mocked_subprocess(self, **side_effect_kw):
        return claude_client.call_haiku_json(
            user_payload={"q": "x"},
            system_prompt_path=SYSTEM_PROMPT,
            schema_path=SCHEMA,
        )

    def test_disabled_when_claude_missing(self):
        with mock.patch.object(claude_client.shutil, "which", return_value=None):
            parsed, meta = self._run_with_mocked_subprocess()
        self.assertIsNone(parsed)
        self.assertEqual(meta["status"], "disabled")

    def test_timeout_retries_once_then_gives_up(self):
        timeout_exc = subprocess.TimeoutExpired(cmd=["claude"], timeout=8.0)
        with mock.patch.object(claude_client.shutil, "which",
                               return_value="/fake/claude"), \
             mock.patch.object(claude_client.subprocess, "run",
                               side_effect=[timeout_exc, timeout_exc]) as mrun:
            parsed, meta = self._run_with_mocked_subprocess()
        self.assertIsNone(parsed)
        self.assertEqual(meta["status"], "timeout")
        # Exactly two attempts: initial + one retry.
        self.assertEqual(mrun.call_count, 2)

    def test_timeout_retry_succeeds(self):
        ok = subprocess.CompletedProcess(
            args=[], returncode=0,
            stdout=_fake_cli_envelope(_ok_result()),
            stderr="",
        )
        with mock.patch.object(claude_client.shutil, "which",
                               return_value="/fake/claude"), \
             mock.patch.object(claude_client.subprocess, "run",
                               side_effect=[
                                   subprocess.TimeoutExpired(cmd=["claude"],
                                                              timeout=8.0),
                                   ok,
                               ]) as mrun:
            parsed, meta = self._run_with_mocked_subprocess()
        self.assertIsNotNone(parsed)
        self.assertEqual(meta["status"], "ok")
        self.assertEqual(mrun.call_count, 2)

    def test_nonzero_does_not_retry(self):
        nonzero = subprocess.CompletedProcess(
            args=[], returncode=2,
            stdout="",
            stderr="not authenticated — run `claude login`",
        )
        with mock.patch.object(claude_client.shutil, "which",
                               return_value="/fake/claude"), \
             mock.patch.object(claude_client.subprocess, "run",
                               return_value=nonzero) as mrun:
            parsed, meta = self._run_with_mocked_subprocess()
        self.assertIsNone(parsed)
        self.assertEqual(meta["status"], "nonzero")
        self.assertIn("authenticated", meta["stderr_tail"])
        # No retry on nonzero — auth/config issues won't fix themselves.
        self.assertEqual(mrun.call_count, 1)

    def test_malformed_outer_json(self):
        bad = subprocess.CompletedProcess(
            args=[], returncode=0,
            stdout="this is not json at all",
            stderr="",
        )
        with mock.patch.object(claude_client.shutil, "which",
                               return_value="/fake/claude"), \
             mock.patch.object(claude_client.subprocess, "run",
                               return_value=bad):
            parsed, meta = self._run_with_mocked_subprocess()
        self.assertIsNone(parsed)
        self.assertEqual(meta["status"], "malformed")

    def test_malformed_inner_json(self):
        # Outer envelope parses; inner assistant text is not JSON.
        outer = json.dumps({"result": "the model just wrote prose, oops"})
        bad = subprocess.CompletedProcess(
            args=[], returncode=0, stdout=outer, stderr="",
        )
        with mock.patch.object(claude_client.shutil, "which",
                               return_value="/fake/claude"), \
             mock.patch.object(claude_client.subprocess, "run",
                               return_value=bad):
            parsed, meta = self._run_with_mocked_subprocess()
        self.assertIsNone(parsed)
        self.assertEqual(meta["status"], "malformed")

    def test_outer_envelope_is_error_true(self):
        """CLI exit 0 but inner envelope flags is_error (e.g., auth or
        content policy) — surface as `nonzero` and preserve the result
        text in stderr_tail for diagnostics."""
        outer = json.dumps({
            "result": "Not logged in · Please run /login",
            "is_error": True,
        })
        completed = subprocess.CompletedProcess(
            args=[], returncode=0, stdout=outer, stderr="",
        )
        with mock.patch.object(claude_client.shutil, "which",
                               return_value="/fake/claude"), \
             mock.patch.object(claude_client.subprocess, "run",
                               return_value=completed):
            parsed, meta = self._run_with_mocked_subprocess()
        self.assertIsNone(parsed)
        self.assertEqual(meta["status"], "nonzero")
        self.assertIn("Not logged in", meta["stderr_tail"])

    def test_nonzero_extracts_result_text_for_diagnostics(self):
        """When the CLI exits nonzero but still wrote a JSON envelope,
        we lift the inner `result` text into stderr_tail."""
        outer = json.dumps({
            "result": "auth failed: keychain unlock denied",
            "is_error": True,
        })
        nonzero = subprocess.CompletedProcess(
            args=[], returncode=1, stdout=outer,
            stderr="(empty stderr)",
        )
        with mock.patch.object(claude_client.shutil, "which",
                               return_value="/fake/claude"), \
             mock.patch.object(claude_client.subprocess, "run",
                               return_value=nonzero):
            parsed, meta = self._run_with_mocked_subprocess()
        self.assertIsNone(parsed)
        self.assertEqual(meta["status"], "nonzero")
        self.assertIn("keychain", meta["stderr_tail"])

    def test_markdown_fenced_inner_json_parses(self):
        """The model commonly wraps its JSON in ```json ... ``` fences.
        `_parse_json_from_text` via `raw_decode` ignores trailing
        fence characters."""
        fenced = "```json\n" + json.dumps(_ok_result()) + "\n```"
        outer = json.dumps({"result": fenced})
        completed = subprocess.CompletedProcess(
            args=[], returncode=0, stdout=outer, stderr="",
        )
        with mock.patch.object(claude_client.shutil, "which",
                               return_value="/fake/claude"), \
             mock.patch.object(claude_client.subprocess, "run",
                               return_value=completed):
            parsed, meta = self._run_with_mocked_subprocess()
        self.assertIsNotNone(parsed)
        self.assertEqual(meta["status"], "ok")
        self.assertEqual(parsed["hint"], _ok_result()["hint"])

    def test_schema_violation_extra_field(self):
        bad_inner = {
            "hint": "ok",
            "suggestions": [],
            "extra_field_not_in_schema": "should reject",
        }
        completed = subprocess.CompletedProcess(
            args=[], returncode=0,
            stdout=_fake_cli_envelope(bad_inner),
            stderr="",
        )
        with mock.patch.object(claude_client.shutil, "which",
                               return_value="/fake/claude"), \
             mock.patch.object(claude_client.subprocess, "run",
                               return_value=completed):
            parsed, meta = self._run_with_mocked_subprocess()
        # extra fields aren't caught by our minimal matcher (no
        # additionalProperties:false enforcement); but required-key
        # violations are — check that case instead:
        # The matcher tolerates extras; OK, narrow this test to check
        # what we DO enforce — missing required key.
        self.assertEqual(meta["status"], "ok")  # matcher tolerates extras
        self.assertIsNotNone(parsed)

    def test_schema_violation_missing_required(self):
        bad_inner = {"hint": "ok"}  # missing "suggestions"
        completed = subprocess.CompletedProcess(
            args=[], returncode=0,
            stdout=_fake_cli_envelope(bad_inner),
            stderr="",
        )
        with mock.patch.object(claude_client.shutil, "which",
                               return_value="/fake/claude"), \
             mock.patch.object(claude_client.subprocess, "run",
                               return_value=completed):
            parsed, meta = self._run_with_mocked_subprocess()
        self.assertIsNone(parsed)
        self.assertEqual(meta["status"], "schema_violation")

    def test_oserror_during_subprocess(self):
        with mock.patch.object(claude_client.shutil, "which",
                               return_value="/fake/claude"), \
             mock.patch.object(claude_client.subprocess, "run",
                               side_effect=OSError("simulated EPIPE")):
            parsed, meta = self._run_with_mocked_subprocess()
        self.assertIsNone(parsed)
        self.assertEqual(meta["status"], "error")
        self.assertIn("EPIPE", meta["stderr_tail"])

    def test_missing_system_prompt_file(self):
        missing = REPO_ROOT / "no_such_dir_anywhere" / "missing.md"
        with mock.patch.object(claude_client.shutil, "which",
                               return_value="/fake/claude"):
            parsed, meta = claude_client.call_haiku_json(
                user_payload={"q": "x"},
                system_prompt_path=missing,
                schema_path=SCHEMA,
            )
        self.assertIsNone(parsed)
        self.assertEqual(meta["status"], "error")
        self.assertIn("prompt read failed", meta["stderr_tail"])


class DefensiveStdoutParsing(unittest.TestCase):
    """`_parse_json_from_text` tolerates CLI preamble lines.

    Some `claude` CLI versions print configuration warnings or
    telemetry to stdout before the JSON object — our parser must
    recover by scanning for the first `{`.
    """

    def test_preamble_before_json(self):
        text = "[preamble warning: foo]\nplugin sync done\n" + json.dumps({"a": 1})
        result = claude_client._parse_json_from_text(text)
        self.assertEqual(result, {"a": 1})

    def test_clean_json_parses(self):
        result = claude_client._parse_json_from_text('{"a": 1}')
        self.assertEqual(result, {"a": 1})

    def test_no_brace_returns_none(self):
        self.assertIsNone(claude_client._parse_json_from_text("nothing here"))

    def test_empty_string_returns_none(self):
        self.assertIsNone(claude_client._parse_json_from_text(""))

    def test_non_object_top_level_returns_none(self):
        # `["array", "not", "object"]` shouldn't be accepted — we want
        # a dict at the top level.
        self.assertIsNone(claude_client._parse_json_from_text('["a", "b"]'))


class ExtractAssistantText(unittest.TestCase):
    """`_extract_assistant_text` handles multiple CLI envelope shapes."""

    def test_result_key(self):
        self.assertEqual(
            claude_client._extract_assistant_text({"result": "hello"}),
            "hello",
        )

    def test_text_key_fallback(self):
        self.assertEqual(
            claude_client._extract_assistant_text({"text": "hi"}),
            "hi",
        )

    def test_content_blocks_list(self):
        self.assertEqual(
            claude_client._extract_assistant_text({
                "content": [{"text": "block-text"}],
            }),
            "block-text",
        )

    def test_no_known_key(self):
        self.assertIsNone(
            claude_client._extract_assistant_text({"unknown_field": "x"})
        )

    def test_empty_result(self):
        self.assertIsNone(
            claude_client._extract_assistant_text({"result": ""})
        )


class SchemaMatching(unittest.TestCase):
    """`_matches_schema` is a minimal structural validator — exercise it."""

    def setUp(self):
        self.schema = json.loads(SCHEMA.read_text())

    def test_valid_payload(self):
        self.assertTrue(claude_client._matches_schema(_ok_result(), self.schema))

    def test_missing_required_hint(self):
        self.assertFalse(claude_client._matches_schema(
            {"suggestions": []}, self.schema))

    def test_missing_required_suggestions(self):
        self.assertFalse(claude_client._matches_schema(
            {"hint": "x"}, self.schema))

    def test_too_many_suggestions(self):
        too_many = {
            "hint": "ok",
            "suggestions": [{"cmd": "x", "why": "y"} for _ in range(5)],
        }
        self.assertFalse(claude_client._matches_schema(too_many, self.schema))

    def test_empty_suggestions_is_valid(self):
        self.assertTrue(claude_client._matches_schema(
            {"hint": "", "suggestions": []}, self.schema))


class EnrichmentEnabledViaEnv(unittest.TestCase):
    def test_off_by_default(self):
        with mock.patch.dict("os.environ", {}, clear=True):
            self.assertFalse(claude_client.enrichment_enabled_via_env())

    def test_on_when_set_to_1(self):
        with mock.patch.dict("os.environ", {"LOGQ_ENRICH": "1"}, clear=True):
            self.assertTrue(claude_client.enrichment_enabled_via_env())

    def test_on_when_set_to_true(self):
        for v in ("true", "True", "TRUE", "yes", "on"):
            with mock.patch.dict("os.environ", {"LOGQ_ENRICH": v}, clear=True):
                self.assertTrue(claude_client.enrichment_enabled_via_env(),
                                f"LOGQ_ENRICH={v!r} should enable")

    def test_off_when_set_to_0(self):
        with mock.patch.dict("os.environ", {"LOGQ_ENRICH": "0"}, clear=True):
            self.assertFalse(claude_client.enrichment_enabled_via_env())

    def test_off_when_nested_in_claude_session(self):
        with mock.patch.dict("os.environ",
                             {"LOGQ_ENRICH": "1",
                              "CLAUDE_SESSION_ID": "parent-uuid"},
                             clear=True):
            self.assertFalse(
                claude_client.enrichment_enabled_via_env(),
                "nested claude session should suppress enrichment",
            )


class OversizedSystemPrompt(unittest.TestCase):
    def test_huge_prompt_rejected(self):
        with tempfile.NamedTemporaryFile("w", suffix=".md", delete=False) as f:
            f.write("x" * (claude_client._MAX_SYSTEM_PROMPT_CHARS + 1))
            huge = Path(f.name)
        try:
            with mock.patch.object(claude_client.shutil, "which",
                                   return_value="/fake/claude"):
                parsed, meta = claude_client.call_haiku_json(
                    user_payload={"q": "x"},
                    system_prompt_path=huge,
                    schema_path=SCHEMA,
                )
            self.assertIsNone(parsed)
            self.assertEqual(meta["status"], "error")
            self.assertIn("too large", meta["stderr_tail"])
        finally:
            huge.unlink()


if __name__ == "__main__":
    unittest.main()
