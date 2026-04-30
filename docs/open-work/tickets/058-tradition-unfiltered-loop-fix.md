---
id: 058
title: §3.5.3 item 1 Tradition modifier — fix unfiltered-loop port
status: parked
cluster: null
added: 2026-04-27
parked: 2026-04-30
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

## Substrate-over-override pattern

Part of the substrate-over-override thread (see [093](093-substrate-over-override-epic.md)).

**Hack shape**: modifier applies a coefficient uniformly across DSEs when it should be action-matched. Tradition boosts Hunt/Patrol/Explore equally on a tile where the cat only ever Foraged — over-broad modifier semantics, currently dormant only because the caller-side bonus is 0.0.

**IAUS lever**: per-action-keyed history axis (option (a)) — `HashMap<Action, f32>` per tile, modifier reads per-DSE-id and adds only on action match. Or redeclare as flat tile-familiarity bonus (option (b)) — formalize Tradition as a location-affinity axis independent of action. Either way, the lever is **history-of-place as a first-class IAUS signal**.

**Sequencing**: no substrate prerequisite (this ticket *is* the substrate fix). Behavior-neutral landing while bonus = 0.0; balance ticket downstream sets the non-zero bonus per the four-artifact methodology.

**Canonical exemplar**: 087 (CriticalHealth interrupt → `pain_level` + `body_distress_composite` axes, landed at fc4e1ab).

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
- 2026-04-30: Parked. The Tradition modifier is dormant in production
  (`tradition_location_bonus = 0.0` hard-coded at
  `src/systems/goap.rs:1330`), so the unfiltered-loop smell is
  invisible on every shipping path. The design choice between (a)
  per-action-keyed structural fix and (b) flat tile-familiarity
  reframe is best decided alongside the magnitude question — i.e.,
  as a balance ticket when someone wants the bonus turned on.
  Pre-emptive refactor front-loads cost on a knob that may never
  activate, or that may activate with a shape that doesn't fit
  either (a) or (b). Unpark when a balance ticket opens for
  Tradition's bonus magnitude.
