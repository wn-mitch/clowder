---
id: 483
title: GOAP-side EatFromOwnInventory plan step — lift the autonomic eat-from-pocket reflex into L2/L3 election
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: [smarter-cats]
added: 2026-05-27
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

429 extracted `resolve_eat_from_own_inventory` as a first-class Sink resolver but kept the per-tick autonomic dispatcher at `src/systems/needs.rs::eat_from_inventory` in place — the dispatcher fires whenever a cat has `hunger < eat_from_inventory_threshold` (0.4) AND food in inventory. The user's framing at 429 plan time was: "shouldn't 'Eat' and 'from own inventory' be handled by GOAP?" — i.e., the L2 `EatDse` should score, GOAP should plan the cheapest chain, and `[EatFromOwnInventory]` should be the 1-step alternative to `[TravelTo(Stores), EatAtStores]` when pocket food is present.

429 punted on this because the per-tick reflex preempts any GOAP plan today (it eats the food before the planner can build the chain), so adding the GOAP-side variant without removing the reflex would be vestigial. Removing the reflex IS the substrate change — but that's a real behavior shift: kittens currently rely on the reflex for autoconsume. The 429 landing soak (logs/tuned-42-f1b699a5) confirmed the autonomic path never fires in healthy seed-42 colonies — cats eat at Stores before hunger drops below 0.4. So the GOAP-side wiring won't shift the seed-42 footer materially; the real verification is the kitten substrate path.

## Scope

- New `Action::EatFromOwnInventory` in `src/ai/mod.rs::Action`; `from_action` mapping in `src/components/disposition.rs` returns `Some(DispositionKind::Eating)` (shares the Eating disposition with `Action::Eat`).
- New `StepKind::EatFromOwnInventory` in `src/components/task_chain.rs`.
- New `GoapActionKind::EatFromOwnInventory` in `src/ai/planner/mod.rs`.
- Extend `eating_actions()` in `src/ai/planner/actions.rs` to include the new variant with precondition `HasMarker(HasFoodInInventory::KEY)` and effect `SetHungerOk(true)`. Cost 1 (cheaper than `EatAtStores` cost 2 so the planner picks the 1-step pocket path when both predicates hold).
- Add `require_any` API to `EligibilityFilter` (`src/ai/dse.rs`): `pub fn require_any(self, markers: &[MarkerKey]) -> Self`. The substrate's first OR-shaped eligibility primitive.
- Extend `EatDse` eligibility from `.require(HasStoredFood::KEY)` to `.require_any(&[HasStoredFood::KEY, HasFoodInInventory::KEY])`.
- Dispatcher in `src/systems/goap.rs` for the new `GoapActionKind::EatFromOwnInventory` arm — calls `resolve_eat_from_own_inventory` (the resolver already exists per 429).
- Decide on the autonomic reflex: either retire it entirely (adults + kittens plan through GOAP) OR gate it on `Has<Kitten>` (kittens autoconsume; adults plan). Pick the safest variant after a scenario-level test.
- Re-enroll `Feature::EatFromOwnInventory` as `expected_to_fire_per_soak() => true` if (and only if) the soak observes it firing reliably.

## Out of scope

- `ItemTransfer` / `ItemSink` trait retrofit of `src/steps/disposition/**` function-shape resolvers — separate work item.
- Further DSE-eligibility OR-shapes beyond Eat — this ticket adds the `require_any` primitive but doesn't audit every other DSE for OR-shape candidates.
- Trader / off-colony Source integration (parked in 381) — GOAP-side eat-from-pocket works once the substrate is in place; 381 adds new Source paths.

## Current state

429 extracted the Sink resolver (`src/steps/disposition/eat_from_own_inventory.rs`) and demoted `Feature::EatFromOwnInventory` to `expected_to_fire_per_soak() => false` post-soak. The dispatcher at `needs.rs::eat_from_inventory` routes through the resolver and emits the Feature via `record_if_witnessed`. Substrate scaffolding for the GOAP-side wiring exists — Action enum, StepKind, GoapActionKind, planner-action factory, dispatcher — but none has been threaded through for the eat-from-pocket case.

The forward-reference comments at `src/components/markers.rs:405` and `src/systems/goap.rs:1425` (post-429) explicitly name "a 429 follow-on will extend `EatDse`'s eligibility filter." This ticket IS that follow-on.

## Approach

Mirror `EatAtStores`'s plumbing across the parallel surface. The `require_any` API addition is the only genuine substrate-axis extension — design it to compose cleanly with `.require(…)` (a DSE can require X AND any-of Y/Z). Implementation: a `Vec<Vec<MarkerKey>>` `required_any` field on `EligibilityFilter` where each inner Vec is an OR-set (and ALL outer Vecs must have at least one match — AND-of-OR semantics).

The autonomic-reflex decision is load-bearing for kitten behavior. Author a scenario (mirror `items_eat_from_own_inventory.rs` from 429) preset with a Stage 1 kitten holding food + hunger 0.15 (begging threshold), and verify both the GOAP path AND the autonomic-only fallback produce equivalent end state. If GOAP planning is reliable for kittens, retire the reflex; if fragile in early stages, keep the reflex gated on `Has<Kitten>`.

## Verification

- `just check` — substrate-stub / marker-snapshot / method-registry lints all pass.
- `just test` — scenarios `items_eat_from_own_inventory` (passes pre-fix and post-fix; behavior identical), plus a new `items_eat_from_own_inventory_via_goap` that asserts the GOAP plan forms and resolves correctly when `HasStoredFood` is unset but `HasFoodInInventory` is set.
- `just soak-trace 42 Simba` + `just verdict <run-dir>` — `EatFromOwnInventory` fires ≥ 1× (re-enroll the canary if so); no drift on survival/continuity. If the autonomic reflex retires entirely, kitten-autoconsume verification rides on the kittenhood_stages scenarios.
- `just frame-diff` paired-archive at this ticket's parent vs HEAD — `EatDse` doesn't drift meaningfully.
- Balance doc at `docs/balance/483-goap-eat-from-own-inventory.md` if soak drift > ±10% on Eat-per-cat mean score; otherwise close as substrate-neutral.

## Log

- 2026-05-27: opened as a 429 follow-on. 429 codified the Sink contract; this ticket lifts the path into GOAP-aware planning + introduces the first OR-shaped DSE eligibility primitive.
