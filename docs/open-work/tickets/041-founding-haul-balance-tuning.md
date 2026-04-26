---
id: 041
title: Founding wagon-dismantling haul — balance the early-game cost so cats don't starve while hauling
status: ready
cluster: null
added: 2026-04-26
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 038 wired the full Pickup → Carry → Deliver pipeline (planner zone + state field + step resolvers + GOAP dispatch + `BuildMaterialItem` marker + `ConstructionSite::new_with_custom_cost`) so the `Feature::MaterialPickedUp` and `Feature::MaterialsDelivered` Features can fire honestly when cats haul ground items to a `ConstructionSite`. The infrastructure is in place and unit-tested.

What's parked is the **founding wagon-dismantling spawn** (`spawn_founding_construction_site` in `src/world_gen/colony.rs`). On the seed-42 deep-soak it produced a starvation regression even with a small 4-Wood founding cost: `Starvation 0 → 5`, `anxiety_interrupt_total 6259 → 25051`, plus broad behavioral drift (`CropTended -78%`, `Socialized 7×`, `BuildingTidied 6×`). The exact mechanism wasn't isolated in the 038 session — likely a combination of: (1) cats with Build disposition spending the early colony minutes on the haul cycle instead of the on-the-ground food the colony brought, (2) the new founding ConstructionSite shifting Build/Construct dispositioner choices for cats not even directly involved, (3) the parallel-scheduler interplay with the new planner state.

The spawn is gated behind the `CLOWDER_FOUNDING_HAUL` env var so the infrastructure can land cleanly. This ticket activates it for real.

## Scope

1. **Reproduce the regression deterministically.** Run a soak with `CLOWDER_FOUNDING_HAUL=1` on seed 42 and capture the per-tick narrative + focal-cat trace. Identify which cats are doing the haul and which are starving.
2. **Identify the load-bearing cause.** Likely candidates to check:
   - **Maslow gating.** Should the planner refuse to enter a Building plan when `hunger_ok=false`? Currently the haul cycle competes with hunting/eating.
   - **Founding cost.** 4 Wood proved too high; even 1 Wood may be appropriate for a bootstrap step. Or zero Wood (immediate complete) defeats the purpose.
   - **Pile placement.** Wood spawned next to the founding site means cats walk away from food sources to haul. Spawning piles ON the cat-spawn tile could remove the walking cost.
   - **Founding building choice.** `Stores` is large (4×3) and disrupts the colony footprint. A `WardPost` (1×1, 2 Stone + 3 Herbs) or hand-rolled minimal blueprint may be a better founding act.
3. **Tune until canaries hold.** Re-run on seed 42 with founding spawn ON; survival canaries pass (`Starvation = 0`, `ShadowFoxAmbush ≤ 10`, footer written, `never_fired_expected_positives = 0`); continuity canaries hold no worse than baseline.
4. **Promote Features.** When canaries pass, flip `Feature::MaterialsDelivered` and `Feature::MaterialPickedUp` from `expected_to_fire_per_soak() → false` back to `true` in `src/resources/system_activation.rs`. Update the demotion comment.
5. **Verify continuity tallies.** Mythic-texture (`ShadowFoxBanished` + `MythicTexture` events) survives the founding spawn. The 038 partial-soak observed mythic-texture going 22 → 0 with the spawn off and the planner expanded — investigate whether the planner-state expansion alone is the cause (in which case the issue is independent of founding spawn) or whether the spawn introduces the regression.

## Out of scope

- The non-founding coordinator-spawned `new_prefunded` calls. Coordinator-built sites continue to magically receive materials at mark-out time. That's a separate (larger) ticket — restoring full physical-causality across the entire build economy is a multi-PR scope and would require tying material delivery to a stockpile / haul-from-anywhere flow.
- Multi-material founding sites. Scope here is one Wood-only founding act.
- Scaling beyond a single founding building.

## Current state

Infrastructure landed in 038 commit `<TBD>`. Founding spawn is gated by `CLOWDER_FOUNDING_HAUL` env var. Features `MaterialsDelivered` + `MaterialPickedUp` are demoted to `expected_to_fire_per_soak() → false` until this ticket lands.

## Approach

Iterative balance tuning per CLAUDE.md methodology — hypothesis / prediction / observation / concordance against `docs/balance/`. Likely 3–5 iterations.

## Verification

- `cargo run --release -- --headless --seed 42 --duration 900` with `CLOWDER_FOUNDING_HAUL=1` set.
- `just verdict logs/<run-dir>` returns pass.
- Multi-seed sweep (seeds 99 / 7 / 2025) once seed-42 holds.
- `just frame-diff` against the parked-spawn baseline confirms haul-cycle DSEs only activate during the founding window.

## Log

- 2026-04-26: Ticket opened. Infrastructure landed in 038; spawn parked behind env var pending balance work.
- 2026-04-26: Diagnostic dive on the spawn-on starvation regression isolated a pre-existing Flee-lock bug (cats stuck in `Action::Flee` for 5000+ ticks because the threat-preempt path didn't remove `GoapPlan`, so `evaluate_and_plan` couldn't re-evaluate). Flee-lock fix landed in 038. With it, iter4 spawn-on soak shows the founding flow firing as designed (`MaterialPickedUp = MaterialsDelivered = 4`) and large continuity-tally surges (`courtship 0→1254`, `mythic-texture 22→43`, `grooming 19→599`, `play 109→1840`, plus `BondFormed` and `CourtshipInteraction` newly firing) — needs characterization to determine whether magnitudes are realistic or over-firing. Iter4 still shows 3 starvation deaths at days 122/167 to be investigated separately.
