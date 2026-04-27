---
id: 048
title: Phase 2C — CarcassScentMap, the 6th §5.6.3 influence map
status: in-progress
cluster: null
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Spec §5.6.3 row #6 commits a `Carcass-location` influence map on
(`Channel::Scent`, `Faction::Neutral`) backing `ScoringContext`'s
`carcass_nearby` and `nearby_carcass_count` fields. Today these are
populated via per-pair `observer_smells_at` ECS iteration in
`src/systems/goap.rs:1133–1145`. There is no shared persistent map.

§5 of the AI substrate refactor (`docs/systems/ai-substrate-refactor.md`)
phased the implementation:

- **Phase 2A** — substrate scaffolding (`src/systems/influence_map.rs` —
  `InfluenceMap` trait, `Channel`/`Faction` enums, `MapMetadata`,
  `Attenuation` pipeline, six concrete impls). Landed.
- **Phase 2B (commit `4afae5d`, 2026-04-20)** — Scent migration off the
  on-demand `wind.rs` + `sensing.rs` pattern; `PreyScentMap` resource
  added; `SpatialConsideration` consumer wired. Landed.
- **Phase 2C** — register the absent §5.6.3 maps starting with
  `CarcassScentMap`. This ticket.

This brick is the proven Phase 2B recipe applied to the carcass slot:
bucketed `f32` grid, per-tick deposit + global decay, `InfluenceMap`
impl, `SimConstants` knobs, trace-emitter registration. The recipe is
the same as `PreyScentMap` (`src/resources/prey_scent_map.rs`,
`prey_scent_tick` in `src/systems/prey.rs:541+`).

## Approach

**Substrate-only landing** (this ticket): ship the deposit + decay +
trace registration. Consumer reads in `goap.rs:1133–1145` continue to
use the existing `observer_smells_at` per-pair detection. The map is
present and traceable but does not yet drive scoring — zero balance
impact.

**Consumer cutover** is a follow-on (next ticket or §6 target-taking
work): replace `observer_smells_at` reads with
`carcass_scent_map.get(pos)` for spatial signal continuity. That
landing is balance-affecting (predicted shift on `CarcassHarvested`)
and gets its own four-artifact soak per `CLAUDE.md`.

Splitting the work this way:
1. Lands the substrate brick fast and cleanly.
2. Decouples the structural change from the balance shift.
3. Validates the deposit/decay shape on its own (trace records will
   show the map populating) before the consumer cutover commits.

## Files

- **New:** `src/resources/carcass_scent_map.rs` — mirror of
  `src/resources/prey_scent_map.rs` (bucketed grid, `deposit` /
  `decay_all` / `get` / `highest_nearby`).
- `src/resources/mod.rs` — export `CarcassScentMap`.
- `src/resources/sim_constants.rs` — add
  `carcass_scent_deposit_per_tick` and `carcass_scent_decay_rate`
  fields on `WildlifeConstants`.
- `src/systems/wildlife.rs` — new `carcass_scent_tick` system (next
  to `carcass_decay`) — deposits scent for actionable carcasses
  (`!c.cleansed || !c.harvested`, matching the existing
  `goap.rs:840–846` filter), applies global decay.
- `src/plugins/simulation.rs` — register `carcass_scent_tick` in the
  wildlife update sub-chain.
- `src/plugins/setup.rs` — insert `CarcassScentMap::default()`
  alongside the other scent-map resources at the existing site
  (line ~331).
- `src/systems/influence_map.rs` — `impl InfluenceMap for
  CarcassScentMap` (channel: Scent, faction: Neutral, name:
  `"carcass_scent"`).
- `src/systems/trace_emit.rs` — add `Option<Res<CarcassScentMap>>`
  param + 7th `emit_l1_for_map` call.

## Acceptance

- `just check` clean (`check_step_contracts.sh` / typecheck).
- `just test` clean — at least 4 unit tests on the new map (deposit,
  decay, out-of-bounds, highest-nearby), mirroring `PreyScentMap`'s
  test suite.
- `just soak 42` + `just verdict logs/tuned-42-<sha>/` — survival
  canaries hold (no behavior change expected since consumer reads
  still go through entity iteration; the map is observed-only).
- `just q trace logs/tuned-42-<sha>/` — `carcass_scent` L1 record
  appears per tick alongside the other six maps.

## Out of scope

- Consumer cutover (`goap.rs:1133–1145` from per-pair detection to map
  sample) — separate balance-affecting follow-on.
- Phase 2D dynamic-registry refactor of `trace_emit.rs:120+` — also
  separate.
- Per-prey-species split of `PreyScentMap` (#5).
- Corruption full migration off `CorruptionLens` borrow adapter.
- Wind-vector / terrain modulation on the deposit kernel — Phase 2B
  ships uniform deposit; carcass mirrors that simplification.

## Log

- 2026-04-27 added.
- 2026-04-27 implementation complete; ready to commit.

  **Files landed (uncommitted):**
  - `src/resources/carcass_scent_map.rs` (new, 6 unit tests)
  - `src/resources/mod.rs` — export
  - `src/resources/sim_constants.rs` — `WildlifeConstants` gains
    `carcass_scent_deposit_per_tick` (0.1) and
    `carcass_scent_decay_rate` (`RatePerDay::new(0.5)`), both
    `#[serde(default)]` for header back-compat
  - `src/systems/wildlife.rs::carcass_scent_tick` (new) — actionable
    filter `!cleansed || !harvested`, decay-then-deposit ordering
  - `src/plugins/simulation.rs` — registers `carcass_scent_tick` in
    the wildlife sub-chain
  - `src/plugins/setup.rs` — inserts `CarcassScentMap::default()`
  - `src/systems/influence_map.rs::impl InfluenceMap for
    CarcassScentMap` — name `"carcass_scent"`, channel `Scent`,
    faction `Neutral`
  - `src/systems/trace_emit.rs` — adds 7th `emit_l1_for_map` call;
    docstring updated to reflect the new walk shape
  - `docs/wiki/*.md` regenerated via `just wiki`
  - `docs/open-work.md` regenerated via `just open-work-index`

  **Verification:**
  - `just check` — clean (cargo check, clippy `-D warnings`,
    step-contract grep, time-units).
  - `cargo test --lib` — 1359 / 1359 passed (6 new
    `carcass_scent_map` tests).
  - `just soak 42 → logs/tuned-42/` — completed; canaries match the
    immediate-prior post-045 commit one-to-one (`Starvation == 0`,
    `ShadowFoxAmbush == 4 ≤ 10`, footer written, never-fired list
    identical: `[FoodCooked, MatingOccurred, GroomedOther,
    MentoredCat, CourtshipInteraction]`). The `verdict` "fail"
    status is the pre-existing condition the ticket-045 landing
    explicitly anticipated; the registered baseline is at commit
    `a879f43` (pre-043+044), so verdict's drift comparison crosses
    multiple landed changes.
  - **Trace verification** — 30-second focal-cat headless run
    (`--focal-cat Simba --duration 30`) emitted **5770**
    `carcass_scent` L1 records alongside the existing six maps;
    schema includes correct attenuation (`species_sens=1.0` for Cat
    × Scent), faction `neutral`, channel `scent`, name
    `carcass_scent`. Wiring is correct.

  **Net behavioral delta:** zero. The map populates and is
  observable in traces but no scoring code consumes it yet.
  `goap.rs:1133–1145` still resolves `nearby_carcass_count` via
  per-pair `observer_smells_at`. The §6.3 cutover (consumer of the
  map for `magic_harvest` and `carcass_nearby` axes) is the
  follow-on; that landing will be balance-affecting and gets its
  own four-artifact soak.

  **Footprint:** ~1 LOC system registration, 1 LOC resource
  insertion, 1 LOC trace walk, ~155 LOC new resource module
  (mostly clone of `prey_scent_map.rs`), 16 LOC new system, 17 LOC
  new InfluenceMap impl, 14 LOC new constants + defaults. Total
  net new ~210 LOC, almost entirely test-covered or copy-of-known-
  pattern.
