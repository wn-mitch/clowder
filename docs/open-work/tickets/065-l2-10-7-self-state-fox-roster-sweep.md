---
id: 065
title: §L2.10.7 SpatialConsideration roster — cat self-state DSEs + fox dispositions
status: in-progress
cluster: null
added: 2026-04-28
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 052 closed scope items 1, 3, and the cat-target-taking half of
scope item 2 — every `TargetTakingDse` under `src/ai/dses/*_target.rs`
now resolves its distance axis through `SpatialConsideration` instead
of a hand-rolled scalar. The §L2.10.7 audit at lines 5599–5656 of
`docs/systems/ai-substrate-refactor.md` covers two more rosters that
remain unported, both of which are structurally distinct from
target-taking DSEs and need their own substrate plumbing decision.

Per §L2.10.7 line 5606's audit finding: "No cat or fox DSE currently
uses continuous distance-to-landmark scoring. All 13/21 cat DSEs with
spatial inputs and 6/9 fox dispositions with spatial inputs use binary
range gates or aggregate-proximity scalars (`unexplored_nearby`,
`tile_corruption`, `nearby_corruption_level`, `local_prey_belief`)."
The 9 cat target-taking DSEs landed in 052 took out the 9 most-visible
spatial-decision rows; this ticket addresses the rest.

## Scope

### Cat self-state DSEs (12 rows)

Per §L2.10.7 line 5621+. Self-state DSEs run through the regular
`evaluate_dse` path — no per-candidate iteration, no `target_position`
in `EvalCtx`. Each row needs a `LandmarkSource` choice that resolves
to either a `Tile` (fixed colony landmark like a kitchen tile or den
floor) or an `Entity` reference (followed via `EvalCtx::entity_position`).

| DSE | Today's signal | Spec landmark | Spec curve |
|---|---|---|---|
| Eat | `binary food_available` | Stores / Kitchen building | `Logistic` |
| Sleep | `N/A` | Own Den / sleeping spot | `Power` |
| Forage | `binary can_forage` | Nearest forageable tile cluster | `Logistic` |
| Explore | `aggregate unexplored_nearby` | Unexplored frontier | `Linear` |
| Flee | `binary has_threat_nearby` | Threat position (inverted) | `Power` |
| Patrol | `needs.safety` | Territory perimeter | `Linear` |
| Build (self-state) | `binary has_construction_site` | Site position | `Logistic` |
| Farm | `binary has_garden` | Garden tile | `Logistic` |
| Herbcraft | `binary has_herbs_nearby` / `has_remedy_herbs` / `thornbriar_available` | Herb patch / ward placement tile | `Logistic` |
| PracticeMagic | `aggregate tile_corruption` / `nearby_corruption_level` | Corrupted tile cluster | `Power` |
| Coordinate | `N/A` | Coordinator's perch / meeting tile | `Logistic` |
| Cook | `binary has_functional_kitchen` + `has_raw_food_in_stores` | Kitchen building | `Logistic` |

### Fox dispositions (9 rows)

Per §L2.10.7 line 5648+. Same self-state shape — fox dispositions
score per-tick via `src/ai/fox_scoring.rs`, no candidate iteration.

| Disposition | Today's signal | Spec landmark | Spec curve |
|---|---|---|---|
| Hunting | `binary prey_nearby` + `local_prey_belief` scalar | Prey-belief cluster centroid | `Quadratic` |
| Feeding | `binary has_cubs` + `cubs_hungry` | Den position | `Power` |
| Patrolling | `N/A` | Territory perimeter | `Linear` |
| Raiding | `binary store_visible` + `store_guarded` | Colony store | `Logistic` |
| DenDefense | `binary cat_threatening_den` + `has_cubs` | Den position | `Power` |
| Resting | `binary has_den` | Den position | `Power` |
| Dispersing | `N/A` (lifecycle) | Map edge / nearest unclaimed territory | `Linear` |
| Fleeing | `needs.health_fraction` / `cats_nearby` count | Nearest map edge | `Power` |
| Avoiding | `cats_nearby` count | Cat cluster centroid (inverted) | `Power` |

### Substrate work the roster sweep will need

1. **`LandmarkSource::Entity` resolution paths.** The substrate exposes
   `EvalCtx::entity_position` already (added in 052's substrate
   landing), but no production caller routes a non-target Entity
   landmark through it yet. Den / Kitchen / Garden / Coordinator-perch
   landmarks need a per-cat resolution path (which Den, which
   Kitchen?). Two approaches: register a `LandmarkSource::Entity`
   per-cat by indexing `KittenDependency.mother → den` or
   equivalent (cheap, denormalized) vs. precomputing a per-tick
   `LandmarkRegistry` resource (cleaner, one-pass). Decision deferred
   to first port.

2. **Aggregate landmarks.** Rows like Explore (unexplored frontier),
   PracticeMagic (corrupted tile cluster), fox Hunting (prey-belief
   cluster centroid) target a *region*, not a single tile or entity.
   Two viable substrate moves: (a) a `LandmarkSource::Centroid(...)`
   variant that accepts a closure / lookup; (b) a per-tick centroid
   precompute exposed via `Tile(...)` resolution. (b) is simpler and
   closer to the existing influence-map cadence.

3. **Inverted-distance rows.** Flee, Avoiding, fox Fleeing target
   the inverted distance from a *threat* — farther is better. The
   §L2.10.7 substrate already supports this via direct curve
   evaluation on `cost = dist/range` (no `Composite{..., Invert}`
   needed); the curve's monotonic-rising shape encodes "more
   distance = higher score." Trivially implementable; just a curve
   choice per row.

## Out of scope

- **The 5 declared "not spatial" rows** (Groom-self, Wander, Idle,
  Cook minimal, Coordinate minimal) per §L2.10.7 line 5658+. These
  rows' rationale already commits "N/A — not spatial"; they need
  no port and can be confirmed in the spec without code changes.

- **Numeric balance tuning of curve midpoints / steepnesses.** Same
  as 052: shape is committed by the spec, parameters are balance
  work that follows the four-artifact methodology if any drift
  exceeds ±10%.

- **Per-DSE landmark-source semantics review.** §L2.10.7's roster
  commits *what* landmark each row uses (e.g., "Kitchen building"
  for Cook). The mapping from spec language to a concrete
  `LandmarkSource::{Tile, Entity}` is part of this ticket; revising
  the spec's landmark choice is not.

## Current state

052 just landed (commits 11f57d9 / 1e5efe7 / dbcb283 / 40a55b5 /
6322c9c / acccdc7) covering the 9 cat target-taking DSEs. The
substrate (`SpatialConsideration` + `LandmarkSource::{TargetPosition,
Tile, Entity}` + `EvalCtx::entity_position` lookup) is in place; the
Hunt port verified `LandmarkSource::TargetPosition` empirically; the
Mate / Mentor / ApplyRemedy / Socialize / GroomOther / Caretake /
Fight / Build ports verified the Quadratic / Logistic / Linear curve
families on the substrate (point-symmetric Logistic via
`Composite{..., Invert}`; non-symmetric Quadratic via the explicit
`(divisor=-1, shift=1)` inversion idiom; Linear via direct
`(slope=-1, intercept=1)`).

`LandmarkSource::Entity` is structurally present but has zero
production callers — this ticket will be its first.

## Approach

Same bisectable per-port discipline as 052: substrate question (if
any) first, then one DSE port per commit, then a paired-baseline
soak between the parent and the port. The cumulative drift across
052's 9 ports landed at ~zero on every characteristic metric (LSB
churn cancellations), so one expectation is the same to hold here.

Suggested ordering: simplest landmark-source first (cat Eat → Tile
of nearest food store, since the colony food map already exists),
then a fox row to validate the substrate parallel for fox-scoring
(probably fox Resting → Den entity), then sweep.

## Verification

- `just check` clean per commit.
- Per-port `cargo test -p clowder --lib <dse_name>` green.
- Per-port paired-baseline soak (seed 42, 15 min) at parent vs.
  WIP — survival canaries identical, continuity tallies within
  per-port noise envelope established by 052 (~±5 on
  grooming/play, ~±20 on plan-failure total dominated by
  `TravelTo(SocialTarget) no reachable` LSB churn).
- Cumulative drift across the entire ticket: no characteristic
  metric exceeds ±10% versus the baseline at the start of this
  ticket (post-052, commit acccdc7).
- Focal-cat trace (`just soak-trace 42 Simba`): inspect L2 records
  for ported self-state DSEs to confirm `SpatialConsideration`
  records emit with correct `landmark_label` ("tile" or "entity"
  depending on the row).

## Log

- 2026-04-28: opened from 052 closeout. Successor work owns the
  remaining §L2.10.7 roster — 12 cat self-state DSEs + 9 fox
  dispositions. Substrate is ready (052 landed
  `SpatialConsideration::Entity` plumbing alongside
  `TargetPosition`); this ticket is purely a per-row port sweep
  with one substrate decision (aggregate-centroid landmarks)
  surfacing on Explore / PracticeMagic / fox Hunting.
- 2026-04-28: in-progress. Investigation surfaced two more
  substrate gaps not visible in the audit: (1) cat-side
  `entity_position` closure was stubbed `|_| None` at
  `src/ai/scoring.rs:549` — `LandmarkSource::Entity` had zero
  production callers on either side, not just fox. (2) Self-state
  DSEs build their `Consideration` list at registration time, so
  per-cat dynamic landmarks (nearest kitchen, own den, frontier
  centroid) can't ride `LandmarkSource::Entity(Entity)` cleanly —
  Entity refs aren't known at registry-population time. Resolved
  by landing a `LandmarkSource::Anchor(LandmarkAnchor)` variant
  + `EvalCtx::anchor_position` closure (the substrate's deferred
  "cat-relative anchor" enumeration per
  `considerations.rs:111`'s comment). 19-variant `LandmarkAnchor`
  enum covers all 25 spec-row landmarks. Closures stub to
  `|_| None` until A2/A3 wires real resolution. Soaks deferred
  to ticket close per user direction; per-port verification is
  `just check` + targeted `cargo test` only.
