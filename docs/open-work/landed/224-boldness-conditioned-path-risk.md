---
id: 224
title: boldness-conditioned path risk
status: done
cluster: pathfinder-risk-awareness
added: 2026-05-07
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-07
---

## Why
With ticket 223 in place, every cat takes the *same* detour around
fox-scent and corruption. That is not lifelike — bold cats should
accept more routing risk, timid cats should detour further. The
ticket-214 user framing was explicit: "not so scared they can't
achieve goals." Personality (`Personality.boldness`) is the right
knob; it already lives on the cat and is read by Patrol's existing
`boldness` axis (`src/ai/dses/patrol.rs:60-64`). Extend the same
reading into path-cost so the personality scalar composes through to
routing decisions.

This is the substrate piece that makes the cluster *characterful*
rather than uniformly cautious. Without it, the sim has cats but no
*kinds* of cats at the routing layer.

## Scope
- Refactor `find_path` / `step_toward` overlay arg from
  `&[&dyn TileCostOverlay]` (ticket 222) to either
  `&[(&dyn TileCostOverlay, f32)]` (parallel weights) or wrap each
  overlay in `WeightedOverlay { inner, weight }`. Pick during impl
  based on which composes more cleanly with call-site
  construction code.
- Each cat-side call site reads the cat's `Personality.boldness` and
  constructs the per-cat overlay weight: suggest
  `weight = (1.0 - boldness).clamp(0.1, 1.0)` so fully bold cats
  still respect a 10% threat-cost (no suicidal direct routes through
  fox dens), fully timid cats use full-weight detours.
- Document the *non-orthogonality* with Patrol's existing `boldness`
  axis: that axis says "bold cats want to patrol more"; this
  weight says "bold cats route through more risk *while* patrolling."
  Both read the same scalar but compose at different layers.

## Out of scope
- Other personality dimensions (curiosity, sociability) influencing
  path cost — possible follow-on; not load-bearing for this cluster.
- Trait-object overlay-construction caching — premature optimization;
  profile first if soak shows hot-path overhead.
- Fox-side personality conditioning — foxes do not have `boldness`
  scalars in the same shape; separate ticket if needed.

## Current state
- Tickets 222 (substrate) and 223 (cat overlays + retire damp
  modifier) precede this. Cluster A→B→C; this is C.
- `Personality.boldness` is already published; existing read-site at
  `src/ai/dses/patrol.rs:60-64` (Boldness scalar consideration with
  Linear curve).

## Approach
1. Pick the API shape during impl (parallel slice vs wrapper struct).
   The wrapper struct probably composes more cleanly with the helper
   function pattern from 223:
   ```rust
   pub struct WeightedOverlay<'a> {
       pub inner: &'a dyn TileCostOverlay,
       pub weight: f32,
   }
   ```
   Inside `find_path`, accumulate as
   `(o.inner.cost_at(pos) as f32 * o.weight) as u32`. Document
   that `weight = 0.0` should be expressed by *omitting* the
   overlay (avoid the f32→u32 truncation footgun on small
   weights with small cost contributions).
2. Per-cat call-site change: every cat-side `find_path` invocation
   constructs its overlay set from the cat's personality. Helper:
   ```rust
   fn cat_path_overlays<'a>(
       fox: &'a FoxScentMap,
       corr: &'a CorruptionLens,
       personality: &Personality,
   ) -> [WeightedOverlay<'a>; 2] {
       let w = (1.0 - personality.boldness).clamp(0.1, 1.0);
       [
           WeightedOverlay { inner: fox, weight: w },
           WeightedOverlay { inner: corr, weight: w },
       ]
   }
   ```
   Each call site needs `Personality` in its system param (via
   `SystemParam` bundle if 16-param limit hits — see CLAUDE.md
   "ECS rules"). Most call sites already read the cat's components
   via the actor query.
3. **Note the boldness-axis double-read.** `boldness` already lives
   in Patrol's L2 score axis (Linear, slope=1). Adding it to the
   path-cost weight means the scalar reads at two layers:
   - DSE layer: bold → higher Patrol score → cat picks Patrol more
   - Path layer: bold → lower threat-cost weight → bold cat's
     Patrol path traverses fox territory rather than detouring
   These are *complementary*, not redundant — the L2 axis decides
   whether to patrol; the path weight decides where to patrol. Call
   this out in the ticket Log line and in code comments at both
   read sites so future refactors do not collapse them.

## Verification
- `just check && just test`.
- Focal-trace **two** cats with extreme boldness:
  - `just soak-trace 42 <high-bold-name>` + soak-trace
    `<low-bold-name>` (pick from the founder roster — Wren is mid;
    look up the founder generation table for the extremes).
- `just frame-diff` between the two focal traces. **Predictions:**
  - Bold cat traverses fox-scent corridors (visible as `position`
    inside fox-scent buckets ≥ 0.4 during Patrol/Hunt steps).
  - Timid cat detours around the same corridors (longer paths,
    fewer fox-scent buckets entered).
  - ShadowFoxAmbush deaths concentrate in bold-cat sample if both
    spend equal action share on Patrol/Hunt — i.e., bold cats pay
    the cost of their personality.
- `just verdict logs/tuned-42` exit 0 or 1; survival gates pass.
- A refactor that changes sim behavior is a balance change. Soak
  + verdict before landing; if drift > ±10%, four-artifact
  hypothesis required.

## Log
- 2026-05-07: opened from work-214 investigation. Blocked-by 223.
- 2026-05-07: landed without soak gate per user direction (next session continues into 225 with cleared context). 228 still pending — decision-time fox-territory damp will compose with this layer when it ships.
