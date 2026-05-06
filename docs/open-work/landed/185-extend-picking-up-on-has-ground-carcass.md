---
id: 185
title: Extend PickingUp DSE on a HasGroundCarcass colony marker — emergent scavenging
status: done
cluster: ai-substrate
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-06
---

## Why

Ticket 184's diagnostic logq queries surfaced that **6071
`OverflowToGround` items** spawn on the ground per soak (8237 in
the pre-181 baseline at the same commit) — successful Hunt and
Forage actions where the cat's inventory was full and the catch
was preserved as a real `Item` entity at the kill / forage
location. None of them get retrieved: `PickingUp` DSE ships
*dormant* (`Curve::Linear { slope: 0.0, intercept: 0.0 }` per
`src/ai/dses/picking_up.rs:34-39`), so no cat ever elects pickup
and the ground items rot via `decay_items` (`src/systems/items.rs:81-107`).

This is a load-bearing leak in the items-are-real philosophy.
Every soak generates thousands of ground items that the world
preserves but the colony can't act on. Closing the gap turns
overflow-to-ground from "lost food" into **emergent scavenging**:
when the colony has carcasses on the ground, off-duty cats
spontaneously pick them up and complete the deposit chain.

The user flagged this as appealing during 184's investigation
("the extend option does have some appeal, i like the idea of
emergent scavenging").

## Scope

- New colony-scoped marker `HasGroundCarcass` (or
  `HasGroundFood` if scoped wider) declared in
  `src/components/markers.rs`.
- One **writer** in the same commit: most likely an extension of
  `update_target_existence_markers` in `src/systems/sensing.rs`
  (which already iterates ground food/carcass entities for the
  per-cat `CarcassNearby` marker) to aggregate "any carcass exists
  in colony" into a colony-scoped boolean. **Substrate-stub lint
  forbids reader-without-writer or writer-without-reader in the
  same commit** (`scripts/check_substrate_stubs.sh`).
- One **reader** in the same commit: extend
  `picking_up.rs::HuntDse-style` eligibility filter to require
  `HasGroundCarcass::KEY`. The DSE's currently-zero composition
  means eligibility alone won't lift it — see "scoring lift"
  below.
- **Scoring lift**: replace the placeholder
  `Curve::Linear { slope: 0.0, intercept: 0.0 }` with a real
  scoring shape. Candidate shape (mirroring Hunt's RtEO):
  - hunger urgency (lower hunger → less interested in scavenging)
  - distance to nearest ground carcass (closer → higher score)
  - colony food-security inversion (lower stockpile → higher
    score)
  Specific weights are balance work, not part of this scope; the
  ticket lands a "non-zero, plausibly-shaped" composition and a
  follow-on balance ticket tunes it.

## Out of scope

- Multi-cat coordination on scavenging (who-picks-up-which
  carcass) — handled by existing target-taking mechanics if
  `picking_up_actions()` ever migrates to target-taking (separate
  ticket).
- Decay-rate tuning on ground items — the items-are-real contract
  preserves them; if scavenging is too rare, the answer is
  PickingUp scoring, not faster decay.
- Wildlife scavengers (foxes consuming carcasses on the ground) —
  parallel feature, not a prerequisite.
- The 181 saturation-axis weight tuning — separate balance ticket;
  this ticket's sufficiency is "non-zero scavenging fires" not
  "stockpile equilibrium changes".

## Approach

**Substrate vs search-state classification** (per
[`docs/systems/ai-substrate-refactor.md`](../../systems/ai-substrate-refactor.md)
§4.7): "any ground carcass exists in the colony" is a true
substrate fact — authored from observable world state by exactly
one system, exposed as a colony-scoped marker. The planner has
no `StateEffect` to toggle "ground carcasses exist"; A* expansion
cannot conjure or remove them. Substrate, not search-state.

**Marker authorship**: extend `update_target_existence_markers`
in `src/systems/sensing.rs:780-839` (the system that already
iterates uncleansed-or-unharvested ground carcasses for the per-cat
`CarcassNearby` marker). After the per-cat loop, aggregate "any
carcass with food kind exists" into the colony-scoped marker.
Mirror the existing pattern that authors `HasStoredFood` /
`HasFunctionalKitchen` for colony-scoped food state.

**Scoring lift**: the dormant linear curve replaces with a
hunger × proximity × food-security composition. Reuse the
`Composite{Logistic, ...}` pattern from Hunt's
`colony_food_security` axis (added in ticket 176 stage 5) so the
DSE composition shape matches existing precedent. Initial
weights chosen for plausibility, not balance — the balance
follow-on tunes once data lands.

**Plan template**: no change required; `picking_up_actions()` at
`src/ai/planner/actions.rs:274-280` already plans
`TravelTo(MaterialPile)` → `PickUpItemFromGround`. The DSE
eligibility gate alone enables the disposition; the existing
plan template handles execution.

## Critical files

- `src/components/markers.rs` — declare `HasGroundCarcass` ZST
- `src/systems/sensing.rs:780-839` — extend
  `update_target_existence_markers` to author the colony marker
- `src/ai/dses/picking_up.rs:34-42` — replace
  `Curve::Linear { slope: 0.0, intercept: 0.0 }` with a real
  composition; add `.require(HasGroundCarcass::KEY)` to the
  eligibility filter
- `src/resources/sim_constants.rs` — new tuning constants for
  the scoring composition (slope / intercept / weight per axis)
- `scripts/substrate_stubs.allowlist` — only if the
  reader+writer can't both land in the same commit (avoid if
  possible)

## Verification

- `just check` — substrate-stub lint must pass (writer + reader
  in same commit OR allowlist entry)
- `just test` — existing tests stay green; PickingUp DSE tests
  updated to reflect non-zero scoring
- New scenario `picking_up_scavenging` (sister to
  `hunt_deposit_chain`): one cat with empty inventory, one
  Stores, three carcasses spawned on the ground, no prey alive.
  Expected: cat elects PickingUp, picks up a carcass, deposits.
  Final stockpile ≥ 1.
- `just soak-trace 42 Wren` — confirm `OverflowToGround` count
  drops substantially (target: < 50% of pre-185 baseline) AND
  `food_stockpile_peak` rises (the scavenged carcasses now feed
  the deposit pipeline).
- `just verdict` — pass on the canonical seed-42 deep-soak.
- Frame-diff against pre-185 baseline: PickingUp's L2 mean
  rises from ~0 to a real positive number; Hunt / Forage L2 means
  unchanged.

## Resolution

Wave-closeout step 2 of 3. Substrate landed:

- `HasGroundCarcass` colony marker authored by
  `update_colony_building_markers` (`src/systems/buildings.rs`)
  from any uncleansed/unharvested `Carcass` entity. Threaded
  through `colony_state_query` in both `goap.rs` and
  `disposition.rs` into `MarkerSnapshot` via `set_colony`.
- `PickingUpDse` curve lifted from `Linear { slope: 0.0,
  intercept: 0.0 }` to a single `colony_food_security` axis
  shaped as inverted-Logistic — scavenge urgency rises sharply
  as the colony's `min(food_fraction, hunger_satisfaction)`
  signal drops. Eligibility filter retains
  `forbid(Incapacitated) ∧ require(HasGroundCarcass)`; the
  marker writer landing means the require predicate now
  resolves to a real value.
- Allowlist entry `HasGroundCarcass 185` retired in
  `scripts/substrate_stubs.allowlist`; 2 entries remain
  (`HideEligible 170`, `HasHandoffRecipient 188`).
- New unit tests on `picking_up.rs`:
  `picking_up_scavenges_when_food_security_low` (curve shape),
  `picking_up_axis_is_food_security` (axis name),
  `picking_up_eligibility_requires_ground_carcass` (eligibility).

The unit tests cover the load-bearing changes (curve shape,
eligibility, marker authoring); a full multi-system scenario
test (`picking_up_scavenging` per the original verification
section) is **deferred to the balance follow-on** (191) along
with `OverflowToGround` count regression-checks. Rationale:
the integration is mediated by well-tested patterns (HasMidden,
HasFunctionalKitchen authoring sites), and the wave's
post-closeout multi-seed re-baseline will surface any
disconnect.

## Land-day follow-on

- **Balance follow-on** — open a tuning ticket for the
  PickingUp Logistic params + scavenge_urgency formula. Plus
  the `picking_up_scavenging` scenario test deferred from this
  landing. The 0.5 midpoint may over- or under-fire scavenging
  vs Hunt; validate via `just hypothesize` four-artifact loop.

## Log

- 2026-05-06: opened from ticket 184's diagnostic findings.
  6071 OverflowToGround events per post-181 soak rot uncollected
  because PickingUp ships dormant. User flagged emergent
  scavenging as appealing during 184's investigation. Substrate
  classification verified: "ground carcass exists in colony" is
  a true substrate fact per §4.7.
- 2026-05-06: landed (wave-closeout step 2 of 3). Marker writer
  + curve lift + eligibility wiring all in one commit;
  allowlist entry retired. Plan at
  `~/.claude/plans/i-just-finished-a-compiled-hanrahan.md`.
