---
id: 305
title: Corpus projection layer — split cluster into cluster+initiative, render two-axis, gate open-time
status: ready
cluster: process-discipline
orchestration: substrate-sensitive
initiative: []
added: 2026-05-13
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

The open-work corpus has crossed the threshold where flat status-major rendering
no longer projects intent. 134 open / 190 landed = 324 tickets, with the open
queue at **90 ready / 26 blocked / 10 parked / 7 in-progress** and adding ~3
tickets/day. Skimming `docs/open-work.md` (396 lines) tells you nothing about
what is *active*, what *groups together*, or what *load-bearing* — everything
blends in a stack. Two structural problems underneath:

1. `cluster:` is doing two jobs simultaneously — categorical (`ai-substrate`,
   `tooling`, `process-discipline`) and accidentally-thematic (`world-ecology`,
   `emotional-fidelity` exist as single-ticket clusters). One field, two axes,
   neither rendered well.
2. The substrate-refactor phase tags (`C`, `D`, `E`) live as cluster values,
   conflating "what phase of the refactor" with "what kind of work."

47% of open tickets (63/134) have `cluster: null` — the discipline is leaking
because there's no open-time gate, and the single-axis taxonomy isn't expressive
enough to motivate the work of tagging.

## Scope

- Add an `initiative:` frontmatter field (list of zero-or-more thematic
  outcomes) alongside the existing `cluster:` field; cluster stays categorical,
  one-per-ticket, required at open-time.
- Extend `scripts/generate_open_work.py` to emit three new projections:
  1. `## Active focus` section (in-progress + ready-that-blocks-active + top-5
     from `just next`)
  2. `## Ready by cluster` (cluster-major rendering with subheadings and counts)
  3. `## Ready by initiative` (initiative-major rollups; tickets may appear in
     multiple)
- Extend `scripts/create_ticket.py` (`just open-ticket`):
  - Refuse to create a ticket without `--cluster`, or interactively prompt
  - Optional `--initiative` flag accepting comma-separated list
- Extend `scripts/similar/chunkers.py` synthetic-header builder to include
  `initiative:` so embedded chunks carry the tag as semantic anchor; mirror in
  the per-chunk `metadata` dict.
- Land a starter cluster + initiative taxonomy and document it in
  `docs/open-work/initiatives/<name>.md` stubs (one paragraph each, naming the
  outcome and the canary signals).
- This ticket lands the *schema and rendering*. Tagging passes and `just`
  filter recipes are follow-on tickets.

## Out of scope

- **Back-catalogue tagging of 190 landed tickets** (initiative 5b). Follow-on
  ticket — uses the embedding-suggestion workflow this ticket lands and
  depends on the `initiative:` schema being live.
- **Cluster-and-initiative tagging pass over 63 untagged active tickets**
  (initiative 5a). Follow-on ticket — same dependency.
- **`just open-work-ready --cluster X` / `--initiative X` filter recipes**
  (interventions 6, 7). Follow-on ticket — cheap once the schema is live.
- **Epic rollup discipline lint** (intervention 7). Follow-on ticket.
- **Linear migration.** Out of scope. See plan file
  `~/.claude/plans/wondrous-greeting-tome.md` for the recommendation against.

## Current state

- `scripts/generate_open_work.py` reads frontmatter from
  `docs/open-work/tickets/*.md` and emits a flat status-major index. Two-axis
  rendering does not exist.
- `scripts/create_ticket.py` accepts `--cluster` as an optional flag. 47% of
  open tickets have `cluster: null` as a consequence.
- `scripts/similar/chunkers.py:170-194` prepends a synthetic header
  (`ticket {id} · status: {S} · cluster: {C} · title: {T}`) to every embedded
  chunk; per-chunk metadata carries `cluster`. `initiative` is not present
  anywhere.
- Existing clusters with counts: `ai-substrate` (26), `C` (19, refactor-phase
  tag), `balance` (10), `process-discipline` (6), `substrate-migration` (4),
  `world-ecology` (1), `tooling` (1), `emotional-fidelity` (1), `E` (1),
  `D` (1), `null` (63).

## Approach

Implementation in five small commits, each shippable on its own:

1. **Schema** — add `initiative: []` default to the two ticket templates
   (`_template.md`, `_template_bugfix.md`). No reader changes yet; the field
   sits idle.

2. **Chunker** — extend `scripts/similar/chunkers.py` (~line 170) to include
   `initiative:` in `header_bits` when present, and add `initiative` to the
   per-chunk `metadata` dict. Rebuild the embedding index
   (`just similar-build`).

3. **Index renderer** — extend `scripts/generate_open_work.py` to emit the
   three new sections (`## Active focus`, `## Ready by cluster`,
   `## Ready by initiative`). Keep the existing flat `## Ready` section
   during the transition; remove once the new sections feel right.

4. **Open-time gate** — extend `scripts/create_ticket.py` to require
   `--cluster` (or interactively prompt) and accept optional `--initiative`.

5. **Taxonomy stubs** — write `docs/open-work/initiatives/<name>.md` stubs
   for the 8 starter initiatives (per the plan file): `world-richness`,
   `full-sensory-perception`, `mythic-texture`, `environmental-simulation`,
   `smarter-cats`, `welfare-fidelity`, `generational-continuity`,
   `predator-prey-dynamics`. Document the cluster taxonomy inline in
   `CLAUDE.md` "Long-horizon coordination" or as a sibling
   `docs/open-work/clusters.md`.

Reference plan: `~/.claude/plans/wondrous-greeting-tome.md` — full taxonomy
table with example tickets per row and overlap notes.

## Verification

- `just check` passes (no regressions in index regen, no broken frontmatter
  parsing on existing tickets, embedding-index rebuild still works).
- `just open-ticket "test title"` refuses without `--cluster` (gate landed).
- `just open-ticket "test title" --cluster ai-substrate --initiative world-richness,smarter-cats` succeeds; frontmatter shows both fields.
- Re-read `docs/open-work.md` after regen: the `## Active focus` section
  surfaces all 7 in-progress tickets + their blockers; cluster-major
  rendering shows an "Uncategorized (63 ready)" headline; initiative-major
  rendering shows rollups across cluster lines.
- Falsifiability check (the test the plan file names): after this ticket
  lands, the diagnosis is "navigable without reading everything." If
  re-reading `docs/open-work.md` still feels homogeneous, the plan's premise
  is wrong and the Linear case needs revisiting.

## Log

- 2026-05-13: opened from corpus-hygiene plan
  (`~/.claude/plans/wondrous-greeting-tome.md`). Follow-on tickets to open in
  the same commit: tagging pass over active + landed; `just` filter recipes
  + epic-rollup lint. Both blocked-by 305.
