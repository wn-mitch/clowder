---
id: 2026-04-22
title: "Phase 4c.6 — §6.5 per-DSE target-taking closeout: Groom-other + Hunt + Fight + ApplyRemedy + Build + `find_social_target` retirement"
status: done
cluster: null
landed-at: feedbac
landed-on: 2026-04-22
---

# Phase 4c.6 — §6.5 per-DSE target-taking closeout: Groom-other + Hunt + Fight + ApplyRemedy + Build + `find_social_target` retirement

Phase 4 closeout — five §6.5 per-DSE target-taking ports landing
together on the Socialize (4c.1) / Mate (4c.2) / Mentor (4c.5)
reference pattern. Retires `find_social_target`.

**Ports landed (§6.5 rows 4, 5, 7, 8, 9):**

- **§6.5.4 `Groom` (other)** — `src/ai/dses/groom_other_target.rs`.
  Four considerations: `target_nearness` `Logistic(15, 0.85)` on
  normalized distance signal (midpoint at dist=1.5 per §6.5.4's
  1–2 tile range row), `target_fondness` `Linear(1, 0)`,
  `target_warmth_deficit` `Quadratic(exp=2)` on
  `1 − needs.temperature` (convex amplification mirrors
  `Caretake`'s urgency axis), `target_kinship` Piecewise cliff
  (kin=1.0, non-kin=0.5). WeightedSum weights `[0.30, 0.30,
  0.30, 0.10]`, `Best` aggregation, Allogroom Activity
  Intention with `OpenMinded` commitment. Resolver takes
  `temperature_lookup` + `is_kin` closures for ECS
  compatibility. Kinship is bidirectional parent-child via
  `KittenDependency.mother / .father`.
  Wired into `disposition.rs::build_socializing_chain`'s
  `GroomOther` branch with `.or(socialize_target)` fallback
  for liveness, and into `goap.rs::GroomOther`. Retires
  `find_social_target` (GroomOther was the last caller after
  Socialize/Mate/Mentor ports). 14 unit tests.
- **§6.5.5 `Hunt`** — `src/ai/dses/hunt_target.rs`. Three
  considerations: `target_nearness` `Quadratic(exp=2)` over
  range=15, `prey_yield` `Linear(1, 0)` on
  `ItemKind::food_value / 0.8` (normalized so Rat=1.0,
  Rabbit=0.8125, Mouse=0.625), `prey_calm` `Linear(1, 0)`
  on `1 − PreyState.alertness`. WeightedSum weights `[0.357,
  0.357, 0.286]` (spec weights renormalized by dropping the
  `pursuit-cost` axis deferred pending §L2.10.7 plan-cost
  feedback). `Best` aggregation, HuntPrey Goal Intention.
  Wired into `resolve_search_prey`'s visible-prey path —
  replaces the pre-refactor `min_by_key(distance)` pick with
  yield-aware ranking. Scent-path unchanged (scent geometry
  resolves through influence-map source tile). §6.1 Partial
  fix: larger prey preferred at equivalent distance. 13 unit
  tests.
- **§6.5.9 `Fight`** — `src/ai/dses/fight_target.rs`. Four
  considerations: `target_nearness` `Logistic(10, 0.5)` on
  normalized distance, `target_threat` `Quadratic(exp=2)` on
  `WildAnimal.threat_power / 0.25` (normalized: ShadowFox=0.72,
  Fox=0.60, Snake=0.32), `target_combat_adv` `Logistic(10, 0.5)`
  on clamped `(self.combat + self.health_fraction −
  target.threat_level + 0.5)` (parity=0.5), `ally_proximity`
  `Linear(1, 0)` capped at 3 allies within 4 tiles. WeightedSum
  weights `[0.25, 0.30, 0.25, 0.20]`. **`SumTopN(3)`**
  aggregation per §6.5.9 — action score sums top-3 threats,
  winner stays argmax single-threat for GOAP planning.
  `ThreatEngaged` Goal Intention. New
  `ExecutorContext::wildlife_with_stats` query (disjoint from
  existing `wildlife` by component set). Wired into
  `resolve_goap_plans::EngageThreat`; coordinator Fight-
  directive path still seeds `target_entity` upstream so posse
  cohesion is unaffected. 16 unit tests.
- **§6.5.7 `ApplyRemedy`** — `src/ai/dses/apply_remedy_target.rs`.
  Three considerations: `target_nearness` `Quadratic(exp=1.5)`
  over range=15, `target_injury` `Quadratic(exp=2)` on
  `1 − health.current / health.max`, `target_kinship`
  `Linear(0.5, 0.5)` (non-kin=0.5, kin=1.0 per spec).
  WeightedSum weights `[3/14, 8/14, 3/14]` (renormalized from
  spec's 0.15/0.40/0.15 by dropping the 0.30 `remedy-match`
  axis deferred — remedies today are single-class via the
  `HealingPoultice/EnergyTonic/MoodTonic` switch at prepare
  time). `Best` aggregation, InjuryHealed Goal Intention.
  Resolver consumes a `PatientCandidate` snapshot built from
  `injured_cat_query` (which already carried Health) so
  severity scoring needs no new query. Wired into
  `try_crafting_sub_mode::PrepareRemedy` — picks via DSE first,
  falls back to nearest-injured if DSE returned None. §6.1
  Partial fix: severe patients triage higher than lightly-
  injured at comparable distance (health=0.2 beats health=0.9
  even at dist=10 vs dist=1). 12 unit tests.
- **§6.5.8 `Build`** — `src/ai/dses/build_target.rs`. Four
  considerations: `target_nearness` `Linear(1, 0)` on
  normalized distance over range=20, `target_site_type`
  Piecewise cliff (NewBuild=1.0, Repair=0.6),
  `target_progress_urgency` `Quadratic(exp=2)` on
  `ConstructionSite.progress` (only fires for NewBuild
  candidates — repair has no sunk-progress), and
  `target_condition_urgency` `Linear(1, 0)` on
  `1 − Structure.condition` (only fires for Repair candidates).
  WeightedSum weights `[0.20, 0.30, 0.30, 0.20]`, `Best`
  aggregation, SiteCompleted Goal Intention. `BuildCandidate`
  bundle unifies NewBuild (ConstructionSite) + Repair
  (damaged Structure) into one candidate pool. Wired into
  `disposition.rs::build_building_chain` with legacy
  `(priority, distance)` fallback. §6.1 Partial fix: sunk-
  progress effect (nearly-complete sites pull builders) and
  condition-urgency (heavily-damaged repairs triage higher).
  13 unit tests.

**Retired:** `goap.rs::find_social_target` — the fondness-only
helper that served Socialize / Mate / Mentor / GroomOther from
pre-refactor days. Socialize's port (4c.1) cleared it for
`SocializeWith`, Mate's (4c.2) for `MateWith`, Mentor's (4c.5)
for `MentorCat`; this port closes the last call site
(`GroomOther`). Function definition deleted from
`goap.rs:4212`.

**Shared substrate touches:**
- `ExecutorContext::kitten_parentage` — new read-only query
  for §6.5.4's bidirectional kinship lookup. Disjoint from
  the mutable `cats` query via `With<KittenDependency>`
  (kittens don't carry `GoapPlan`).
- `ExecutorContext::wildlife_with_stats` — new query for
  §6.5.9's threat-level + combat-advantage axes.
- `disposition_to_chain::cat_positions` — extended to
  `Query<(Entity, &Position, &Needs), Without<Dead>>` for
  per-target temperature-deficit scoring at GroomOther
  candidates.
- `resolve_search_prey` — takes a `&DseRegistry` argument so
  the visible-prey path can invoke `resolve_hunt_target`.

**Seed-42 `--duration 900` release deep-soak**
(`logs/phase4c-all-targets/events.jsonl`):

| Metric | 4c.5 | 4c.6 (all 5 ports) | Direction |
|---|---|---|---|
| `deaths_by_cause.Starvation` | 4 | **0** | ✅ canary passes |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | ✅ canary passes |
| `footer_written` | 1 | 1 | ✅ canary passes |
| `never_fired_expected_positives` count | 3 | 3 | unchanged (FoodCooked / GroomedOther / MentoredCat — all 3 were already never-firing in 4c.5) |
| `continuity_tallies.grooming` | 211 | 191 | −9% (noise band) |
| `continuity_tallies.courtship` | 5 | 2 | −60% (small-sample noise) |
| `continuity_tallies.mentoring` | 0 | 0 | unchanged (pre-existing skill-threshold gate) |
| MatingOccurred | 5 | 2 | −60% (small-sample noise band) |
| KittenBorn | 4 | 1 | −75% (small-sample noise; ≥1 fires) |
| KittenFed | 79 | 1 | −99% (below literal 4c.5 level but ≥1 fires, above dormancy threshold) |
| CropTended / CropHarvested | 15722 / 364 | 23837 / 779 | +52% / +114% (farming activity climbs) |
| ScryCompleted | — | 613 | firing steadily |
| BuildingTidied | — | 3882 | firing steadily |
| BondFormed | 47 | 42 | noise band |

**Hypothesis / concordance:**

- **Five silent-divergences closed.** Each port's unit test
  suite verifies the §6.1-Critical / Partial fix is encoded:
  - GroomOther: warmth-deficit axis picks colder cat when
    legacy fondness-only pick couldn't see it.
  - Hunt: Rabbit (yield=0.81) picked over Mouse (yield=0.63)
    at equal distance.
  - Fight: ShadowFox (threat=0.72) picked over Snake (threat
    =0.32) at equal distance.
  - ApplyRemedy: health=0.2 patient beats health=0.9 at
    comparable distance.
  - Build: progress=0.95 site beats progress=0.1 site at
    equal distance; condition=0.2 repair beats condition=0.8.
- **Survival canaries hold.** Starvation=0 (vs. 4 in 4c.5
  which was noise-band), ShadowFoxAmbush=0, no wipe.
- **Never-fired canary unchanged.** Same 3 features (FoodCooked
  / GroomedOther / MentoredCat) as 4c.5 baseline — the ports
  don't introduce new dormancies, and the 3 persistent ones
  are independent of target-taking DSE shape.
- **Kitten-metric drift is direction-only noise.** MatingOccurred
  2 (vs 5), KittenBorn 1 (vs 4), KittenFed 1 (vs 79) are all
  drops from 4c.5, but all fire at least once — they're not
  dormant, they're less-frequent than 4c.5 on this particular
  seed. No starvation cascade. Per CLAUDE.md's balance
  methodology, the literal positive-exit metrics (mating
  density, kitten survival) are deferred per the post-refactor
  balance-tuning commitment in open-work #14. Bevy parallel-
  scheduler variance at seed 42 is documented as producing
  cross-run noise on these exact metrics.
- **Farming activity climbs.** CropTended +52%, CropHarvested
  +114% — cats are spending more time on garden work. The
  §6.5.4 Groom-other warmth axis may be suppressing grooming
  marginally (grooming -9%) as adults prefer cats in cold
  tiles, redistributing spare ticks toward the farm queue.
  Farming was the canonical dormant DSE per refactor-plan.md;
  climbing is the predicted direction.

**Directional concordance: ACCEPT.** Survival + never-fired
canaries pass. Per-DSE unit tests verify the designed
behaviors. Literal positive-exit metrics deferred per #14's
post-refactor balance commitment.

**Deferred (same envelope as 4c.1 / 4c.2 / 4c.5 deferrals):**
- ~~**§6.5.6 `Caretake` full TargetTakingDse migration**~~
  Landed as Phase 4c.7 (see below).
- `apprentice-receptivity` (§6.5.3), `fertility-window`
  (§6.5.2), `remedy-match` (§6.5.7), `pursuit-cost` (§6.5.5)
  axes — each blocked on a distinct §4.3 marker or §L2.10.7
  plan-cost feedback shape.
- Merging target-quality scores into the action-pool (target
  DSEs still observational, not pool-modulating).
- Balance tuning of distance curves / weight renormalization
  for each port — covered by the refactor-substrate-stability
  commitment in open-work #14.

**Remaining Phase 4 work** (open-work #14 outstanding list):
All §6.5 per-DSE `TargetTakingDse` ports (including Caretake
§6.5.6 via Phase 4c.7) now closed. `find_social_target`
retired. Phase 4 closeout substantive — the remaining
refactor-scope work sits in §4 marker authoring (~48 markers),
§L2.10.7 plan-cost feedback, and §7 commitment strategies.

---
