---
id: 146
title: 088 BodyDistressPromotion courtship-coverage investigation
status: done
cluster: ai-substrate
added: 2026-05-02
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, distress-modifiers.md]
related-balance: [146-distress-substrate-inert.md]
landed-at: TBD
landed-on: 2026-05-02
---

## Why

Ticket 111 attempted to retire 088's `BodyDistressPromotion` once the four
kind-specific axis modifiers (047 / 106 / 107 / 110) had landed. The
verification soak revealed 088 is structurally load-bearing for the
courtship/mating chain in a way the per-axis modifiers don't cover —
even after surfacing per-axis lifts at conservative magnitudes. Both
`CourtshipInteraction` and `PairingIntentionEmitted` joined the
never-fired-expected-positives list and `continuity_tallies.courtship`
collapsed 999 → 0 in two consecutive verification soaks.

This ticket investigated whether the dependency was structural (some
code path reading `BodyDistressed` marker or `body_distress_composite`
scalar) or behavioral (DSE-contest dynamics).

## Findings

### Structural-coupling hypothesis: refuted

`BodyDistressed` (087 ZST marker) is **write-only**. Authored at
`src/systems/interoception.rs:365-393` from the
`body_distress_composite() ≥ threshold` predicate, read by exactly one
site: the marker authoring system's own `Has<BodyDistressed>`
idempotency check (line 326). No DSE eligibility filter, no step
resolver, no social/pairing/mate code reads it.

`body_distress_composite()` (`interoception.rs:124-133`) is computed
every tick but called only by the marker author. No modifier reads it.

### The behavioral mechanism: knife-edge fondness ceiling

Mocha (Adult Queen, the most-bonded courtship-eligible cat in seed-42)
has these max-fondness ceilings across runs:

| Run                           | Mocha max fondness | vs 0.30 gate | CourtshipDrifted events |
|-------------------------------|--------------------|--------------| ------------------------|
| baseline (088 active)         | 0.303              | +0.003 above | 999 (all Mocha+Birch)   |
| removal-bare (088 removed)    | 0.297              | -0.003 below | 0                       |
| surfaced-lifts (088+per-axis) | 0.297              | -0.003 below | 0                       |

Six thousandths of fondness is the entire courtship-chain margin. 088
nudged the fondness ceiling above 0.30 for one specific dyad
(Mocha+Birch, who bonded as Friends pre-Birch's Adult maturation, then
Birch matured and the gate opened). This is fortuitous, not designed.

### U-curve in per-axis lift magnitudes

Single-seed-42 manual sweep across five lift configurations:

| Config                      | Aggregate | Deaths | Seasons | Courtship |
|-----------------------------|-----------|--------|---------|-----------|
| inert (0.0) — baseline      | **997.5** | 8      | 7       | **999**   |
| half (0.10) + cap=0.60      | 950.6     | 8      | **4**   | 0         |
| full (0.20) — surfaced      | 968.5     | 8      | 6       | 0         |
| full + cap=0.60             | 876.4     | 8      | 6       | 0         |
| inert + cap=0.30 (saturating)| 909.6    | 8      | 10      | 0         |

Non-monotonic: half-magnitudes are WORSE than either full or inert —
they route cats into partial-satisfaction Sleep/Eat windows that leave
them weaker against shadow-fox pressure than the inert regime where
the un-lifted IAUS contest handles needs.

## What ships under 146

See `docs/balance/146-distress-substrate-inert.md` for the iteration
record.

1. Per-axis distress modifier defaults set to `0.0` (matches inert
   baseline regime). Modifiers remain registered and configurable —
   ship dormant pending a follow-on tuning ticket with multi-seed
   variance evidence.
2. Saturating-composition pipeline cap added to
   `ModifierPipeline::apply_with_trace`. Default `0.0` (disabled).
   Reshapes cumulative positive lift via
   `MAX * (1 - Π(1 - lift_i / MAX))` when `cap > 0` and `>= 2 positive
   deltas` fire. Set `max_additive_lift_per_dse = 0.60` to activate
   (matches 047 single-modifier Flee design value).
3. **088 stays active.** Investigation found no structural reason to
   retire it; the courtship-chain dependency is incidental, not
   causal. 111 closes parked-without-retirement.

## Out of scope

- Re-tuning 047 / 102 / 105 / 108 magnitudes.
- Retiring 087 `BodyDistressed` ZST marker (it is write-only — could
  be retired as a separate cleanup follow-on, but not in this ticket).
- Multi-seed variance verification of the inert regime (deferred to
  the follow-on tuning ticket).

## Follow-on tickets opened

- **NEW** — Per-axis distress modifier value tuning (multi-seed
  hypothesize). The 0.0 defaults are placeholder; proper balance work
  needs `just hypothesize` + `just sweep-stats` evidence across
  multiple seeds for each per-axis lift value.
- **NEW** — Courtship-chain fondness ceiling vs gate fragility.
  Fondness for the most-bonded Adult-Adult pair tops out at ~0.30 in
  seed-42, exactly at the courtship-drift gate. Investigate whether
  this is a Socialize / GroomOther rate problem, a fondness ceiling
  problem, or a colony-coherence (Adult cats not co-locating) problem.

## Log

- 2026-05-02: Opened from the ticket-111 verification soak finding.
  Three preserved soaks + the original baseline as input artifacts.
- 2026-05-02: **Investigation complete.** Structural-coupling
  refuted via grep+read-site enumeration. Behavioral mechanism
  identified as knife-edge fondness ceiling at 0.30 gate. U-curve in
  per-axis lift magnitudes characterized via 5-config single-seed-42
  manual sweep. Final inert-with-cap-disabled regime reproduces
  baseline behavior (agg 997.5, courtship 999, deaths 8). Per-axis
  modifier defaults landed at 0.0; saturating cap landed at 0.0; both
  scaffolding for follow-on tuning. Status `ready` → `done`.
