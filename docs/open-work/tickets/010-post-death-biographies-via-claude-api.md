---
id: 010
title: Post-death biographies via Claude API (presenter)
status: ready
cluster: null
added: 2026-04-21
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

**Why it matters:** Lights the **mythic-texture** continuity canary
(≥1 named event per sim year, currently zero from live-sim sources)
plus §5 **preservation** and **generational knowledge**. On `CatDied`
(or post-hoc over `logs/events.jsonl`), extract the cat's lifelog,
feed it to a prebuilt Claude API skill, emit prose into
`logs/biographies/<cat>.md`. The closest Clowder gets to DF's legend
mode.

**Architectural contract (load-bearing for the score):** LLM runs as
a **strict presenter** — reads finalized sim artifacts only, writes
sidecar files the sim never reads back. The `CLAUDE.md` "No LLMs"
rule defends authorial intent (sim behavior auditable back to math
the user wrote); presenter-only discipline is compatible with that
rule because the presenter contributes nothing to the `ground-truth →
math → outcome` chain. Audit test for the contract: `rm -rf
logs/biographies && just soak 42` produces byte-identical
`events.jsonl` + verification-tier `narrative.jsonl`. Assert this in
CI.

**Cross-reference:** `docs/systems-backlog-ranking.md` rank 1 —
V=4/F=4/R=4/C=4/H=4 → **1024** (cheap win; do first). Lands the
presenter-layer infrastructure (per-cat event indexing, Claude API
client, sidecar routing, CI audit test) that #11 below reuses.

**Open design choices:**
- Live-on-death vs. post-hoc log-processing tool. Post-hoc is
  strictly easier; live-on-death couples the sim binary to an
  external service.
- Sidecar directory vs. `narrative.jsonl` tier. **Strongly prefer
  sidecar** — keeping biographies out of verification-tier files
  preserves the byte-identical-across-matching-headers property that
  balance soaks rely on.
- Which lifelog events feed the prompt (cost and prose quality are
  both sensitive — more isn't better).

**Soft prerequisites:** audit whether every lifecycle-relevant event
in `logs/events.jsonl` carries a `cat_id` (spawns, significant
interactions, deaths); denormalize where missing.

**Memory write-back on landing:** commit an
`ongoing-tax-biographies` pattern memory per the skill's schema so
the next external-service triage has a prior to query.
