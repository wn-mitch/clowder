---
id: 286
title: Lower ThreatProximityAdrenalineFlee viability threshold so cornered cats receive flee_lift
status: ready
cluster: ai-substrate
added: 2026-05-11
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

271's scenario triage surfaced an audit-gap on the
`ThreatProximityAdrenalineFlee` modifier: its eligibility gate
(`escape_viability >= 0.4`) excludes the exact profiles that most
need the lift. On the new `flee_calibration_critical_cornered`
scenario (boldness=0.9, health=0.26, cornered 3×3 patch,
viability ≈ 0.13), the modifier delta on Flee is
`body_distress_promotion + 0.027` only — the `+0.600` `flee_lift`
that fires on `flee_calibration_open_terrain` (viability 0.7) is
gated out. 271's R1a curve fix carries the load on its own and
saves 2 predator deaths on seed 42, but the substrate intent
(adrenaline lifts Flee when threat ramps) is missing for cornered
cats. This ticket lowers the threshold so the modifier reaches
the profiles it was designed for.

## Scope
- Drop `threat_proximity_adrenaline_viability_threshold` from
  0.4 to a value (target ~0.15) where cornered profiles (viability
  ≈ 0.13) just-pass the gate.
- Verify `flee_calibration_critical_cornered` shows the modifier
  delta `threat_proximity_adrenaline_flee+0.600` on Flee post-change.
- Re-soak seed 42; assert no regression on threat-driven deaths
  vs the post-271 baseline.

## Out of scope
- Replacing the viability gate with a different shape (e.g.
  Logistic vs hard threshold) — gate redesign is a separate
  question.
- 271's R3 structural fix (boldness as modifier) — that's
  ticket 287.

## Current state
Blocked by 271 (the curve fix) which landed 2026-05-11. The audit
table row in 271 was promoted to `[verified-defect-when-cornered]`
based on the scenario triage; 271's balance doc
(`docs/balance/271-flee-boldness-axis-shape.md`) captures the
audit-gap discovery and names this follow-on.

## Approach

Single-constant tuning. The default lives in
`src/resources/sim_constants.rs::default_threat_proximity_adrenaline_viability_threshold`
(currently `0.4`). Drop to `0.15`. Scenario triage answers whether
the gate now opens for cornered profiles (~3s); soak triage
confirms colony-level survival doesn't regress.

## Verification
- `just scenario flee_calibration_critical_cornered` — Flee's
  L2 modifier deltas now include `threat_proximity_adrenaline_flee+0.600`.
- `just scenario flee_calibration_cornered` — same modifier delta;
  Flee L3 score lifts above current 0.78 final.
- `just scenario flee_calibration_low_threat` / `open_terrain` /
  `sleep_partner` / `flee_commitment` — winners unchanged.
- `just soak-trace 42 Mocha` + `just verdict` — no regression on
  hard gates or continuity canaries; `ShadowFoxAmbush` and
  `WildlifeCombat` either unchanged or further reduced from
  post-271 baseline.

## Log
- 2026-05-11: opened by 271 landing. Named in 271's audit table
  promotion and `docs/balance/271-flee-boldness-axis-shape.md`.
