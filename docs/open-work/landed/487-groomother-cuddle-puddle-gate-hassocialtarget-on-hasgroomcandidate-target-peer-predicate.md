---
id: 487
title: GroomOther cuddle puddle — gate HasSocialTarget on HasGroomCandidate target-peer predicate
status: done
cluster: ai-substrate
initiative: []
orchestration: substrate-sensitive
added: 2026-05-29
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: pending
landed-on: 2026-05-29
---

## Why
The "cuddle puddle" — `GroomOther` chain-grooming dominating day-one L3
winners across the founder cohort — is observable again. It was previously
narrowed but not extinguished:

- `ca5d59c4` (gate `PickingUpDse` on `HasFoodStorageAccessible`) recorded
  the puddle as an open follow-up in its commit body: *"the same fix
  unmasked an emergent 'cuddle puddle' in the first ~5k elapsed ticks —
  GroomOther chain-grooming dominance from the broad-phase HasSocialTarget
  gate, which has no destination/target peer predicate analogous to
  HasFoodStorageAccessible."*
- `b24d333b` (warm-floor founder relationship init) tried to shape the
  puddle by lifting founder fondness/familiarity floors so first-tick
  novelty axes wouldn't dominate. Its own commit body reports the test
  honestly: *"this change did NOT dent the GroomOther L3-winner share in
  the first 5k elapsed ticks (37.2% treatment vs 36.8% baseline)."* The
  warm floors diversified the tail (Flee, Cook, Coordinate moved) but the
  head of the distribution stayed pinned on `GroomOther`.

The b24d333b commit explicitly names the headline lever still living
elsewhere: *"the structural eligibility-asymmetry follow-up
(HasGroomCandidate marker mirroring HasFoodStorageAccessible) and the
day-one-work-surfaces layer-walk row in the session plan."* This ticket
promotes that follow-up out of the session plan and into open work.

## Scope
- Add per-cat `HasGroomCandidate` marker (or analogous `HasSocializeDest`)
  authored from the real target-peer set the `GroomOther` resolver would
  accept — same shape as `HasFoodStorageAccessible` is authored from the
  colony's `Stores` buildings.
- Add `.require(HasGroomCandidate)` to the broad-phase social-target
  surface that currently feeds `GroomOther` (most likely
  `HasSocialTarget` itself, or a sibling gate that fires before
  `GroomOther` selection — confirm at audit time).
- Ship reader, writer, and `MarkerSnapshot::set_*` together per the
  substrate-stubs convention (precedent: ca5d59c4).

## Out of scope
- Tuning the `GroomOther` DSE response curves or weights. The pillar
  ([[pillar-substrate-over-hacks]]) directs structural gating first.
- The wider novelty-axis decomposition implied by Pillar #3 (richer
  perception) — a separate question of whether `socialize_target`'s
  `1 - familiarity` is itself too sharp. Park behind this gate.
- Any narrative re-styling of grooming behaviour. The puddle isn't a
  narrative problem; it's an eligibility-asymmetry problem.

## Current state
- Precedent ticket landed: `484` / commit `ca5d59c4` (PickingUpDse +
  HasFoodStorageAccessible). Read this one first — same shape.
- Empirical baseline: b24d333b session-plan layer-walk row, GroomOther
  L3-winner share 36.8% baseline / 37.2% post-warm-floor across the
  first 5k elapsed ticks on seed-42 focal Simba.
- Verification context: 485 (sustained-copresence retain retirement) and
  486 (NearPairCache retain retirement) have landed/are landing in
  parallel — neither perturbs the social/grooming surface; this ticket
  is independent of both.

## Approach
1. **Audit first** — confirm the broad-phase gate the resolver actually
   uses. `HasSocialTarget` is the strong suspect from the ca5d59c4 commit
   body, but verify against `src/ai/dse_registry.rs` / `src/steps/`. Run
   `just q trace` on seed-42 Simba's first 5k ticks and check which
   marker is on `GroomOtherDse.require`.
2. **Define the candidate set** — what does the `GroomOther` resolver
   accept as a target? At a minimum: within reach, not Dead, not
   Incapacitated, not currently being groomed by someone else. Read
   `src/steps/disposition/groom_other.rs` (or wherever the resolver's
   target query lives) and mirror its filter set in the marker author.
3. **Author the marker** — new `HasGroomCandidate` ZST in
   `src/components/markers.rs`. Author from a single late-tick system
   (or piggyback on an existing per-cat marker authoring pass to avoid a
   new schedule edge). Set/clear with hysteresis if the candidate set
   flickers.
4. **Gate the DSE** — `.require(HasGroomCandidate)` on the GroomOther
   surface. Plumb the marker through `MarkerSnapshot::set_*` for
   `evaluate_and_plan` AND `build_planner_markers` parity (same
   substrate-stubs requirement that ca5d59c4 satisfied).
5. **Exhaustive-match coverage** in the canary surface (silent-canary
   convention).

## Verification
- Focal trace re-run on seed-42 Simba, first 5k elapsed ticks. Expect
  `GroomOther` L3-winner share to drop from ~37% toward the structural
  floor implied by the candidate-set size and other-DSE eligibility.
- Day-one Action distribution diversifies further — `Cook`, `Build`,
  `Forage`, `Patrol` should pick up the freed bandwidth.
- `passive_familiarity_total_pairs` (or equivalent footer field) holds
  at baseline.
- All hard survival gates + continuity canaries hold (the marker is
  additive — gating an over-eager surface; it should not silence any
  legitimate grooming).
- `just frame-diff` against the b24d333b focal trace shows GroomOther
  scoring shape unchanged in shape (only its `.require` gate masks the
  early-game cohort).

## Log
- 2026-05-29: opened from a session observation. Headline lever named in
  `ca5d59c4` (PickingUpDse fix unmasked the puddle) and `b24d333b`
  (warm-floor relationship init did not extinguish it; named the
  HasGroomCandidate follow-up explicitly). Promoted from the session-plan
  layer-walk row into a sized ticket.
- 2026-05-29: landed three substrate layers + two latent-defect fixes
  bundled into a single commit. **Layer A** — `HasGroomCandidate` ZST
  authored in `evaluate_and_plan` + `resolve_goap_plans` parity site,
  excludes both groomers and groomees from each other's viability set,
  gates `GroomOtherDse` eligibility. **Layer B** — `ColonySelfDirectiveQueue`
  resource populated by `assess_colony_needs` when no able coordinator
  exists (Forage/Build/Herbcraft only), `ActiveDirective.coordinator`
  widened to `Option<Entity>`, `dispatch_urgent_directives` drains both
  queues, fallback bonus `colony_self_directive_weight = 0.5`. **Layer
  C** — `ColonyAlignmentScore` EWMA component (decay 0.99965, increment
  0.00035 per aligned-action tick) wired into `evaluate_coordinators`
  as `(1 + score × 0.5)` multiplier; new `flag_coordinator_incapacitated`
  system strips Coordinator marker on incapacitation and triggers
  re-election. **Defect (a) follow-on** — `resolve_groom_other_target`
  now consumes a `currently_groomed: Option<&HashSet<Entity>>` and skips
  mid-groom peers at the candidate-loop head; the prior layer-A author
  filtered the marker but the resolver still picked mid-groom peers,
  letting chains extend across the marker gate. **Defect (b) follow-on**
  — `validate_target_for_step` carve-out admits `Incapacitated +
  NewbornKitten` targets for `FeedKitten` steps; the generic alive-gate
  was rejecting eyes-closed kittens (who are `Incapacitated` by design
  via `incapacitation.rs`'s OR clause), surfacing 131 false-positive
  `PlanStepFailed` events per 5-min soak. Mirrors `Bury`'s existing
  per-step carve-out shape.
- 2026-05-29: validation against `logs/afk-487-validation/` (the
  layer-A/B/C soak; defect (a)+(b) not exercised by this run). Hard
  gates pass: Starvation 0, ShadowFox 0, deaths {}, all cats alive at
  health 1.0. Continuity canaries hold (grooming 892, play 12,
  mentoring 237, courtship 3189). **Cuddle-puddle structural win**:
  Simba first-5k-tick Grooming disposition share dropped from 36.8%
  (b24d333b baseline) to 4.2% (19 GroomOther plans / 450). Open
  follow-on observations (NOT blockers): (1) Exploring absorbed 90.9%
  of freed bandwidth — [[project_l3_patrol_absorption_cascade]]
  textbook; Layers B+C did not visibly redirect into Forage/Build/Cook
  (likely `colony_self_directive_weight = 0.5` too small or alignment
  feedback not firing yet). (2) Economy degraded: `BuildingRepaired=0`,
  5 wards (all decayed), 75 herbs deposited / 3 retrieved. (3) 0
  matings despite 3189 courtship interaction-ticks — anomaly worth its
  own ticket. (4) 1427 `no path and stuck` failures (pre-existing in
  the b24d333b baseline too). The puddle is broken; what to do with
  the freed bandwidth is the next layer.
- 2026-05-29: landed three substrate layers (HasGroomCandidate gate, colony-self directives, emergent-coordinator alignment) plus two latent-defect fixes (resolver-side currently_groomed filter, FeedKitten Incapacitated-newborn carve-out). Simba first-5k-tick Grooming share 36.8% → 4.2% on logs/afk-487-validation/. Patrol absorption + economy degradation surfaced as follow-on items.
