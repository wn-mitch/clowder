---
id: 506
title: NearPairCache admits prey and items — 26k of 26k pairs are non-cat; social pipeline does x800 phantom work and cats accrue meaningless prey familiarity (480 child)
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-07-05
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 3e4f7caf356f
landed-on: 2026-07-05
---

## Why
A composition probe on the 505 landing found `NearPairCache.pairs`
holding **~26,000 pairs, of which ≥99.9% have at least one non-cat
endpoint** (`NEARPAIR tick_pairs=26141 noncat_pairs=26121`, seed-42,
cat-membership tested via `With<CatBeliefs>`). `update_near_pair_cache`
builds the cache from `Query<(Entity, &Position), (Without<Dead>,
Without<Structure>)>`, which admits every prey animal and every
OnGround item. Downstream, per tick: `passive_familiarity` mints
`Relationships` entries for thousands of prey/item pairs (the map
bloat that made `iter_for`/`modify_familiarity` flamegraph knives —
459/500), `track_sustained_copresence` counts prey×prey co-presence
(the 485/504 knives), threshold-crossing prey pairs emit
`SustainedCoPresence` whose prey actors become the `CatBeliefs`
ballast that kept `integrate_beliefs` at 14.1% self AFTER 505 removed
the FleeFrom leak. The entire 459→500→504→505 knife lineage was
shaving work that should not exist. The defect predates 431 (the old
O(N²) `passive_familiarity` sweep used the same permissive filter —
the 64% frame in the 2026-05-20 baseline was also mostly prey).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Cache builder | `social.rs::update_near_pair_cache` query | `(Without<Dead>, Without<Structure>)` admits prey + items + wildlife | `[verified-hostile]` (probe above) |
| Familiarity writer | `social.rs::passive_familiarity` | walks all cache pairs → cat×prey and prey×prey `Relationships` entries accrue familiarity every tick | `[verified-hostile]` |
| Social sums | `coordination.rs::social_weight`, `interoception.rs::social_status_distress` | sum `iter_for(cat)` — cat×prey entries inflate cat social graphs → **restriction is behavior-affecting**, not byte-neutral | `[verified-correct]` (mechanism) |
| Copresence | `sustained_copresence.rs` | prey pairs accumulate + emit; emitter's `Query<&Position, Without<Dead>>` position lookup succeeds for prey | `[verified-hostile]` |
| Belief ballast | `belief_integrator.rs` SustainedCoPresence arm | `cats.models.entry(*actor)` keys prey actors → 300-700 unread models per cat (post-505 probe: `cats_are_cats=7 non_cat=313..712`) | `[verified-hostile]` |
| Befriend surface | `social.rs` befriend_wildlife author | reads cat×WILDLIFE familiarity — cross-species familiarity IS a designed surface (§9.2 BefriendedAlly); wildlife must STAY in the pipeline. The `social.rs` note claiming "no production system writes familiarity for such pairs today" is factually wrong and predates this analysis | `[verified-correct]` (surface exists) |
| Kitten coverage | `setup.rs::spawn_cat_from_blueprint` (line ~139) | ALL cats incl. kittens bundle `CatBeliefs` (pregnancy.rs:114 pins kitten spawn to the blueprint) → `With<CatBeliefs>` is a safe cat discriminator | `[verified-correct]` |

## Fix candidates
- R1 (parameter) — filter pairs inside `passive_familiarity` only.
  Leaves the cache itself bloated; copresence and belief ballast
  remain. Rejected.
- R2 (**structural — restrict the substrate**) — cache admission
  filter becomes `Or<(With<CatBeliefs>, With<WildAnimal>)>`: cats
  (incl. kittens) + wildlife (preserves the §9.2 befriend surface),
  excludes prey and items. One query change; the `last_seen` diff
  self-heals any stale pairs. Debug parity query in
  `passive_familiarity` updated to match.
- R3 (retire) — N/A; the cache is load-bearing (431).

## Recommended direction
R2. Four-artifact gate (code-shape change — manual artifacts, not
`just hypothesize` which is constants_patch-shaped):
- H: removing prey/item pairs deletes phantom social substrate with
  modest, explainable social-metric drift and a large perf win.
- P1 ticks_per_sec +25–40%; P2 survival gates + continuity canaries
  hold; P3 cache holds tens of pairs, not ~26k (structural probe in a
  unit test); P4 frame-diff vs `logs/tuned-42-837b1aaf` trace: social-
  family DSE drift (bond_asymmetry / social_weight deflate), Hunt /
  Forage scoring shape stable; P5 kittens_born ≥ 1.
- O: `just soak-trace 42 Simba` + `just verdict` + `just frame-diff`.
- C: balance doc `docs/balance/near-pair-composition.md`.

## Out of scope
- Pruning pre-existing prey-pair `Relationships` entries mid-run
  (fresh soaks start clean; save-compat pruning is a follow-on if
  saved colonies matter before 0.4.0).
- Prey-side social substrate (266 prey AI reads nothing from
  Relationships).

## Verification
Unit test: cat+prey+item+wildlife world → cache holds only cat/wild
pairs. Four-artifact soak gates above. `ShadowFoxAmbush <= 10`,
`Starvation == 0` hard.

## Log
- 2026-07-05: opened from 505's post-landing flamegraph
  (integrate_beliefs unchanged at 14.1%) + composition probes.
- 2026-07-05: R2 landed: Or<(With<CatBeliefs>, With<WildAnimal>)> admission; four-artifact in docs/balance/near-pair-composition.md — tps +65.4% (67.7->112.0), zero deaths, kittens 4, all canaries; social/belief frames all below flamegraph threshold; welfare recalibration spun to 507
