---
id: 255
title: ThreatProximityAdrenalineFlee Flee-axis calibration audit (post-252 readable)
status: done
cluster: ai-substrate
added: 2026-05-10
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 2f3236ff80cb
landed-on: 2026-05-10
---

## Why

`ThreatProximityAdrenalineFlee` (ticket 108, `src/ai/modifier.rs:2334`)
lifts both `Action::Flee` and `Action::Sleep` scores when the
`threat_proximity_derivative` ramps. Pre-252, the L3 softmax filter at
`scoring.rs:2411` excluded `Action::Flee` from the disposition pool —
the modifier's Flee-axis lift was architecturally orphaned (only the
Sleep-axis lift mattered, per the 047 / 108 "in-pool partner"
doctrine). 252 lifted the filter; the Flee-axis lift is now reachable.

The lift was originally tuned on the assumption that Flee would NEVER
win L3 (it was filtered). With 252 making Flee a real contender, the
calibration may now over-elect Fleeing relative to its substrate
intent. Independently of whether 254 (PickFleeTarget witness fix)
makes the elected Fleeing actually move the cat, this ticket
audits whether the Flee-axis lift magnitude is right for the new
"Flee can win" regime.

## Scope

- Audit `flee_lift` (`sim_constants.rs:1635`, currently set per
  ticket 108) against post-254 soak data. Determine whether Flee's
  share of L3 election is at the doctrinal target for "rare,
  threat-driven" or has crept into "common, dominates Sleep".
- If miscalibrated, draft a hypothesis spec and run
  `just hypothesize` per CLAUDE.md balance discipline.
- Confirm or reframe the doctrinal claim "Sleep is the in-pool
  partner; Flee is rare" — that framing inherits from 047 (now
  retired by 251). Post-251, `health_deficit`-driven Sleep lift
  comes from the Logistic axis on Sleep DSE itself; the
  108-modifier's Sleep-axis lift may now be redundant.

## Out of scope

- The PickFleeTarget witness contract (ticket 254 owns that).
- Re-architecting the modifier pipeline composition (108's order
  in `default_modifier_pipeline` is fine).
- Sleep DSE's `health_deficit` Logistic axis re-tuning (that's
  251's territory).

## Current state

108 still in `default_modifier_pipeline` (`src/ai/modifier.rs:3501`).
Constants `acute_health_adrenaline_threshold` (preserved for 102 / 105),
`flee_lift = 0.6`, `sleep_lift = 0.5` (read from the most recent soak
header at `logs/tuned-42-post-252-fleeing-collapse/events.jsonl:1`).

## Approach

1. Read the 108 modifier scoring shape; trace the lift magnitudes
   through the L2→L3 pipeline post-252 (now that Flee is in the
   softmax pool).
2. **Scenario microexperiment** instead of post-254 soak. The post-252
   collapse soak (`tuned-42-post-252-fleeing-collapse`) is contaminated
   by the 254 picker defect (~95% Fleeing→HoldUntilSafe conversion in
   heavy fleers) AND the dominant Patrol-cascade absorber (63.65% of
   action elections vs Flee's 4.45%) — neither of which a flee-lift
   sweep would isolate. A `just scenario flee_calibration_*` family
   probes L3 election directly across the four corners of the
   (`threat_proximity_derivative`, `escape_viability`) plane plus a
   Sleep-partner doctrine probe. Variants land at
   `src/scenarios/flee_calibration.rs`.
3. Read each variant's L2 score table + L3 winner. If gates trip as
   designed and the doctrinal partner regime preserves Sleep ≈ Flee,
   `flee_lift = 0.6` is correctly tuned. Else: sweep via
   `just hypothesize`.
4. **Sleep-axis redundancy verdict** (Q2): NOT redundant. 251 absorbed
   `health_deficit`-driven Sleep urgency into the Sleep-DSE Logistic
   axis; 108's `sleep_lift` keys on `threat_proximity_derivative` —
   an orthogonal signal Sleep DSE has no consideration for. Verify
   via `flee_calibration_sleep_partner` that removing `sleep_lift`
   would collapse the in-pool partner doctrine.

## Verification

- `just scenario flee_calibration_low_threat` — Flee does NOT win
  (no `threat_proximity_adrenaline_flee` modifier delta on Flee).
- `just scenario flee_calibration_open_terrain` — Flee wins (delta
  +0.600 on Flee, +0.500 on Sleep; softmax picks Flee).
- `just scenario flee_calibration_cornered` — viability gate trips
  off (no `threat_proximity_adrenaline_flee` delta on Flee or Sleep).
- `just scenario flee_calibration_sleep_partner` — Sleep ≈ Flee in
  softmax probability (in-pool partner; gap ≤ ~15%).
- `cargo test --test scenario_feature_assertions` — passes (variants
  opt out of Feature gating per `expected_features: &[]`).

## Log

- 2026-05-10: opened from 252 land. The Flee-axis orphan question
  has been latent since 108 landed; 252 makes it actionable.
- 2026-05-10: investigation of `tuned-42-post-252-fleeing-collapse`
  reframed the §Why premise. The post-252 reproductive collapse is
  dominantly a **Patrol-cascade**, not a Flee-elevation: Patrol =
  63.65% of action elections; Flee = 4.45%; Mate-action snapshots = 0;
  Courtship-disposition plans = 0. Death Zapruder views: Cedar morale-
  broke during a Patrol→EngageThreat with Flee never elected;
  Calcifer was Patrol-exposed and chose Engage over Flee post-ambush;
  only Bramble matches the 254 picker-defect-stuck-in-Fleeing pattern
  (frozen at [27, 12] for 463+ ticks). 254's framing — "Fleeing
  absorbs the courtship bandwidth" — holds for 1 of 3 deaths; the
  larger absorber is Patrol. Substrate cause: Patrol DSE pulls cats
  toward a fixed colony-anchor tile (`TerritoryPerimeterAnchor` at
  `src/systems/disposition.rs:973`) via vanilla A* with no ward /
  corruption / fox-scent awareness. **Out of 255's scope; opens a
  follow-on ticket.**
- 2026-05-10: **Q2 verdict — `sleep_lift` NOT redundant** with 251's
  Sleep-DSE `health_deficit` Logistic axis. Code reading: 251 absorbed
  health-deficit-driven Sleep urgency into Sleep substrate; 108's
  `sleep_lift` keys on `threat_proximity_derivative` (an axis the
  Sleep DSE has no consideration for). Empirical confirmation:
  `flee_calibration_sleep_partner` shows Sleep raw = 0.363 (already
  carrying 251's health-deficit lift), final = 0.863 after 108's
  +0.500 lift. Removing `sleep_lift` would drop Sleep's final to
  0.363, collapsing the in-pool partner doctrine (Flee 0.94 / Sleep
  0.36 → Flee dominates 2.6× instead of 1.1×). Retain `sleep_lift`.
- 2026-05-10: **Q1 verdict — `flee_lift = 0.6` correctly calibrated**
  for the post-252 regime. Scenario evidence:
  - `flee_calibration_low_threat`: derivative ≈ 0.05 → 108 ramp gates
    off → no Flee/Sleep lift; Flee at 16.98% softmax probability,
    non-dominant. ✅
  - `flee_calibration_open_terrain`: derivative ≈ 0.7, viability ≈
    0.7 → both gates trip; Flee gets +0.600, Sleep gets +0.500;
    Flee wins L3 with 99.81% probability. ✅ (substrate-level Flee
    raw 0.62 was already 3.4× Sleep raw 0.18 before lifts; the
    modifier preserves the right ordering rather than inverting it.)
  - `flee_calibration_cornered`: viability ≈ 0.13 → 108 viability
    gate trips off; no `threat_proximity_adrenaline_flee` delta on
    either Flee or Sleep. ✅ (Substrate Flee still wins 94% — that's
    a separate 102-Fight-DSE calibration question, not 108's.)
  - `flee_calibration_sleep_partner`: composed wound + tired +
    threat → both lifts fire on top of 251's health-deficit Sleep
    axis; Flee = 0.94 vs Sleep = 0.86 in softmax 55/45. ✅ True
    in-pool partner behavior; doctrine preserved.
  No constant changes proposed. No `just hypothesize` run needed
  (no drift > 10% to audit; the calibration was correct as shipped).
- 2026-05-10 (lands-day): opens follow-on **256** (Patrol DSE
  recalibration — influence-map-driven smart pathing + wildlife
  deterrent affect) for the cascade root cause surfaced during this
  audit. 256's layer-walk pre-staged from this ticket's
  investigation.
