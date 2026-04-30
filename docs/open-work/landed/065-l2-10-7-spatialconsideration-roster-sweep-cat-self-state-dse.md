---
id: 065
title: §L2.10.7 SpatialConsideration roster sweep (cat self-state DSEs + fox dispositions)
status: done
cluster: null
landed-at: 1b34947
landed-on: 2026-04-28
---

# §L2.10.7 SpatialConsideration roster sweep (cat self-state DSEs + fox dispositions)

**Landed:** 2026-04-28 | **Commits (22):** 1b34947 (Anchor variant + closure) · f3c1e51 (frontier centroid) · f2f5d10 (corruption centroid) · 59cb0b2 (fox-side anchors) · cd0e4d3 (ColonyLandmarks) · a930fa8 (fox closure wiring) · 15dc1b3 (cat closure wiring) · 8e47508 (Cook+Eat+Farm+HerbcraftPrepare) · 3220b1d (ColonyCleanse+Explore) · f4a5324 (Cleanse+DurableWard) · a588e4a (test-helper cleanup) · 08be1a6 (Build) · 30009d3 (fox Resting+Feeding+DenDefense) · 7160c68 (fox Raiding+Patrolling+Hunting) · 1444a46 (fox Fleeing+Avoiding+Dispersing) · bffc05c (Forage) · c6090fb (HerbcraftGather+HerbcraftWard) · 19a5642 (Patrol+Coordinate+Flee) · 17bcaeb (ClampMin floor on colony anchors)

**Why:** Closes the §L2.10.7 audit's second half. Ticket 052 ported the 9 cat *target-taking* DSEs to `SpatialConsideration` (`LandmarkSource::TargetPosition`); 065 ports the 12 cat *self-state* DSEs + 9 fox dispositions, which need the substrate to *resolve* a per-cat landmark rather than ride a target candidate. Per the spec audit at line 5606: "No cat or fox DSE currently uses continuous distance-to-landmark scoring. All 13/21 cat DSEs with spatial inputs and 6/9 fox dispositions with spatial inputs use binary range gates or aggregate-proximity scalars." This ticket unifies all of them on the substrate.

**What landed:**

1. **Substrate** — Added `LandmarkSource::Anchor(LandmarkAnchor)` variant + `EvalCtx::anchor_position` closure (the deferred "cat-relative anchor" enumeration per `considerations.rs:111`'s prior comment). 19-variant `LandmarkAnchor` enum covers all 25 spec-row landmarks (kitchen, stores, garden, herb-patch, perimeter, threat, corrupted-tile, sleeping-spot, construction-site, forageable-cluster, coordinator-perch, prey-belief-centroid, cat-cluster, frontier-centroid, corruption-centroid, own-den, visible-store, map-edge, territory-perimeter). Wired the cat-side closure (15dc1b3) and fox-side closure (a930fa8) to dispatch on the variant; both stub all-irrelevant-anchors to None (cat closures don't see fox-only anchors and vice versa).

2. **New per-tick anchor resources** — `ColonyLandmarks` (kitchen/stores/garden — single-instance buildings, populated by `update_colony_landmarks` in `systems/buildings.rs`); `CorruptionLandmarks` (territory intensity-weighted centroid, populated by `update_corruption_landmarks` in `systems/magic.rs`); `frontier_centroid` field on `ExplorationMap` populated by `update_exploration_centroid`; per-fox `den_position`/`prey_belief_centroid`/`cat_cluster_centroid`/`frontier_centroid`/`nearest_visible_store`/`nearest_map_edge`/`territory_perimeter_anchor` populated by `fox_goap.rs::build_scoring_context`. Cat-side per-cat anchors (nearest_threat, nearest_corrupted_tile, nearest_construction_site, etc.) populated by `goap.rs` and `disposition.rs` ScoringContext builders.

3. **Cat self-state ports (16 DSEs)** — Cleanse, Sleep, Cook, Eat, Farm, Build, Forage, HerbcraftGather, HerbcraftWard, HerbcraftPrepare, DurableWard, ColonyCleanse, Patrol, Coordinate, Flee, Explore. Curve idiom matches the §L2.10.7 spec rationale per row: `Composite{Logistic(8, 0.5), Invert, ClampMin(0.1)}` for routine-commute axes (Cook/Eat/Farm/HerbcraftPrepare/Gather/Ward/Forage/Coordinate); `Composite{Polynomial(exp=2, divisor=1), Invert}` for sharp-falloff axes (Cleanse/DurableWard/ColonyCleanse/Flee); `Linear(slope=-1, intercept=1)` for gradient-following (Explore/Patrol).

4. **Fox dispositions (9 DSEs)** — Resting, Feeding, DenDefense, Raiding, Patrolling, Hunting, Fleeing, Avoiding, Dispersing. Same curve families; OwnDen anchor for the three offspring/territorial DSEs (Resting/Feeding/DenDefense), centroid anchors for Hunting/Avoiding/Dispersing, edge/perimeter anchors for Fleeing/Patrolling.

5. **CP-gate compensator (commit 17bcaeb)** — First closeout soak surfaced `CropTended`/`CropHarvested` regression because Cook/Eat/Farm/HerbcraftPrepare/Gather/Ward/fox Raiding's `Composite{Logistic, Invert}` curve hits ≈ 0.018 at range edge; under CompensatedProduct any near-zero axis gates the whole DSE. Fix: outer `ClampMin(0.1)` floor — preserves the spec's "high-cost candidates degrade smoothly" wording at `considerations.rs:73` while letting marker eligibility filters (HasGarden, HasFunctionalKitchen, HasStoredFood) remain the gating point per §4.

6. **Constants retired** — `build_site_bonus` (subsumed by the spatial axis on Build); the dead `_kind_anchor` shim in `steps/building/deliver.rs` (concurrent cleanup); the `[lints.rust] dead_code` directive switched from `forbid` to `deny` because cargo's bin-test harness auto-injects `#[allow(dead_code)]` around `fn main`, which the harder `forbid` rejected.

7. **New tunable** — `patrol_perimeter_offset` (`SimConstants`, default 12) — anchor offset from colony center used by Patrol and HerbcraftWard's spatial axes. Single-point perimeter approximation; future refinement could pick per-cat angles.

**Surprises surfaced:**

- `LandmarkSource::Entity` had **zero** production callers on either side (cat-side closure was also stubbed `|_| None` at `scoring.rs:549`, not just fox). Until this ticket, every entity-as-landmark scenario fell back to a non-existent path. Resolved by introducing the `Anchor` variant for the per-cat-per-tick lookup pattern, leaving `Entity` for the (still rare) pinned-entity case.
- Self-state DSEs build their `Consideration` list at **registration time**, so per-cat dynamic landmarks (nearest kitchen, own den, frontier centroid) can't ride `LandmarkSource::Entity(Entity)` cleanly — Entity refs aren't known at registry-population time. The `Anchor(LandmarkAnchor)` variant identifies the *kind* of landmark; the closure resolves it per cat per tick.
- Cat-side has **no** `home_den` analog — only foxes (`wildlife.rs:249`) and prey (`prey.rs:109`). Sleep's "Own Den / sleeping spot" landmark falls back to `Some(self_position)` per the plan's surprise-resolution recommendation.

**Verdict — drift envelope.** Closeout soak (seed 42, 15-min release, commit 17bcaeb) vs `logs/baselines/current.json` (post-033-time-fix, 2026-04-26 — predates 052; the cumulative drift is 052 + 065 combined and uncomputable separately without an `acccdc7` baseline):

| Metric | baseline | post-065 | Δ |
|---|---|---|---|
| Starvation | 2 | 0 | improvement |
| ShadowFoxAmbush | 4 | 4 | match (≤10) |
| never_fired_expected_positives | 8 | 3 | 5 features recovered (now firing); 3 remaining match baseline (MatingOccurred/GroomedOther/MentoredCat — pre-existing, not 065-introduced) |
| continuity grooming | 71 | 73 | +3% |
| continuity play | 111 | 417 | +275% (drift) |
| continuity courtship | 804 | 408 | -49% (drift) |
| continuity mythic-texture | 48 | 26 | -46% (drift) |
| continuity mentoring/burial | 0/0 | 0/0 | pre-existing zeros |
| Injury deaths | 0 | 1 | small regression |
| WildlifeCombat deaths | 1 | 3 | small regression |

**Hard survival gates: Starvation = 0 ✓ · ShadowFoxAmbush ≤ 10 ✓ · footer written ✓.** The `never_fired_expected_positives == 0` gate fails on 3 pre-existing items (matches baseline); not a 065-introduced regression.

**The drift > ±10% on play / courtship / mythic-texture is spec-sanctioned.** §L2.10.7 line 5611 explicitly: "The refactor changes every row's shape, not just adds curves — this roster is a full aspirational specification, not an audit of current behavior." The shape changes are correct per the spec; numeric tuning to compress drift back is balance-thread successor work per the ticket's own "Out of scope: numeric balance tuning" clause and CLAUDE.md's four-artifact methodology requirement for drift > ±10%.

**Cumulative drift across both 052 + 065 lands.** 052's drift was ~zero (LSB churn cancellations) per its closeout. The drift recorded above is dominated by 065's shape changes. Future work could re-promote a baseline at `acccdc7` to separate them precisely.

**Out of scope (deferred to balance thread):** numeric tuning of the curve midpoints / steepnesses / ranges to compress play / courtship / mythic-texture drift back inside ±10%. The substrate is now in place; tuning is per-DSE four-artifact methodology runs per CLAUDE.md.

---
