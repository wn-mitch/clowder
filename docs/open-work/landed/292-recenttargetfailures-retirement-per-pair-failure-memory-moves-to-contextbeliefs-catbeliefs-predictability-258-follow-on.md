---
id: 292
title: RecentTargetFailures retirement — per-pair failure memory moves to ContextBeliefs / CatBeliefs predictability (258 follow-on)
status: done
cluster: belief-perception
orchestration: substrate-sensitive
initiative: [full-sensory-perception]
added: 2026-05-11
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: ea55e329
landed-on: 2026-07-07
---

## Why

`RecentTargetFailures` (`src/components/recent_target_failures.rs`) is a per-cat HashMap keyed `(GoapActionKind, Entity)` that records when a plan step failed against a specific target. The 258 plan-agent audit identified it as a typed-failure proxy that semantically *is* "my model of how predictable this target is for this action" — exactly the `MentalModel<Cat>.predictability` (and in some cases `MentalModel<Predator>.predictability`) facet that C3 substrate now carries. Retiring it folds two redundant memory layers into one, removes a per-cat HashMap-of-tuples whose key shape doesn't survive the per-target DSE consideration model, and frees the six target DSEs (HuntTarget / FightTarget / SocializeTarget / MateTarget / GroomOtherTarget / MentorTarget) to read a unified belief-state instead of an action-keyed proxy.

## Scope

- Add `WitnessableEvent::TargetActionFailed { actor: Entity, action: GoapActionKind, target: Entity, position: Position, tick: u64 }` variant.
- Emit at the two writer sites: `src/systems/plan_substrate/lifecycle.rs:50` (`record_step_failure`) and `src/systems/plan_substrate/lifecycle.rs:96` (`abandon_plan` plan-destruction record).
- `belief_integrator::apply_observation` handles the new variant: when `actor == witness`, lower `CatBeliefs[target].predictability` (or `PredatorBeliefs[target].predictability` for wildlife targets) via EMA toward `OBSERVED_FAIL`.
- Rewire `target_recent_failure_age_normalized` in `src/systems/plan_substrate/sensors.rs:51` to read `CatBeliefs / PredatorBeliefs[target].predictability` (action-agnostic — the new substrate doesn't key on action).
- Update the 6 target DSE consideration sites: `src/ai/dses/{hunt,fight,socialize,mate,groom_other,mentor}_target.rs` — each currently passes `recent: Option<&RecentTargetFailures>` to the sensor; change to `Option<&CatBeliefs>` (or unified Option<&Beliefs> if cleaner).
- Delete `src/components/recent_target_failures.rs`, the spawn-time bundle insert at `src/plugins/setup.rs:108`, and the `prune_recent_target_failures` system registration at `src/plugins/simulation.rs:497`.
- Decide on action-keying granularity: the legacy proxy keyed by `(action, target)` so failing to Hunt cat-X didn't penalize Socializing with cat-X. The new substrate is target-keyed only. Either (a) accept the loss of granularity (per CLAUDE.md pillar-3 "richer perception, better strategy" — predictability *is* the load-bearing axis), or (b) bring action-keying back via `EnvironmentalContextKey::ActionExecution(GoapActionKind)`. Default: (a). Validate via hypothesize.

## Out of scope

- 290 (RDF reader cutover) — predates this ticket; predictability tunables live in 258's `BeliefAxisTunables` and 290's hypothesize cycle calibrates them.
- HuntingPriors retirement (293) — overlapping concern but different keying (location, not target).
- RecentAmbushMap retirement (294) — Resource, not Component.

## Current state

258 landed 2026-05-11 (commit `c3bce3500e6e`). Substrate is alive; `CatBeliefs[target].predictability` is computed and decayed but has no writer for target-action failures yet (only `WitnessedFleeFrom` and `WitnessedHunt` lift it via EMA).

The 6 target DSEs currently consume `target_recent_failure_age_normalized` as one consideration each (see `src/ai/dses/socialize_target.rs:152` for the canonical shape). The legacy `RecentTargetFailures` `(action, target)` keying gives action-specific cooldowns — Hunting failing on cat-X doesn't block Mating with cat-X. This ticket's design choice (a) vs (b) is the key call.

## Approach

Four-artifact methodology required (per CLAUDE.md drift > 10% rule). The migration touches 6 DSEs simultaneously — variance across seeds will be wider than 290's single-disposition swap.

1. **Hypothesis**: Target-keyed (action-agnostic) predictability preserves L2 score shape for the 6 target DSEs within ±10% mean-score drift. Action-specific cooldown loss is absorbed by personality + relationship modifiers + scoring-axis composition.
2. **Observation**: `just hypothesize docs/balance/292-recent-target-failures-retirement.yaml` — sweep baseline (dual-emit + legacy reader) vs treatment (cutover).
3. **Concordance**: direction match + magnitude within 2×.
4. **Balance doc**: `docs/balance/292-recent-target-failures-retirement.md`.

If hypothesis fails (action-agnostic too lossy), pivot to choice (b): add `EnvironmentalContextKey::ActionExecution(GoapActionKind)` as a third key axis and key the new emit's facet update on the (context, target) pair. Log the pivot in `## Log` and re-run hypothesize.

Implementation order: emit-site additions first (substrate populates), tests pass; then reader cutover + 6 DSE wire updates in one commit; then proxy deletion; then hypothesize.

## Verification

- `just check` clean.
- `cargo test plan_substrate::sensors` — refactored unit tests pass.
- `just soak 42` + `just verdict` — survival + continuity canaries hold.
- `just hypothesize docs/balance/292-recent-target-failures-retirement.yaml` — concordance pass per six-DSE final-score sweep.
- `just frame-diff` baseline vs treatment focal trace — confirm per-DSE drift on Hunt/Fight/Socialize/Mate/Groom/Mentor targets all within hypothesis band.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **  7** (done, ai-substrate, score 0.88 (cross-cluster)) — Deliberation-layer (Cluster C)
- ✓ landed **261** (done, ai-substrate, score 0.87 (cross-cluster)) — ActionAffordances substrate — per-action success scalars + ActionKind enum + he…
- ✓ landed ** 52** (done, ai-substrate, score 0.87 (cross-cluster)) — §L2.10.7 plan-cost feedback substrate + cat target-taking roster

<!-- linkages:end -->
## Log

- 2026-05-11: opened as 258 follow-on. Per-pair failure memory is one of three typed-failure proxies that 258's plan-agent audit identified as belief-substrate-redundant. Sibling proxies: 290 (RecentDispositionFailures), 293 (HuntingPriors), 294 (RecentAmbushMap).
- 2026-05-19: accuracy audit pass — 258/261 (prerequisites) are landed; all file paths and Rust symbols verified; four-artifact methodology structure sound.
- 2026-07-07: implemented as three commits — emit sites (5da9c48d:
  `TargetActionFailed` variant + lifecycle emits + integrator arm,
  first-person only, kind-routed cat→CatBeliefs /
  wildlife→PredatorBeliefs / prey-corpse-structure unmodeled per the
  505 ballast rule, `prior = 1.0` pinned for recovery), reader
  cutover (3511e9a6: `target_predictability_signal` sensor, SEVEN
  target DSEs — `bury_target` landed post-audit and joined the six —
  input renamed `target_recent_failure` → `target_predictability`
  for trace honesty, `Feature::TargetCooldownApplied` preserved),
  deletion (ea55e329: component + prune system + spawn insert +
  legacy sensor + `target_failure_cooldown_ticks` constant retired).
  Design choice (a) taken as pre-registered; noted deltas: recovery
  window 8000-tick linear → ~3000-tick convex EMA (mirrors 290), and
  prey/corpse targets now permanently fail-open (churn-suppression
  owned by 467/514 structural fixes; plan-failure canary is the
  net). Four-artifact record:
  `docs/balance/292-recent-target-failures-retirement.md`. Gate soak
  `tuned-42-ea55e329`: all four predictions confirmed (survival +
  continuity pass; TargetCooldownApplied 971× via the belief path;
  the one new plan-failure spike is an early-run trajectory burst,
  not a loop; tps at par). Hypothesize-sweep deviation recorded in
  the balance doc's Concordance with the pivot-(b) watch signals.
  LANDED.
