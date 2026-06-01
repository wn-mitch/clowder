---
id: 496
title: split radial perception from tactical reach — sensing reads back to euclidean_distance
status: done
cluster: substrate-migration
initiative: []
added: 2026-06-01
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 9b3f5d43
landed-on: 2026-06-01
---

## Why

Ticket 494 routed `Position::distance_to` from Euclidean to Chebyshev so
perception aligns with 8-direction movement cost — the right call for tactical
reach (target picking, "how many steps until I get there"). But the switch was
metric-wide and incidentally rerouted *radial perception* reads too: sight,
hearing, scent, and the threat-dampening proximity checks all flow through
`pos.distance_to(...)`. A Chebyshev radius-R ball is `(2R+1)²` tiles
(441 for R=10); a Euclidean radius-R disc is `πR² ≈ 314` tiles. So every
wildlife position now triggers `HasThreatNearby` on ~40% more cats per tick.

Surfaced as 494 post-fix follow-on row #2: `HoldUntilSafe: global step
timeout` plan-failures went 243 → 742 between the pre-494 anchor
(`logs/tuned-42-09411128-pre-494-anchor`) and the post-494 soak
(`logs/tuned-42-d6d76811`). 3.05× absolute / 2.33× per-tick. More cats
classified as threatened → more flee plans → more `HoldUntilSafe` steps →
more timeouts at the 500-tick global watchdog.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 perception | `src/systems/sensing.rs::detect` (line 257) | Reads `observer.position.distance_to(&target.position)` — Chebyshev post-494. Drives sight/hearing/scent/tremor gating for every `observer_sees_at` / `observer_smells_at` / `cat_sees_threat_at` / `prey_cat_proximity` call. | `[verified-correct]` |
| L1 perception | `src/systems/sensing.rs::prey_cat_proximity` (line 550) | Prey's awareness-of-cat scalar — `prey_pos.distance_to(&cat_pos)` — same shape as detect(). | `[verified-correct]` |
| L1 perception | `src/systems/sensing.rs::update_target_existence_markers` (line 998) | `HasUnburiedCorpse` author — `pos.distance_to(dp) <= burial_range`. Radial scent-mediated awareness of a colony-mate's death. | `[verified-correct]` |
| L2 threat-dampening | `src/systems/goap.rs:1316,1328` | `near_buildings` / `ally_count` dampening filters use `pos.distance_to(...)` for radial safety perception. | `[verified-correct]` |
| L2 allies-fighting | `src/systems/goap.rs:1909,1918` + `src/systems/disposition.rs:665,674` (fwd-compat parity) | Nearest-threat scan + co-fighter range use `pos.distance_to(...)`. Radial visual perception of threat and allies. | `[verified-correct]` |
| Resolver | `src/steps/disposition/hold_until_safe.rs` | Gates exit on `RouteCostField` cost ≤ 100 AND `safety_need` ≥ 0.6. Uses substrate truth (route_cost), not metric-mediated distance — *correctly substrate-aware, not the source of the regression*. | `[verified-correct]` |
| Resolver | `src/steps/disposition/pick_flee_target.rs` | Uses `chebyshev_distance` directly for `effective_cost = field.cost_at(c) - chebyshev_distance_to_threat`. Already substrate-correct for tactical reach. | `[verified-correct]` |
| Substrate | `src/components/physical.rs:163-180` | `Position::euclidean_distance` is the documented escape hatch for "scent diffusion gradients, ward-glow falloff, sound amplitude, visual perception." Authored in 494 for exactly this case. | `[verified-correct]` |

## Fix candidates

**Parameter-level options:**

- R1 — re-tune `wildlife_threat_range` from 10 → 7 (or similar) to recover the
  Euclidean-radius-10 area under Chebyshev. Rejected: loses substrate clarity
  (the constant's intent is "radius beyond which a cat doesn't perceive the
  threat" — a radius, not a step count). Doesn't address the broader sensing
  layer using the wrong metric.

- R2 — re-tune `route_cost_safe_threshold` from 100 upward, so the HoldUntilSafe
  exit predicate is easier to satisfy. Rejected: addresses the symptom, not
  the cause. The threshold reflects "how cheap-to-reach must a tile be to count
  as safe?" — that's substrate-correct; the upstream over-classification of
  threats is the real bug.

**Structural options:**

- R3 (**rebind**) — split radial perception from tactical reach by rebinding
  the 8 perception-layer `distance_to` reads to `euclidean_distance`. The
  escape hatch already exists; this is a per-call-site metric flip with no
  new methods or API. **Wins.**

- R4 (**split**) — introduce `Position::radial_distance` as a one-line alias
  for `euclidean_distance` so perception sites read `pos.radial_distance(&wp)`
  for self-documenting clarity. Rejected: CLAUDE.md says no abstractions
  beyond what the task requires; the doc-comment on `euclidean_distance`
  already establishes the perception use case, and adding an alias is a
  small refactor on top of a substrate fix.

- R5 (**retire**) — N/A. Nothing to retire.

## Recommended direction

**R3 — rebind 8 production sites + 4 test references.**

| File:line | Phenomenon |
|---|---|
| `src/systems/sensing.rs:257` | `detect()` unified sight/hearing/scent gate — dominant hot-path read |
| `src/systems/sensing.rs:550` | `prey_cat_proximity` — prey's awareness of a cat |
| `src/systems/sensing.rs:998` | `HasUnburiedCorpse` author — burial-sense scent radial |
| `src/systems/disposition.rs:665` | allies-fighting nearest-threat scan (fwd-compat parity) |
| `src/systems/disposition.rs:674` | allies-fighting `ally.distance_to(threat)` |
| `src/systems/goap.rs:1316` | `near_buildings` threat-dampening |
| `src/systems/goap.rs:1328` | `ally_count` threat-dampening |
| `src/systems/goap.rs:1909` | live allies-fighting nearest-threat scan |
| `src/systems/goap.rs:1918` | live allies-fighting `ally.distance_to(threat)` |

Test sites at `sensing.rs:1556,1587,1628,1677` swap their reference
`center.distance_to(&target)` to `center.euclidean_distance(&target)` to match
production. Test function names containing `manhattan_check` rename to
`radial_check` (they always actually tested the radial behavior; the legacy
name was misleading). Stale comment at `sensing.rs:992-994` saying
"Manhattan" gets updated to reflect the radial read.

## Out of scope

- Per-DSE score shifts under Chebyshev (494 follow-on #5) — substrate-correct
  shift, not regression.
- `EngagePrey: lost prey during approach` (494 follow-on #3) — prey-evasion
  math; separate substrate question.
- Welfare drop / shelter regression (494 follow-on #6) — shelter-as-belief
  (ticket 374) territory.
- `surrounded_colony::*` ring-coverage tests (494 follow-on #7) — pre-existing
  failures, not caused by metric. Same shape under Euclidean and Chebyshev.
- Tactical-reach sites with `distance_to` — `coordinating_target_range`,
  `building_search_range`, `herb_stash_accessible_for`,
  `den_discovery_range`. Their doc-comments establish them as tactical
  reach; `herb_stash_accessible_for` explicitly says "Manhattan is the right
  grain for plan-template gating."
- `Position::distance_to` itself — 494's Chebyshev default stays.

## Verification

Hard-gate / canary: `HoldUntilSafe: global step timeout` count returns toward
the pre-494 anchor's 243/soak. Acceptance band: ≤ 300 (within ~25% of pre-494
anchor).

1. `cargo build --release`
2. `cargo test --release --lib sensing` — 45 tests including the 3 renamed
   `*_matches_radial_check` reference-equivalence checks
3. `just soak-trace 42 900 --focal Simba`
4. `just verdict <run-dir>` — survival/continuity canaries
5. `/logq plan-failures <run-dir>` — confirm HoldUntilSafe timeout drop
6. Cross-check that 494 follow-ons #3 (EngagePrey lost-prey) and #6
   (welfare/shelter) remain in the regression set, ungated by this work

## Log

- 2026-06-01: opened. Cause traced via two Explore agents + a Plan agent.
  Root cause: `sensing.rs:257` and 7 sibling perception-layer reads inherited
  the 494 Chebyshev default but should always have been radial-Euclidean —
  documented as the use case for `Position::euclidean_distance` since 494.
  Fix is a rebinding at 8 production sites; no new substrate, no API change.

- 2026-06-01: landed at `9b3f5d43`. Post-fix soak `logs/tuned-42-9b3f5d43`:
  `HoldUntilSafe: global step timeout` = 94 (vs post-494 742 and pre-494
  anchor 243 — well under the ≤300 acceptance band). Per-tick rate
  0.0079 → 0.00156/tick (5× reduction). Continuity canaries pass
  (grooming 1618 · play 44 · mentoring 802 · courtship 8712).
  `shadow_foxes_avoided_ward_total` recovered 0 → 1024;
  `wards_placed_total` 0 → 10. Welfare/shelter (494 follow-on #6) and
  SearchPrey scent residual (494 follow-on #3 territory) remain
  out-of-scope per this ticket; they'll get their own follow-ons.
  cargo test --lib clean (2596 pass, 0 new failures; 2 pre-existing
  surrounded_colony ring tests still red — pre-existing per 494 row #7).
