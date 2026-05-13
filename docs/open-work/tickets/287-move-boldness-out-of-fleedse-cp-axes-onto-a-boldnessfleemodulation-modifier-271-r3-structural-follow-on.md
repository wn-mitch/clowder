---
id: 287
title: Move boldness out of FleeDse CP axes onto a BoldnessFleeModulation modifier (271 R3 structural follow-on)
status: ready
cluster: combat-threat
initiative: []
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

271 landed an R1a parameter tweak (boldness-invert curve floor at
0.5) that resolves the immediate bold-injured-cornered Flee
collapse. The tweak preserves the boldness axis inside the
FleeDse CompensatedProduct, which violates the doctrine in memory
`feedback_single_axis_perception_scalars`: each scalar in
`src/systems/interoception.rs` encodes one orthogonal axis;
personality, phobias, and ambient anxiety should compose at the
modifier layer, never fold into the underlying perception. R3
moves boldness out of FleeDse's CP entirely onto a substrate-side
modifier so the FleeDse considers only threat-perception scalars.
This makes "bold healthy cats with allies fight" (currently
preserved by the `behavior_gate_check` reckless override at the
final-action layer) visible at the L2 score-shape level — the
right layer for ethological encoding.

## Scope
- Add a `BoldnessFleeModulation` modifier under `src/ai/modifier.rs`
  that scales Flee additively by
  `boldness × (1 − health_deficit) × max_suppress`. Bold healthy
  cats see suppression; bold-injured cats see less because the
  `(1 − health_deficit)` term goes to 0 at low health.
- Remove the boldness consideration from `FleeDse::new`'s
  CompensatedProduct axes (drop to 3 axes:
  safety_deficit, threat_distance, health_deficit).
- Re-balance the FleeDse weights / compensation to keep the
  pre-R3 magnitude envelope on the verification scenarios.
- Update the layer-walk audit row on FleeDse boldness axis
  (currently `[verified-defect]` → after R3, the axis no longer
  exists; doctrine moves to the modifier layer).
- Consider whether the `behavior_gate_check` reckless override
  can be retired once the substrate handles the bold-healthy
  case at the score layer (substrate-over-override doctrine).

## Out of scope
- Re-introducing `AcuteHealthAdrenalineFlee`. 251 retired it on
  principle; 087's boldness-CP-axis fix is being replaced by a
  modifier, not by reviving a retired interrupt.
- Changing the FleeDse stance / eligibility filter set (§9.3
  binding stays intact).
- Lowering `threat_proximity_adrenaline_viability_threshold` —
  that's ticket 286.

## Current state
Blocked by 271 (the parameter-level R1a fix) which landed
2026-05-11. The boldness-on-CP-axis pattern is named as a
structural debt in `docs/balance/271-flee-boldness-axis-shape.md`
§Follow-ons. 271's `bold_cat_fights_when_allies_present` test
rewrite documents the doctrine-relocation question this ticket
resolves — the override is currently the load-bearing mechanism;
R3 makes the substrate carry it.

## Approach

Substrate-over-override per the design pillar. Read CLAUDE.md's
§"Substrate over hacks" precedent (tickets 087/093/163) before
drafting the modifier shape. The post-R3 invariant:

1. `FleeDse` CompensatedProduct over 3 perception scalars only
   (safety_deficit / threat_distance / health_deficit). Each
   encodes one orthogonal axis per memory
   `feedback_single_axis_perception_scalars`.
2. `BoldnessFleeModulation` modifier composes personality
   (boldness) with state (health_deficit) at the modifier layer.
   Reckless cats with health-headroom see Flee suppressed; bold-
   injured cats see less suppression.
3. `behavior_gate_check`'s reckless override either retires (the
   substrate carries the load) or stays as a backstop for
   edge-case profiles the modifier doesn't fully suppress —
   decide via scenario triage.

Verification per CLAUDE.md substrate-migration discipline: the
substrate axes land first; the parameter-level R1a curve fix
retires second (replace the boldness Linear curve with no boldness
axis at all). Never the reverse — partial substrate adoption
collapses behavior during transition (precedent: tickets 091, 111).

## Verification
- All 6 flee scenarios (`flee_calibration_*` + `flee_commitment`
  + `flee_calibration_critical_cornered`) — winners unchanged or
  improved on the critical_cornered case.
- New unit tests on the `BoldnessFleeModulation` modifier shape.
- `bold_cat_fights_when_allies_present` test reverts to checking
  raw scoring (post-R3 the substrate carries the doctrine; the
  override no longer has to).
- `just soak-trace 42 Mocha` + `just verdict` — no regression on
  hard gates or continuity canaries; threat-driven deaths in line
  with post-271 baseline or better.
- `just frame-diff` on focal-trace sidecars vs post-271 baseline
  for the affected DSEs.

## Log
- 2026-05-11: opened by 271 landing as the named R3 structural
  follow-on. See `docs/balance/271-flee-boldness-axis-shape.md`
  §Follow-ons for the design rationale.
