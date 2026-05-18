---
id: 271
title: Restore substrate Flee score lift on critical-health + active-threat (251 balance follow-on)
status: done
cluster: ai-substrate
initiative: []
added: 2026-05-10
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: d013af9a8056
landed-on: 2026-05-11
---

## Why

The post-254 verification soak (seed 42, `logs/tuned-42`) showed 4 cats
killed by ambush despite each having known-active threats and (3 of 4)
critical health. In NO death-window did `Action::Flee` reach
competitive L3 softmax score:

| Cat | Boldness | Health | Safety | Top action | Flee score |
|---|---|---|---|---|---|
| Calcifer | 0.40 | 1.00 | 0.71 | Wander 0.387 | not in top 15 |
| Heron | 0.72 | **0.08** | 0.42 | Sleep 0.748 | **0.024** |
| Bramble | 0.49 | **0.24** | **0.10** | Socialize 0.581 | **0.035** |
| Mocha | **0.90** | **0.26** | **0.003** | Wander 0.535 | **−0.025** |

Mocha is the smoking gun: a bold cat at 26% health with safety = 0.003
(predator one tile away) had Flee scored **−0.025**, ~0.56 below the
winner. The picker-side fix (254 R5) is correct but invisible in this
soak because the L3 gate (Flee winning softmax) never opens.

This is a balance-iteration question, not a structural one — the
substrate is doing what it's designed to do, but the curve parameters
in `src/ai/dses/flee.rs` produce a hard zero on the boldness axis at
boldness = 1.0, which the CompensatedProduct geometric mean then
collapses regardless of how high other axes go. Fixing the parameters
restores Flee competitiveness on the bold-and-injured profile that
killed Mocha.

Bit-for-bit identical footer between this run and the pre-254 cedar
run (commit `12023b1c`) confirms the deaths are pre-existing on main —
they trace back to ticket 251 retiring `AcuteHealthAdrenalineFlee` and
moving the load to Sleep's `health_deficit` Logistic axis. Sleep does
not help against an active predator; the cat lies down while the fox
closes.

## Scope

- Adjust the boldness-axis curve in `src/ai/dses/flee.rs:75-82` so a
  fully bold cat retains a non-zero contribution.
- Re-run the seed-42 soak; assert `Feature::FleeTargetPicked >= 1` and
  `Action::Flee` wins L3 in at least one Mocha-profile death-window
  (boldness > 0.7, health < 0.3, safety < 0.1).
- Confirm `flee_commitment` scenario still passes (the picker fires
  end-to-end on the synthetic profile).

## Out of scope

- Restructuring the FleeDse axes (move boldness from CP-axis to
  modifier per `feedback_single_axis_perception_scalars`). That's the
  structural follow-on; this ticket is a minimum-surface balance tweak.
- Re-introducing `AcuteHealthAdrenalineFlee` or any interrupt-class
  modifier. 251 retired that path on principle (substrate-over-override).
- Re-tuning `flee_safety_threshold`, `flee_distance`, `flee_hold_ticks`,
  or any picker-side parameter — 254 R5 fixed the picker witness, that
  layer is correct.

## Current state

- 230 carved `DispositionKind::Fleeing` and the substrate-aware picker.
- 251 retired `AcuteHealthAdrenalineFlee` (substrate-over-modifier);
  health-crisis Flee lift moved to Sleep's `health_deficit` axis.
- 252 audited why `Feature::FleeTargetPicked = 0` (substrate softmax
  filter excluded Flee + picker witness contract was unreachable).
- 254 R5 closed the picker witness contract.
- 255 calibrated ThreatProximityAdrenalineFlee `flee_lift` and
  `sleep_lift`.
- 256 recalibrated Patrol DSE substrate (restored other canaries).
- This ticket (271): the remaining substrate gap — bold-injured cats
  still don't elect Flee because the FleeDse boldness axis hard-zeros.

## Layer-walk audit

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 perception | `src/systems/interoception.rs` | `health_deficit`, `safety_deficit`, `threat_proximity_derivative` all author correctly. | `[verified-correct]` |
| L2 DSE base score | `src/ai/dses/flee.rs:74-130` | `CompensatedProduct` of 4 axes [safety_deficit, **boldness_invert**, threat_distance, health_deficit] with weights [1,1,1,1]. Geometric mean. | `[verified-correct]` (running as designed) |
| L2 DSE boldness axis | `src/ai/dses/flee.rs:75-82` | `Curve::Composite { Linear(slope=1, intercept=0), Invert }` — output range [0, 1]; bold cat (boldness=0.9) → axis = **0.1**; max-bold (boldness=1.0) → **0.0**. | `[verified-defect]` (hard zero collapses CP) |
| L2 DSE health axis | `src/ai/dses/flee.rs:112-115` | `Curve::Linear(slope=0.4, intercept=0.6)` — health_deficit=0 → 0.6; health_deficit=1 → 1.0. Floor lift, not Logistic (Logistic was rejected at ticket 087 because healthy cats couldn't flee). | `[verified-correct]` (working as intended) |
| L2 modifier (lift) | `src/ai/modifier.rs:2334-2417` `ThreatProximityAdrenalineFlee` | Adds `flee_lift` (≈0.6) to Flee when `threat_proximity_derivative` ramps and `escape_viability >= viability_threshold` (default 0.4). | `[verified-defect-when-cornered]` — the eligibility gate excludes cornered profiles. `flee_calibration_critical_cornered` scenario shows the modifier delta on Flee is `body_distress_promotion+0.027` only — the `+0.600` lift does NOT fire when escape_viability ≈ 0.13 (cornered 3×3 patch). The ticket's pre-promotion claim ("modifier IS reaching Mocha") was wrong; corrected post-scenario triage. |
| L2 modifier (suppression) | `src/ai/modifier.rs:1417-1500` `AcuteHealthAdrenalineFight` | Suppresses Flee additively by `ramp(health_deficit) × fight_lift`, gated on `escape_viability < viability_threshold`. `fight_lift` defaults to **0.0** (inert at ship). | `[verified-correct]` (modifier is inert; doesn't cause the −0.025) |
| Final score noise | `src/ai/scoring.rs:1620` | Adds `jitter(rng, s.jitter_range)` to every scored Action. | `[verified-correct]` (explains why Mocha's Flee = −0.025 instead of exactly 0) |

**Diagnosis:** the boldness-invert axis in the FleeDse hard-zeros at
boldness = 1.0. CompensatedProduct geometric mean: even if the other 3
axes are saturated at 1.0, `(1 × 0.1 × 1 × 1)^(1/4) = 0.56` for Mocha;
add `flee_lift` ≈ 0.6 from `ThreatProximityAdrenalineFlee` and you get
**1.16 — Flee should win**. But Mocha shows −0.025, which means the
modifier isn't firing for her in this window OR there's a downstream
multiplicative damp not visible in `last_scores`.

**Promotion path:** focal-trace soak with `just soak-trace 42 Mocha`
to capture L1/L2 axis values + per-tick modifier set at her death
tick. Promote whichever interpretation the trace supports before
committing to a curve change.

## Approach

**R1 (parameter-level — recommended for this ticket).** Adjust the
boldness axis to keep a non-zero floor at boldness = 1.0:

```rust
// src/ai/dses/flee.rs:75-82
// Before: Linear(slope=1, intercept=0) then Invert →
//   boldness=0 → 1.0; boldness=1 → 0.0  (hard zero kills CP)
// After:  Linear(slope=-0.5, intercept=1.0) — already inverted →
//   boldness=0 → 1.0; boldness=0.5 → 0.75; boldness=1 → 0.5
let boldness_invert = Curve::Linear { slope: -0.5, intercept: 1.0 };
```

Effect on the four sample profiles:

| Profile | Old boldness axis | New boldness axis | Old CP | New CP |
|---|---|---|---|---|
| Mocha (b=0.9) | 0.10 | 0.55 | (1·0.10·1·0.90)^¼ = 0.55 | (1·0.55·1·0.90)^¼ = 0.86 |
| Calcifer (b=0.4) | 0.60 | 0.80 | (·0.60·...)^¼ varies | (·0.80·...)^¼ varies (slightly higher) |
| Bold healthy (b=0.9, h=1.0, s=0.5) | 0.10 | 0.55 | (0.5·0.10·~·0.6)^¼ ≈ 0.41 | (0.5·0.55·~·0.6)^¼ ≈ 0.61 |
| Timid (b=0.3, h=1.0) | 0.70 | 0.85 | similar | slightly higher |

The healthy-bold-cat case is the one to watch — under the new curve,
that cat's Flee axis goes from 0.41 to 0.61, which combined with
modifier lifts could elevate Flee above Wander (currently top at
0.535). The reckless-bravery override at `scoring.rs:2200` should
still flip Flee → Fight for boldness > `gate_reckless_flee_threshold`
(typically 0.9), preserving "bold healthy cat fights" semantics.

**R2 (parameter — alternative, gentler).** Use the same `Linear(slope=-0.5, intercept=1.0)`
shape but apply only to the Flee DSE; leave other DSEs that read
boldness alone (Fight, Patrol, etc) unchanged. (R1 is already
DSE-local because the curve lives inside FleeDse::new.)

**R3 (structural — out of scope, name only).** Move boldness out of
the FleeDse CP axes entirely. Replace with a substrate-side modifier
`BoldnessFleeModulation` that suppresses Flee additively scaled by
`boldness × (1 − health_deficit) × max_suppress`. Bold healthy cats
see suppression; bold-injured cats see less suppression because the
`(1 − health_deficit)` factor goes to 0 at low health. Aligns with
`feedback_single_axis_perception_scalars` (perception axes orthogonal
in the DSE; personality composes at modifier layer). Open as
follow-on if R1's tweak reveals second-order issues.

**R4 (parameter).** Increase `ThreatProximityAdrenalineFlee::flee_lift`
from current ~0.6 to 0.9. Doesn't address the hard-zero CP collapse
on the boldness axis; just makes the additive lift larger. Strictly
worse than R1 because it inflates Flee scores even for non-critical
profiles where the current behavior is correct.

## Verification

- Hard gate: `never_fired_expected_positives` does not regress (current
  main: `['MatingOccurred']`; this ticket should not add new entries).
- `just scenario flee_commitment` — still passes; `Feature::FleeTargetPicked`
  fires on the synthetic profile.
- New scenario or extension to `flee_commitment`: bold (boldness=0.9)
  cat at health=0.26, safety=0.003 with adjacent fox; assert
  `Action::Flee` wins L3 softmax on the first tick post-fix.
- Soak: `just soak-trace 42 Mocha` (focal = the smoking-gun cat); assert
  Flee elects in at least one Mocha tick during a threat window.
- Drift gate: `just verdict logs/tuned-42-<commit>` should not
  introduce new continuity-canary failures or push deaths_by_cause beyond
  pre-271 levels. Drift on Flee-related metrics is expected and OK.
- Follow-on: if R1's tweak reveals second-order issues (bold healthy
  cats fleeing too readily), open R3 as a follow-on for the structural
  fix.

## Log

- 2026-05-10: opened from 254's verification soak. 4 ambush deaths
  (3 ShadowFox, 1 WildlifeCombat) traced to Flee never electing
  L3 in any death window — Mocha's `Flee = −0.025` at boldness=0.9 +
  health=0.26 + safety=0.003 is the smoking gun. Layer-walk identifies
  the boldness-invert axis hard-zero at boldness=1.0 as the
  CP-collapsing factor. Recommended R1 parameter tweak (slope=−0.5,
  intercept=1.0) as the smallest surface change; R3 structural
  (boldness as modifier) named as the follow-on if needed.
- 2026-05-11: landed. **Phase 1 — scenario triage.** Added
  `flee_calibration_critical_cornered` variant (Mocha profile) +
  baseline-scanned all 6 scenarios pre/post. R1a (`slope=-0.5,
  intercept=1.0`) lifted critical_cornered Flee CP raw from
  **0.43 → 0.75 (+73%)**; R1c (`slope=-0.7`) was too sharp (Hunt
  overtook Flee). Scenario discovered the audit gap: the
  `ThreatProximityAdrenalineFlee` modifier was gated OUT at
  escape_viability ≈ 0.13 for cornered profiles — the row's
  `[verified-correct]` was wrong (corrected above). **Phase 2 — soak.**
  `just soak-trace 42 Mocha` on R1a: Mocha **survived** (vs pre-271
  ambush death); colony-level deaths_by_cause shifted
  ShadowFoxAmbush 2→1, WildlifeCombat 1→0 (2 predator deaths saved).
  Starvation=1 unchanged (pre-existing, not 271). **Test updates:**
  2 FleeDse tests rewritten (boldness-curve invariant + bold-cat
  doctrine moved from raw scoring to reckless-override mechanism in
  `bold_cat_fights_when_allies_present`). **Follow-ons opened:**
  R3 structural (boldness-as-modifier per
  `feedback_single_axis_perception_scalars`) and viability-threshold
  reduction (lower `threat_proximity_adrenaline_viability_threshold`
  from 0.4 so the modifier reaches cornered cats).
