---
id: 2026-04-23
title: "Phase 4c.7 — §6.5.6 `Caretake` target-taking DSE port + `resolve_caretake` retirement"
status: done
cluster: null
landed-at: feedbac
landed-on: 2026-04-23
---

# Phase 4c.7 — §6.5.6 `Caretake` target-taking DSE port + `resolve_caretake` retirement

Final §6.5 per-DSE target-taking port, landing on the Socialize
(4c.1) / Mate (4c.2) / Mentor (4c.5) reference pattern. Closes
the last §6.5 row and retires the Phase 4c.3 plain-helper
`resolve_caretake`.

- New `src/ai/dses/caretake_target.rs`:
    - `caretake_target_dse()` factory — four per-§6.5.6
      considerations: `target_nearness` `Quadratic(exp=1.5)`
      (range=12, spec §6.4 row #9), `target_kitten_hunger`
      `Quadratic(exp=2)` on `1 − needs.hunger` deficit,
      `target_kinship` `Piecewise([(0.0, 0.6), (1.0, 1.0)])`
      (Cliff: parent=1.0, non-parent=0.6 — floor preserves the
      colony-raising pattern Phase 4c.4's alloparenting Reframe A
      established), `target_kitten_isolation` `Linear(1, 0)` on
      binary "no sibling / parent within 3 tiles". WeightedSum
      weights `[0.20, 0.40, 0.25, 0.15]` verbatim from spec. No
      axes deferred — this is the first 4c port to land the full
      §6.5 weight vector. `Best` aggregation, `Goal { label:
      "kitten_fed", strategy: SingleMinded }` Intention.
    - `resolve_caretake_target(registry, adult, adult_pos,
      kittens, cat_positions, tick) → CaretakeResolution` — wraps
      `evaluate_target_taking` in the caller-side
      `CaretakeResolution` shape `disposition.rs` +
      `goap.rs` already consume (target / target_pos /
      target_mother / target_father / is_parent / urgency).
      `urgency` now carries the aggregated-`Best` score (0..1)
      rather than the pre-refactor hand-rolled
      `deficit × decay × kinship_boost` product. `is_parent`
      stays bloodline-override semantics (any own hungry kitten in
      range → true, not just the argmax) so `CaretakeDse`'s
      self-state parent-bonus axis keeps firing for colony-kitten
      argmax wins.
    - Candidate filter: kittens with `needs.hunger <
      KITTEN_HUNGER_THRESHOLD (0.6)` and Manhattan distance ≤
      `CARETAKE_TARGET_RANGE (12)` from the scoring adult —
      preserves the Phase 4c.3 pool shape so the port is a
      pure scoring-shape swap.
    - Isolation predicate: a candidate kitten counts as isolated
      iff neither (a) another kitten sharing its mother or father
      nor (b) an adult matching its `KittenDependency.mother /
      .father` sits within `ISOLATION_RADIUS = 3` Manhattan tiles.
      Sated siblings still count as co-located — isolation
      describes "who is nearby," not "who else needs caretaking."
    - 18 unit tests covering id stability, axis count, weight
      sum, `Best` aggregation, Goal-Intention factory,
      empty-registry / empty-kittens / well-fed-filtered /
      out-of-range filtering, hunger-Quadratic argmax at tied
      distance, kinship-floor picks non-parent when it's the only
      candidate, kinship-cliff breaks ties in favor of own
      kitten, bloodline `is_parent` fires even when a stranger
      wins argmax, distance tie-break, isolation beats
      co-located sibling, parent-presence suppresses isolation,
      resolution surfaces target_mother / target_father /
      target_pos, and urgency stays in [0, 1] on the extreme
      case.
- `src/ai/caretake_targeting.rs` — `resolve_caretake` function
  deleted along with `CARETAKE_RANGE`, `HUNGER_THRESHOLD`, and
  `PARENT_KINSHIP_BOOST` constants (moved-or-replaced in the
  new module). `KittenState` + `CaretakeResolution` stay (still
  the public caller surface). `caretake_compassion_bond_scale`
  unchanged per the Phase 4c.4 alloparenting Reframe A
  commitment — scaling modulates the *self-state*
  `caretake_compassion` axis, not a per-candidate axis, so it
  belongs caller-side, not in the target-DSE bundle.
- Registration at `main.rs::build_app`, `main.rs::build_schedule`,
  and `plugins/simulation.rs::SimulationPlugin::build` — three
  registration sites per the headless-mirror rule. Per-site
  ordering places `caretake_target_dse` immediately after
  `caretake_dse()` so the self-state + target-taking pair sits
  together in the registry vector.
- Caller cutover (three sites):
    - `disposition.rs::evaluate_dispositions` scoring path —
      swapped to `resolve_caretake_target`, passing
      `cat_positions` for the isolation axis.
    - `disposition.rs::disposition_to_chain` — same swap;
      `cat_pos_list` flows to the isolation axis. Winning
      kitten feeds `build_caretaking_chain` (navigate-to-
      Stores → retrieve-food → navigate-to-kitten → feed).
    - `goap.rs::evaluate_and_plan` scoring path — same swap
      with `cat_positions`. `caretake_resolution.target`
      continues to seed `FeedKitten.step_state[idx].target_entity`
      at plan-creation time (Phase 4c.4's target-entity
      persistence — re-resolving at step-time would return
      `None` from the stale adult position once the adult walks
      to Stores).
    - `goap.rs::resolve_goap_plans::FeedKitten` fallback —
      swapped to `resolve_caretake_target` too; the goap-path
      `kitten_snapshot` stays `Vec::new()` (Phase 4c.3 comment
      — avoiding `&mut Needs` query conflict), so the fallback
      still returns `None` and the primary seeding at
      plan-creation time is the real path.

**Registration invariant.** DSE count at each registration site:
Phase 4c.6 had 9 target-taking DSEs (`hunt_target`, `fight_target`,
`socialize_target`, `groom_other_target`, `mentor_target`,
`mate_target`, `build_target`, `apply_remedy_target` + implicit
through plugins); Phase 4c.7 brings it to 10 with
`caretake_target_dse`.

**Hypothesis.** The spec-shape four-axis bundle — especially the
kitten_isolation axis, which the pre-refactor
`deficit × decay × kinship_boost` product could not see — should
pick argmax kittens closer to the colony-raising design intent.
Predicted direction: no KittenFed starvation cascade (survival
canary holds); some drift in which specific kitten wins in edge
cases (orphan-at-3-tiles beats co-located sibling), which in
aggregate shouldn't change the KittenFed count materially on a
15min soak where there are usually only a handful of
simultaneously-hungry kittens.

**Observation — seed-42 15min release soak:**

| metric | pre (4c.6 footer, reference) | post (this landing) | delta |
|---|---|---|---|
| `deaths_by_cause.Starvation` | 0 | 0 | unchanged |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | unchanged |
| `footer_written` | 1 | 1 | unchanged |
| `never_fired_expected_positives` count | 3 | 3 | unchanged (same FoodCooked / GroomedOther / MentoredCat persistents as 4c.5 / 4c.6 baseline) |
| `KittenFed` | 55 / 10 / 79 (range across recent soaks) | 27 | within seed-42 parallel-scheduler noise envelope per CLAUDE.md |
| `continuity_tallies.grooming` | 191 (4c.6) | 141 | −26% (noise band; grooming / courtship / play survival unchanged) |
| `continuity_tallies.courtship` | 2 | 2 | unchanged |
| `continuity_tallies.mentoring` | 0 | 0 | unchanged (pre-existing skill-threshold gate) |

**Directional concordance: ACCEPT.** Survival canaries pass
(starvation=0, shadowfox=0, footer written, KittenFed=27 ≥ 1
gate). Never-fired-expected unchanged — the three persistent
dormancies (FoodCooked / GroomedOther / MentoredCat) are
pre-existing from 4c.5 baseline, not new regressions from this
port. KittenFed magnitude within the seed-42 noise envelope
(documented 10–79 range across recent soaks at the same
commit-tree depth). Per CLAUDE.md's balance methodology, the
literal positive-exit metric is deferred per the post-refactor
balance-tuning commitment in open-work #14.

**Acceptance gate (per task spec: "KittenFed ≥ 1 and survival
canaries pass"):** PASS.

**Deferred (same envelope as 4c.1 / 4c.2 / 4c.5 / 4c.6
deferrals):**
- Merging target-quality scores into the action-pool (target
  DSEs still observational, not pool-modulating) — cross-cutting
  with the other six ports.
- Balance tuning of the four §6.5.6 axis weights and the
  kitten_isolation radius — covered by the refactor-substrate-
  stability commitment in #14.
- §7 CommitmentStrategy::Blind for severe-hunger kitten targets
  (zealot-pursuit posture when deficit > threshold) — queued
  as a distinct §7 scope, not in the §6.5 slate.

**Remaining Phase 4 work** (open-work #14 outstanding list):
All §6.5 per-DSE `TargetTakingDse` ports now closed. The
remaining refactor-scope work sits in §4 marker authoring (~48
markers), §L2.10.7 plan-cost feedback, and §7 commitment
strategies.

---
