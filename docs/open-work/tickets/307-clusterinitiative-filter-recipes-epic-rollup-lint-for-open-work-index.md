---
id: 307
title: Cluster/initiative filter recipes + epic-rollup lint for open-work index
status: blocked
cluster: process-discipline
initiative: []
added: 2026-05-13
parked: null
blocked-by: [305]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

305 lands the schema and rendering; 306 backfills tags across the corpus.
This ticket lands the *query surface* — `just` recipes that filter the
tagged corpus + a lint that flags epic-roster gaps, plus the embedding-
pipeline extensions that make initiative-scoped retrieval possible.

The shape of the felt friction is "outside of `next` or `similar`, it's hard
to judge what is active." Filter recipes close that gap by giving every
session a `just open-work-active`, `just open-work-ready --cluster X`,
`just open-work-ready --initiative Y` surface. The initiative-aware
embedding queries (`--centroid`, `--not-tagged`) make the corpus a
discovery surface, not just a filter target.

## Scope

- Justfile recipes (extensions of existing `just open-work-*` family):
  - `just open-work-active` — surfaces the auto-generated `## Active focus`
    section (intervention 1) as a focused CLI view
  - `just open-work-ready --cluster <name>` — filter ready queue by cluster
  - `just open-work-ready --initiative <name>` — filter by initiative
  - `just open-work-stale [--days N]` — list `status: parked` tickets older
    than N days (default 30)
  - `just open-work-blocking <id>` — show transitive blockers of a given
    ticket
  - `just initiatives` — list active initiatives with member counts
    (X open / Y landed per initiative)
- Embedding-pipeline extensions (in `scripts/similar/`):
  - `just similar --initiative <name>` — neighbor query scoped to initiative
  - `just similar --centroid <initiative>` — embed-average all tagged
    members, return nearest neighbors (discovery surface for un-tagged
    tickets that *belong* in the initiative)
  - `just similar --not-tagged <initiative>` — negative-space query for
    the same discovery
  - `just next --initiative <name>` — weight by initiative-corpus similarity
- Epic-rollup lint (`just open-work-epics --check`):
  - Flag any clusterless ticket in a domain that has an active epic
  - Flag any active epic with a roster older than 30 days (to catch
    drift between siblings and roster children)

## Out of scope

- Schema/rendering changes (305).
- The tagging pass itself (306).
- Refactoring the existing `just open-work-ready` / `open-work-wip` recipes
  beyond what's needed for the new flags.

## Current state

Blocked on 305 (schema) and 306 (content). The query surface has no value
until both upstream tickets are populated.

## Approach

1. Extend justfile recipes one at a time; each is a small script that filters
   the auto-generated `docs/open-work.md` or queries frontmatter directly.
2. Extend `scripts/similar/retrieve.py` with `--initiative` and `--centroid`
   flags; the chunker change in 305 already puts initiative in metadata.
3. Write `scripts/epic_lint.py` for the rollup check; wire into
   `just open-work-epics --check`.
4. Update `CLAUDE.md` "Long-horizon coordination" section with the new
   query surface so future sessions reach for it.

## Verification

- `just open-work-ready --cluster ai-substrate` returns expected count
  (matches `## Ready by cluster → AI substrate` section).
- `just open-work-ready --initiative world-richness` returns rollup
  membership (matches `## Ready by initiative → World richness`).
- `just similar --centroid world-richness` returns coherent neighbors
  (qualitative; nearest neighbors should *feel like* world-richness work).
- `just similar --not-tagged world-richness` followed by manual review
  surfaces ≥1 ticket that should have been tagged but wasn't (proves the
  discovery loop works).
- `just open-work-epics --check` flags expected violations (induce one by
  removing a cluster from a known epic roster member).
- `just open-work-stale` returns the park-bankruptcy candidates from 306.

## Log

- 2026-05-13: opened as follow-on to 305 per the corpus-hygiene plan
  (`~/.claude/plans/wondrous-greeting-tome.md`).
