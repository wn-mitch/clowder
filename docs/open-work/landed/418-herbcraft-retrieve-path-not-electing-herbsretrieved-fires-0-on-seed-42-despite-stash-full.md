---
id: 418
title: Herbcraft retrieve-path not electing — HerbsRetrieved fires 0 on seed-42 despite stash full
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-19
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 37422733937a
landed-on: 2026-05-19
---

## Why

Ticket 084 Commit 3 landed the herb-stash economy: `Feature::HerbsDeposited` fires 108× on seed-42 (gather→deposit pipeline live), `HasStoredThornbriar` and `ColonyThornbriarChronicallyLow` markers author correctly. But the symmetric retrieve→weave branch is silent: `Feature::HerbsRetrieved` = 0 in the verification soak (`logs/tuned-42/`, commit `fe8e1f77`). `HerbcraftSetWard`'s retrieve-path GOAP branch — `[Travel(Stores) → RetrieveHerbs(Thornbriar) → Travel(WardSite) → SetWard]` — is structurally reachable (eligibility `CanWardFromSupply::KEY` fires; planner action defs present at `actions.rs:684-714`) but A* / L3 selection never composes it.

The 2 `WardPlaced` events that did fire (down from baseline 5) presumably came via the carry-direct branch — i.e., a cat who happens to be carrying thornbriar when `Action::HerbcraftSetWard` is picked. The retrieve substrate is in place but isn't being elected.

## Scope

Layer-walk per CLAUDE.md bugfix discipline to localize:

1. **L3 selection.** Does `Action::HerbcraftSetWard` win the softmax frequently enough? Compare action-share against pre-084 baseline. If `HerbcraftSetWard` is rarely picked, the upstream issue is scoring, not branching.
2. **Plan-cost asymmetry.** Within `HerbcraftSetWard`, is A* always picking the carry-direct chain over the retrieve chain? Log via focal trace. Likely culprit: the retrieve chain's total cost (`Travel(Stores) + 2 + Travel(WardSite) + 3`) vs carry-direct (`Travel(HerbPatch) + 3 + Travel(WardSite) + 3`) — depends on zone distances at the cat's position.
3. **`CanWardFromSupply` writer cadence.** Verify the marker actually fires for cats without `HasWardHerbs` when colony stash is non-empty. Race condition possible if `update_capability_markers` reads stale colony-marker state from a prior tick.

## Out of scope

- Adjusting `HerbcraftGather` to NOT terminate at Stores (would unwind 084 Commit 2's structural change).
- Re-promoting `Feature::HerbsRetrieved` to `expected_to_fire_per_soak() => true` — this ticket's land flips that switch.

## Approach

- Run `just q trace logs/tuned-42 --cat Simba` for the focal cat's per-tick decision landscape.
- Run `just q events logs/tuned-42 --kind GoapPlanCreated --action HerbcraftSetWard` (or grep) to count how often the disposition was elected and whether the retrieve branch appeared in any plan.
- If L3 selection is the issue: layer-walk `score_dse_by_id("herbcraft_ward", ...)` and check whether the new `CanWardFromSupply` eligibility filter passes consistently.
- If plan-cost asymmetry is the issue: try reducing `RetrieveHerbs` cost from 2 → 1, or audit travel-zone-distance precomputation.

## Verification

`Feature::HerbsRetrieved ≥ 1` on a soak with the fix. Promote to `=> true` once verified.

## Log

- 2026-05-19: opened as 084 follow-on. Verified zero retrievals in `logs/tuned-42/` despite 108 deposits and ~100+ thornbriar in stash by end of run.
- 2026-05-19: Landed via 084-Commit-3 follow-on (2026-05-19). Two-line MarkerSnapshot population fix in goap.rs + disposition.rs. Verified: scripts/check_marker_snapshot_wiring.sh (ticket 217) catches the regression class if removed.
