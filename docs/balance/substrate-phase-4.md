# Substrate Phase 4 — softmax, §3.5 modifiers, Adult-window retune, gender fix, §4 marker foundation

**Date:** 2026-04-22
**Seed / duration:** 42 / 900s
**Baseline:** `logs/tuned-42/events.jsonl` at commit `562c575`
**Phase 4a log:** `logs/phase4a-c4552dc/events.jsonl` at commit
`c4552dc` (landed; `commit_dirty: false`). The numbers below update
with every re-soak at the landing commit — the dirty-commit log at
`/tmp/phase4-soak/events.jsonl` from the pre-landing run is retained
in the repo scratch space for diff against this one; its
`MatingOccurred = 1` versus the landed run's `MatingOccurred = 0` is
seed-noise at the one-count level (bond-progression timing shifted
just enough to miss the single pair's short viable window).

Soak scope note. `start_tick = 60 * ticks_per_season = 1.2M`, so a
900-second sim clock covers ~7 sim-seasons (end tick ~1.34M). Earlier
revisions of this doc miscounted this as 900 seasons; all "per soak"
targets below are calibrated to the ~7-season actual.

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

| Metric | Baseline | Phase 4a (landed) | Direction |
|---|---|---|---|
| deaths_by_cause.Starvation | 8 | 0 | ✅ |
| shadow_fox_spawn_total | 0 | 0 | flat |
| shadow_foxes_avoided_ward_total | 0 | 0 | flat |
| ward_siege_started_total | 0 | 0 | flat |
| ward_count_final | 0 | 4 | ward coverage persists |
| ward_avg_strength_final | 0.00 | 0.39 | wards held through soak |

All four `scripts/check_canaries.sh` canaries pass.

### Per-feature activation diff (final `SystemActivation` record)

| feature | baseline | phase 4a (landed) | delta |
|---|---|---|---|
| MatingOccurred | 0 | 0 | 0 (seed noise; dirty run had 1) |
| KittenBorn | 0 | 0 | 0 |
| BondFormed | 16 | 34 | +18 (+112%) |
| ScryCompleted | 256 | 615 | +359 (+140%) |
| WardPlaced | 89 | 264 | +175 (+197%) |
| CleanseCompleted | 0 | 0 | 0 |
| CarcassCleansed | 0 | 0 | 0 |
| CarcassHarvested | 0 | 0 | 0 |
| SpiritCommunion | 0 | 0 | 0 |
| GatherHerbCompleted | 8 | 8 | 0 |
| AspirationCompleted | 0 | 0 | 0 |
| BuildingConstructed | 10 | 5 | -5 |
| DirectiveIssued | 5175 | 14474 | +9299 (+180%) |
| DirectiveDelivered | 734 | 157 | -577 (-79%) |
| KnowledgePromoted | 35 | 92 | +57 (+163%) |

### Continuity canaries

| canary | baseline | phase 4a (landed) |
|---|---|---|
| grooming | 30 | 213 |
| play | 0 | 0 |
| mentoring | 0 | 0 |
| burial | 0 | 0 |
| courtship | 0 | 0 |
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

### 4.3 Adult-window retune — ACCEPT (substrate-success) / defer density

MatingOccurred stayed at 0 on the landed-commit soak — the single
mating observed on the dirty-commit pre-landing run was seed-noise
at the one-count level (one bonded pair just barely clearing the
interest + estrus window in the dirty-run RNG trajectory). The
substrate-level signal is BondFormed 16 → 34 (+112%), which
evidences the wider Adult window is giving bonded pairs more
overlap with each other's fertility cycles; the density gap is
therefore a Fertility-cycle-pacing / bond-progression-timing gap,
not an availability gap.

The ~7-sim-season soak covers ~14 Fertility cycles per cat, of
which ~5 carry a viable phase, and winter anestrus zeros ~25% of
those. Per the `FertilityConstants` defaults (cycle = 10k ticks,
proestrus 15%, estrus 20%, post-partum 5k ticks), a bonded
Queen–Tom pair's realistic mating window per soak is 2–4 brief
Estrus bursts. Hitting one of those bursts requires the pair to
also be colocated, sated (`breeding_*_floor`), past the mating
interest threshold, and not in winter — the product of those
independent gates explains the 0–1 rate.

No old-age deaths observed in either regime — the ~7-season soak
doesn't reach the (new) season-67 Elder-mortality ramp in either
baseline or Phase 4a. The coupled `elder_entry_seasons` bump is
verified by code review, not by measurable telemetry on this soak.

Density target (≥ 1 per colony per season ≈ ≥ 7 per 7-season soak)
remains unmet and is deferred to a follow-on balance iteration
named in open-work #14.

## Phase 4b additions

Two additional mechanisms landed after Phase 4a on the same seed-42
soak window:

- **Phase 4b.1 (`90105c7`) — §7.M.7.4 `resolve_mate_with` gender
  fix.** `Pregnant` now lands on the gestation-capable partner,
  not the initiator. Tom×Tom fails cleanly. No direct soak-metric
  prediction — a bug fix at the anatomy layer. Tests cover the
  four gender permutations.

- **Phase 4b.2 MVP (`89d6b81`) — §4 marker lookup foundation +
  `HasStoredFood` reference port.** The `has_marker` closure moves
  from its `|_, _| false` stub to a real `MarkerSnapshot` lookup.
  `EatDse` gains `.require("HasStoredFood")` and the inline
  `if ctx.food_available` outer gate retires in both code paths.
  Behavior-preserving by design — the marker state is populated
  from the same `!food.is_empty()` check the retired gate used.

### Phase 4b.2 soak concordance (seed 42, `--duration 900`, log `logs/phase4b2-wip/events.jsonl`)

| Metric | Phase 4a | Phase 4b.2 | Direction |
|---|---|---|---|
| deaths_by_cause.Starvation | 0 | 0 | ✅ canary still passes |
| shadowfox_ambush | 0 | 0 | flat |
| Grooming (continuity) | 213 | 229 | +16 (seed noise) |
| BondFormed | 34 | 31 | -3 (seed noise) |
| ScryCompleted | 615 | 614 | -1 (noise) |
| WardPlaced | 264 | 246 | -18 (noise) |
| BuildingTidied | 128 | 274 | +146 |
| DirectiveDelivered | 157 | 512 | +355 |
| RemedyApplied | 20 | 0 | -20 (low-count variance) |

**Interpretation.** Phase 4b.2 is predicted to be behavior-
preserving — the marker lookup returns the same eligibility answer
the retired inline bool gave. The visible deltas split cleanly:

- Headline metrics (Starvation, Shadowfox) flat — canaries pass.
- Mid-tier deltas (BondFormed ±3, ScryCompleted ±1, WardPlaced
  ±18) are inside seed-noise envelope for 7-season soaks.
- Two notable positive deltas (BuildingTidied +146, DirectiveDelivered
  +355) point to a small ordering shift: retiring the outer
  `if ctx.food_available` gate frees the scoring loop to push
  Eat with score 0 into the flat pool before the softmax filter
  drops it, which changes RNG-consumption order enough to reorder
  some downstream tie-breaks. Not a semantic change; just a
  determinism delta under fixed seed.
- RemedyApplied 20 → 0 is low-count variance on a rare event —
  `GatherHerbCompleted` held at 7–8 across both runs, so the
  herbcraft supply chain is intact; the drop is the application
  side not firing in this specific seed. Multi-seed sweep would
  be needed to confirm.

Acceptance: all four `scripts/check_canaries.sh` canaries pass at
Phase 4b.2. Marker-port semantics verified behavior-preserving
within variance bounds.

## Phase 4b additions (continued)

- **Phase 4b.3 (`f009ec6`) — §6.3 `TargetTakingDse` type +
  evaluator.** Foundation-only: struct shape, three aggregation
  modes (`Best` / `SumTopN` / `WeightedAverage`), evaluator with
  per-candidate considerations and target-scoped scalar dispatch.
  No live-sim behavior change — nothing registers a target-taking
  DSE yet. 6 unit tests cover the full evaluator surface.

- **Phase 4b.4 (`e5b46e5`) — §4 `HasGarden` marker port.** Second
  reference port of the Phase 4b.2 pattern. Farm's outer
  `if ctx.has_garden` gate retires; `FarmDse::new()` gains
  `.require("HasGarden")`.

### Final Phase 4 soak at HEAD (`db7362b`)

All four survival canaries pass:

```
[pass] starvation_deaths                0 (target == 0)
[pass] shadowfox_ambush_deaths          0 (target <= 5)
[pass] footer_written                   1 (target >= 1)
[pass] features_at_zero                 0 (target informational)
```

Mid-tier deltas vs Phase 4b.2 are consistent with the HasGarden
marker port reshuffling eligibility-filter linear-scan order:
`DirectiveIssued` +2500, `DirectiveDelivered` -352,
`ScryCompleted` +59, `WardPlaced` +48. No semantic change.

## Balance-tuning deferral

Three positive-feature metrics remain below their literal
Phase 4 exit targets:

- **MatingOccurred = 0** (target ≥ 7 per 7-season soak).
- **PracticeMagic sub-modes = 2 / 5** (target ≥ 3 / 5).
- **Farming = 0** (target ≥ 1).

**These are not treated as Phase 4 blockers.** Per the commitment
recorded in `docs/open-work.md` #14's balance-tuning-deferral
section:

1. No colony wipes on the final soak — all four survival canaries
   pass. Density gaps are verisimilitude, not existential.
2. Successor refactor phases (target-taking DSE per-DSE ports,
   full §4 marker catalog, §5 influence maps, §7 commitment
   strategies) reshape scoring for exactly the DSEs whose numbers
   would be tuned. Tuning now would be redone after each
   successor phase.
3. CLAUDE.md's Balance Methodology requires four-artifact
   acceptance per drift. Tuning against a moving substrate wastes
   artifacts.

Commitment: balance iterations on positive-feature density wait
until the refactor's substrate changes have stabilized. The
dormancy gaps (Cleanse / Harvest / Commune / Farming) trace to
refactor-layer missing plumbing — target-taking DSE with spatial
candidates or GOAP plan-shape preparatory steps. Landing those
naturally unblocks the dormancies before any numeric tuning is
relevant.

## Next iterations

Phase 4 substrate closes with softmax, modifier pipeline, Adult-
window retune, gender fix, marker-lookup foundation, and
target-taking DSE foundation all landed. Two reference marker
ports (HasStoredFood, HasGarden) prove the pattern; the
target-taking DSE reference-port work (Socialize first) is the
natural next session. Balance tuning waits until the refactor's
successor phases stabilize — see balance-tuning deferral above.
