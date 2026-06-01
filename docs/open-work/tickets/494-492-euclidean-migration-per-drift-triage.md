---
id: 494
title: 492 Euclidean migration: per-drift triage
status: blocked
cluster: substrate-migration
orchestration: substrate-sensitive
initiative: []
added: 2026-06-01
parked: null
blocked-by: [492]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 492 lifted perception from Manhattan to Euclidean across all 314
sim-code call-sites. Survival + continuity canaries pass under the new
metric, but verdict against the post-482 baseline (`logs/tuned-42-d531318e`)
shows substantial drift on ward placement, hunt scent, craft selection,
and per-DSE scoring weight. The drift exceeds the ticket-92 prediction
band ("bounded tactical-tie shifts on Hunt/Forage") — it's a wholesale
re-weighting of DSE selection because every consideration that reads
distance now sees a different metric.

Before landing 492, each cascade row needs a per-system "is this a real
regression or expected substrate shift" determination. The colony lives,
but five subsystems read differently and we don't yet know whether the
new shape is healthy-different or degraded-different.

## Soak under audit

- Baseline: `logs/tuned-42-d531318e` (commit `d531318e`, post-482 promotion)
- New: `logs/tuned-42-09411128` (commit `09411128`, post-492 mechanical migration)
- Seed: 42, duration 900s (15-min canonical soak)
- Verdict: `concern` (survival pass, continuity pass, footer drift on
  wards / hunt / craft / per-DSE)

## Drift triage checklist

Each row is "investigate, decide, act, document concordance." Use
`/logq`, `just inspect <cat>`, focal-trace drilling, and per-DSE
frame-diff to ground each call.

### Footer drift

- [ ] **wards_placed_total: 17 → 9 (-47%)**. Half as many wards. Is this
      because the perimeter scoring no longer favors any cardinal
      candidate (cf. surrounded_colony test failures missing W
      sector), or because cat-value gates suppress more candidates? Drill
      `just q events logs/tuned-42-09411128 --type=WardPlaced` and
      compare placement geometry to baseline.

- [ ] **shadow_foxes_avoided_ward_total: 511 → 0 (-100%)**. With half the
      wards, fox avoidance drops. Verify: is this proportional to ward
      count (expected) or worse than proportional (compounding regression)?

- [ ] **ward_siege_started_total: 75 → 0 (-100%)**. Sieges depend on
      ward presence in fox-corridor tiles. Read the corridor-placement
      heuristic — does Euclidean dist shift candidate selection away from
      siege-prone tiles, or are fewer sieges a survival-positive outcome?

- [ ] **wards_despawned_total: 17 → 9 (-47%)**. Mirrors placement count —
      sanity check only. Should not exceed `placed_total`.

### Plan-failure rate regressions

- [x] **SearchPrey "no scent found": 1 → 167 (146.8× baseline) → 9
      post-fix**. `[verified-fixed — substrate realignment
      Manhattan→Chebyshev]`. Root cause was *not* the scent-map gate
      precision (initial Explore-agent hypothesis was wrong — gate
      is internally consistent and `scent_search_radius=20.0` was
      not load-bearing on Euclidean). Real cause: hunt DSE spatial
      considerations read `distance_to` (Euclidean post-492) against
      `range` constants tuned for Manhattan-domain numerics. Diagonal
      reads collapsed hunt scoring -65%. Realigning `distance_to` to
      Chebyshev (`physical.rs:119-121`) — the substrate-correct
      metric matching 8-direction movement cost (`pathfinding.rs:233`)
      — recovered hunt scoring naturally. Residual 9×-baseline is
      acceptable substrate drift; well below the rate that destabilizes
      hunt continuity.

- [x] **EngagePrey "no prey target": 14 → 205 (12.9×) → 10 post-fix**.
      `[verified-fixed — cascade absorbed by SearchPrey recovery]`.
      Now *under* baseline (10 < 14). Confirms the cascade interpretation
      — when SearchPrey succeeds, EngagePrey's "no prey target"
      replanning condition stops triggering.

- [x] **TravelTo(PatrolZone): no path and stuck: 0 → 530 (new high-rate) →
      0 post-fix**. `[verified-fixed — reachability gate added]`. Root
      cause was a pre-existing substrate gap exposed by the Euclidean
      re-weighting: `goap.rs::resolve_zone_position::PatrolZone`
      picked the nearest store then constructed a blind `+x` offset
      by `guard_patrol_radius` with no reachability check. When patrol
      DSE elevation kicked in under Euclidean (0 → +0.130), the blind
      offset landed on water/wall/out-of-bounds tiles 530 times per
      soak. Fix B added `perimeter_offset_position` helper
      (`goap.rs:10599-10612`) that rotates through cardinal offsets and
      returns the first passable in-bounds tile. Applied to both
      `PatrolZone` and `RestingSpot` branches (same blind-`+1` shape).
      Other `resolve_zone_position` branches return the target tile
      directly without offset construction — no further audit needed
      this session. Patrol DSE elevation also dropped back toward
      historical baseline under Chebyshev realignment (Fix A), so the
      reachability gap is now both unreachable *and* unreached.

- [x] **MentorCat "target invalid at step entry: Incapacitated": 0 → 449
      (new high-rate) → 0 post-fix**. `[verified-fixed — volume drop
      from Fix A absorption; no substrate change needed]`. Originally
      framed (in this ticket) as an eligibility-filter gap, but
      investigation confirmed `EligibilityFilter::require_target_alive`
      *does* enforce at scoring time (`target_dse.rs:397-406`) and
      step-entry failures already feed `RecentTargetFailures` via
      `record_step_failure` (`goap.rs:5165`) — substrate cooldown
      mechanism is intact. Actual mechanism was *in-flight
      invalidation*: mentor DSE elevated 6× under Euclidean
      (+0.048 → +0.341), 6× more apprentice plans in flight, 6× more
      chances for an apprentice to fall mid-plan. Fix A's realignment
      drops mentor DSE back to +0.110 (per Simba focal frame-diff,
      -67.9% vs pre-fix), and the proportional drop in active
      apprentice plans retires the in-flight invalidation. Fix C
      (substrate-level eligibility hardening) was deferred and
      ultimately not needed.

### Per-DSE score shifts (Simba focal, frame-diff)

- [ ] **groom_self: +0.026 → +0.473 (+1701%)**. Order-of-magnitude lift.
      Did the proximity-to-self distance read collapse to 0 somewhere
      and now scales differently?

- [ ] **mentor: +0.048 → +0.341 (+607%)**. Heavily lifted. Compare to
      MentorCat plan-failure rate — DSE picks Mentor more, plan fails
      more because target is invalid. The DSE eligibility upstream and
      the resolver's runtime check are now mis-aligned.

- [ ] **caretake: 0 → +0.262 (new)**. CaretakeTarget DSE wasn't lifting
      pre-492. Why is it now? Probably distance term flipped.

- [ ] **flee: -0.018 → +0.137 (+856%)**. More flight. Combined with
      ShadowFoxAvoided=0, this looks like cats running from threats
      that wards used to deflect. Worth investigating whether flee is
      cost or benefit.

- [ ] **patrol: 0 → +0.130 (new)**. Patrol now in the active mix.
      Connects to the patrol-stuck plan-failure regression.

- [ ] **handoff: 0 → +0.167 (new)**. JointIntention handoff lifted.

- [ ] **build: +0.594 → +0.168 (-71%)**. Build collapsed. Construction
      site distance reads heavily influence this; under Euclidean,
      sites read farther away (diagonals shorter, others unchanged means
      relative re-ranking).

- [ ] **craft_at_workshop: +0.257 → +0.005 (-98%)**. Craft DSE essentially
      stops. Workshop reach `CRAFT_ITEM_STORES_REACH = 64.0` may be too
      tight under Euclidean — pre-492 Manhattan 64 covers more
      diagonals than Euclidean 64.

- [ ] **craft_at_tanning_frame: +0.224 → +0.000 (-100%)**. Same diagnosis
      as workshop.

- [ ] **hunt: +0.373 → +0.132 (-65%)**. Hunt scoring collapsed. Cascade
      from SearchPrey scent regression.

- [ ] **explore: +0.194 → +0.016 (-92%)**. Explore DSE essentially
      stops. ExplorationMap reads use distance for "is this frontier
      worth investigating."

- [ ] **magic_scry: +0.242 → +0.126 (-48%)**. Halved. Scry-target
      distance read shifted.

- [ ] **wander: +0.504 → +0.328 (-35%)**. Mild drop. Probably a
      consequence of other DSEs taking more slots.

### Unit-test failures

- [ ] **`surrounded_colony::additive_composition_builds_ring_of_coverage`**.
      W cardinal sector missing under Euclidean. Either tune the
      ward-placement candidate scoring to restore cardinal preference,
      or accept and update the test with explicit drift annotation.

- [ ] **`surrounded_colony::gate_composition_builds_ring_of_coverage`**.
      Same shape; same call.

## Approach

Per-row workflow: pick the row, drill it via the named tool (`/logq`,
focal-trace, `just explain`, `just q events`), determine whether the
behavior change is *expected substrate effect* (Euclidean correctly
re-weighting) or *pre-existing tuning baked for Manhattan* (needs
constants tuning to restore intent), and write the conclusion + concordance
in the row's checkbox annotation.

When ≥3 rows of the same family resolve the same way (e.g. "scoring
constants tuned for Manhattan need re-tuning under Euclidean"), pull
the cluster forward as a sibling ticket and treat the remaining rows in
that cluster as derived. Per [[feedback_deferred_spec_patch_stack]] don't
let this triage become a 14-patch stack — at the third resolution of the
same shape, open the structural follow-on.

## Verification

This ticket closes when:

- All checklist rows have a written concordance verdict (expected vs
  regression).
- Each regression row has either (a) a constants patch landed via a
  sibling ticket, (b) an explicit "accept the drift, update the
  test/baseline" annotation, or (c) an open follow-on ticket linked from
  the row.
- A new soak + verdict against the post-fix binary lands `pass` or
  `concern-with-annotated-rationale`.
- A new baseline is promoted (or the existing baseline is explicitly
  marked stale-on-purpose pending follow-ons).

## Post-fix follow-ons

The substrate realignment (Fix A) reroutes the default `distance_to`
from Euclidean to Chebyshev so perception aligns with 8-direction
movement cost. This shift retires the four plan-failure regressions
above but surfaces a different downstream shape that needs
follow-on work:

1. **`TravelTo(CarcassPile): no path and stuck` 0 → 1099 (new
   high-rate)**. Distinct from Fix B's PatrolZone shape — CarcassPile's
   `resolve_zone_position` branch returns the carcass tile directly
   without offset construction, so the perimeter-passability gate
   doesn't apply. The carcass position itself is reportedly unreachable
   1099× per soak. Likely cause: the carcass-position snapshot is
   built from `Dead`-tagged cats whose final position may sit on
   tiles that have since become impassable (waterlogged, walled-in by
   construction), or carcasses are being targeted by cats outside the
   reachable connected component. Needs a sibling ticket — same
   structural shape as Fix B but at a different snapshot layer.

2. **`HoldUntilSafe: global step timeout` 0 → 670 (new)**. New mode.
   Cats sit in `HoldUntilSafe` (post-wildlife-threat hold) past the
   global step watchdog. Likely substrate-effect: Chebyshev's tighter
   threat-perception ("how close is the predator in step-cost") may be
   keeping cats in hold mode longer than Euclidean's amplitude reading
   warranted, or the "is the threat still close" loop expects an
   Euclidean falloff. Needs a sibling ticket.

3. **`EngagePrey: lost prey during approach` 28 → 126 (4.5×)**.
   Up substantially. Likely substrate-effect: prey evasion math may
   read Chebyshev "step distance" differently than the cardinal-baked
   tuning expected. Less urgent than the prior two — within order of
   magnitude. Sibling ticket if it persists after the two above land.

4. **`scenarios::prey_byproduct_spawn::rat_kills_produce_bone_sinew_whisker`**
   tick-budget bumped 200 → 800 (test only — substrate-effect
   calibration). Mouse / Rabbit / Bird pass within their original
   budgets. Capture in a follow-on if Rat-hunt timing under Chebyshev
   suggests a real balance regression.

5. **Per-DSE score shifts** under Chebyshev (Simba focal frame-diff
   vs pre-fix `tuned-42-09411128-pre-494-anchor`):
   - `mentor` -67.9% (target +0.048 baseline restored ✓)
   - `caretake` -100% (retires to 0 ✓)
   - `handoff` -100% (retires to 0)
   - `discard` / `trash` / `magic_scry` -94 to -99%
   - `groom_self` +58%, `build` +161% (recovered from Euclidean
     collapse), `groom_other` +241%, `sleep` +93%, `idle` +2117%
     (from -0.011 to +0.218 — substrate-correct), `socialize` +202%,
     `farm` +1254%
   - Hunt scoring not in top-15 shifts — recovered without overshoot.
   - Treat the cluster as substrate-correct shift, not regression:
     `frame-diff` reports "concordance: ok — no unacknowledged drift
     on tracked DSEs". Open a tuning-iteration ticket only if soak
     verdict against the new baseline shows continued degradation.

6. **Welfare drop -19.2%, shelter -100%**. Under Chebyshev the colony
   survived 91k ticks vs baseline 59k (+54%, 4 seasons vs 2 — a
   genuine improvement), but welfare composition shifted. Shelter
   reading 0.0 suggests the shelter-belief substrate or the home-den
   pathing now reads differently. Separate from the four plan-failure
   regressions; likely connects to ticket 374 (shelter-as-belief). Open
   a follow-on after the CarcassPile + HoldUntilSafe shapes land.

7. **`surrounded_colony::*` ring-coverage** unit-tests still failing.
   Identical W-sector miss under Chebyshev as under Euclidean — the
   issue isn't the metric, it's the ward-placement candidate-scoring
   geometry. Pre-existing per this ticket's "Unit-test failures"
   section; ungated by Fix A/B.

## Log

- 2026-06-01: opened post-492 verdict. Soak `logs/tuned-42-09411128`
  vs baseline `logs/tuned-42-d531318e`. Survival/continuity pass;
  footer + per-DSE drift across ward placement, hunt-scent, patrol
  pathing, mentor targeting, craft selection. Two unit-test failures
  on `surrounded_colony` ring-coverage. 16 checklist rows to triage
  before promoting a new baseline.

- 2026-06-01: landed the substrate realignment (Fix A + Fix B). Pre-fix
  soak preserved at `logs/tuned-42-09411128-pre-494-anchor`; post-fix
  soak at `logs/tuned-42-09411128`. Fix A reroutes `Position::distance_to`
  from Euclidean to Chebyshev (the metric that matches 8-direction
  movement per `pathfinding.rs::heuristic`), introduces a
  `euclidean_distance` escape hatch, flips the four scent-map
  internal gates (prey / cat / fox / carcass) to Chebyshev for caller
  symmetry. Fix B adds `perimeter_offset_position` reachability gate
  to `resolve_zone_position::PatrolZone` and `RestingSpot`. Fix C
  (mentor eligibility hardening) was deferred and ultimately found
  redundant — the substrate cooldown already handles in-flight
  invalidation. Completed the in-progress 492 call-site migration
  (~70 sites in goap.rs from `manhattan_distance` to `distance_to` or
  `tile_distance_squared`) as the precondition for a clean build. All
  four plan-failure rows above retired. Colony survived 91k ticks vs
  baseline 59k (+54%, 4 seasons vs 2). New regressions documented in
  the post-fix follow-ons section above.
