---
id: 444
title: Smoking-chain expected-positive features never fire under seed-42
status: done
cluster: items-crafting
orchestration: swarm-safe
initiative: []
added: 2026-05-21
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: pending
landed-on: 2026-05-21
---

## Why

`MeatLoadedOnSmokingRack`, `SmokingRackTended`, and `MeatSmoked` are classified `Positive` and enrolled in the `expected_to_fire_per_soak()` canary at `src/resources/system_activation.rs:871-874` (the bare `_ => true` default sweeps them in). Under seed-42 deep-soak they never fire — observed at `logs/tuned-42-40397a72/` and also at the earlier `logs/tuned-42-53a6bd27/` (the 323 backfill commit, before 340 and 290), so this is a 443-era wiring gap, not regression from 290. The footer's hard survival gate `never_fired_expected_positives == 0` fails on every soak.

## Current architecture (layer-walk audit)

Quick layer-walk; rows are `[suspect]` until promoted via a fresh query in the next session.

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Resolver | `src/steps/disposition/retrieve_smokeable_from_stores.rs` | 443 added the retrieve step. | `[verified-correct]` (file present on main at 40397a72) |
| Feature emission | (wherever MeatSmoked is recorded) | Resolvers must call `record_if_witnessed(Feature::MeatSmoked, …)` on completion. | `[suspect]` — confirm the resolver actually emits |
| Plan template | `src/ai/planner/actions.rs` (touched by 443) | 443 added planner templates referencing the new step; need to confirm a complete chain (organ harvest → load rack → tend → smoke meat) exists and reaches GoapPlan emission. | `[suspect]` |
| L2 DSE | `src/ai/dses/smoke_meat.rs` (touched by 443) | DSE registered + scored. | `[suspect]` — does any cat in seed-42 ever pick `SmokeMeat`? |
| Upstream item availability | `RawOrgan` from 367-Commit-6 hunt-drop | RawOrgan / SmokeableMeat must exist on the ground when a cat is ready to load the rack. | `[suspect]` |
| Canary classification | `src/resources/system_activation.rs:1399-1403` | `_ => true` default enrolls every Positive in the per-soak canary. New features without explicit overrides are silently load-bearing. | `[verified-correct]` |

## Fix candidates

**Parameter-level**:
- R1 (**threshold**) — lower whatever IAUS/DSE-score gate is suppressing `SmokeMeat` from winning in any cat's L3 selection.
- R2 (**eligibility**) — review the DSE's eligibility filter; maybe a required marker is never set.

**Structural** (at least one mandated):
- R3 (**retire** the canary enrollment for now) — flip the three features to `Feature::expected_to_fire_per_soak() => false` in `system_activation.rs` until upstream chain lands. Cheapest, defers verification. Honest about the system's current readiness.
- R4 (**extend** the chain) — add the missing plan templates (organ-from-hunt → load-rack → tend-rack → cure → produce SmokeableMeat) so seed-42 actually produces smoked meat under healthy colony conditions. Heavier; aligns with 367's broader food-preservation epic.
- R5 (**rebind** to a synthetic seed scenario) — leave the per-soak canary as-is but add a `just scenario smoking_chain_complete` that exercises the chain deterministically. Avoids long soaks while still gating regressions.

## Recommended direction

Not yet decided. R3 is the smallest fix and the most honest about state; R4 is the right end-state but probably blocks on more of 367. Layer-walk should promote `[suspect]` rows before picking.

## Out of scope

- Wiring the entire 367 cooking/preservation epic. That's its own arc.
- mythic-texture continuity canary (sibling ticket — same soak run, different gate).

## Verification

- `just check && just test` clean (no regressions in unit suite).
- `just soak 42` writes `logs/tuned-42-<sha>/`; footer `never_fired_expected_positives` no longer contains `MeatLoadedOnSmokingRack`, `SmokingRackTended`, or `MeatSmoked`.
- `just verdict` survival canary flips from `fail` to `pass` on that count (other failures may persist — see sibling 445).

## Log
- 2026-05-21: opened as 443 follow-on. Surfaced by the 290 landing soak at `logs/tuned-42-40397a72/`, but verified pre-existing at `logs/tuned-42-53a6bd27/` (same three never-fired positives at the 323 backfill commit, before 340 and 290).
- 2026-05-21: Retire MeatLoadedOnSmokingRack / SmokingRackTended / MeatSmoked from expected_to_fire_per_soak() canary. 446 layer-walk verified the chain is structurally complete (DSE + dispatcher + registry + plan template + resolvers + marker writers all symmetric to drying) but the meat-AND-fuel conjunction inside HasSmokeableAccessible never resolves under healthy seed-42 — SmokeMeat never appears in any CatSnapshot last_scores. Regression coverage migrates to ticket 447's scenario preset.
