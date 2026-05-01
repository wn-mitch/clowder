---
id: 062
title: Prey-species split — per-species scent maps (§5.6.3 row #5)
status: ready
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

§5.6.3 row #5 calls for per-prey-species sight maps (mouse / rat /
rabbit / fish / bird). Today `PreyScentMap` is one aggregate map
across all prey species; ticket 048 landed the substrate and explicitly
deferred the per-species split.

Per-species discrimination matters for two cases the aggregate map
masks:
1. **Dietary specialization** — a cat with a bird-hunting tradition
   (or fox stalker tradition) would prefer to point itself toward
   *its* species's scent gradient, not whichever prey happens to
   smell strongest.
2. **Ward / fox-deterrent placement** — a prey-rich tile near a den
   might deserve a different ward than one that's just bird-rich.

Inherits from ticket 048's "Phase 2C — Carcass scent map landed,
per-prey-species split deferred" log entry.

## Scope

1. Replace `PreyScentMap` (single-resource aggregate) with `PreyScentMaps`
   (plural) — a new `Resource` holding `[PreyScentMap; 5]` indexed by
   `PreyKind as usize` (0=Mouse … 4=Bird). `PreyScentMap` loses its
   `Resource` derive and becomes the inner grid type used only by the
   registry. The existing bucketed-grid struct shape and methods
   (`deposit`, `decay_all`, `highest_nearby`, `get`) are unchanged.

2. `InfluenceMap` impls — the old `impl InfluenceMap for PreyScentMap`
   (aggregate) is removed. A new `PerSpeciesScentRef<'_>` newtype
   wrapping `(&PreyScentMap, PreyKind)` implements `InfluenceMap` with
   per-species metadata (`name: "prey_scent_mouse"` etc.,
   `faction: Faction::Species(SensorySpecies::Prey(kind))`). The trace
   emitter loops over all five kinds using this adapter.

3. Writer cutover in `prey_scent_tick` — query expands to
   `Query<(&PreyConfig, &Position), (With<PreyAnimal>, Without<Dead>)>`
   so `kind` is available; each deposit is routed to the correct
   sub-map via `scent_maps.deposit_for_kind(...)`. Decay is applied
   uniformly across all five sub-maps via a single
   `scent_maps.decay_all(rate)` call; `constants.prey.scent_decay_rate`
   is shared across species (per-species decay tuning is out of scope).
   The deposit amount is scaled by the prey species' scent emission
   strength from `SensoryConstants`:
   `deposit = scent_deposit_per_tick × (profile.scent.base_range / scent_deposit_normalizer)`
   where `scent_deposit_normalizer` is a new `PreyConstants` field
   (default `6.0`, matching Rat = the strongest prey scent; Mouse 5,
   Rabbit 4, Fish 5, Bird 2). This makes Bird tiles deposit at ~33% the
   rate of Rat tiles, matching the ecological profile. This is a
   **structural behavioral change** and must carry a hypothesis +
   concordance check before landing (see `## Verification`).

4. Existing aggregate consumer cutover (`Hunt` / `Hunting` DSEs in
   `disposition.rs` and `goap.rs`) — both `PreyHuntParams` structs
   replace `prey_scent_map: Res<PreyScentMap>` with
   `prey_scent_maps: Res<PreyScentMaps>`; call sites use the new
   `highest_nearby_any` / `get_any` aggregate helpers, preserving
   behaviour relative to the old single-map reads.

5. Backward-compat aggregate lens — the old `PreyScentMap` resource is
   dropped entirely (no tombstone). `PreyScentMaps` provides two
   aggregate helpers (`get_any` → `f32::max` across all five sub-maps
   at a position; `highest_nearby_any` → max-aggregate spatial scan)
   plus a single-species read (`highest_nearby_for(kind, x, y, radius)`)
   for future dietary-specialization consumers. No callers for the
   single-species variant yet; implement now as the clean hook.

6. **Sensory substrate preparation hook** — `PerSpeciesScentRef::metadata()`
   tags each map with `Faction::Species(SensorySpecies::Prey(kind))`.
   This is the key data surface that unlocks Phase 3 observer-side
   attenuation: when a cat reads `prey_scent_bird`, the Phase 3
   pipeline can eventually apply a per-emitter-species signal modifier
   distinct from the observer's own sensitivity (`species_sensitivity`
   returns a binary 1.0/0.0 gate today per the Phase 2A decision; the
   numeric `base_range` profile values drive the emitter-side deposit
   scaling in this ticket and are distinct from observer-side
   calibration). No Phase 3 changes in this ticket — the hook is
   structural, not behaviorally active beyond the deposit-scale change
   in item 3. Document this clearly in the `PerSpeciesScentRef` impl
   comment.

## Execution plan

### Step 1 — `src/resources/prey_scent_map.rs`

- Remove `#[derive(Resource)]` from `PreyScentMap`. Retain `Debug,
  Clone, serde::Serialize, serde::Deserialize`. All existing methods
  (`new`, `default_map`, `bucket_index`, `get`, `deposit`, `decay_all`,
  `highest_nearby`) are unchanged.
- Add `pub fn scent_map_name(kind: PreyKind) -> &'static str` free
  function (or inherent on `PreyKind` if that module is preferred) —
  `match kind { Mouse => "prey_scent_mouse", Rat => "prey_scent_rat",
  Rabbit => "prey_scent_rabbit", Fish => "prey_scent_fish",
  Bird => "prey_scent_bird" }`. Used by `PerSpeciesScentRef`.
- Add `pub struct PreyScentMaps { maps: [PreyScentMap; 5] }` with
  `#[derive(Resource, Debug, Clone, serde::Serialize,
  serde::Deserialize)]`.
- `impl PreyScentMaps`:
  - `pub fn new(map_w: usize, map_h: usize, bucket_size: i32) -> Self`
    — constructs five `PreyScentMap::new(...)` copies.
  - `pub fn default_maps() -> Self` — five `PreyScentMap::default_map()`
    copies (120×90, bucket_size=3). Used by setup.
  - `pub fn for_kind(&self, kind: PreyKind) -> &PreyScentMap` —
    `&self.maps[kind as usize]`.
  - `pub fn for_kind_mut(&mut self, kind: PreyKind) -> &mut PreyScentMap`
    — `&mut self.maps[kind as usize]`.
  - `pub fn get_any(&self, x: i32, y: i32) -> f32` — iterates
    `self.maps.iter()`, returns `f32::max` fold across all five
    `.get(x, y)` values.
  - `pub fn highest_nearby_any(&self, x: i32, y: i32, radius: i32)
    -> Option<(i32, i32)>` — for each candidate tile in radius, takes
    `get_any` at that tile; returns the tile with the highest value.
    Preserve the same radius-scan logic already in
    `PreyScentMap::highest_nearby`.
  - `pub fn highest_nearby_for(&self, kind: PreyKind, x: i32, y: i32,
    radius: i32) -> Option<(i32, i32)>` — delegates to
    `self.for_kind(kind).highest_nearby(x, y, radius)`. No callers yet;
    present as the dietary-specialization hook.
  - `pub fn decay_all(&mut self, decay: f32)` — calls
    `m.decay_all(decay)` for each sub-map in `self.maps.iter_mut()`.
  - `pub fn deposit_for_kind(&mut self, kind: PreyKind, x: i32, y: i32,
    base_amount: f32, sensory: &SensoryConstants, normalizer: f32)` —
    computes `emission_scale = sensory.profile_for(SensorySpecies::Prey(kind))
    .scent.base_range / normalizer.max(f32::EPSILON)`, clamps to
    `[0.0, 1.0]`, then calls
    `self.for_kind_mut(kind).deposit(x, y, base_amount * emission_scale)`.
    The tick system always calls this method; `for_kind_mut(kind).deposit(...)`
    remains available as a raw path for use in tests only.

### Step 2 — `src/systems/influence_map.rs`

- Remove the existing `impl InfluenceMap for crate::resources::PreyScentMap`
  block entirely (the `name: "prey_scent"` aggregate impl).
- Add `pub struct PerSpeciesScentRef<'a>(pub &'a PreyScentMap, pub PreyKind);`
- Add an impl-level doc comment above `PerSpeciesScentRef` explaining
  the Phase 3 readiness hook: _"The
  `Faction::Species(SensorySpecies::Prey(kind))` tag lets the
  attenuation pipeline identify which emitter species produced this
  map's signal. `species_sensitivity` returns a binary gate today
  (Phase 2A decision); Phase 3+ can apply a per-emitter-species signal
  modifier via this faction tag without changing this type's
  interface."_
- `impl InfluenceMap for PerSpeciesScentRef<'_>`:
  - `fn metadata(&self) -> MapMetadata` — returns `MapMetadata { name:
    scent_map_name(self.1), channel: ChannelKind::Scent,
    faction: Faction::Species(SensorySpecies::Prey(self.1)) }`.
  - `fn base_sample(&self, pos: Position) -> f32` — `self.0.get(pos.x,
    pos.y)`.
- Import `crate::resources::{PreyScentMap, PreyScentMaps}` and
  `crate::components::PreyKind` at the top of the impl block (or via
  `use` at file scope if not already present).

### Step 3 — `src/resources/mod.rs`

- Change `pub use prey_scent_map::PreyScentMap;` to
  `pub use prey_scent_map::{PreyScentMap, PreyScentMaps};`.
- No other changes.

### Step 4 — `src/plugins/setup.rs`

- Replace `world.insert_resource(crate::resources::PreyScentMap::default());`
  with `world.insert_resource(crate::resources::PreyScentMaps::default_maps());`.
- Confirm no other `PreyScentMap::default` call sites exist (`grep
  PreyScentMap::default` before landing).

### Step 5 — `src/systems/prey.rs` + `src/resources/sim_constants.rs`

- In `src/resources/sim_constants.rs`, add `pub scent_deposit_normalizer: f32`
  to `PreyConstants`. Default `6.0`. Doc comment:
  `"Denominator for per-species scent emission scaling. Set to the
  maximum prey scent base_range (Rat = 6.0) so Rat deposits at 1.0×
  and Bird at ~0.33×. Changing this value rescales all five emission
  strengths proportionally without touching per-species constants."`
  Wire into `impl Default for PreyConstants`.
- In `prey_scent_tick` signature:
  - Query: `Query<&Position, (With<PreyAnimal>, Without<Dead>)>`
    → `Query<(&PreyConfig, &Position), (With<PreyAnimal>, Without<Dead>)>`.
  - Resource: `ResMut<crate::resources::PreyScentMap>`
    → `ResMut<crate::resources::PreyScentMaps>`.
- Body changes:
  - `scent_map.decay_all(rate)` → `scent_maps.decay_all(rate)`.
  - Loop variable: `for pos in &prey` → `for (config, pos) in &prey`.
  - Deposit: `scent_map.deposit(pos.x, pos.y, amount)` →
    `scent_maps.deposit_for_kind(config.kind, pos.x, pos.y,
    p.scent_deposit_per_tick, &constants.sensory,
    p.scent_deposit_normalizer)`.
- Verify `PreyConfig` is already imported in this file; add import if
  missing.

### Step 6 — `src/systems/goap.rs`

- In the local `PreyHuntParams` `SystemParam`:
  - Rename field: `prey_scent_map: Res<'w, crate::resources::PreyScentMap>`
    → `prey_scent_maps: Res<'w, crate::resources::PreyScentMaps>`.
- At call site (~L4826, `resolve_search_prey`):
  - `prey_params.prey_scent_map.highest_nearby(...)` →
    `prey_params.prey_scent_maps.highest_nearby_any(...)`.
  - `prey_params.prey_scent_map.get(sx, sy)` →
    `prey_params.prey_scent_maps.get_any(sx, sy)`.
- Grep the file for any remaining `prey_scent_map` field references and
  update them before landing.

### Step 7 — `src/systems/disposition.rs`

- Identical rename to Step 6 — the `PreyHuntParams` definition here is
  independent of the one in `goap.rs`.
- At call site (~L3505, `dispatch_chain_step` Hunt branch):
  - `prey_params.prey_scent_map.highest_nearby(...)` →
    `prey_params.prey_scent_maps.highest_nearby_any(...)`.
  - `prey_params.prey_scent_map.get(sx, sy)` →
    `prey_params.prey_scent_maps.get_any(sx, sy)`.
- Grep for remaining `prey_scent_map` references in this file.

### Step 8 — `src/systems/trace_emit.rs`

- Change parameter: `prey_scent_map: Option<Res<PreyScentMap>>` →
  `prey_scent_maps: Option<Res<PreyScentMaps>>`.
- Replace the single `emit_l1_for_map` call block with a loop:
  ```
  if let Some(ref maps) = prey_scent_maps {
      for kind in [Mouse, Rat, Rabbit, Fish, Bird] {
          let adapter = PerSpeciesScentRef(maps.for_kind(kind), kind);
          emit_l1_for_map(&mut trace_log, tick, &cat_name, *pos,
                          &adapter, &constants);
      }
  }
  ```
- Add imports: `use crate::systems::influence_map::PerSpeciesScentRef;`
  and `use crate::components::PreyKind::{Mouse, Rat, Rabbit, Fish,
  Bird};` (or the fully-qualified form matching existing import style
  in the file).
- The `emit_l1_for_map` signature is unchanged — it is already generic
  over `impl InfluenceMap`, so `PerSpeciesScentRef` satisfies it
  without modification.

### Step 9 — Tests

All tests are inline (`#[cfg(test)]` mod) in the file under test.

**`src/resources/prey_scent_map.rs`** — add four unit tests:

- `test_registry_indexes_all_kinds` — create `PreyScentMaps::new(...)`,
  call `for_kind_mut(kind).deposit(x, y, 1.0)` for each of the five
  kinds at distinct `(x, y)` positions, then assert `for_kind(kind).get(x,
  y) > 0.0` for each kind and that the *other* four sub-maps read
  `0.0` at that position. Proves index mapping is injective.

- `test_get_any_returns_max` — deposit distinct amounts (0.1, 0.5, 0.9)
  into three species at the same tile; assert `get_any(x, y) == 0.9`
  (the maximum). Deposit 0.0 for the remaining two; assert `get_any`
  is unaffected by them.

- `test_highest_nearby_any` — deposit into Mouse sub-map at tile A and
  Fish sub-map at tile B where tile B has a higher value; assert
  `highest_nearby_any` returns tile B's coordinates.

- `test_highest_nearby_for_isolates_species` — same two-tile setup;
  assert `highest_nearby_for(PreyKind::Mouse, ...)` returns tile A
  (ignoring the hotter Fish tile at B), and
  `highest_nearby_for(PreyKind::Fish, ...)` returns tile B.

**`src/systems/influence_map.rs`** — add one unit test:

- `test_per_species_scent_ref_metadata` — for each of the five
  `PreyKind` variants, construct a `PerSpeciesScentRef` over a
  throw-away `PreyScentMap::new(10, 10, 1)` and call `.metadata()`;
  assert `name` equals the expected `"prey_scent_<species>"` string
  and `faction` equals
  `Faction::Species(SensorySpecies::Prey(kind))`.

## Verification

- `just check` — validates the full compile (`cargo check --all-targets`)
  plus the step-resolver contract linter (`check_step_contracts.sh`)
  and time-unit linter. Confirms that: `PreyScentMap` is no longer
  `Resource`, `PreyScentMaps` is registered, neither `PreyHuntParams`
  struct exceeds the Bevy 16-param limit, and all `resolve_*` rustdoc
  headings in `goap.rs` and `disposition.rs` are intact.

- `just test` — runs the four new tests in `prey_scent_map.rs` and the
  one new test in `influence_map.rs`; run as
  `cargo nextest run --features all -E 'test(prey_scent)'` to scope to
  just these tests before running the full suite.

- **Hypothesis filing (required before `just soak`)** — create or
  append to `docs/balance/prey-scent-emission-scaling.md`:
  ecological fact → _"Bird emits roughly ⅓ the scent signature of a
  Rat (base_range 2 vs 6)"_ → predicted direction: fewer
  scent-triggered hunt decisions in bird-heavy ecologies; predicted
  magnitude: < 10% drop in overall hunt-initiation frequency (because
  the default ecology is not bird-dominated) → observation: record
  Hunt DSE score distribution shift and scent-detect narrative event
  count from `frame-diff` and log query → concordance check: direction
  match + magnitude within ~2×. `just hypothesize` can scaffold this
  if a spec YAML is prepared.

- `just soak 42` followed immediately by `just verdict logs/tuned-42/`
  — canonical 15-min deep-soak on seed 42. Verdict must exit 0. Hard
  gates: `deaths_by_cause.Starvation == 0`,
  `deaths_by_cause.ShadowFoxAmbush <= 10`, footer line present,
  `never_fired_expected_positives == 0`. The serialized
  `events.jsonl` header will now contain `prey_scent_maps` (a
  5-element array) in place of `prey_scent_map`; confirm the new key
  is present as a smoke-check that the resource registered correctly.
  If Hunt or Hunting DSE scores drift > ±10%, the emission scaling is
  a balance change requiring full concordance check against the
  hypothesis filed above before landing.

- `just frame-diff logs/baselines/current logs/tuned-42/` — two
  checks:
  1. **Structural registry cutover (behaviour-neutral gate):** Hunt and
     Hunting DSE score distributions should drift ≤ ±5% on mean and
     p50 relative to the `highest_nearby_any` / `get_any` aggregate
     semantics vs the old single map. Larger drift indicates the
     max-aggregate changes hunt targeting in mixed-species ecologies
     and needs a separate hypothesis.
  2. **Emission-scaling concordance:** compare Hunt DSE score
     distribution and scent-detect narrative event counts against the
     hypothesis prediction (< 10% drop in hunt-initiation frequency).
     Use the jq recipe from `docs/diagnostics/log-queries.md` to
     filter `event_type == "scent_detect"` entries in the narrative
     log, or run
     `just frame-diff logs/baselines/current logs/tuned-42/ hunt_scent_detect`
     if that named filter is registered. Direction match + magnitude
     within ~2× of prediction = concordance confirmed.

- `just soak-trace 42 Simba` — inspect L1 trace records. Expected keys:
  `prey_scent_mouse`, `prey_scent_rat`, `prey_scent_rabbit`,
  `prey_scent_fish`, `prey_scent_bird` (one record per tick per
  species). The old aggregate key `prey_scent` must not appear. If
  Simba is not in a run that produces trace output, substitute another
  cat name present in the seed-42 focal-cat list.

## Out of scope

- Cross-species prey-selection scoring tuning (balance work; lives in
  the post-cutover balance thread once consumers are wired).
- Per-cat hunting-tradition memory persistence (different ticket).
- Per-species `scent_detect_threshold` tuning —
  `DispositionConstants::scent_detect_threshold` stays uniform;
  species-discriminating observer-side thresholds are Phase 3 balance
  work that follows once Phase 3's per-emitter-species attenuation
  goes live.

## Log

- 2026-04-27: opened from ticket 006 closeout. Inherits the deferral
  ticket 048 logged when carcass-scent landed.
