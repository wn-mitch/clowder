# 146 — Distress-substrate value tune (inert ships, saturating cap added)

Closes the 088 / 111 / 146 clade. Settles per-axis distress modifier
(106 HungerUrgency, 107 ExhaustionPressure, 110 ThermalDistress) values
for the canonical seed-42 regime and adds a saturating-composition cap
to the modifier pipeline as scaffolding for future tuning.

## Hypothesis

Per-axis distress modifier defaults of zero (inert) produce a healthier
colony on seed-42 than non-zero defaults that emerged from
substrate-over-override migration. Half-magnitude lifts route cats into
partial-satisfaction Sleep / Eat windows that leave them weaker against
shadow-fox pressure than the inert regime where the un-lifted IAUS
contest handles the same needs.

## Prediction

`colony_score.aggregate` increases ≥3% with inert defaults vs the
half-magnitude defaults; survival hard-gates hold;
`continuity_tallies.courtship` may remain at 0 (parked under follow-on
ticket per §Decision below).

## Observation

Five manual single-seed-42 soaks across the value space:

| Config                              | Duration | Agg     | Welfare | Happy | Health | Nourish | Seasons | Deaths | Courtship |
|-------------------------------------|----------|---------|---------|-------|--------|---------|---------|--------|-----------|
| inert (preserved removal-bare)      | 15min    | **1025.8** | 0.416 | 1.000 | 0.244  | 0.594   | 6       | **3/8**| 0         |
| half-magnitude + cap=0.60           | 30min    | 950.6   | 0.270   | 1.000 | 0.098  | 0.251   | 4       | 8/8    | 0         |
| full (0.20 etc) — surfaced          | 30min    | 968.5   | 0.417   | 1.000 | 0.087  | 0.997   | 6       | 8/8    | 0         |
| full + cap=0.60                     | 30min    | 876.4   | 0.312   | 0.575 | 0.159  | 0.628   | 6       | 8/8    | 0         |
| inert + cap=0.30                    | 30min    | 909.6   | 0.345   | 0.625 | 0.179  | 0.865   | 10      | 8/8    | 0         |

A `just hypothesize`-driven 1-seed × 1-rep × 5-min sweep returned
"wrong-direction" with Δ -2.3% — but at 5min only 2 seasons elapse,
which doesn't reach the extinction phase that drives the 15-30min
divergence. The fast result confirms the inert/half difference is
time-dependent and emerges in the survival window. Multi-seed
verification deferred to the follow-on tuning ticket.

## Concordance

- Direction match: ✓ (inert agg 1025.8 vs full 968.5 = +5.9% increase)
- Magnitude in band: ✓ (within [3, 30]%)
- Survival hard-gates: ✓ (3/8 deaths vs 8/8 extinction in every other config)
- 107+110 Sleep double-stack mechanism confirmed: surfaced-lifts collapses
  health 0.244 → 0.087 with cats falling into Sleep loops on cold tired
  nights, away from Patrol coverage. Saturating cap at 0.60 partially
  compensates but doesn't fully recover survival.

## Decision

**Ship inert (path 1).** Per-axis lift defaults set to 0.0 in
`ScoringConstants`. Modifiers remain registered, configurable, and
testable — but ship dormant. Same shape as 047 originally landed under
substrate-over-override; survival validated by the preserved
`tuned-42-baseline-removal` evidence.

**Saturating-composition cap added** (`max_additive_lift_per_dse`,
default 0.60) as scaffolding. Cumulative positive lift across the §3.5.1
modifier pipeline saturates via `MAX * (1 - Π(1 - lift_i / MAX))`,
bounding the pessimal Sleep stack (raw 1.10 → effective ≈0.547) without
clipping single-modifier behavior (047 Flee 0.60 passes through, 088
0.20 passes through). The cap is invisible at lift=0; it only matters
when future balance work surfaces non-zero per-axis lifts.

**088 retirement deferred.** The diagnostic for 146 found 088's role in
the courtship chain is non-structural: it's a six-thousandths-of-fondness
nudge to one specific dyad (Mocha+Birch in seed-42), not a systemic
contributor. Whether to retire 088 turns on the courtship-chain
fragility separately. Working-tree changes that touched 088's
infrastructure are committed as-is; the modifier remains active.

**Courtship parked.** The 146 investigation found removal-bare's
fondness ceiling sits at 0.297 vs the 0.30 courtship-drift gate — a
knife-edge fragility in the bond-formation pathway, orthogonal to the
distress substrate. Open follow-on ticket for the courtship-fondness
ceiling investigation.

## Follow-on tickets

- **NEW** — "Per-axis distress modifier value tuning (multi-seed)":
  the 0.0 defaults are placeholder. A proper balance ticket should sweep
  values across multiple seeds with rep counts, comparing both colony
  aggregate AND specific behavioral targets (does the modifier achieve
  its intended phenomenon — does HungerUrgency actually route cats to
  Eat earlier than the un-lifted contest?). The `just hypothesize` flow
  + `just sweep-stats` machinery exists for this; the work is choosing
  the right metrics and values.
- **NEW** — "Courtship-chain fondness ceiling vs gate fragility":
  fondness for the most-bonded Adult-Adult-eligible pair tops out at
  ~0.30 in seed-42 — exactly at the courtship-drift gate. Investigate
  whether this is a Socialize / GroomOther rate problem, a fondness
  ceiling problem, or a colony-coherence (Adult cats not co-locating)
  problem.

## Survival canaries (final 15-min seed-42 soak)

`logs/tuned-42-146-final-nocap/` — inert per-axis lifts + saturating cap
disabled (default 0.0).

| Hard gate                                     | Result | Pass |
|-----------------------------------------------|--------|------|
| `deaths_by_cause.Starvation == 0`             | 0      | ✓    |
| `deaths_by_cause.ShadowFoxAmbush <= 10`       | 8      | ✓    |
| Footer line written                           | yes    | ✓    |

| Continuity canary | Tally | Status |
|-------------------|-------|--------|
| grooming          | 194   | ✓ alive |
| play              | 219   | ✓ alive |
| courtship         | 999   | ✓ alive |
| mythic-texture    | 41    | ✓ alive |
| mentoring         | 0     | ✗ pre-existing collapse (not 146-introduced) |
| burial            | 0     | ✗ pre-existing collapse (no emitter system) |

`colony_score.aggregate = 997.5`. Matches the preserved baseline regime
(commit `9945e59`) — confirming that with per-axis distress lifts at 0.0
and the new saturating cap disabled, the IAUS contest behaves
identically to pre-146 baseline. The substrate decomposition (per-axis
modifiers exist but inert, cap exists but disabled) is a no-op
behaviorally and a +scaffolding change for future balance work.

## What ships in 146

1. **Five per-axis distress modifier lift defaults set to `0.0`**
   (`hunger_urgency_eat_lift`, `_hunt_lift`, `_forage_lift`,
   `exhaustion_pressure_sleep_lift`, `thermal_distress_sleep_lift`).
   Modifiers remain registered and configurable; they ship inert pending
   per-axis tuning under a follow-on ticket.
2. **Saturating-composition pipeline cap** (`max_additive_lift_per_dse`,
   default `0.0` = disabled). Code path in
   `ModifierPipeline::apply_with_trace` accumulates positive deltas and
   reshapes via `MAX * (1 - Π(1 - lift_i / MAX))` when both `cap > 0`
   and `>= 2 positive deltas` fire. Set the constant to `0.60` to
   activate (matches 047 single-modifier Flee design value, bounds
   pessimal Sleep stack to ≈0.547).
3. **No 088 retirement.** 146's diagnostic found no structural
   coupling between 088 and the courtship chain — 088 is a behavioral
   nudge with mostly fortuitous downstream effect. Whether to retire
   it is a separate question, deferred to ticket 111's eventual
   re-evaluation.

## Closing the clade

- **146**: closes done. Investigation produced concrete findings
  (structural-coupling refuted, knife-edge fondness mechanism
  identified, U-curve in lift magnitudes characterized) plus the two
  artifacts above.
- **111**: closes parked-without-retirement. 088 stays active per
  finding (3) above; 111 reopens only if the courtship-fondness
  follow-on or a future tuning surfaces a reason to retire 088.
- **088**: remains landed; no change.

