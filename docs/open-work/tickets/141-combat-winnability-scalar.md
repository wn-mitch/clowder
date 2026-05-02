---
id: 141
title: combat_winnability perception scalar — sibling to escape_viability
status: ready
cluster: ai-substrate
added: 2026-05-02
parked: null
blocked-by: []
supersedes: [046]
related-systems: [ai-substrate-refactor.md, body-zones.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

046 (retired in this commit) raised the cat-side question
*"is this engagement winnable?"* — the substrate-correct framing of
two override-shaped fixes (formula rebuild for FightTarget's
combat-advantage axis; ally-proximity eligibility gate for ShadowFox).
103's out-of-scope explicitly parked `combat_winnability` as a
"separate ticket if Fight-branch needs more than escape-viability
inversion." 105's spec text noted "v1 may use inverse
`escape_viability` as a proxy" but that's a conflation: escape-viability
is terrain-coupled physics (openness + dependents); combat-winnability is
opponent-coupled physics (skill differential + dps balance + ally count).
They aren't inverses, they're orthogonal.

Without this scalar:

- 105's Freeze gate has no real "combat not winnable" predicate.
- 102's Fight gate only checks "escape not viable," ignoring the
  "combat winnable" half of its stated framework.
- 108's rising-threat lurch fires Flee for a posse near a
  ShadowFox — breaking ShadowFoxBanished mythic-texture by removing
  the substrate that distinguishes lone cat from posse.
- 095's dynamic `threat_power` (per key-part condition) reaches the
  IAUS but the FightTarget DSE's combat-advantage axis still uses
  `combat + health − threat` units-mismatch math. The opening
  engagement (full-health cat vs full-health SF) still scores
  "12× advantage" — 046's original collapse-probe shape.

`combat_winnability` is the substrate piece that closes both halves of
046's intent without any DSE override.

## Design

`combat_winnability(cat_state, threat_state, ally_positions, constants)
-> f32`. Single-axis discipline (per the project-wide perception-scalar
principle landed alongside 103): one scalar = engagement-winnability
physics. Personality (boldness, temper), trait modifiers (cornered-cat
ferocity, maternal defense), and ambient combativeness compose at the
modifier layer, not inside this scalar.

Three sub-axes composed multiplicatively (or weighted-sum — pick during
implementation, mirroring 103's choice):

1. **dps-balance** — estimated cat dps vs threat dps:
   `cat_dps = cat.combat * dmg_coeff − target.defense`,
   `target_dps = target.threat_power * key_part_modifier(target)
   − cat.armor` (cat.armor is 0 today; placeholder for future). Returns
   high when cat outpaces threat per-tick, low when threat outpaces.
   Dependent on **095** Phase 2 for `key_part_modifier` to be
   meaningful — pre-095 this term collapses to flat `target.threat_power`,
   which is fine for v1 calibration.

2. **time-to-kill ratio** — `ttk = (cat.health_derived / target_dps) /
   (target.health_derived / cat_dps)`. Saturates via clamp or sigmoid so
   extreme advantages don't dominate the scalar. `health_derived` is
   095 §Shared Formulas: `1.0 − (total_pain / max_possible_pain)` for
   both sides. Pre-095 cats use raw `Health.current / Health.max`;
   pre-095 predators use a placeholder constant.

3. **ally factor** — saturating count of allies within
   `ally_proximity_radius` (default 4 tiles, matching the existing
   FightTarget §6.5.9 ally-proximity weight). Linear-with-cap shape
   (Composite { Linear(slope), Clamp(max) } per the substrate-spec
   §"Saturating-count anchor"). First ally has the largest marginal
   effect.

Defaults calibrated so:
- A healthy cat 1v1 vs a Snake/Hawk reads ≥ 0.6 (cat should engage).
- A healthy cat 1v1 vs a ShadowFox reads ≤ 0.3 (cat should not engage
  alone).
- A 3-cat posse vs a ShadowFox reads ≥ 0.6 (posse should engage —
  ShadowFoxBanished narrative requires this).

Returns `1.0` when no threat is present (analog of 103's
no-threat-short-circuits — undefined-but-safe).

## What lands

1. **`src/systems/interoception.rs`** — pure helpers
   `combat_winnability(...)` and any sub-helpers (e.g.
   `count_allies_in_radius`). Module rustdoc gains a
   `combat_winnability` entry under "Scalars published" naming the
   single-axis discipline.

2. **`src/resources/sim_constants.rs`** —
   `CombatWinnabilityConstants` struct with `serde(default = ...)`
   on every field, `Default` impl, and free-fn defaults. Nested as
   `pub combat_winnability: CombatWinnabilityConstants` on
   `SimConstants` so the full constants block round-trips into the
   `events.jsonl` header (the comparability invariant).
   Fields: `ally_proximity_radius`, `dmg_coeff`, `dps_weight`,
   `ttk_weight`, `ally_weight`, `ally_count_cap`.

3. **`src/ai/scoring.rs`** — `ScoringContext.combat_winnability: f32`
   field. `ctx_scalars` inserts `"combat_winnability"` adjacent to
   `"escape_viability"`. All test fixtures updated.

4. **`src/systems/disposition.rs` + `src/systems/goap.rs`** — both
   populator paths compute the scalar via
   `interoception::combat_winnability(...)`. Ally positions sourced
   from the existing same-faction cat queries (mirror of how
   FightTarget's resolver builds `ally_positions`).

5. **Tests:**
   - Unit tests in `interoception::tests`: no-threat short-circuits to
     1.0; healthy cat vs Snake reads high; healthy cat vs SF alone
     reads low; healthy cat vs SF with 3 allies reads high; injured
     cat vs Snake reads lower than healthy; clamp safety.
   - Integration test in `tests/combat_winnability_scenarios.rs`
     covering the three calibration anchors above.

## Out of scope

- **Modifier consumers** — this ticket ships the scalar only.
  Per the one-modifier-at-a-time discipline (088 → 106/107/110), each
  consumer is its own ticket:
  - Wire 105's Freeze gate to read this scalar (drop the
    inverse-`escape_viability` proxy).
  - Wire 102's Fight gate to also read this scalar (per 102's stated
    framework: "escape not viable AND combat winnable" — currently
    102 only checks the first half).
  - `EngagementUrgency` modifier — substrate replacement for 046
    Layer 2's lone-cat-no-engage intent. Lifts Fight when winnability
    high; suppresses Fight when low. This is the modifier that stops
    a lone full-health cat from engaging a SF without firing an
    interrupt or eligibility gate.

  Open these as their own tickets when this scalar lands and its
  defaults are calibrated.

- **095 dependency** — the scalar is authored to read 095's
  dynamic `threat_power` and `health_derived` *when present*. Pre-095
  it falls back to the existing flat `WildAnimal.threat_power` and
  raw `Health.current / Health.max`. The scalar lands functional
  pre-095; 095 Phase 2 enriches it.

- **Cat armor / damage-mitigation** — placeholder zero in dps-balance
  per 046's same out-of-scope. Real armor is a future mechanic.

- **Predator-side combat_winnability** — predators have their own
  retreat substrate (095 §IAUS §3 key-weapon-broken + pain-threshold).
  This scalar is cat-side only.

## Verification

Five-phase playbook mirroring 103:
- **Phase 1** — pure helpers + constants + populator wiring + unit
  tests. `just check` clean.
- **Phase 2** — focal-trace soak `just soak-trace 42 <cat>` near a
  ShadowFox: scalar value visible in L1 records; lone cat reads low,
  cat with 2+ allies reads high.
- **Phase 3** — survival canaries hold (`just verdict logs/tuned-42`);
  the scalar lands without consumers, so behavior shouldn't shift.
- **Phase 4** — open consumer tickets (105 rewire, 102 rewire,
  EngagementUrgency modifier).
- **Phase 5** — landing checklist per the auto-memory
  landing-routine: clear blocked-by + regen index.

## Log

- 2026-05-02: Opened as the substrate-over-override replacement for
  ticket 046's two override-shaped fixes (formula rebuild + eligibility
  gate). Sibling to 103's `escape_viability` (terrain-coupled physics);
  this scalar is opponent-coupled physics. Single-axis discipline
  honored — personality / phobias / ambient combativeness compose at
  the modifier layer.
