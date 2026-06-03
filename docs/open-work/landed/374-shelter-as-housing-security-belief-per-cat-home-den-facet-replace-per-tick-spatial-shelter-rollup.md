---
id: 374
title: Shelter as housing-security belief — per-cat home-den facet, replace per-tick spatial shelter rollup
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-16
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 136619c75a7b
landed-on: 2026-06-03
---

## Why

`compute_shelter` at `src/systems/colony_score.rs:20-39` models the welfare
`shelter` axis as a per-tick spatial proximity check: count cats within
`den_shelter_radius` of a functional Den, divide by total cats. The
coordinator's `pressure.shelter` (`src/systems/coordination.rs:889-897`,
`973-978`) has the same shape, gated even tighter — only cats in
`Action::Sleep` AND without a Den within 4 tiles count as "unsheltered
sleepers". Both signals collapse a *continuous psychological state* —
"do I feel housed?" — into a *transient spatial query* — "am I currently
near a Den?".

Real shelter doesn't work that way. A cat at work (out hunting) with a
home Den they consider theirs has high shelter. A cat whose home Den was
recently raided has falling shelter even if they're standing on it right
now. A cat with no claimed Den at all has low shelter regardless of
proximity. The cat the coordinator should worry about is the one whose
*belief* about housing is degrading, not the one who happens to be
awake-and-roaming this tick.

The structural gap that makes this hard today: cats have no
`home_den: Option<Entity>` field. `PreyAnimal::home_den`
(`src/components/prey.rs:109`) and `WildAnimal::home_den`
(`src/components/wildlife.rs:344`) exist as first-class fields — prey
have a home, foxes have a home, cats don't. The belief infrastructure
(`CatBeliefs`, `LocationBeliefs`, `MentalModel`, `Facet`, `EvidenceKind`,
`CandidateFacet` in `src/components/beliefs.rs`) is rich enough to host
this, but no shelter facet is plugged in.

This is the exact pattern memory `feedback_model_perception_as_beliefs`
points at: fix shelter at the 258 belief layer (WitnessableEvent →
belief_integrator → ShelterFacet), not by adding more raw spatial axes.
Memory `feedback_single_axis_perception_scalars` warns against folding
multiple distinct situations into one scalar — this ticket's job is
the opposite, decomposing shelter into orthogonal sub-axes the cat
can perceive independently.

## Scope

1. **`home_den: Option<Entity>` on the canonical per-cat component**
   (mirroring `PreyAnimal::home_den` / `WildAnimal::home_den`).
   First-class "this Den is mine."

2. **`ShelterFacet` on `CatBeliefs`** with sub-axes (each independently
   updateable; each contributes orthogonally to the rollup):
   - `belonging` — do I have a home_den claimed at all?
   - `quality` — belief about that den's condition / effectiveness / spaciousness
   - `continuity` — how long has it been mine? (slow decay when absent)
   - `threat` — belief about active siege / decay / contestation

3. **Belief integrator** for `ShelterFacet`. New `WitnessableEvent`
   variants (or reuse existing) for:
   - `DenClaimed` / `DenLost` — belonging axis updates
   - `DenDamaged` / `DenRepaired` — quality axis
   - `DenSieged` / `DenSiegeBroken` — threat axis
   - Continuity decays slowly per tick when cat isn't near home den
     (or accrues when they are)

4. **Rewrite `compute_shelter`** as roll-up of per-cat `ShelterFacet`
   confidence × belonging × (1 - threat). Cats with high belief security
   contribute fully; cats with falling beliefs contribute partially.
   `welfare.shelter` becomes "how secure does the colony feel about
   housing" not "how many cats are spatially near a Den right now".

5. **Rewrite `pressure.shelter`** to read from per-cat belief sum.
   Coordinator elects Den construction when *enough cats believe
   themselves to be housing-insecure*, not when *N cats are visibly
   homeless-while-sleeping*. This is the cleaner trigger — it fires
   on the rising-edge of belief decay, not on the rare spatial
   coincidence of `Action::Sleep` + distance > 4.

## Out of scope

- **Den claiming logic.** Who picks which Den as their home_den, how
  ties resolve when multiple cats want the same one, how kittens
  inherit / transition, what happens when a cat outgrows their Den.
  This is its own social/spatial scoping problem. Park as a follow-on
  once the belief substrate exists.
- **Tuning the new `pressure.shelter` thresholds.** Once the substrate
  is in place, balance-tuning the trigger threshold is a hypothesize
  ticket, not this one.
- **Modifying Den construction itself.** Build DSE, ConstructionSite,
  plan templates — all unchanged. This ticket changes what *triggers*
  the request, not how the request is fulfilled.
- **Migrating other welfare axes** (acceptance, mastery, purpose,
  social_warmth, respect) from spatial rollups to belief rollups. Each
  axis has its own substrate-correctness analysis. Ticket scope is
  shelter only.

## Current state

Discovered 2026-05-16 during ticket 190's bug-hunt for "why isn't the
colony building more Stores when chronically full". Layer-walking the
welfare metrics surfaced that the parallel question — "why doesn't
the colony build more Dens" — has a similar shape but a different root
cause. Chronic-full latches reliably (10K+ trace lines per soak); the
shelter signal almost never fires because its trigger is structurally
narrow. Two adjacent problems, two different layers; 190 fixes its
own composition weights, this ticket fixes shelter's perception model.

## Approach

Builds on the 258 belief substrate (`src/components/beliefs.rs` —
`MentalModel`, `Facet`, `EvidenceKind`, `CandidateFacet`). Mirrors the
shape of 294 (RecentAmbushMap → LocationBeliefs.recency_of_threat_cue)
and 293 (HuntingPriors → LocationBeliefs) — those tickets retired
colony-resource per-tick spatial rollups in favor of per-cat belief
state. Same pattern, applied to shelter.

Implementation phases:

1. **Phase A — `home_den` field.** Add `home_den: Option<Entity>` to
   the canonical per-cat component. At spawn, founder cats claim the
   nearest pre-spawned Den. Kittens born in a Den inherit that as
   home_den.
2. **Phase B — `ShelterFacet`.** Add the facet to `CatBeliefs` with
   the four sub-axes. Define `WitnessableEvent` variants for the
   updates. Wire the belief integrator.
3. **Phase C — Roll-up rewrite.** Change `compute_shelter` to read
   per-cat facet, aggregate. Change `pressure.shelter` to read same.
   Keep the old function as a doc-deprecated fallback for one cycle
   if needed for migration.
4. **Phase D — Verification.** Scenario microexperiment:
   `shelter_belief_security` — a cat with a claimed home_den, no
   threats, sees high shelter belief. Then siege the den, watch
   threat axis spike and belonging axis decay. Verify
   `welfare.shelter` tracks the per-cat belief change, not the
   spatial position.

## Verification

- Scenario `shelter_belief_security` passes (per phase D).
- Frame-diff on `welfare.shelter` vs the 190-promoted baseline. Expect
  the value to *change* (it's now a different signal); concordance
  should be a documented re-baselining, not a regression.
- `pressure.shelter` firing-rate increases under stress scenarios
  (Den raided, fox siege), decreases when colony has many cats with
  stable home_dens. Validates the directional change.
- Continuity canaries hold; survival hard-gates pass.
- `just check && just test` clean.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **  7** (done, ai-substrate, score 0.88) — Deliberation-layer (Cluster C)
- · **303** (ready, buildings-zones, score 0.87 (cross-cluster)) — split cat_value into movement-intensity and residence axes (298 structural foll…
- ✓ landed **313** (done, ai-substrate, score 0.87) — re-examine cat_value and distance_cost in ward-placement scoring (301 FO-3)

<!-- linkages:end -->
## Log

- 2026-05-16: opened from 190's layer-walk. User flagged the welfare
  axis bug specifically — "I still feel housed when I'm at work."
  Existing prey/wildlife `home_den` shape provides the structural
  precedent for the cat-side field; 258 belief substrate provides
  the facet shape; 294 / 293 provide the retirement pattern (colony
  per-tick rollup → per-cat belief state).
- 2026-05-19: accuracy audit pass — blocked-by empty and status ready; ShelterFacet not yet in src/ (aspirational); home_den exists in prey.rs/wildlife.rs but not on canonical cat component yet; 258 belief substrate exists in src/components/beliefs.rs
- 2026-06-02: empirical anchor from ticket 494 closure. The post-494
  Chebyshev realignment shifted `welfare.shelter` from baseline
  `0.125` to `0.0` in `logs/tuned-42-9b3f5d43` (-100%), with
  `welfare` aggregate dropping `0.554 → 0.480` (-19.2%). This is
  exactly the per-tick spatial rollup pathology this ticket targets:
  a metric change to `Position::distance_to` (Euclidean → Chebyshev,
  matching 8-direction movement cost) ripples through
  `compute_shelter`'s `den_shelter_radius` count and collapses the
  signal to zero, despite the colony surviving 91k ticks vs baseline
  59k (+54%, a substrate-positive outcome on every other axis).
  Concrete evidence the spatial rollup is fragile under metric
  changes; belief-rollup migration is well-motivated. The new
  baseline `logs/tuned-42-9b3f5d43` carries a welfare/shelter
  caveat pointing here so future verdicts treat the axis as
  tracked-known-shape, not a fresh concern.
- 2026-06-03: 2026-06-03: landed substrate cutover. New ShelterBeliefs component carrying home_den + 4-axis ShelterFacet; 6 WitnessableEvent variants (DenClaimed/Lost/Damaged/Repaired/Sieged/SiegeBroken); 4 per-stagger systems own the home_den claim, continuity, damage-emit, siege-detect paths; compute_shelter and pressure.shelter rewritten as belief rollups. Diagnostic pass: pre-fix continuity_weight=1.0 silently zeroed welfare.shelter; fixed by seeding quality from condition on DenClaimed and defaulting continuity_weight=0.3. Soak verdict: concern (drift expected per balance doc) — survival/continuity canaries pass, never_fired_expected_positives resolved, shelter 0.0→0.0498 (new-nonzero re-baselining off the post-494 structurally-zero spatial signal). Same mating-renaissance schedule-edge shape as 293.
