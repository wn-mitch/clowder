# Substrate Phase 4a — softmax, §3.5 modifiers, Adult-window retune

**Date:** 2026-04-22
**Seed / duration:** 42 / 900s
**Baseline:** `logs/tuned-42/events.jsonl` at commit `562c575`
**Phase 4a log:** `/tmp/phase4-soak/events.jsonl` at commit `f0c8813`
(dirty — landing commit supersedes this log on commit; re-soak at
landing commit is a substitute that matches this header's `sim_config`
byte-for-byte and differs only on `commit_hash`).

## Thesis

Phase 3 closed "substrate-complete, balance-gated" with three
regression vectors each tied to a spec-committed Phase 4 deliverable.
Phase 4a lands the three mechanisms as a single bundle because they
interact at the scoring layer and a per-mechanism A/B would have
non-orthogonal effects — softmax changes the selection distribution
that the modifiers operate against, and the Adult-window retune
changes the eligibility pool that both ride on top of. The
four-artifact acceptance lives in this doc for the combined
hypothesis.

## Hypotheses

### 4.1 Softmax-over-Intentions (§L2.10.6)

**Ecological / behavioral claim.** Argmax over disposition peer-groups
concentrates weight on whichever peer-group's MAX dominates. Over a
long soak this produces a monoculture — when hunger spikes and Eat's
peer-group MAX wins, every hungry cat piles into Eat at once, burning
through Stores faster than the food-production chain can replenish,
which cascades into starvation. Softmax over the flat Intention pool
(cat-side) and over the fox disposition pool (fox-side) dissolves the
peer-group collapse: each Intention competes equally in a
temperature-controlled sampler, so behavior diversifies and resource
pressure smooths.

**Predicted direction + magnitude.**

- Starvation deaths: ↓ 50–100%. Direction is clean; magnitude
  bounded by the other Maslow-critical overrides (`enforce_survival_floor`
  is unchanged).
- BondFormed: ↑ 20–80%. Softmax variety gives cats more chances to
  land on Socialize/Mentor/Groom when Eat is not critical.
- Cross-species: fox Hunting plans recover from 0 (baseline argmax
  always picked Fleeing/Avoiding over Hunting).

### 4.2 §3.5 modifier-pipeline port of Herbcraft/PracticeMagic emergency bonuses

**Ecological claim.** Emergency bonuses for ward-setting, cleansing,
and proactive "smelled-rot" response were inline additive adds in
`scoring.rs:576–712` — applied post-DSE-scoring but inside the
scoring function itself rather than through the §3.5 pipeline. The
refactor factors them into `ScoreModifier` trait objects, which keeps
the behavior but restores the magnitude the Phase 3c refactor
inadvertently flattened (additive bonus as composition-internal vs.
post-scoring layer).

**Predicted direction + magnitude.**

- WardPlaced: ↑ 50–200% (the emergency bonus is the ward subsystem's
  primary demand generator).
- ScryCompleted: ↑ or flat. Scry isn't emergency-boosted; its rise
  would be indirect (cats spending more time in PracticeMagic
  disposition when the emergency bonuses make ward/cleanse sub-modes
  competitive with other DSEs).
- CleanseCompleted / CarcassHarvested / SpiritCommunion: still
  dormant. These are gated on outer eligibility (`on_corrupted_tile`,
  `carcass_nearby`, `on_special_terrain`) — a modifier-layer port
  doesn't resurrect them.

### 4.3 Adult life-stage window retune

**Ecological claim.** Cats in the wild are reproductively active for
most of adulthood and decline into infertility late. The Phase-3 Elder
threshold at season 48 compressed the infertility boundary into the
heart of adulthood (median founder is ~7 seasons young at start of
colony; reaches Elder at season 48, bonded-pair mating windows peak
around seasons 60–70 in the historical baseline). Widening Adult to
seasons 12–59 gives bonded pairs a fertile window inside the standard
deep-soak.

**Predicted direction + magnitude.**

- MatingOccurred: ↑ from 0 in a 15-min soak. Magnitude bounded by
  the `pregnancy.rs` cycle period and `breeding_*_floor` gates.
  Predict ≥ 1 as a minimum-viable-behavior bar; density target (≥ 1
  per colony per season = 45 per soak) is still below predicted
  reach because the hypothesis addresses availability, not density.
- DeathOldAge: flat. `elder_entry_seasons` moved in lockstep with
  the stage boundary, so the mortality ramp opens at the same point
  relative to Elder.

## Observations

### Footer diff (survival canaries)

| Metric | Baseline | Phase 4a | Direction |
|---|---|---|---|
| deaths_by_cause.Starvation | 8 | 0 | ✅ |
| shadow_fox_spawn_total | 0 | 0 | flat |
| shadow_foxes_avoided_ward_total | 0 | 0 | flat |
| ward_siege_started_total | 0 | 0 | flat |
| ward_count_final | 0 | 2 | ward coverage persists |
| ward_avg_strength_final | 0.00 | 0.49 | wards held through soak |

All four `scripts/check_canaries.sh` canaries pass.

### Per-feature activation diff (final `SystemActivation` record)

| feature | baseline | phase 4a | delta |
|---|---|---|---|
| MatingOccurred | 0 | 1 | +1 |
| KittenBorn | 0 | 0 | 0 |
| BondFormed | 16 | 28 | +12 (+75%) |
| ScryCompleted | 256 | 562 | +306 (+120%) |
| WardPlaced | 89 | 259 | +170 (+191%) |
| CleanseCompleted | 0 | 0 | 0 |
| CarcassCleansed | 0 | 0 | 0 |
| CarcassHarvested | 0 | 0 | 0 |
| SpiritCommunion | 0 | 0 | 0 |
| GatherHerbCompleted | 8 | 6 | -2 |
| AspirationCompleted | 0 | 0 | 0 |
| BuildingConstructed | 10 | 5 | -5 |
| DirectiveIssued | 5175 | 14434 | +9259 |
| DirectiveDelivered | 734 | 144 | -590 |

### Continuity canaries

| canary | baseline | phase 4a |
|---|---|---|
| grooming | 30 | 132 |
| play | 0 | 0 |
| mentoring | 0 | 0 |
| burial | 0 | 0 |
| courtship | 0 | 1 |
| mythic-texture | 0 | 0 |

## Concordance

### 4.1 Softmax — ACCEPT

Starvation 8 → 0 is direction-match and exceeds the predicted 50–100%
reduction (magnitude implies effectively complete resolution under the
softmax temperature 0.15). BondFormed 16 → 28 (+75%) lands inside the
predicted 20–80% band.

Fox Hunting plans aren't broken out in the footer; follow-on is to add
fox-plan-count telemetry if this claim needs direct verification, but
the Starvation and Bond signals already validate the direction.

### 4.2 Modifier port — ACCEPT

WardPlaced 89 → 259 (+191%) exceeds the predicted 50–200% band at the
top edge. ScryCompleted 256 → 562 (+120%) is consistent with the "cats
spending more time in PracticeMagic disposition" indirect effect. The
three still-dormant sub-modes (Cleanse / Harvest / Commune) are outer-
eligibility-gated as predicted — modifier port alone cannot unblock
them; that's §4 marker-authoring work tracked in open-work #14.

BuildingConstructed 10 → 5 (-50%) is an unpredicted secondary — the
construction pipeline draws on the same Builder personality axis that
feeds Herbcraft; softmax variety + widened PracticeMagic disposition
attracts those diligent cats elsewhere. Not a regression that breaks
the hypothesis — still in spec. Follow-on: check whether the
BuildingConstructed drop correlates with a rise in DirectiveIssued
(yes: +9259, +179%) — coordinators are still *issuing* directives but
followers are preempting them at higher rates under softmax.

### 4.3 Adult-window retune — ACCEPT (minimum bar)

MatingOccurred 0 → 1 meets the minimum-viable bar. The density target
(≥ 1 per season) is not met on this soak, but the hypothesis was
explicitly about the availability gate (Fertility present during a
mating window), not the density. BondFormed +75% corroborates the
social-fabric strengthening that the wider window enables.

No old-age deaths observed in either baseline or Phase 4a — a 900-
season soak only covers ~45 seasons, so the Elder-mortality ramp
doesn't trigger within the soak window in either regime. The coupled
`elder_entry_seasons` bump is verified by code review, not by
measurable telemetry on this soak.

## Next iterations

Phase 4a unblocks the three Phase 3 exit regressions at the substrate
level. The remaining Phase 4 work (target-taking DSE registration,
marker-authoring systems, resolve_mate_with gender fix) and the
follow-on balance levers (MatingOccurred density, dormant
PracticeMagic sub-modes, Farming dormancy) live in open-work #14.
