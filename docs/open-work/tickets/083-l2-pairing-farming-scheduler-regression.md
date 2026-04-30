---
id: 083
title: L2 PairingActivity activation collapses Farming via Bevy 0.18 schedule shift
status: ready
cluster: planning-substrate
added: 2026-04-29
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [027-l2-pairing-activity.md]
landed-at: null
landed-on: null
---

## Why

Sub-epic 071 hardened the planning substrate so that activating L2 PairingActivity no longer triggers the long-horizon starvation cascade that killed 027b's first attempt. Wave 4 ticket 082 then re-tried activation on the hardened HEAD (`43cc38a7`, working-copy edit only — uncommenting `crate::ai::pairing::author_pairing_intentions` at `src/plugins/simulation.rs:327`, inside the chain 2a marker batch right after `update_mate_eligibility_markers`).

The starvation cascade is gone (`Starvation = 0` in `logs/tuned-42-082-pairing-active-farming-regress/`, vs `Starvation = 3` in `logs/tuned-42-027b-active-failed/`). The L2 author runs vigorously: `PairingIntentionEmitted = 14651`, `PairingDropped = 14650`. But registering one more system in chain 2a's marker batch perturbs Bevy 0.18's topological sort enough to silently collapse an unrelated disposition: **Farming plans drop from 448 (`logs/tuned-42-072-refactor/`) to 0**, `CropTended` from 5070 to 0, `CropHarvested` from 176 to 0. The other five continuity canaries (grooming, play, courtship, mythic-texture; mentoring/burial pre-existing zeros) are unchanged.

This is exactly the "topological reshuffle" hazard the original 027b deferral block warned about. Substrate hardening absorbed the starvation pathway but does not address scheduler-perturbation effects on systems whose registration order matters.

## Scope

- Reproduce the regression at HEAD with the activation edit and capture the schedule trace via `bevy::ecs::schedule::ScheduleBuildSettings { report_sets: true, .. }` (or equivalent — verify the 0.18 API). Diff the system-execution order with vs. without the `author_pairing_intentions` line.
- Walk the four hypotheses (below), narrow to a root cause, and apply the smallest fix that restores Farming.
- Land the L2 PairingActivity activation atomically with the fix.
- Promote ticket 082 (and via 082, the parent 027b) once the soak gate clears.

## Out of scope

- Balance-tuning Farming defaults (`farm.rs` Considerations, crop-growth constants).
- Redesigning `author_pairing_intentions` or the L2 substrate.
- Broader Bevy 0.18 schedule-determinism audit across all chains — separately tracked.
- The 027c bias-channel / mate-target Intention-pin work (still 082 → 027c hand-off).

## Approach

Hypotheses, in descending likelihood:

1. **Same-resource topological shift.** `author_pairing_intentions` writes `Res<SystemActivation>` (and likely `EventWriter<Feature>` and `Commands`). Adding a writer reorders the chain. Diagnostic: print the schedule with and without the line, grep for the farm DSE eligibility / scoring / step systems, identify which one moved and what now precedes it. Fix: add explicit `.after(crate::ai::mating::update_mate_eligibility_markers)` and `.before(crate::ai::capabilities::update_capability_markers)` to `author_pairing_intentions` to pin its slot, or pin Farming systems to the position they held pre-activation.
2. **Marker-snapshot race.** Chain 2a's marker batch feeds the per-tick `MarkerSnapshot` read by GOAP / disposition scoring. If `author_pairing_intentions` mutates components that `MarkerSnapshot` population reads, Farming candidates could see an off-by-one snapshot. Diagnostic: grep `MarkerSnapshot` population for any read that pairing's `PairingActivity` component insert/remove could race; check whether snapshot population runs `.after(...)` the entire marker batch.
3. **Component-presence side effect.** `PairingActivity` is a component. Diagnostic: `grep -rn "PairingActivity" src/ai/dses/farm.rs src/ai/scoring.rs src/ai/dses/mod.rs src/ai/eval.rs` — confirm no Farming filter inadvertently gates `With/Without<PairingActivity>`.
4. **ExecutorContext / ScoringContext access pattern.** 272's `pairing_q` plumbing + 078's sensor read `PairingActivity` during scoring. Diagnostic: confirm the per-cat scoring loop's query set didn't acquire conflicting access that forces a chain split affecting Farming systems.

The expected fix is hypothesis 1 with explicit `.before/.after` constraints on `author_pairing_intentions`. If hypothesis 2/3/4 lands, document the mechanism in the activation pointer comment so the next perturbation has a precedent.

If a passing soak still leaves Farming below the post-072 baseline by less than 20% with `CropTended ≥ 1` and `CropHarvested ≥ 1`, treat the residual as acceptable balance shift and document in `docs/balance/027-l2-pairing-activity.md`. A persistent zero is not acceptable.

## Verification

Acceptance gate (single-seed):

- `src/plugins/simulation.rs` — `crate::ai::pairing::author_pairing_intentions` registered in chain 2a (uncomment line 327 + collapse the deferral block to a one-line activation pointer noting ticket 083 + mechanism).
- `src/resources/system_activation.rs::expected_to_fire_per_soak()` — `Feature::PairingIntentionEmitted` and `Feature::PairingBiasApplied` flipped to `true`.
- `just soak 42 && just verdict logs/tuned-42-083` clears: `Starvation = 0`, `ShadowFoxAmbush ≤ 10`, four pass-canaries ≥ 1 (mentoring/burial documented as pre-existing zeros), **Farming plan count ≥ 360 (within ±20% of the 448 post-072 baseline)**, **`CropTended ≥ 1`**, **`CropHarvested ≥ 1`**, neither in `never_fired_expected_positives`, `PairingIntentionEmitted > 0`.

On pass:

- Flip 082 status to `done`, fill `landed-at` / `landed-on`, move file to `docs/open-work/landed/2026-MM.md` per CLAUDE.md "Long-horizon coordination" §"When work lands". Append observation block to `docs/balance/027-l2-pairing-activity.md` per 082's verification spec. The multi-seed P1–P4 sweep stays inside 082's scope (now released) and runs after 083 lands.
- Flip 083 status to `done` and move to landed.

Investigation-style ticket: if hypothesis search verdict is "regression is acceptable balance shift" within the 20% band, the ticket lands with no code change beyond the activation edit + a documented mechanism in the activation pointer comment.

## Log

- 2026-04-29: Opened. Wave 4 (ticket 082) re-parked because activating L2 PairingActivity collapsed Farming (448 → 0 plans, CropTended 5070 → 0, CropHarvested 176 → 0) despite substrate hardening fixing the originating starvation cascade. Reproducer: `logs/tuned-42-082-pairing-active-farming-regress/` vs baseline `logs/tuned-42-072-refactor/`. HEAD `43cc38a7` plus the working-copy uncomment at `src/plugins/simulation.rs:327`.
