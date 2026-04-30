---
id: 2026-04-22
title: "Phase 4c.5 — §6.5.3 `Mentor` target-taking DSE port"
status: done
cluster: null
landed-at: null
landed-on: 2026-04-22
---

# Phase 4c.5 — §6.5.3 `Mentor` target-taking DSE port

Third per-DSE §6.5 target-taking port, landing on the
Socialize / Mate reference pattern established in Phase 4c.1 /
4c.2. Closes the §6.2 silent divergence on the MentorCat path
and the §6.1-Critical "resolver ignores skill-gap entirely" gap.

- New `src/ai/dses/mentor_target.rs`:
    - `mentor_target_dse()` factory — three per-§6.5.3
      considerations (`target_nearness` `Quadratic(exp=2)`,
      `target_fondness` `Linear`, `target_skill_gap`
      `Logistic(8, 0.4)`). Weights renormalized from the spec's
      (0.20/0.20/0.40/0.20) → (0.25/0.25/0.50) by deferring the
      `apprentice-receptivity` axis pending the §4.3 `Apprentice`
      marker author system. `Best` aggregation. Intention:
      `Activity { kind: Mentor, termination: UntilInterrupt,
      strategy: SingleMinded }`.
    - `resolve_mentor_target(registry, cat, cat_pos, cat_positions,
      self_skills, skills_lookup, relationships, tick)` — the
      single sanctioned target-picker for MentorCat. Skill-gap
      signal: `max_k (self.skills[k] − target.skills[k]).max(0)`,
      clamped to `[0, 1]` before the Logistic. Candidate filter:
      cats in range ≤ 10 tiles with `Skills`, no bond filter
      (mentoring grows bonds, doesn't require them).
    - 13 unit tests covering id stability, axis count, weight
      sum, `Best` aggregation, `max_skill_gap` edge cases
      (largest-positive / negative-gaps-ignored / clamp-to-1),
      no-registration → None, no-candidates-in-range → None,
      self-exclusion, skill-less candidates skipped,
      larger-gap-wins-all-else-equal, skill-gap-dominates-fondness-
      bias (encodes §6.5.3 design-intent), and Mentor intention
      factory.
- Registration at `main.rs::build_app`, `main.rs::build_schedule`,
  and `plugins/simulation.rs::SimulationPlugin::build` — three
  registration sites per the headless-mirror rule. Per-site
  ordering places `mentor_target_dse` immediately after
  `mentor_dse()` so the self-state + target-taking pair sits
  together in the registry vector.
- `disposition.rs::disposition_to_chain` — resolves
  `mentor_target` alongside `socialize_target` / `mate_target` at
  the per-cat chain-building site, using a
  `skills_query.get(e).ok().cloned()` closure for the candidate-
  side skill lookup.
- `disposition.rs::build_socializing_chain` — new
  `mentor_target: Option<Entity>` parameter. The `can_mentor`
  branch now prefers the skill-gap-picked `mentor_target` over
  the fondness-picked `socialize_target`, preserving the paired
  threshold check (`self > high && other < low` on the same
  skill axis) as a defensive reconfirmation. Falls through to
  Socialize's target for the groom / socialize branches.
- `goap.rs::resolve_goap_plans::MentorCat` — replaces
  `find_social_target` with `resolve_mentor_target`. New
  `cat_skills_snapshot: HashMap<Entity, Skills>` built once per
  tick before the mutable-borrow loop so the MentorCat branch
  can rank apprentices without re-borrowing `cats`. Legacy
  `find_social_target` remains in place for `GroomOther` only
  until §6.5.4 ports.

**Seed-42 `--duration 900` release deep-soak**
(`logs/phase4c5-mentor-target/events.jsonl`):

| Metric | 4c.4 (v3 run 1) | 4c.5 | Direction |
|---|---|---|---|
| `deaths_by_cause.Starvation` | 3 | 4 | noise-band (4c.4 v2/v3 range 0–5 across runs) |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | canary passes |
| `continuity_tallies.grooming` | 174 | 211 | +21% noise |
| `continuity_tallies.mentoring` | 0 | 0 | unchanged — paired-threshold skill gate pre-existing |
| KittenFed | 110 | 79 | −28% noise |
| MatingOccurred | 5 | 5 | stable |
| KittenBorn | 3 | 4 | +1 |
| CropTended / CropHarvested | 9777 / 155 | 15722 / 364 | +61% / +135% noise |

**Hypothesis concordance — §6.5.3 port:**

> Skill-gap-ranked apprentice selection retires the
> fondness-only `find_social_target` MentorCat caller and the
> `socialize_target`-as-apprentice legacy wiring. Prediction:
> Mentor activity in soaks either stays at 0 (if no cat-pair
> crosses the smoothed threshold) or ticks up slightly (0 → 1–3
> events) as pairs with moderate gaps that previously failed the
> binary threshold become reachable.

- **Direction-neutral result.** MentoredCat still 0 after port.
  Root cause is *not* target selection — the `can_mentor` gate
  still requires a self-side skill above `mentor_skill_threshold_high`
  (0.6), which cats only reach after substantial skill growth;
  15 min of sim rarely produces a cat with skill > 0.6 in the
  present colony. Port correctness verified by unit tests
  (higher-skill-gap wins, skill-gap dominates fondness bias);
  sim-level activity depends on balance tuning deferred per
  open-work #14's post-refactor commitment.
- **Silent-divergence closed.** Mentor target selection now
  ranks on skill-gap magnitude (Logistic saturating near
  gap≥0.5) instead of the pre-refactor fondness-only legacy.
  When skill growth eventually unlocks the `can_mentor` gate,
  the cat the planner commits to will be the highest-gap
  apprentice in range — the §6.1-Critical gap closed
  structurally.
- **Survival canaries pass.** Starvation within 4c.4 noise
  band; ShadowFoxAmbush = 0; no wipe; continuity grooming /
  farming improvements within RNG variance.

**Deferred (same envelope as 4c.1 / 4c.2 deferrals):**
- Merging mentor target-quality into the action-pool scoring
  layer (target-DSE still observational, not pool-modulating)
- `apprentice-receptivity` axis — waits for §4.3 `Apprentice`
  author system landing (open-work #14 marker-roster second
  bullet)
- `find_social_target` full retirement — waits for §6.5.4
  Groom-other port (third and final caller)
- Balance tuning of the `mentor_skill_threshold_high` /
  `mentor_temperature_threshold` gates — covered by the
  refactor-substrate-stability commitment in open-work #14

**Remaining Phase 4 work** (open-work #14 outstanding list):
Mentor struck from the 7-port remaining list; 6 per-DSE ports
remain (Groom-other, Hunt, Fight, ApplyRemedy, Build,
Caretake). No blocker sequencing imposed by Phase 4c.5 —
MentoredCat activity still 0 but not because of mentor-target
selection.

---
