---
id: 534
title: Prey sentinel and forage division of labor — one grazing-group member holds high-cadence vigilance, foragers run cheap detection, sentinel alarm propagates group Bolt/Scatter (perf-positive: N detection passes to 1)
status: ready
cluster: wildlife
initiative: [predator-prey-dynamics]
added: 2026-07-09
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
Same-kind prey that graze together currently each run `try_detect_cat`
independently every cadence tick — N redundant detection passes for one group,
on a path that was already ~8% of the flamegraph before prey scoring existed
(`ai/prey_scoring.rs` docs). Real herd/warren/flock prey don't do this: one
individual takes the lookout role and scans while the rest forage heads-down at
reduced vigilance, and the sentinel's alarm flips the whole group at once. This
is the rare feature that is **both narratively rich and net-negative on
compute** — it replaces N full detection passes with one, so it complements the
compute work (528 skip-gate, 532 tick-budget) rather than taxing it.

## Scope
- On the existing same-kind-neighbor census (the count that already gates
  `prey_scatter_group`), elect one group member as sentinel per locality.
- Sentinel runs `try_detect_cat` at full cadence + elevated vigilance; foragers
  run detection rarely or not at all (and may graze at reduced `alert_radius`).
- Sentinel detection writes an alarm signal group members read as an immediate
  threat-inject → the group enters the 266 escape election (Bolt/Scatter)
  without each member independently detecting the cat first.
- Role rotation so no single prey is starved of foraging time.
- New knobs (sentinel-per-N ratio, alarm radius, forager cadence divisor) in
  `PreyConstants`.

## Out of scope
- Cross-species mixed sentinels (a bird warning rabbits) — single-species only.
- Predator counter-play against sentinels (targeting the lookout) — later.
- Reworking the 266 escape election itself; this only changes *who* detects and
  *how the alarm spreads*, not Bolt/Scatter resolution.

## Out-of-band note on perf
This ticket's headline benefit is a throughput *win*, so it should carry a
before/after flamegraph on `prey_ai`/`try_detect_cat` (memory
`feedback_perf_refactor_needs_flamegraph`), on a short perf soak
(`feedback_perf_soaks_short_runs`), not only a balance soak.

## Current state
Nothing landed. Census/eligibility machinery exists in `prey_ai`'s election arm
(`src/systems/prey.rs`) and `prey_scatter_group`. Alarm propagation could reuse
`prey_scent_map` or a dedicated ZST/marker inject.

## Approach
1. Sentinel election: cheapest is a deterministic per-locality pick (entity-id
   parity / lowest-id in the census set) recomputed on a slow cadence — avoid a
   new per-tick scan.
2. Gate `try_detect_cat` cadence on the sentinel flag; foragers skip or downshift.
3. Alarm: sentinel on detection records a group alert (map write or targeted
   marker) that foragers consult on their cheap tick and jump straight to Alert.
4. Watch the substrate-stub + silent-inert rules: any new marker ships
   reader+writer together; any new prey DSE ships its dispatcher branch (memory
   `project_score_actions_dispatch_antipattern`). This ticket likely adds no new
   DSE — it modulates detection cadence + injects into the existing election.

## Verification
- Focal trace: a grazing group shows one sentinel scanning, foragers idle-grazing;
  on cat approach the whole group Bolts/Scatters within an alarm-propagation
  window.
- Flamegraph: `prey_ai`/`try_detect_cat` inclusive % drops vs baseline at equal
  prey population.
- `just verdict`: survival + continuity canaries hold; `ShadowFoxAmbush <= 10`
  not regressed (prey escaping *earlier* must not perturb predator mortality
  balance the wrong way).

## Log
- 2026-07-09: Opened from `/ideate` prey-ecology pass (idea #1). Composes with
  533 (real forage) — sentinel/forager split is most legible once foraging has a
  real cost.
