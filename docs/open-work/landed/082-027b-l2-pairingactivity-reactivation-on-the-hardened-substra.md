---
id: 082
title: 027b L2 PairingActivity reactivation on the hardened substrate
status: done
cluster: null
landed-at: 43cc38a
landed-on: 2026-04-29
---

# 027b L2 PairingActivity reactivation on the hardened substrate

**Why:** Sub-epic 071 (planning-substrate hardening, Wave 2 closed at `43cc38a`) absorbed the planning-fragility damage that made 027b's first activation attempt produce `Starvation = 3` (`logs/tuned-42-027b-active-failed/`). 082 is the reactivation pass that flips L2 PairingActivity live on the hardened substrate.

**What landed:** Activation rolls up into ticket 083's commit (the `simulation.rs` schedule-edge edit + `expected_to_fire_per_soak` flips for `PairingIntentionEmitted`). 082 is a meta-ticket that tracks the 027b structural-mating-commit lineage; the actual code change is 083's territory.

**Verification — soak `logs/tuned-42-083/` matches both 082 and 083 acceptance gates:**

- `Starvation = 0` (hard gate held; absorbed by Wave 2 hardening — was 3 in `tuned-42-027b-active-failed/`)
- `ShadowFoxAmbush = 5 ≤ 10` (hard gate held)
- `Feature::PairingIntentionEmitted = 14651` ≫ 0 (L2 author runs and emits Intentions; substrate hardening prevents the original starvation cascade)
- `Feature::PairingDropped = 14650` (drop-gate active; near-1:1 oscillation noted as a watch item for ticket 082's drop-cadence multi-seed look — see balance-doc Risks)
- Four pass continuity canaries (grooming, play, courtship, mythic-texture) each ≥ 1
- 027b balance doc's P4a prediction (Starvation within ±10% of baseline) flips from "regression-confirmed" to "pass" (0 vs 0)

**Multi-seed sweep deferred.** Per the user's 2026-04-29 PM call, single-seed gate is sufficient closeout — the multi-seed sweep that tests P1/P2/P3 (`MatingOccurred`, `BondFormed_Partners`, `PairingBiasApplied / SocializeTargetResolves`) is informational and runs whenever balance work next touches the L2 layer.

**Unblocks:** Ticket 084 (Farm herb/ward-demand axis) — picks up the Crop-canary re-promotion once herb-pressure motivation lands.

---
