---
id: 452
title: Kitten spawn uses spawn_cat_from_blueprint to match founder-cat component shape
status: ready
cluster: ai-substrate
initiative: [parenting-substrate]
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

The production kitten spawn in `src/systems/pregnancy.rs:107-165` is a hand-rolled component bundle that drifts from the canonical founder-cat bundle in `src/plugins/setup.rs::spawn_cat_from_blueprint`. Ticket [451] surfaced one element of the drift (`PendingUrgencies` missing — load-bearing for `resolve_goap_plans`'s cats query AND `check_anxiety_interrupts`, caused the post-Phase-A dispatch dead-end: kittens installed Begging plans that never advanced past `Idle`, starving with their L3 winners pinned in `last_scores`). 451 patched `PendingUrgencies` + `PrevSafetyDeficit` narrowly to unblock dispatch. Several other founder-cat components are STILL missing from the production kitten spawn: `Fulfillment` (Optional in queries today, but consumers like `proxies_for_plan` and the §3.5 modifier pipeline assume present-or-absent semantics) and the entire ticket-258 belief substrate (`CatBeliefs` / `LocationBeliefs` / `PredatorBeliefs` / `ContextBeliefs` / `ColonyReservesBelief`). The 258 belief integrator populates these lazily on first `WitnessableEvent`, so kittens function — but they cold-start with zero belief state and pay a per-witness lazy-insert tax for their first observation of every subject category, which differs from adults' "seeded empty at spawn" trajectory. The structural risk is recurrence: any future founder-cat component (a new C3 substrate slice, a new Component for ticket-X) added to `spawn_cat_from_blueprint` will silently NOT land on production kittens unless the author also remembers to touch `pregnancy.rs`. The cleanest fix is to route production kitten spawn THROUGH `spawn_cat_from_blueprint` so the bundle has exactly one author and divergence can't recur.

## Scope

1. **Extract or reuse `spawn_cat_from_blueprint`.** Build a `CatBlueprint` for the newborn kitten (name, gender, orientation, personality, appearance, born_tick, zodiac_sign, magic_affinity, skills) inside the births loop in `pregnancy.rs`, then call `spawn_cat_from_blueprint(world, blueprint, position, needs, fulfillment)`. Post-spawn `entity_mut().insert(...)` for the kitten-specific components that aren't in the founder bundle (`KittenDependency`, `BornInSim`).
2. **Decide newborn defaults for the 258 belief substrate.** `CatBeliefs` / `LocationBeliefs` / `PredatorBeliefs` / `ContextBeliefs` / `ColonyReservesBelief` all spawn empty for founder cats. For newborns: empty is the right structural answer (a kitten hasn't witnessed anything yet), but verify that `belief_integrator`'s first-witness path treats empty-from-spawn the same as empty-from-lazy-insert. If they diverge (e.g. the lazy-insert path stamps a "first encounter" flag that a spawn-empty kitten doesn't get), reconcile.
3. **Newborn `Fulfillment` default.** Founder cats spawn with a per-blueprint fulfillment value (set at world-gen). Newborns get a Default — possibly the same `Fulfillment::default()` adults use, but confirm against the §3.5 modifier pipeline (especially anything that reads `fulfillment` as a multiplier on cost / lift).
4. **`tick_pregnancy` test coverage.** Add a test that exercises `process_pregnancy_births` (or its extracted helper) and asserts the spawned kitten entity contains the SAME set of Components as a founder cat — using a structural comparison rather than a hand-maintained list (which itself becomes drift bait). Possibly via `world.entity(kitten).archetype()` against a reference founder cat.
5. **Audit non-pregnancy kitten-spawn sites.** `src/scenarios/env.rs::spawn_kitten` already calls `spawn_cat_from_blueprint` and is fine. `src/systems/death.rs` and `src/systems/growth.rs` test-helper spawns are test-only — out of scope.

## Out of scope

- **Production adoption spawn paths.** Tickets 403 (BondFormed parental adoption) and 404 (Adopted lifecycle) will introduce new "kitten gains a parent" paths that don't go through pregnancy. Their kitten-spawn routing is THEIR scope; the shared helper this ticket extracts will simply be the canonical call site.
- **Wiring kittens into the 258 belief integrator's WitnessableEvent emit pipeline.** That's the production activation of belief participation, distinct from the spawn-time component shape this ticket fixes. Open as a separate ticket if/when behavior depends on it.
- **Save-load forward-migration.** Pre-451 / pre-452 saves have kittens lacking `PendingUrgencies` etc. The lazy-insert paths in `check_anxiety_interrupts` / `belief_integrator` cover the load-side; no migration step needed for THIS spawn-shape fix.

## Current state

Opened 2026-05-23 alongside [451]'s landing per CLAUDE.md "Antipattern migration follow-ups are non-optional". 451's narrow fix (`PendingUrgencies` + `PrevSafetyDeficit` inline in `pregnancy.rs`) unblocks Begging dispatch but leaves the broader pregnancy.rs-vs-setup.rs spawn drift in place. Ready (no blockers).

## Approach

Layer-walk: spawn site (`pregnancy.rs` births loop) is the only producer; consumers are the existing `resolve_goap_plans` / `check_anxiety_interrupts` / `belief_integrator` queries that already match adults. Pattern-class match to memory `feedback_substrate_over_filtering_kittens_are_cats` and the 451 dispatch-dead-end precedent. Structural-option menu:
- **split** — N/A (not about Disposition/DSE/Marker shape).
- **extend** — N/A.
- **rebind** — could rebind by making founder spawn use a shared kitten helper that accepts adult-vs-newborn overrides; cleaner separation but adds a layer.
- **retire** — N/A (the founder spawn is canonical, kitten spawn is the duplicate).
- ✅ **route through canonical** — call `spawn_cat_from_blueprint` from `pregnancy.rs`, eliminating the duplicate bundle authoritatively.

Implementation order: (a) build `CatBlueprint` inside the births loop, (b) route through `spawn_cat_from_blueprint`, (c) post-insert KittenDependency + BornInSim, (d) test asserting archetype match against a founder cat.

## Verification

- Unit test in `src/systems/pregnancy.rs::tests` (or alongside) asserting the spawned kitten's archetype matches a founder cat's archetype after stripping known-kitten-only markers (`KittenDependency`, `BornInSim`).
- `just check` / `just test` green.
- `just scenario kittenhood_stages` still passes with `KittenBegged` firing.
- Seed-42 soak (`just soak-trace 42 Simba` + `just verdict`) — Starvation == 0, KittenBegged ≥ 1, no continuity-canary regression vs the post-451 baseline (`logs/tuned-42-ccb698db` taken at 451's landing).

## Log

- 2026-05-23: opened as follow-on to [451]'s landing. 451 patched `PendingUrgencies` + `PrevSafetyDeficit` narrowly inline in `pregnancy.rs`; this ticket carries the structural fix (route through `spawn_cat_from_blueprint`) so future founder-bundle additions land on production kittens automatically.
