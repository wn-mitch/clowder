---
id: 058
title: §3.5.3 item 1 Tradition modifier — fix unfiltered-loop port
status: ready
cluster: null
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The `Tradition` modifier in `src/ai/modifier.rs` is a faithful port of
the retiring inline block — it applies the caller-pre-computed
`tradition_location_bonus` to **every** DSE rather than filtering by
the action whose history matched this tile.

Today this is a no-op in production: the caller at `goap.rs:900` sets
`tradition_location_bonus = 0.0`, so the unfiltered-loop never adds
anything visible. But shipping non-zero Tradition would expose the
bug.

## Scope

Spec §3.5.3 item 1 names two candidate fixes:

- **(a) Structural fix** — caller pre-computes a `HashMap<Action, f32>`
  keyed by the matched action; the modifier reads a per-DSE-id scalar
  and adds only on hits. Preserves the original *intent*: tile-specific
  history boosts the action that historically happened there.
- **(b) Semantic fix** — declare Tradition *is* a flat tile-familiarity
  bonus (not action-specific); update §2.3's Tradition row in the spec
  to formalize. Behavior: cats slightly prefer familiar tiles for
  *any* action, not specifically the action they did there before.

## Out of scope

- Setting the caller-side bonus to a non-zero value (that's a balance
  decision downstream of choosing (a) vs (b)).

## Approach

Resolving this is a behavior change under CLAUDE.md's Balance
Methodology — requires a hypothesis + prediction + measured A/B +
concordance before landing. Author a balance doc framing the choice
between (a) and (b), run a sweep on whichever option is chosen with a
non-zero bonus, validate against prediction.

Recommended order: pick (a) or (b) as the design choice (a is more
faithful to the spec's original intent; b is simpler), implement,
land with bonus = 0.0 still in production (refactor-scope, behavior-
neutral), then a separate balance ticket sets a non-zero bonus.

## Verification

- Unit tests for whichever variant: (a) modifier reads per-DSE-id and
  adds only on hits; (b) modifier adds flat bonus to every DSE on
  familiar tiles.
- `just check` green.
- Soak verdict: behavior-neutral if landed with bonus = 0.0; behavior
  shift documented per balance methodology when bonus > 0.

## Log

- 2026-04-27: opened from ticket 013 retirement (spec-follow-on debts
  umbrella decomposition). Original sub-task 13.7 in spec
  `docs/systems/ai-substrate-refactor.md` §3.5.3 item 1.
