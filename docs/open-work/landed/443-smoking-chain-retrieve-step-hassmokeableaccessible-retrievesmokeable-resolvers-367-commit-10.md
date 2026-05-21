---
id: 443
title: Smoking chain retrieve step — HasSmokeableAccessible + RetrieveSmokeable resolvers (367 Commit 10)
status: done
cluster: items-crafting
initiative: [world-richness]
added: 2026-05-21
parked: null
blocked-by: []
supersedes: []
related-systems: [crafting.md]
related-balance: []
landed-at: 892eef9bd0db
landed-on: 2026-05-21
orchestration: substrate-sensitive
---

## Why

Ticket 367 landed Commits 1–9 (drying + smoking rack substrate, dispatch wiring via 436–439), but the smoking chain's retrieve-from-stores step was explicitly deferred to "Commit 10." Three Features remain in `never_fired_expected_positives` as a result: `MeatLoadedOnSmokingRack`, `SmokingRackTended`, `MeatSmoked`. The root cause mirrors the pre–Commit-9 drying defect: `SmokeMeatDse` requires `HasSmokeableInInventory`, but cats deposit raw meat at Stores on hunt-return, making the per-cat inventory marker false at scoring time. The DSE silently filters every cat. Closes the last hard gate preventing `just verdict` from passing on the 367 cascade.

## Scope

- `HasSmokeableInStores` colony marker — fires when Stores hold ≥1 smokeable-meat item AND ≥1 fuel item.
- `HasSmokeableAccessible` per-cat composite — fires when cat has both in inventory, OR has a free slot AND the colony marker is true.
- `GoapActionKind::RetrieveSmokeableMeat` + `::RetrieveSmokeableFuel` — two new plan-step variants.
- `resolve_retrieve_smokeable_meat` + `resolve_retrieve_smokeable_fuel` resolvers.
- `smoking_meat_actions()` upgraded from single-step to multi-step: `[DropItem, RetrieveSmokeableMeat, RetrieveSmokeableFuel, SmokeMeat]`.
- `SmokeMeatDse` eligibility updated from `HasSmokeableInInventory` → `HasSmokeableAccessible`.
- Three scenario fixtures for regression coverage.

## Out of scope

- `TendSmokingRackDse` retrieve logic — tend fires when a rack is already loaded; it doesn't need a retrieve step.
- 434 (sprite variants), 435 (RecipeInput::AnyOf), 440 (EligibilityFilter migration) — independent follow-ons.

## Current state

Substrate landed in 367 Commits 1–9 + 436–439. Smoking rack constructs, DSE is dispatched (438), zone resolver works (439). The retrieve-from-stores layer is the only missing link.

## Approach

Two-ingredient retrieve requires two separate `GoapActionKind` variants (meat + fuel), each with a resolver that no-ops if the cat already carries the target kind. Mirrors the drying `RetrieveDryable` pattern but split to handle each ingredient independently. See plan at `~/.claude/plans/time-to-tackle-367-streamed-moonbeam.md` for step-by-step detail.

## Verification

1. `just check` green (substrate-stub + marker-snapshot-wiring CI scripts)
2. `just test` green — 3 new scenario fixtures pass
3. `just scenario smoking_chain_stores_has_smokeable` — `smoke_meat` surfaces in L2 table
4. `just soak-trace 42 Simba` → `just verdict` — `never_fired_expected_positives` empty; starvation canary holds

## Log

- 2026-05-21: opened as 367 Commit 10 follow-on — deferred smoking retrieve step. Session in progress.
