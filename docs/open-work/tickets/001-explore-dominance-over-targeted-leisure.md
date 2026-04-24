---
id: 001
title: Explore dominance over targeted leisure
status: in-progress
cluster: null
added: 2026-04-20
parked: 2026-04-21
blocked-by: []
supersedes: []
related-systems: [refactor-plan.md, ai-substrate-refactor.md, strategist-coordinator.md]
related-balance: [social-target-range.report.md]
landed-at: null
landed-on: null
---

## Current state

> **2026-04-24 — Sub-2 addressed.** Four fixes across three layers:
>
> **Fog-of-war layer (inputs):**
> 1. **Passive exploration stamping** — new `stamp_passive_exploration`
>    system (`src/systems/needs.rs`) marks a radius-2 (5×5) area around
>    every living cat each tick. RPG-style passive perception: cats
>    notice their surroundings by existing somewhere. Home territory
>    stays explored from cats walking through it.
> 2. **Decay rate 4× slower** — `exploration_decay_rate` reduced from
>    0.00005 to 0.0000125. Tiles take ~2 seasons to half-fade instead
>    of ~0.5 seasons. Only genuinely unvisited frontier fades.
> 3. **Decoupled perception radius** — new `explore_perception_radius = 6`
>    (13×13 = 169 tiles) for `unexplored_fraction_nearby` queries,
>    replacing `explore_range = 20` (41×41 = 1681 tiles). The action
>    range and perception radius were conflated; passive stamp radius 2
>    covering 25 tiles could never make a dent in a 1681-tile query area.
>
> **Commitment layer (plan retention):**
> 4. **Explore `still_goal` wired to area familiarity** — the §7.2
>    commitment gate's `still_goal` proxy for Exploring was a deferred
>    TODO (always `true`). Now checks `unexplored_nearby >=
>    explore_satiation_threshold` (default 0.3, matching the Logistic
>    saturation curve midpoint). When a cat's local area is well-explored,
>    the OpenMinded gate drops the Explore plan so re-evaluation can pick
>    a higher-scoring action.
>
> The Logistic(10, 0.3) saturation curve on the scoring layer was already
> correct — fixes 1–3 make its inputs realistic, fix 4 ensures the
> commitment gate respects the signal.
>
> **Previous state (parked 2026-04-21)** for AI substrate refactor:
> - **Sub-1 (social-target-range iter 3)** — superseded by refactor
>   Phase 4 target-selection (§6 `TargetTakingDse` replaces
>   `has_social_target` existence gate with target-quality scoring);
>   the pair-stickiness alternative named in iter-2's report becomes
>   a natural per-target consideration there.
> - **Sub-2 (Explore saturation curve)** — root causes now fixed at
>   fog-of-war, scoring, and commitment layers. Soak pending.
> - **Sub-3 (strategist-coordinator)** — unchanged; still C4 in the
>   deliberation cluster, gated on cluster A.

**Why it matters:** Explore claims 44–47% of all action-time in a seed-42
soak. Groom sits at 0.4–0.5%, Mentor / Caretake / Cook at exactly 0. The
user's "narrative leisure isn't happening" observation is real but it's a
target-availability problem, not a survival-lock problem.

**Root cause:** Explore has the loosest gate (just `unexplored_nearby > 0`).
Other leisure actions require specific targets (`has_social_target`,
apprentice, kitten, Kitchen, mate) that aren't consistently present.
Choosing Explore moves cats toward unexplored periphery → away from other
cats → `has_social_target` turns false → Explore wins again. Dispersion
feedback loop.

**Three directions agreed in the 2026-04-19 session** (ordered by blast
radius):

1. **Broaden `social_target_range`** (`src/resources/sim_constants.rs:1672`)
   from 10 → ~20–30 Manhattan tiles. Current 10 is combat-adjacent range,
   not cat-socializing range. In a 120×90 map with 8 cats, 10 is too
   tight for clustered-at-infrastructure moments to register.
   - **Iter 1 (range=25) REJECTED** — 2026-04-19. Mating (−67%), Kittens
     (−75%), bonds (−44%) regressed.
   - **Iter 2 DIAGNOSTIC (instrumented)** — 2026-04-20. Full score
     distributions (commit `290a5d9`) reframe the mechanism: Mate is
     gate-starved (0% of snapshots), never competed with Socialize in the
     scoring layer. The true regression is **bond attenuation** — wider
     range spreads Socialize interactions across more partners; each pair
     builds fondness/familiarity slower; Partners/Mates bond progression
     stalls; `has_eligible_mate` never opens. Treatment had 0 matings and
     0 kittens vs baseline 4/5.
   - **Sub-task 1 fundamentally compromised** — lowering/raising
     `social_target_range` can't fix the dispersion loop without bond
     attenuation. See `docs/balance/social-target-range.report.md` §
     Proposed iteration 3 for alternatives: (a) pair-stickiness in
     social-target selection, (b) pursue sub-task 2 (Explore saturation)
     which doesn't touch social dynamics.
2. **Saturation curve on Explore's weight.** Real cats don't explore
   indefinitely — past a local familiarity threshold it becomes
   indistinguishable from Wander. Current formula multiplies by
   `unexplored_nearby` linearly; at 50% locally explored, Explore still
   scores 0.5× its raw weight (enough to beat Wander's 0.08 floor).
   Target: sharp decay once local exploration fraction crosses ~0.7.
   Touch points: `src/ai/scoring.rs:302–309` and the radius/threshold
   args to `ExplorationMap::unexplored_fraction_nearby`.
3. **Strategist coordinator task board**
   (`docs/systems/strategist-coordinator.md`). The structural fix: a
   two-layer planner (strategic goal → tactical action) that gives cats
   a colony-level task board to align behavior against. Explore becomes
   "I have no better goal" rather than "I have no target." The doc itself
   gates this on the Cook loop firing end-to-end first — which is partly
   unblocked by the eat-threshold balance change above.

   **Cross-reference:** this is **C4** in the deliberation-layer cluster
   (see #7 below). It sits above BDI intentions (C1), social practices
   (C2), and belief modeling (C3) — HTN-style hierarchical planning. The
   existing `docs/systems/strategist-coordinator.md` design stub remains
   the primary design document; the cluster context adds the
   architectural framing for when it gets picked up.

**Ordering:** (1) and (2) are small scoring-layer tunes with seed-42
A/B verification. (3) is real engineering and wants its own design pass.
Do them in order; (1) and (2) should make the strategist's value visible
before it's scoped.
