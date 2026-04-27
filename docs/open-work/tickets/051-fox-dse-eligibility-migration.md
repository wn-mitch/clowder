---
id: 051
title: Fox DSE eligibility migration — `.require()`/`.forbid()` cutover for §4 fox markers
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

Ticket 014 (landed 2026-04-27) authored 8 §4 fox markers (4 spatial +
4 lifecycle) and populated the per-fox `MarkerSnapshot` for the first
time inside `fox_evaluate_and_plan`. But the fox-side DSEs still
read the old inline `FoxScoringContext` boolean fields
(`store_visible`, `store_guarded`, `cat_threatening_den`, `has_cubs`,
`cubs_hungry`, `is_dispersing_juvenile`, `has_den`) as outer gates in
`fox_scoring.rs::score_fox_dispositions`.

The cat-side parallel migration (Mentoring batch in Commit 1, plus
Magic colony / sensing batches) retired those inline gates by
adding `.require(KEY)` / `.forbid(KEY)` to each DSE's
`EligibilityFilter`. This ticket finishes that pattern on the fox
side.

## Scope

DSE-by-DSE eligibility cutover for fox-side DSEs that today carry
hardcoded outer gates in `fox_scoring.rs:280–370`:

- **`FoxRaidingDse`** — `.require(StoreVisible::KEY).forbid(StoreGuarded::KEY)`,
  retire the `store_visible && !store_guarded` outer gate.
- **`FoxDenDefenseDse`** — `.require(CatThreateningDen::KEY)`, retire
  the `cat_threatening_den && has_cubs` outer gate (HasCubs already
  implied by the predicate but also gate explicitly for symmetry).
- **`FoxFeedingDse`** — `.require(HasCubs::KEY).require(CubsHungry::KEY)`,
  retire the `has_cubs && cubs_hungry` outer gate.
- **`FoxDispersingDse`** — `.require(IsDispersingJuvenile::KEY)`,
  retire the `is_dispersing_juvenile` outer gate. Optionally
  `.forbid(HasDen::KEY)` for symmetry.

Then retire the `FoxScoringContext` boolean fields entirely:
`store_visible`, `store_guarded`, `cat_threatening_den`, `has_cubs`,
`cubs_hungry`, `is_dispersing_juvenile`, `has_den`. The `ward_nearby`
field can stay for now (no consumer; ticket 050 promotes the
predicate to truthful).

Update `build_scoring_context` in `fox_goap.rs` to drop the inline
computations now retired by markers.

## Out of scope

- Predicate refinements (ticket 050 covers `WardNearbyFox` truth +
  event-driven authoring + species-attenuated threat).
- New fox DSEs.
- §9.2 faction overlay (ticket 049).

## Approach

The cat-side Mentoring batch (commit `56f0586`) is the canonical
pattern: add `.require(KEY)` to the DSE constructor, retire the
inline `if ctx.X` outer gate, retire the ScoringContext field. Each
fox-side DSE follows the same pattern.

Each DSE gets its own commit so a soak regression is bisectable to
which gate's migration shifted behavior.

Behavior preservation: today's outer gates short-circuit scoring
entirely (`if !store_visible { return None; }`); eligibility filters
skip the DSE at evaluator level. The two paths should be
behavior-identical for these gates because the outer gate is the
final filter before DSE scoring — but watch the soak `wards_placed_total`
/ `shadow_foxes_avoided_ward_total` deltas as a sanity check.

## Verification

- Lib tests: `score_fox_dse_by_id` returns 0.0 when eligibility
  fails; integration tests for each DSE's gate.
- `just check` green per commit.
- Soak verdict on canonical seed-42: behavior-neutral expected.
  Compare `wards_placed_total` / `FoxStoreRaided` / `FoxDenDefense`
  / `FoxCubMatured` / FoxBred-related counters against
  `logs/baseline-2026-04-25/`.

## Log
- 2026-04-27: opened from ticket 014 closeout. Fox-side DSE
  eligibility migration was deferred from Commit 5/6 to keep those
  commits focused on author + snapshot plumbing without changing
  the eligibility surface.
