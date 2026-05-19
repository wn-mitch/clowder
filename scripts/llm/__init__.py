"""Shared LLM-presenter infrastructure.

Strict-presenter discipline (per ticket 010): the LLM reads finalized
artifacts and writes sidecar output the sim never reads back. Nothing
in this package can influence sim determinism.

Subscription-billed via the `claude` CLI in `--bare --print` mode — no
`ANTHROPIC_API_KEY` involved. See `claude_client.py`.

First caller: `scripts/logq/logq.py` (ticket 417 — `next` field
enrichment). Future callers: tickets 010 (biographies), 011 (cat
conversations).
"""
