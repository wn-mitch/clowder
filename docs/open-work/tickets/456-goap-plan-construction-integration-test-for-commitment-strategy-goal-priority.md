---
id: 456
title: GOAP plan-construction integration test for commitment-strategy goal priority
status: ready
cluster: tooling-diagnostics-ui
initiative: []
orchestration: swarm-safe
added: 2026-05-23
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Test coverage on the AI hot path is **asymmetric**: scoring and modifier mechanics are heavily unit-tested (147 inline tests in `src/ai/modifier.rs`, 64 in `src/ai/scoring.rs`), and the scenario harness gates substrate-fires + L3 score invariants, but there is no dedicated test verifying that the **commitment-strategy persistence bonus holds across ticks under competing goal pressure.** Today this property is exercised only by the seed-42 deep-soak (15 minutes) and indirectly by scenario L3-score invariants. A unit-time test closes the loop in milliseconds.

The commitment layer per `docs/systems/ai-substrate-refactor.md` §L2.10.6 + §7.4 is *the* single substrate by which a held Intention should resist being preempted by competing per-tick DSE scores. The "Commitment is one mechanism, not two" design pillar names this as load-bearing. The 364→397 kitten-arc cluster (named in CLAUDE.md) demonstrates what happens when a parallel commitment substrate is wired alongside §L2.10.6 — four follow-on patches and a multi-month refactor. A test that pins the convergent behavior would have made the regression diagnostic immediate.

## Scope

A new test (or pair of tests) that:

1. Preloads a cat with two simultaneously-pressing needs (e.g., Hunger and Social, both above their saturation thresholds) such that two different DSEs would score competitively at L2.
2. Runs the simulation for ≥ 50 ticks via the scenario harness or a direct `App::new()` + `SimulationPlugin` setup.
3. **Asserts that once the cat picks a goal at tick T, the held Intention persists across the next K ticks under reasonable persistence-bonus values** (does not oscillate goal-to-goal tick-by-tick).
4. Optionally: a sibling test that verifies the *opposite* — when the suppressed DSE crosses a critical threshold (e.g., Hunger > starvation cliff), commitment correctly yields. This is the "softmax_winner_preempts_pin Caretake exception" shape per ticket 397.

## Out of scope

- Adding new DSEs, modifiers, or commitment substrate. The test asserts current behavior.
- Tuning the persistence-bonus magnitude. If the test reveals the bonus is mis-tuned (oscillation occurs), that's a separate substrate-sensitive ticket; this one only ships the probe.
- Replacing the soak-level commitment validation. The seed-42 deep-soak continues to be the integration backstop; this test is a fast complement.
- Covering HTN-method-driven multi-tick goals. `RaiseOffspringAspiration` and similar HTN flows have their own commitment story (§7.M); a separate test follows once those land more broadly. The test here targets per-tick DSE-emitted Intentions only.

## Current state

Integration-test precedents already in the repo:

- `tests/integration.rs` — uses real `SimulationPlugin` + `HeadlessIoPlugin` via canonical plugin path. The seam where this test would slot in.
- `tests/scenarios.rs` — drives the `src/scenarios/` harness; asserts substrate-fires and L3 invariants over scenarios. A new scenario under `src/scenarios/` named something like `commitment_persistence_hungry_social.rs` with an assertion entry in `tests/scenarios.rs` follows the existing convention.
- `tests/hawk_goap_smoke.rs` / `tests/snake_goap_smoke.rs` — GOAP-specific smoke tests for predator AI. Precedent for a `tests/goap_commitment_smoke.rs` if the scenario-harness path is awkward.

The persistence-bonus implementation lives in `src/ai/scoring.rs::select_disposition_via_intention_softmax_with_trace` and the read of `HeldGoalStack` in `src/systems/goap.rs::evaluate_and_plan`. The Architecture audit (2026-05-23 health pass) verified the commitment mechanism is single-channel and read-only against `HeldGoalStack`.

## Approach

### Structural-option menu

- **scenario-harness (chosen)** — add a scenario under `src/scenarios/` preloading the two-need cat, and an assertion in `tests/scenarios.rs` matching the pattern of `kitten_cry_basic_emits_focal_trace_with_caretake_in_ranked_list`. Fits the existing convention; the scenario also serves as a debugging surface (`just scenario commitment_persistence_*`).
- **direct integration test (rejected)** — bypass the scenario harness; spin up `App::new() + SimulationPlugin` directly in `tests/goap_commitment_smoke.rs`. Rejected because the scenario harness already provides the deterministic preset-cat setup this test needs; duplicating that scaffolding diverges from the canonical path (the `PE-001` drift problem `tests/integration.rs` calls out in its preamble).
- **unit test in `src/ai/scoring.rs`** (rejected) — would have to mock `HeldGoalStack` and the disposition softmax pool, which is exactly the "hand-mocked, not exercising real code paths" failure mode the user's CLAUDE.md prohibits.

### Sequence

1. Write the scenario under `src/scenarios/commitment_persistence_<archetype>.rs`. Preset: one adult cat with Hunger and Social both above their L2 score-competitive bands; food and another cat both at moderate distance.
2. Register in `src/scenarios/mod.rs::ALL`.
3. Add assertion in `tests/scenarios.rs`: after 50 ticks, `commitment_streak >= 10` for whichever Intention was chosen at tick T, where T is the first tick the cat exits Idle. (Exact threshold tunable via the scenario-spec — start permissive, tighten after first observation.)
4. `just test` green; `just check` green.

## Verification

- `cargo test --test scenarios commitment_persistence` runs in < 5s and passes.
- A synthetic mutation (e.g., set the persistence-bonus to 0.0 in a hand-edited test build) MUST fail the assertion. Implementer demonstrates this in the commit message or PR.
- `just scenario commitment_persistence_<archetype>` runs interactively and prints the per-tick winning DSE for the focal cat (manual verification surface).
- No new flakiness in `just test`; seed-42 deep-soak unchanged.

## Log

- 2026-05-23: opened from session audit (the "is this project vibe-coded" health pass). The Tests subagent identified plan-construction commitment as the thinnest area in an otherwise well-covered AI hot path. Cluster `tooling-diagnostics-ui` per the scenario-harness home; orchestration `swarm-safe` because writing the test is mechanical against verified-correct existing behavior. If the test surfaces that current behavior is not what we expect, that's a different ticket.
