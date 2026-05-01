---
id: 120
title: Characterize shadow-fox spawn-rate coupling to cat-presence (047 Phase 3 surfaced +93%)
status: ready
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [047-acute-health-adrenaline.md]
landed-at: null
landed-on: null
---

## Why

Surfaced during ticket 047 Phase 3 hypothesize sweep (3 seeds × 3 reps × 900s, baseline=047-inert vs treatment=047 modifier active at flee_lift=0.60, sleep_lift=0.50). Treatment crossed one metric into the `significant` band:

- `shadow_fox_spawn_total`: 17.0 → 32.8 (+92.8%, p=0.017, d=1.35)

This is **characterization work, not a regression fix.** The 047 substrate makes injured cats retreat to a den; the colony equilibrium shifts as cats live longer in injured states. The +93% spawn rate could be:

- Coupling: spawn-rate weighted by cat absence from outer regions (cats inward → more fox opportunity)
- Downstream: longer-lived colony → more game-ticks of fox-spawn opportunity per run, regardless of cat presence
- New equilibrium: a real ecological feature of the regime, not a bug

The point of this ticket is to *find out which*, so future substrate work knows what to expect. Hard survival gates (`ShadowFoxAmbush <= 10`) hold across the 047 sweep, so this is exploratory rather than blocking.

## Scope

- Audit the shadow-fox spawn system in `src/systems/wildlife.rs` (or wherever fox spawn logic lives — search for `ShadowFoxSpawn` event emission). Confirm whether spawn rate is uniform-random per region or weighted by cat presence/absence.
- If the coupling exists: decide whether it's an intended ecological feedback (foxes opportunistic; cats away → foxes move in) or an unintended noise floor.
- If unintended: cap spawn rate or add a colony-radius dampener.
- If intended: document the coupling in `docs/systems/` so future substrate work expects it.

## Verification

- Re-run the 047 hypothesize spec with whatever fix lands; expect `shadow_fox_spawn_total` delta band to drop from `significant` to `noise`.
- Survival canary `deaths_by_cause.ShadowFoxAmbush <= 10` stays satisfied.

## Out of scope

- Ward perimeter coverage gaps (ticket 045).
- Shadow-fox motivations distinct from normal foxes (ticket 023).
- Tuning the 047 modifier magnitudes — those are blocked on 114 (momentum gap) which likely also affects this coupling.

## Log

- 2026-05-01: Opened from ticket 047 Phase 3 sweep findings. The +93% shadow-fox spawn rate was the only `significant`-band metric in the cross-metric comparison; explanation hypothesis is cat-presence coupling.
