---
id: 447
title: smoking-chain scenario coverage for retired per-soak canary
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

Ticket 444 retires `MeatLoadedOnSmokingRack` / `SmokingRackTended` / `MeatSmoked` from the per-soak never-fired-positives canary (the meat-AND-fuel conjunction inside `HasSmokeableAccessible` never resolves under healthy seed-42 colony shape — see 446's verified layer-walk). Retiring the canary loses regression coverage: a future refactor that breaks the smoking resolvers' Feature emission or eligibility filter would now go undetected at soak time. A `just scenario smoking_chain_complete` preset that preloads a cat with meat + fuel + a functional smoking rack restores deterministic regression coverage in the ~3-second harness without waiting for a colony to organically produce meat + wood co-presence.

## Scope

- New scenario file `src/scenarios/smoking_chain_complete.rs` (or whatever the convention is — mirror the existing closest scenario in `src/scenarios/`). Preloads one cat (Adult, `CanSmoke` capability, no `Incapacitated`) at a SmokingRack with `RawMouse`/`RawRat`/`RawRabbit`/`RawBird` + `Wood` in inventory or adjacent Stores. Asserts the ranked L2 score table shows `SmokeMeat` winning, and that the cat advances through `RetrieveSmokeable` → `LoadSmokingRack` → `TendSmokingRack` → emits `Feature::MeatSmoked` on tend-cycle completion.
- Wire the scenario into `just scenario` discovery.
- Document the scenario in CLAUDE.md §Verification or wherever the existing scenario list lives, so future bugfixers know to reach for it when touching the smoking pipeline.

## Out of scope

- The structural fix to make the smoking chain fire under organic colony shape (split the conjunction into sequential retrievals, or add a fuel-acquisition DSE). That's a separate substrate arc with its own balance-doc hypothesis — drying-side disjunction has a reason (substitutable inputs), smoking-side conjunction has a reason (fuel is consumed). Re-pricing requires deliberate balance work.
- Re-enrolling the three smoking Features in the per-soak canary. That happens when the structural arc lands and a follow-up balance hypothesis verifies firing under organic conditions.
- Drying-side scenario coverage. The drying pipeline still fires under healthy seed-42 (3 `DryingFood` plans in `logs/tuned-42-40397a72/`); the per-soak canary continues to gate it. If drying ever silences, open a separate ticket.

## Current state

- 446 landed 2026-05-21 with the layer-walk root cause (sha 3beeb7de).
- 444 lands the canary retirement and cites this ticket.
- Existing scenarios under `src/scenarios/` (per ticket 162) provide the harness shape; no new infrastructure required.

## Approach

Pick the closest existing scenario (probably a cooking / drying preset under `src/scenarios/`) and clone its structure. Key invariants for the preset:

- One Adult cat with the marker stack `CanSmoke + !Incapacitated`.
- Inventory or adjacent Stores containing one meat item + one Wood item simultaneously (this is the precondition the organic colony never delivers).
- A SmokingRack entity at a known tile, with `HasFunctionalSmokingRack` true at preset-spawn.
- Expected output: deterministic ranked L2 table with `SmokeMeat` at the top; plan dispatch through the four resolver steps; `Feature::MeatSmoked` emitted on completion.

## Verification

- `just scenario smoking_chain_complete` runs in ~3 seconds and prints the focal cat's per-tick winning DSE; first tick shows `SmokeMeat` chosen, subsequent ticks advance through the smoking-rack step sequence.
- Footer of the scenario's events stream contains all three Features (`MeatLoadedOnSmokingRack`, `SmokingRackTended`, `MeatSmoked`) with `count >= 1`.
- A deliberate regression check: comment out one of the smoking resolvers' `record_if_witnessed` calls — the scenario assertion fails. This proves the scenario actually gates the regression class that the retired canary used to catch.

## Log
- 2026-05-21: opened as 444's antipattern-migration follow-on. 444 retires the per-soak canary for the smoking triple; this ticket restores deterministic regression coverage in the scenario harness instead.
- 2026-05-21: 2026-05-21: landed. New `just scenario smoking_chain_complete` preloads Adult cat with RawMouse + Wood adjacent to a functional idle SmokingRack; unit test pins seed=1 (probe in tests::diagnostic_probe_seeds_for_smoke_meat_election shows 39/49 seeds elect SmokeMeat at tick 0; seed 42 lands on Forage hence `expected_features: &[]`) and asserts all three Features (`MeatLoadedOnSmokingRack`, `SmokingRackTended`, `MeatSmoked`) fire within 2000 ticks. Regression sanity verified: temporarily commenting out `act.record(Feature::MeatSmoked)` at goap.rs:7891 fails the `done >= 1` assertion with the named-resolver diagnostic; restored before commit. Mirrors the drying-completes pattern (439) but asserts the full triple (smoking completes in 3 × 416 ≈ 1248 ticks vs drying's 15k → load-only).
