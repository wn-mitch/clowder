---
id: 198
title: Substrate-fires landing gate — DSE curve-non-zero requires sibling scenario (194 P1)
status: ready
cluster: process-discipline
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Closes 194 F8 / P1. When 185 / 188 landed, verification was
`just check` clean + unit tests pass + allowlist drops.
Nothing in the landing flow checked that the substrate's
*Feature events actually fire ≥ 1×* in a representative
scenario. The wave shipped with PickingUp's L2/L3 path
successfully reaching plan-creation and then failing every
plan unreachable — invisible to landing gates.

The CLAUDE.md "never-fired canary" only fires AFTER a soak,
and only on positive Features expected to fire per soak (which
the disposal Features aren't, because they're conservative).
The landing-time gap is real.

## Direction

When a DSE curve is lifted from default-zero (or a new DSE
registers with non-zero scoring), the same commit must include
a deterministic scenario test that asserts the corresponding
`Feature::*` emits ≥ 1× when conditions are set up to exercise
it. The curve-shape + eligibility tests (which 185 / 188
included) are necessary but insufficient.

### Implementation sketch

1. **Track DSE curve-non-zero status.** Extend
   `scripts/check_substrate_stubs.sh` (or a sibling lint
   script) with a parser pass over `src/ai/dses/*.rs` that
   detects whether the scoring curve is the default-zero
   `Linear { slope: 0.0, intercept: 0.0 }` or some non-zero
   shape (Logistic, non-zero Linear, Composite, etc.).
2. **Sibling-scenario requirement.** For every non-zero DSE,
   require a scenario file under `src/scenarios/` whose
   `expected_features` metadata (new field) names a Feature
   that the DSE's plan template ultimately writes through
   `record_if_witnessed`. The lint maps DSE → expected
   Features via either:
   - explicit `expected_features` declaration on the scenario
     struct, or
   - a small per-DSE registry in `src/scenarios/mod.rs` keyed
     by DSE name.
3. **Run the scenario in CI.** The scenario harness already
   runs deterministically in ~3s (per 162). Extend
   `just check` to invoke each scenario in
   `src/scenarios/registry` and assert the expected Feature
   counters reach ≥ 1× in the scenario footer (or whatever
   the per-scenario equivalent is — `runner.rs` may need a
   small instrumentation hook).
4. **Allowlist for legitimately-rare DSEs.** Scenarios
   exercising rare paths (e.g., legend-tier outcomes) opt out
   via `expected_features: vec![]` plus a comment. The lint
   passes them silently — the absence is the contract.

### Files this would touch

- `scripts/check_substrate_stubs.sh` (or new `check_dse_scenarios.sh`)
- `src/scenarios/runner.rs` (instrumentation hook)
- `src/scenarios/mod.rs` (per-DSE registry, or per-scenario
  metadata field)
- 1+ new scenario per currently-non-zero DSE that lacks one
  (audit pass during implementation)

## Out of scope

- Retro-adding scenarios for every existing non-zero DSE in
  the same commit. Land the gate; let new tickets bring
  scenarios up incrementally. Scenarios already in
  `src/scenarios/` (15 today) cover most current DSEs.
- Replacing the never-fired canary — that catches per-soak
  expected-fire; this catches per-scenario does-fire-at-all.
  Different layers, both useful.
- Per-Feature negative-emission gating (more complex; defer
  unless needed).

## Verification

- `just check` fails when a DSE moves from default-zero to
  non-zero without a sibling scenario file (constructed test:
  flip `discarding_dse` curve in a scratch branch).
- Existing scenarios in `src/scenarios/` continue to pass
  the new gate — no spurious failures.
- Scenario runtime stays under ~30s aggregate (single-scenario
  budget × 15 = ~45s today; new gate adds Feature-count
  assertions, not new sim ticks).

## Log

- 2026-05-06: opened from 194's closeout. Cluster
  `process-discipline`. Medium scope — touches lint, scenario
  harness, scenario metadata. The 185 closeout cost a full
  wave's worth of follow-up; this ticket exists to make that
  failure mode hard to repeat.
