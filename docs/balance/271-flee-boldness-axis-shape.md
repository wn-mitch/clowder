# Re-shaping the FleeDse boldness-invert axis from a hard-zero to a 0.5 floor restores CP competitiveness for bold-injured-cornered cats (2026-05-11)

Ticket: [`271`](../open-work/landed/271-restore-substrate-flee-score-lift-on-critical-health-active-threat-251-balance-follow-on.md).

## Hypothesis

The pre-271 `FleeDse` boldness-invert axis (`Composite { Linear(slope=1,
intercept=0), Invert }`) hard-zeros at `boldness = 1.0`, collapsing the
CompensatedProduct geometric mean even when the other three axes
(safety_deficit, threat_distance, health_deficit) saturate. Re-shaping
to a pre-inverted `Linear { slope: -0.5, intercept: 1.0 }` floors the
axis at 0.5 for fully bold cats while preserving the "less bold ⇒
flees more" monotonicity. Predicted effect: bold-injured-cornered cats
gain enough CP headroom that soak-side suppressors (preference_penalty,
disposition_failure_cooldown, etc.) no longer push their Flee score
below competing actions, restoring the substrate intent the
`AcuteHealthAdrenalineFlee` modifier carried pre-251.

## Prediction

| Field | Value |
|---|---|
| Primary metric | `deaths_by_cause.ShadowFoxAmbush` |
| Direction | decrease |
| Rough magnitude band | ±30–80% |
| Secondary | `deaths_by_cause.WildlifeCombat` decrease in same band |

## Observation

Single-seed verification per the user's "scenarios over soaks" preference.

**Scenario triage (all 6 flee scenarios, ~3s each):**

| Scenario | Pre-271 Flee final | Post-271 Flee final | Winner pre / post |
|---|---:|---:|---|
| `flee_calibration_low_threat` | 0.253 (3rd) | 0.281 (3rd) | Fight / Fight |
| `flee_calibration_open_terrain` | 1.201 (1st 99.8%) | 1.292 (1st 99.95%) | Flee / Flee |
| `flee_calibration_cornered` | 0.668 (1st 94%) | 0.776 (1st 99%) | Flee / Flee |
| `flee_calibration_sleep_partner` | 0.941 (1st 55%) | 0.996 (1st 72%) | Flee / Flee |
| `flee_calibration_critical_cornered` (271 new) | 0.445 (1st 52%) | 0.753 (1st 84%) | Flee / Flee |
| `flee_commitment` | 1.183 (1st 99%) | 1.364 (1st 99.9%) | Flee / Flee |

**Soak (`just soak-trace 42 Mocha`, 900s release):**

| Field | Pre-271 (commit 7c0c1b12 baseline) | Post-271 (commit 7c0c1b12 + R1a) | Δ |
|---|---:|---:|---:|
| `deaths_by_cause.ShadowFoxAmbush` | 2 | **1** | −50% |
| `deaths_by_cause.WildlifeCombat` | 1 | **0** | −100% |
| `deaths_by_cause.Starvation` | 1 | 1 | 0 (pre-existing) |
| `shadow_fox_spawn_total` | 18 | 19 | +5% (noise) |
| Mocha (focal) survives | **NO** (ambushed) | **YES** | — |

Continuity canaries (post-271): grooming 1276, play 14, mentoring 294,
courtship 2796, mythic-texture 41. All ≥1, all pass.

## Concordance

**Verdict: concordant**

- Direction match: ✓ (decrease for both `ShadowFoxAmbush` and
  `WildlifeCombat`)
- Magnitude in band: ✓ (`ShadowFoxAmbush` −50% within ±30–80%;
  `WildlifeCombat` −100% beyond the upper band but expected given
  the small baseline count)
- Survival canaries: `Starvation = 1` fails the hard `== 0` gate
  on this commit. **Pre-existing on main** — pre-271 also had
  `Starvation = 1` at the same commit. NOT introduced by 271 and
  not in scope for this ticket; opened as follow-on if needed.

## Survival canaries

`just verdict logs/tuned-42` exits 2 with `survival: fail` due to
the pre-existing `Starvation = 1`. The 271 change is net-positive
on the survival axis (−2 predator deaths) and does not regress any
continuity canary.

## Cross-metric findings

- **Audit-gap discovery.** The scenario triage revealed that
  `ThreatProximityAdrenalineFlee` (the `flee_lift ≈ 0.6` modifier)
  is gated OUT at low `escape_viability` (cornered profiles ≈ 0.13;
  gate is 0.4). On `flee_calibration_critical_cornered`, Flee's
  only modifier delta is `body_distress_promotion + 0.027` — not
  the `+0.600` lift that fires on `open_terrain` (viability 0.7).
  This means R1a's CP-only repair has to carry the entire load on
  the cornered profile. R1a is sufficient on this seed but the
  underlying gate exclusion is named in the 271 follow-on for the
  structural fix (lower `threat_proximity_adrenaline_viability_threshold`
  OR add a dedicated cornered-critical Flee branch).
- **Doctrine relocation.** The pre-271 invariant "raw Fight > raw
  Flee for bold cats with allies" was an artifact of the boldness-
  invert axis hard-zero. Post-271 the invariant moves to the
  `behavior_gate_check` reckless override (final-action layer). The
  `bold_cat_fights_when_allies_present` test was rewritten to
  assert the new mechanism explicitly (override flips Flee→Fight
  at boldness > 0.9 + health > 0.5).
- **No regression** in any of the 5 existing `flee_calibration_*` or
  `flee_commitment` scenario invariants. The `cornered` variant's
  docstring claim ("Fight wins; Flee suppressed") was already stale
  on main because `AcuteHealthAdrenalineFight::fight_lift` defaults
  to 0.0 (inert at ship); R1a does not make this worse.

## Follow-ons (opened with 271's land commit)

- **Lower the viability-gate threshold.** The
  `ThreatProximityAdrenalineFlee` modifier currently excludes the
  exact profiles that most need it (cornered + dying). The right
  knob is `threat_proximity_adrenaline_viability_threshold`
  (default 0.4); a value near 0.15 would let the modifier reach
  cornered cats. Parameter change, separate verification.
- **R3 structural (boldness as modifier).** Move boldness out of the
  FleeDse CompensatedProduct axes entirely; replace with a
  substrate-side modifier `BoldnessFleeModulation` that scales
  Flee additively by `boldness × (1 − health_deficit)` — bold
  healthy cats see suppression; bold-injured cats see less. Aligns
  with `feedback_single_axis_perception_scalars` (perception axes
  orthogonal in DSE; personality composes at modifier layer).
