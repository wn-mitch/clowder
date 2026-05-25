---
id: 465
title: Cat approach pathing failure at stalk range
status: done
orchestration: substrate-sensitive
cluster: wildlife
initiative: [predator-prey-dynamics]
added: 2026-05-24
parked: null
blocked-by: []
supersedes: []
related-systems: [sensory.md, ai-substrate-refactor.md]
related-balance: [100-tremor-action-multiplier.md]
landed-at: 657505ad89a5
landed-on: 2026-05-25
---

<!--
Bugfix-shape ticket. Use this template (rather than `_template.md`) when the
work is a fix to observed defective behavior. The "Bugfix discipline" section
of CLAUDE.md REQUIRES at least one structural-revision candidate per fix-shape
decision tree — the slots below force that to be drafted, named, and considered.
-->

## Why

Ticket 464 (the `effective_stalk_distance` defaults retune) landed and
recovered colony-wide hunt success 13.3% → 18.47% and Rat success
14.1% → 45.4%, but **Rabbit success stayed pinned at 17.05%** — basically
unchanged from the broken post-100 state of 17.5%. The failure pattern
is sharply localized: **"stuck during approach" accounts for 97.8% of
Rabbit losses** (352 of 360 losses in run `tuned-42-cfc6f4fa`). The
narrative line: "stuck during approach (1068, 80.9% of losses)" —
colony-wide, but Rabbit is the dominant species in that count.

This is the pathing weakness 464 §Out of scope explicitly named for
escalation if R1's soak still showed residual `lost_during_stalk`
elevation. It surfaced. The mechanism is *separate* from the stalk-distance
tuning: even at the tightened R1 distances (~7 tiles for a typical
patient cat on Rabbit), the A*-driven approach pathing gives up on a
non-trivial fraction of attempts. Rats catch fine at the same distances
because rat alert_radius is smaller (4 vs Rabbit's 6) — the approach
window is shorter and less likely to wander through clutter.

Run-dir: `logs/tuned-42-cfc6f4fa` (constants reflect 464's R1 — verify
via header's `disposition.species_push: 1.0` / `alertness_push: 1.5`,
not via the `commit_hash` field which reads the parent due to a
build.rs/jj interaction).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | n/a | Not a marker-driven defect; the cat has Hunt-eligible markers and the prey is sensed | `[verified-correct]` (via 464's audit + colony-wide kill rate recovery on Rat) |
| L2 DSE scores | `src/ai/dses/hunt_target.rs` | Hunt selection rate per-10kt unchanged colony-wide; cats DO commit to Hunt | `[verified-correct]` (464 frame-diff: Hunt not in top-15 movers) |
| L3 softmax | `src/ai/scoring.rs` | Hunt wins softmax; the GoapPlan is built | `[verified-correct]` |
| Plan template | `src/ai/planner/...` (`EngagePrey`) | Plan transitions Approach → Stalk/Chase/Pounce by distance threshold | `[verified-correct]` (464 layer-walk) |
| EngagePrey resolver | `src/systems/goap.rs::resolve_engage_prey:9037-9042` | Stalk-start distance now tuned via 464's R1; the per-tile lift is reduced | `[verified-correct]` |
| Approach pathing | `src/ai/pathfinding.rs::step_toward:342-423` | Cat moves toward prey via **greedy** `step_toward` (NOT A*-backed — `find_path` exists at `pathfinding.rs:235-336` but is never called during hunt approach). Tries diagonal → horizontal → vertical, returns `None` if all three impassable. Rustdoc: "will get stuck in local minima (e.g. concave obstacles)." | `[suspect]` — load-bearing defect lives here |
| Stuck/lost emit-sites | `src/systems/goap.rs:9489-9527` | Approach loop calls `step_toward` up to `approach_speed=3` times/tick; if no movement, increments `no_move_ticks`; emits `failure_reason="stuck during approach"` when `no_move_ticks > chase_stuck_ticks=10`, OR `"lost prey during approach"` when `dist > approach_give_up_distance=60`. **Both flow into `EventKind::HuntAttempt.failure_reason` (NOT `PlanFailure`).** | `[verified-correct]` (emit site located) |
| Stalk-phase pathing | (same) | Stalk-specific pathing fires once `actual_distance ≤ effective_stalk_distance`; `lost_during_stalk` recovered from 301 → 108 with 464's R1 | `[verified-correct]` (R1 fixed this layer) |

## Findings (pre-investigation code-read, 2026-05-24)

Three load-bearing corrections to the original framing, surfaced by a
Phase 1 layer-walk before the Stage 1 `/logq` investigation runs:

1. **`step_toward` is greedy, not A*-backed**
   (`src/ai/pathfinding.rs:342-423`). Tries diagonal → horizontal →
   vertical and returns `None` if all three neighbors are impassable.
   Rustdoc explicitly: *"will get stuck in local minima (e.g. concave
   obstacles). That is acceptable for Phase 1."* A* (`find_path`)
   exists in the same file (lines 235-336) but is **never called
   during hunt approach** — only foxes, disposition MoveTo/PatrolTo,
   and other long-haul movement use it.

2. **"Stuck during approach" is a step-failure reason, not a
   `PlanFailure`.** Emitted at `src/systems/goap.rs:9502-9523` via
   `record_hunt_attempt(..., Some("stuck during approach".into()))`
   into `EventKind::HuntAttempt.failure_reason`. The original recommended
   query `--kind=PlanFailure` returns zero relevant rows because
   `L3PlanFailure` only carries `replan_cap` / `anxiety_interrupt` /
   `modifier_preemption` / `morale_break` reasons.

3. **The stuck branch is gated on `no_move_ticks > chase_stuck_ticks`
   (default 10)**, distinct from the "lost prey during approach"
   branch gated on `dist > approach_give_up_distance` (default 60).
   Both branches share the same emit-site; the `failure_reason` field
   disambiguates. `approach_speed = 3` tiles/tick.

The `HuntAttempt` event carries `location` (prey position at failure),
`outcome`, `start_distance`, `failure_reason` — but **not cat position
at failure**. Stage 1 spatial clustering uses prey location as a proxy
(adequate for terrain-cluster vs uniform-distribution discrimination,
inadequate for per-cat path reconstruction).

## Fix candidates

**Parameter-level options:**

- **R1 — relax the "stuck" giveup threshold** by raising
  `chase_stuck_ticks` (default 10) for the approach phase, OR widen the
  `step_toward` candidate set beyond the three-direction greedy pick.
  Cheap; risks longer-lived failed approaches consuming Hunt slots.
  Doesn't address local-minimum trap shape — only postpones giveup.
- **R2 — add a one-tile-jitter retry** when greedy `step_toward` returns
  `None`. The cat re-attempts from a perturbed start tile rather than
  incrementing `no_move_ticks`. Mirrors prey's evasive-jink pattern.
  Localized to the approach-block caller; doesn't reshape substrate.
- **R3 — bias the approach path's heuristic away from terrain features
  that produce stuck-cluster events** (e.g., LightForest density,
  water-adjacent tiles). Requires identifying *which* tile-shapes
  correlate with stuck events via the Stage 1 `/logq` characterization.

**Structural options:**

- **R4 (split) — give Approach its own resolver step distinct from
  `resolve_engage_prey`.** Currently EngagePrey is a single GOAP action
  (`src/ai/planner/actions.rs:143-153`) whose resolver internally
  state-machines through Approach/Stalk/Chase/Pounce. A dedicated
  `ApproachPrey` step with its own pathfinder profile would isolate
  the failure mode at the substrate level.
- **R5-restated (extend) — switch the approach phase from greedy
  `step_toward` to A* (`find_path`).** The substrate already exists
  at `src/ai/pathfinding.rs:235-336`; foxes already use it
  (`src/steps/fox/mod.rs:45`). Blast radius contained — only the
  approach branch in `resolve_engage_prey` changes. Cache the path
  per-attempt; re-path when prey moves more than N tiles. Resolves
  the local-minimum weakness at its root.
- **R6 (rebind) — n/a.** No Action→Disposition mapping change applies.
- **R7 (retire) — n/a.** Approach is load-bearing.

## Recommended direction

**Investigate first via `just q events --kind=HuntAttempt
logs/tuned-42-cfc6f4fa`** (NOT `--kind=PlanFailure` — "stuck during
approach" is emitted into `EventKind::HuntAttempt.failure_reason` via
`record_hunt_attempt`, not into a `PlanFailure` trace record), filtered
to `outcome=LostDuringApproach && failure_reason="stuck during approach"`,
clustered by `location` tile region and `prey_species`. Then:

- **stuck events cluster on concave terrain** → R5-restated (A* in
  approach). Substrate-over-hacks: the local-minimum weakness is
  precisely what A* solves.
- **stuck events distributed broadly, often correlated with other-cat
  or prey occupancy** → R2 (jitter retry). Cheap, localized.
- **stuck events correlated with prey-jink** → R3 + R5 hybrid
  (intercept-prediction). Bigger change; reserve as escalation.

R4 (split Approach into its own resolver step) is the broader structural
option held in reserve if neither focused fix lands Rabbit success
within ±10% of baseline (target ≥ 29.7%).

## Outcome (landed)

Shipped R5-restated: A*-fallback in the hunt approach loop. When greedy
`step_toward` returns `None` (concave-terrain local-minimum), call
`find_path` and take the first step. ~5-line inline change at
`goap.rs:9523-9538`; no substrate restructure.

| Species | Pre-fix | Post-fix | Δ |
|---|---:|---:|---:|
| Colony | 18.47% | 23.89% | +5.4 pp |
| Rabbit | 17.05% | **96.06%** | **+79.0 pp** |
| Rat    | 45.40% | 100.0% | +54.6 pp |
| Mouse  | 54.40% | 87.10% | +32.7 pp |
| Bird   | 16.34% | 22.01% | +5.7 pp |
| Fish   | 4.48%  | 3.17%  | −1.3 pp (structurally unhuntable — orthogonal) |

Verdict: `concern` (not fail). Survival/never-fired gates pass.
Welfare overshoots baseline coherently (seasons_survived +50%,
structures_built +36%, bonds_formed +33%) — predicted direction per
pillar #3, magnitudes exceed concordance bands because the prior
baselines were bug-corrupted by the greedy-stuck defect.

`EngagePrey: lost prey during approach` plan-failure: 1069 → 579
(−46% absolute). Of the remaining 578 stuck events, 78.9% are Fish
(`find_path` correctly refuses impassable targets) — see follow-on.

Full hypothesis / observation / concordance in
`docs/balance/100-tremor-action-multiplier.md` §Iter-3.

## Out of scope

- The `effective_stalk_distance` arithmetic — 464 handled it.
- Multi-focal sweep on a high-boldness focal — deferred per 464's §Out
  of scope.
- Per-personality pathfinder profiles (bold cats give up faster) — would
  be a *second* axis on top of any fix here; not needed post-fix.
- **TravelTo same-defect** — `TravelTo(HerbPatch): no path and stuck`
  uses the same greedy `step_toward`; opened as follow-on.
- **Fish-on-water hunt** — `find_path` correctly refuses impassable
  targets; Fish remain unhuntable; opened as follow-on.
- **CraftAtWorkshop recipe-not-satisfied surface** — new plan-failure
  mode (0.01364/tick) likely secondary to larger / better-fed
  population; opened as follow-on.

## Verification

- `just check` + `just test` clean.
- `just q events --kind=HuntAttempt logs/tuned-42-cfc6f4fa` (first; filter to `outcome=LostDuringApproach && failure_reason="stuck during approach"`, cluster by `location` and `prey_species` to characterize the failure terrain).
- `just soak-trace 42 Simba` post-fix.
- `just verdict logs/tuned-42-<sha>` — survival/continuity must pass.
- `just q hunt-success logs/tuned-42-<sha> --species=rabbit` — must be ≥ 29.7%
  (within ±10% of baseline 33.0%).
- Colony-wide hunt success ≥ baseline 19.7% (R1 brought it to 18.47%, just
  below; this ticket should restore the missing margin).

## Log

- 2026-05-24: opened from 464's R1 soak. Colony-wide hunt success recovered
  to 18.47% (target ≥ 17.7% met), Rat fully recovered (14.1% → 45.4%), but
  Rabbit stalled at 17.05% (target ≥ 29.7% missed by ~13 pp). Failure pattern
  "stuck during approach" 97.8% of Rabbit losses — the pre-existing A* path-
  weakness 464 §Out of scope explicitly named. R1/R2 likely; R4 structural
  fallback if parameter fixes don't recover Rabbit within band.
- 2026-05-24: Phase 1 layer-walk surfaced three corrections to the original
  framing — added §Findings. (a) `step_toward` is greedy, not A*-backed;
  (b) the stuck event is `EventKind::HuntAttempt` not `PlanFailure`; (c) the
  stuck branch is gated on `no_move_ticks > chase_stuck_ticks=10`. R5
  reformulated from "prey-aware path budget for step_toward" to "switch
  approach to A* (`find_path`)" — the substrate-over-hacks form, with
  foxes as precedent caller. Investigation query target corrected from
  `--kind=PlanFailure` to `--kind=HuntAttempt`. Layer-walk row for the
  stuck/lost emit-site promoted to `[verified-correct]`; the
  `step_toward` row stays `[suspect]` pending Stage 1 evidence cluster.
- 2026-05-25: Stage 1 `/logq` characterization. Per-species stuck-rate
  is high colony-wide (49-97.8% of losses) — not Rabbit-specific.
  **Spatial clustering is sharp**: 112 of 352 Rabbit stuck events at the
  exact same tile (18,15); top 5 tiles = 54%. Rabbit/Bird/Mouse hotspots
  overlap in region x=12-21, y=12-18; Fish at (52-53, 24-26) — water-edge
  pattern. Layer-walk row promoted to `[verified-defect:
  greedy-local-minimum-no-backtrack]`. Same trap × same prey-resting-tile
  × repeated cat attempts = the 1068-events-per-soak observation.
  Mechanism: `step_toward` tries diag → horiz → vert toward prey only;
  no perpendicular or reverse moves; concave terrain absorbs cat for
  `chase_stuck_ticks=10` then bails. Documented in §Findings.
- 2026-05-25: Shipped R5-restated — `.or_else(find_path…)` inline fallback
  at `goap.rs:9523-9538`. Rabbit success 17.05% → 96.06%, all land-prey
  recovered, Fish unchanged (orthogonal). Verdict `concern` (welfare
  overshoots in predicted direction; survival gates pass). Balance
  doc append at `docs/balance/100-tremor-action-multiplier.md` §Iter-3.
  Opened follow-ons: TravelTo β-fix, Fish-on-water, CraftAtWorkshop
  recipe-not-satisfied investigation.
