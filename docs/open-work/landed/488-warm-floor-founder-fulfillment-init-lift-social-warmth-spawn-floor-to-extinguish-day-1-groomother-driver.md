---
id: 488
title: warm-floor founder Fulfillment init — lift social_warmth spawn floor to extinguish day-1 GroomOther driver
status: done
cluster: ai-substrate
initiative: []
orchestration: substrate-sensitive
added: 2026-05-30
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-30
---

## Why

487 landed three substrate layers + two latent-defect fixes that
narrowed the day-1 "cuddle puddle" (Simba L3 Grooming-disposition
share 36.8% → 4.2%), but the puddle was an eligibility-and-resolver
fix. The **underlying SELF-state driver** that makes `GroomOtherDse`
score high on tick 0 still fires — and on inspection the substrate
is doing exactly what it's told.

`GroomOtherDse` is CompensatedProduct over three axes (`groom_other.rs:60-90`):

- `warmth` — reads `Personality.warmth` (a static trait; high for
  warm-personality founders).
- `phys_satisfaction` — high at spawn (founders spawn well-fed).
- `social_warmth_deficit` — reads `Fulfillment.social_warmth_deficit()`
  (= `1.0 - social_warmth`).

The third axis is the legitimate-need driver, and **founders spawn with
`social_warmth ∈ [0.5, 0.7]`** via `Fulfillment::staggered(i, n)`
(`src/plugins/setup.rs:478` → `src/components/fulfillment.rs:54-61`).
That's a 30-50% deficit at game tick 0. The DSE is honestly responding
to a real, mechanical, substrate-stated need: cats fulfill the action
whose purpose is to refill that bank.

The mismatch is architectural: **b24d333b warm-floored founder
`Relationships`** (fondness/familiarity pre-baked so first-tick novelty
axes wouldn't dominate target picking) **but did not touch the
`Fulfillment` bank** (the self-state need driver). The two banks model
the same fiction — "founders arrived from somewhere and are not
strangers to each other" — but only one was migrated.

`Fulfillment::newborn()` (line 77) already implements this exact
pattern for kittens: `social_warmth: 0.9` because "newborn arrives in a
maximally-bonded post-gestation state." Founders need the same lift,
for the same reason: they arrived from an established prior social
context, not from isolation.

## Scope

- New `Fulfillment::founder(index, group_size)` constructor (or amend
  `staggered` with a `founder: bool` parameter) producing
  `social_warmth ∈ [0.85, 1.0]` staggered the same way the current
  `[0.5, 0.7]` is. Use the existing stagger shape so per-cat phase
  offset is preserved (avoids same-tick mass-threshold-crossings —
  the original rationale for `staggered`).
- Update the founder spawn call at `src/plugins/setup.rs:478` to call
  the new constructor.
- Keep `Fulfillment::default()` unchanged at 0.6 — that's the
  legitimate "no prior history" spawn condition (e.g. test fixtures,
  save-load fallback). Founders are the special case, not the default.
- Add a unit test asserting founder-spawned cats start with
  `social_warmth_deficit ≤ 0.15` (so day-1 GroomOther's third axis
  contributes at floor, not as a moderate driver).

## Out of scope

- Touching the passive refill rate (`social_warmth_bond_proximity_rate`
  in `src/systems/fulfillment.rs:70-97`) — the alternative lever I
  considered, but it shapes the long-term flow, not the day-1 stock.
  This ticket fixes the spawn stock; flow tuning is a separate
  question if post-fix soaks show the bank decays unhealthily.
- Changing `Fulfillment::newborn()` (already correct at 0.9).
- Restructuring `GroomOtherDse`'s composition shape — the substrate is
  fine; the input is wrong.
- Tuning `Fulfillment` decay rate — orthogonal to the spawn condition.
- Anything to do with `Needs.warmth` (temperature) — distinct axis;
  see ticket 12 for the original split.

## Current state

- 487 landed at SHA `9a05a29c` (2026-05-29) — `HasGroomCandidate`
  marker, colony-self directives, emergent-coordinator alignment,
  resolver-side `currently_groomed` filter, FeedKitten-newborn
  carve-out. Headline: Simba first-5k-tick Grooming share dropped
  36.8% → 4.2%. Open follow-ons named in 487 Log: Patrol absorption
  (Exploring 90.9% of freed bandwidth), `BuildingRepaired=0`, 0
  matings despite 3189 courtship-ticks. This ticket addresses a
  *different* root cause from those follow-ons — the source of the
  GroomOther scoring pressure itself.
- b24d333b landed warm-floor founder `Relationships` init — the
  pre-existing pattern this ticket mirrors. Same reasoning ("founders
  came from somewhere"), different substrate bank.
- 24 (Fulfillment register MVP) and 12 (Warmth split) are the
  substrate landings that made this bank exist. Both `done`.

## Approach

1. **Audit current spawn footprint.** Confirm `setup.rs:478` is the
   only founder spawn site touching `Fulfillment`. Check whether the
   `Fulfillment::staggered` shape is referenced anywhere else (test
   fixtures, scenarios) that might break under a value change.
2. **Pick the floor range.** `[0.85, 1.0]` is the obvious mirror of
   `Fulfillment::newborn()`'s 0.9. Specifically:
   - `social_warmth = 1.0 - t * 0.15` over `t = i / (n-1)`, giving
     `[1.0, 0.85]` linearly staggered.
   - Single-cat group: `social_warmth = 0.95` (mid-range, mirrors
     current `staggered_single_cat_uses_default` pattern).
3. **Naming.** Prefer a new `Fulfillment::founder(i, n)` constructor
   over a flag on `staggered`. Two reasons: (a) the existing
   `staggered` is referenced in tests that assert the [0.5, 0.7]
   range; preserving its semantics avoids churn. (b) `founder()`
   names the design intent explicitly — mirrors `newborn()`'s shape
   and reads at the call site as "this is the founder-spawn variant."
4. **Wire the call site.** `setup.rs:478` becomes
   `Fulfillment::founder(i, cat_count)`.
5. **Test.** Unit test in `fulfillment.rs::tests`:
   `founder_starts_with_low_social_warmth_deficit` — assert deficit
   ≤ 0.15 across all founders in a typical 5-cat clowder.

## Verification

- `cargo test --lib` — new test passes, prior `Fulfillment` tests
  unchanged.
- `just check` — substrate-stubs / silent-canary / compile-time
  contracts unaffected.
- Focal soak on seed-42, 5-min: Simba first-5k-tick Grooming-
  disposition share should drop further from the post-487 4.2%
  toward whatever the structural floor is once the SELF driver is
  near zero. Expected: Grooming below 1.5% but non-zero (continuity
  canary `grooming` still ≥ 1).
- Freed bandwidth observation — Patrol absorption may shrink as
  GroomOther stops winning even when eligible, OR may persist if the
  Patrol absorption is its own structural issue separate from this
  one (a tell either way for the 487 follow-on tickets).
- `just verdict <run-dir>` — hard gates pass, continuity canaries
  hold, drift bands inside expected envelope.

## Log
- 2026-05-30: opened from a /next session investigation following
  487's landing. User observed "we're still cuddle puddling at the
  start" and asked why cats "desperately need grooming at the start."
  Audit traced the SELF driver to `Fulfillment::staggered`'s
  [0.5, 0.7] spawn range — founders spawn 30-50% socially-warmth-
  deficient *by design*. The 487 layers gated eligibility and resolver
  picking but left the underlying need untouched. This ticket lifts
  the spawn stock to match the warm-floor `Relationships` pattern
  b24d333b established, on the same architectural rationale
  ("founders arrived from somewhere; their banks should reflect
  prior social context").
- 2026-05-30: landed. Founder Fulfillment::founder(i, n) constructor lifts social_warmth ∈ [0.85, 1.0] staggered, mirroring Fulfillment::newborn()'s 0.9 pattern. Replaces the staggered [0.5, 0.7] floor at src/plugins/setup.rs:478 that drove a 30-50% social_warmth_deficit at game tick 0 — the SELF-state driver of the day-1 GroomOther cuddle puddle that 487 narrowed at the eligibility/resolver layer. Architectural mirror of b24d333b's warm-floor Relationships init: same fiction ('founders arrived from somewhere'), second substrate bank. 2578 lib tests pass (3 new), just check green.
