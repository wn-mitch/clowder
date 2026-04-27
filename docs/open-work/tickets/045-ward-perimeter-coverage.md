---
id: 045
title: Ward perimeter is a fiction — 1-day decay × reactive-only placement leaves 0-2 wards alive across the entire map
status: ready
cluster: null
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [magic.md]
related-balance: [healthy-colony.md]
landed-at: null
landed-on: null
---

## Why

A 1-hour collapse-probe soak (`logs/collapse-probe-42-fix-043-044/`, post-043+044, 17 in-game years) lost five cats in a single 1,200-tick wildlife-combat cluster (year 15). Drilling in: the proximate cause was a ShadowFox patrolling into the colony region. The deeper cause is that the colony had **2 active wards in the entire run at the moment of the cluster, both co-located at one tile**.

The user has confirmed: this matches what they see in the windowed UI soak as well — the ward placement looks reactive and tightly clustered around the colony altar, with no perimeter coverage along approach corridors. So this isn't a headless-only artifact.

## Root cause

Three compounding factors:

**(1) Ward lifetime is fixed at ~1 in-game day.**
- `set_ward_duration` is the *placement-action* duration (8 ticks), not the ward's lifetime.
- Actual ward lifetime is determined by `decay_rate`. `thornward_decay_rate = RatePerDay::new(1.0)` (1.0 strength/day → ~1 day life at strength=1.0). `DurableWard.decay_rate` defaults to the same shape.
- Empirically: 40 wards in the post-fix run despawned with mean lifetime **1,000 ticks (1.00 days), stdev ~3 ticks** — extremely tight. Every ward despawned with `sieged: false` (decay, not combat).

**(2) Placement is reactive-to-corruption, never proactive perimeter.**
- `HerbcraftWardDse`'s top consideration is `territory_max_corruption` via Logistic(8, 0.1) — the priestess scores high *only* when corruption is near her.
- Corruption near the priestess means corruption near the colony center, which means wards get placed near the colony center.
- 40 placements distributed across 13 unique locations: **11 within Manhattan-3 of colony centroid [~31, ~20]**, the remaining two at distant corruption tiles (one-shot reactive responses).
- There's no "patrol the perimeter" or "cover known approach corridors" consideration anywhere.

**(3) Single-priestess throughput.**
- Calcifer (priestess role) is the only consistent ward-placer in this run. Mallow assists occasionally.
- With one priestess + ~3-day cadence on her ward placements + 1-day ward lifetime, the colony's *steady-state* ward count is ~0.3.
- Even with the duplicate-placement bug (every ward spawns twice 9 ticks apart at the same tile, doubling apparent strength) the effective coverage is ~2 wards alive at one tile at any time.

## Mechanism in the cluster

- Tick 1,210,000–1,215,000: priestess maintains 1-2 wards at [34, 19] / [29, 22] / [29, 23] (colony altar zone).
- Tick 1,215,000: ShadowFox moves to (39, 9) — Manhattan-15 from the colony, no wards within range.
- Tick 1,215,500: SF at (34, 14) — **Manhattan-6 from colony center, fully inside cat territory, no wards on its path**.
- The `shadow_foxes_avoided_ward_total: 2,172` footer field shows wards *do* deflect SFs when present — but the SF only encountered wards 2,172 times total because the wards are concentrated at one spot. Most SF paths bypass them entirely.
- SF reaches Ivy → cluster begins → 5 cats dead in ~1,200 ticks.

## Empirical anchors

| Metric | This run | Healthy bands (15-min) | Per-year extrapolation |
|---|---|---|---|
| `wards_placed_total` | 40 | 141 ± 66 | ~36/yr expected, got 2.4/yr |
| `wards_despawned_total` | 40 | 141 ± 66 | same |
| `ward_count_final` | 0 | 0.1 ± 0.4 | matches |
| `ward_siege_started_total` | 310 | 703 ± 416 | sieges actually fired (Bug A fix worked) |
| Active wards at peak (cluster moment) | **2 (both at one tile)** | not measured | the load-bearing miss |

## Fix candidates (not yet decided)

**(A) Cheap experiment — relax ward decay.** Change `thornward_decay_rate` from `1.0/day` to `0.25/day` (4-day lifetime). Same priestess cadence now sustains ~10× more standing wards. Single-line constant change. Tests the "coverage is the issue, not the strategy" hypothesis directly. Risk: makes wards over-permanent and uninteresting; balance work to find the right value.

**(B) Teach the priestess approach-path placement.** Add a consideration to `HerbcraftWardDse` (or a new `HerbcraftWardPerimeter` DSE) that scores tiles based on "is this between known SF spawn zones and the colony centroid?". Bigger design surgery. Probably needs a `KnownThreatVectors` resource or similar. Higher leverage long-term but not a one-line fix.

**(C) Increase priestess throughput.** More priestess-class cats, or shorter ward placement duration. Doesn't fix the strategy; just amortizes the reactive-placement weakness. Not recommended on its own.

The right next step is probably (A) as a probe, then decide whether to invest in (B) based on whether (A)'s coverage uplift translates into colony survival.

## Verification

- **Pre-fix anchor:** `logs/collapse-probe-42-fix-043-044/` — 5 wildlife-combat deaths in cluster, 2 wards alive at cluster moment, mean ward lifetime 1.00 days.
- **Acceptance for (A):** Re-run 1-hour collapse probe with `thornward_decay_rate = 0.25/day`. Predict: standing-ward count rises to ~5-10 at any time. Acceptance: cluster either disappears OR shifts to a different cause (starvation, separate predator). Specifically: cluster of 4+ wildlife/ambush deaths within ~1,500 ticks should not occur.
- **Acceptance for (B):** Wards distribute across map (not concentrated at colony center). `shadow_foxes_avoided_ward_total` rises proportional to placement count. SFs Manhattan-distance-to-colony histogram shifts farther out (foxes deflected before reaching cats).
- **UI verification:** Run a windowed soak with the same seed. Ward placement pattern visible in-game should show non-trivial perimeter coverage, not just altar-clustering.

## Out of scope

- Combat-advantage math (ticket 046).
- CriticalHealth interrupt treadmill (ticket 047).
- Ward duplication bug (every WardPlaced fires twice 9 ticks apart at same location). Helpful to coverage but worth a separate ticket — a `set_ward` step or spawn-site is double-firing.
- Herb gathering subsystem (silent — `GatherHerbCompleted = 0` in 17 years). DurableWards don't require herbs so coverage is independent of herb supply, but Thornwards (2/40 in this run) do, and the herb pipeline being entirely silent suggests separate plumbing breakage.

## Log

- 2026-04-27: Ticket opened during post-043+044 collapse-probe analysis. User confirmed ward placement pattern matches what's visible in the UI soak — this is a real, observable design issue, not a headless quirk.
