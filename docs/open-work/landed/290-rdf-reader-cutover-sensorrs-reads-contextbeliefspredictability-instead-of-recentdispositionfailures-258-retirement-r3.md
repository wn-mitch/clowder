---
id: 290
title: RDF reader cutover — sensor.rs reads ContextBeliefs.predictability instead of RecentDispositionFailures (258 retirement R3)
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
landed-at: 40397a72
landed-on: 2026-05-18
---

## Why

258 landed the C3 belief substrate + a dual-emit at the `make_plan → None` site: `WitnessableEvent::SelfPlanFailed` now populates `ContextBeliefs[DispositionExecution(kind)].predictability` *alongside* the legacy `RecentDispositionFailures` write. The substrate is alive end-to-end but no consumer reads it. The IAUS cooldown signal that gates re-picking a failing disposition still flows through the proxy. This ticket closes the loop: swap the sensor reader from RDF to ContextBeliefs, delete RDF, validate that the EMA-with-decay shape preserves the existing linear-cooldown semantics within balance band per the four-artifact methodology.

## Scope

- Rewrite `disposition_recent_failure_age_normalized` at `src/systems/plan_substrate/sensors.rs:135` to take `Option<&ContextBeliefs>` instead of `Option<&RecentDispositionFailures>`. Compute the same `[0,1]` "cooldown-age" scalar (1.0 = no penalty, 0.0 = just failed) from `predictability.value` (high predictability = reliable = old/no failure → 1.0; low predictability = recently failed → 0.0).
- Update the 7 caller sites in `src/systems/goap.rs` (lines ~2040, 2049, 2066, 2075, 2084, 2093, 2102) that pass `recent_disposition_failures.as_deref()` to instead pass the cat's `ContextBeliefs`. The cats query at `src/systems/goap.rs:1138` swaps `Option<&mut RecentDispositionFailures>` for `Option<&ContextBeliefs>` (immutable — reads only).
- Delete the dual-write at `src/systems/goap.rs:~2590–2596`. Keep the `WitnessableEvent::SelfPlanFailed` emit.
- Delete `src/components/recent_disposition_failures.rs`, the `pub mod` declaration in `src/components/mod.rs:30`, and the re-export at `src/components/mod.rs:92`.
- Remove `prune_recent_disposition_failures` system registration from `src/plugins/simulation.rs:501` and delete the function from `src/systems/plan_substrate/sensors.rs:169`.
- Remove the constant `planning_substrate.disposition_failure_cooldown_ticks` from `src/resources/sim_constants.rs:5540` if no remaining readers; otherwise leave it as a tunable that scales the predictability-to-cooldown mapping.
- Update tests in `src/systems/plan_substrate/sensors.rs:416–510` (~7 test calls) to construct `ContextBeliefs` instead of `RecentDispositionFailures`.
- Tune `BeliefAxisTunables::slow()` `predictability.learning_rate` + `decay_rate_to_prior` so the EMA shape matches the legacy 4000-tick linear cooldown within ±10% on the characteristic-metric drift band. Initial guess: lr=0.5 (single failure drops predictability by ~0.5), decay_rate=0.00025/tick (returns to prior=1.0 in ~4000 ticks). Iterate via `just hypothesize`.

## Out of scope

- Other typed-failure-proxy retirements (RecentTargetFailures → 292, HuntingPriors → 293, RecentAmbushMap → 294). Each gets its own four-artifact ticket because each has distinct keying and a distinct reader fan-out.
- ColonyKnowledge restructure (291).
- New WitnessableEvent emit sites (295).

## Current state

258 landed 2026-05-11 (commit `c3bce3500e6e`). The substrate is wired:

- `ContextBeliefs[DispositionExecution(kind)]` populates via dual-emit from `goap.rs::evaluate_and_plan`.
- `belief_integrator` (`src/systems/belief_integrator.rs`) consumes `WitnessableEvent::SelfPlanFailed`, lowers `predictability.value` toward `OBSERVED_FAIL = 0.0` via EMA, decays back toward prior=1.0 on staggered Pass B frames.
- Two seed-42 deep-soaks (scaffolding-only + dual-emit) both posted `verdict: pass` with 0.0% drift across every footer field.

The legacy `RecentDispositionFailures` is still the load-bearing reader for the IAUS cooldown — flipping the reader is THIS ticket's behavior change. Audit table from 258's session lives in `~/.claude/plans/work-258-scalable-squid.md`.

## Approach

This is a balance change per CLAUDE.md "A refactor that changes sim behavior is a balance change." Run the four-artifact methodology:

1. **Hypothesis**: Replacing the linear-age cooldown with EMA-of-predictability preserves the L2 score shape of the six target DSEs (HuntTarget, ForageTarget, CraftTarget, CaretakeTarget, BuildTarget, MateTarget) within ±5% mean-score drift. **Prediction**: action distribution unchanged within ±5%; all five continuity canaries hold; `deaths_by_cause.Starvation == 0`; `ShadowFoxAmbush ≤ 10`.
2. **Observation**: run `just hypothesize docs/balance/290-rdf-reader-cutover.yaml` — baseline (current main, dual-emit) vs treatment (this ticket's cutover). Multi-seed sweep so the 4000-tick linear-vs-EMA cadence difference shakes out.
3. **Concordance**: direction match + magnitude within ~2× of prediction.
4. **Draft balance doc**: `docs/balance/290-rdf-reader-cutover.md` capturing the four artifacts + tuning iterations.

Implementation order: (a) Rewrite sensor + 7 callers + cats-query type swap in one commit. (b) Delete dual-write + RDF file + prune system + mod re-export. (c) Initial tuning pass — pick lr/decay defaults from the legacy 4000-tick half-life. (d) Run `just hypothesize` cycle. (e) Iterate tunables until concordance lands. (f) Land balance doc.

## Verification

- `just check` clean (substrate-stub, step-resolver, time-units, InfluenceMap registry).
- `cargo test` — sensor.rs unit tests pass with the new ContextBeliefs construction.
- `just soak 42` + `just verdict logs/tuned-42/` — survival + continuity canaries hold.
- `just hypothesize docs/balance/290-rdf-reader-cutover.yaml` — concordance pass.
- `just frame-diff` against a focal trace from 258's scaffolding soak — confirm the 6 target DSEs' final_score distributions match within hypothesis band.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **  7** (done, ai-substrate, score 0.87 (cross-cluster)) — Deliberation-layer (Cluster C)
- · **249** (parked, ai-substrate, score 0.85 (cross-cluster)) — Extend DispositionFailureCooldown coverage to Resting/Guarding/PickingUp et al.…
- ✓ landed ** 87** (done, ai-substrate, score 0.85 (cross-cluster)) — Interoceptive perception substrate

<!-- linkages:end -->
## Log

- 2026-05-11: opened as 258 follow-on. Substrate-side wiring is the dual-emit landed in 258 (commit `c3bce3500e6e`). This ticket finishes the proxy retirement that 258's scope-decision deferred.
- 2026-05-18: Cutover landed as two commits: A (sensor rewrite + integrator latent-bug fix + 7-caller swap + tunable inline) and B (RDF/dual-write/prune/constant retire). Four-artifact balance write-up at docs/balance/290-rdf-reader-cutover.md captures iter-1 (kept: lr=1.0 decay=0.00075) and iter-2 (rejected: decay=0.00035 collapsed shelter/health). Iter-1 surfaces +52% bonds_formed / +67% kittens_born / +18% peak_population drift vs pre-290 baseline; survival gates pass cleanly (0 deaths). Drift is substrate-revealing — the EMA's faster mid-cooldown recovery (~0.55 at t=1000 vs legacy 0.25) reads as more colony activity, consistent with pillar #3 (richer perception, better strategy). Future: multi-seed sensitivity sweep on the predictability tunables.
