---
id: 064
title: Carcass-scent consumer cutover — replace observer_smells_at (§5.6.3 row #6)
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

Ticket 048 landed `CarcassScentMap` as substrate-only — the producer
seeds the map per tick, but the consumers still use per-pair
`observer_smells_at` sensing loops. The spec (§5.6.3 row #6) calls for
the carcass-aware DSEs (`HarvestCarcass`, `CleanseCarcass`, and future
scavenging foxes) to read the map directly rather than redo the per-pair
scan.

Two motivations:
1. **Performance** — the per-pair `observer_smells_at` loop is O(cats ×
   carcasses) per tick. The map is O(cats) — one lookup per cat per
   tick.
2. **Substrate uniformity** — leaving carcass scent on a different
   sensing path than fox scent / prey scent makes the §5.6.6
   attenuation pipeline harder to reason about.

This is **balance-affecting**. The map's spatial grid resolution is
coarser than the per-pair scan's tile-exact distance check; behavior
shifts are expected. Per CLAUDE.md, drift > ±10% on a characteristic
metric requires hypothesis + soak verdict.

## Scope

This ticket adopts **Option B (full scope)**: replace both the scoring
consumer in `goap.rs` and the marker-author consumer in `sensing.rs`,
and add `carcass_scent_level` infrastructure to `FoxScoringContext` for
future scavenging DSEs. The fox Scavenging *disposition* does not exist
yet and is explicitly out of scope; the field addition here positions
that future DSE to wire directly to the map without a follow-on
substrate change.

All file paths below are relative to the repo root.

## Steps

---

### Step 1 — Add `carcass_scent_map` to `ColonyContext`

**File:** `src/systems/mod.rs`

`evaluate_and_plan` in `src/systems/goap.rs` already sits at the
Bevy 16-param limit (16 top-level system params counting all
`#[derive(SystemParam)]` bundles as one param each). Adding
`Res<CarcassScentMap>` directly to `evaluate_and_plan` would exceed
that limit. The correct home is `ColonyContext`, the existing bundle
that already holds `fox_scent_map: Res<'w, crate::resources::FoxScentMap>`.

Add one field to `ColonyContext`:

```rust
pub carcass_scent_map: Res<'w, crate::resources::CarcassScentMap>,
```

Place it immediately after the `fox_scent_map` field so the two scent
maps are grouped. The bundle's field count goes from 8 to 9 — well
under the 16-param limit that applies to the *system*, not to the
bundle internals.

`ColonyContext` is consumed by both `evaluate_and_plan` (goap.rs) and
`evaluate_dispositions` (disposition.rs), so both systems gain access
to the map through `colony.carcass_scent_map` after this step.
`CarcassScentMap` is already inserted as a world resource by the
simulation (ticket 048); no `init_resource` change is needed.

**Bevy 16-param note:** `ColonyContext` is a `#[derive(SystemParam)]`
bundle, so it counts as exactly one param toward `evaluate_and_plan`'s
16-param limit regardless of how many fields the bundle has internally.
No restructuring of either system is required.

---

### Step 2 — Replace the `nearby_carcass_count` per-pair loop in `goap.rs`

**File:** `src/systems/goap.rs`, function `evaluate_and_plan`

**Remove** the `carcass_positions` snapshot that feeds the per-pair
scoring loop (approximately lines 948–953):

```rust
// Snapshot actionable carcasses for scoring.
let carcass_positions: Vec<Position> = world_state
    .carcass_query
    .iter()
    .filter(|(c, _)| !c.cleansed || !c.harvested)
    .map(|(_, p)| *p)
    .collect();
```

**Critical distinction — two separate `carcass_query` fields:**
`WorldStateQueries.carcass_query` (the snapshot above, read-only,
used only for scoring) and `MagicResolverParams.carcass_query` (the
mutable resolver in `dispatch_step_action` that reads and mutates
`Carcass` components) live on **separate `SystemParam` structs**. Only
`WorldStateQueries.carcass_query` is touched in this step.

After removing the snapshot, remove `WorldStateQueries.carcass_query`
from the `WorldStateQueries` struct definition (lines 106–113 in the
struct body). Confirm with:

```
grep -n "carcass_query" src/systems/goap.rs
```

The only remaining hits must be under `MagicResolverParams` and inside
`dispatch_step_action`. Any hit in `WorldStateQueries` or
`evaluate_and_plan` (outside dispatch) is a residual that must be
removed.

**Remove** the stale comment block immediately above the old loop
(the ticket 014 note beginning "Ticket 014 §4 sensing batch —
`has_herbs_nearby` / ... `nearby_carcass_count` remains an inline
count..."). This rationale is now obsolete.

**Remove** the `nearby_carcass_count` per-pair loop
(approximately lines 1215–1229):

```rust
let nearby_carcass_count = carcass_positions
    .iter()
    .filter(|cp| {
        crate::systems::sensing::observer_smells_at(
            crate::components::SensorySpecies::Cat,
            *pos,
            &res.constants.sensory.cat,
            **cp,
            crate::components::SensorySignature::CARCASS,
            sc.carcass_detection_range as f32,
        )
    })
    .count();
```

**Replace** with a single O(1) map read:

```rust
// §5.6.3 row #6 cutover — reads CarcassScentMap at the cat's own
// tile. Replaces the per-pair observer_smells_at loop (ticket 064).
// carcass_detection_range is no longer consumed on this path.
let carcass_scent_level: f32 = colony.carcass_scent_map.get(pos.x, pos.y);
```

**Tests:** The `goap.rs` internal test module (`mod tests` at line 5847)
covers `find_nearest_tile` and `mix_hash` — it has no tests for the
scoring-loop path. No test changes required in this file.

---

### Step 3 — Rename `ScoringContext` field and update all construction sites

**Files:** `src/ai/scoring.rs`, `src/systems/goap.rs`,
`src/systems/disposition.rs`

Rename the field in `pub struct ScoringContext`:

```
nearby_carcass_count: usize   →   carcass_scent_level: f32
```

Update the doc comment on the renamed field:

```rust
/// Carcass scent intensity at the cat's current tile, read directly
/// from `CarcassScentMap::get(pos.x, pos.y)`. Range 0.0–1.0.
/// Replaces the per-pair `observer_smells_at` count (ticket 064).
pub carcass_scent_level: f32,
```

**Construction site 1 — `src/systems/goap.rs` (`evaluate_and_plan`):**
The local binding from Step 2 is already named `carcass_scent_level`,
so the struct literal field uses the shorthand form and requires only
a rename:

```rust
// Before
nearby_carcass_count,

// After
carcass_scent_level,
```

**Construction site 2 — `src/systems/disposition.rs`
(`evaluate_dispositions`):**

```rust
// Before
nearby_carcass_count: 0,

// After
carcass_scent_level: 0.0,
```

The adjacent comment reads "the field is unused on this code path (no
DSE input reads it; goap.rs provides the count)." Update it to:
"goap.rs provides the authoritative value via map read; this path is
currently unregistered in the schedule." Do **not** add
`Res<CarcassScentMap>` to `evaluate_dispositions` — the disposition
path does not use this scalar in any downstream DSE consideration and
is unregistered in the schedule. See Follow-on (b) for future wiring.

**Test changes — `src/ai/scoring.rs`:**
Eight `ScoringContext` struct literals in the test module carry
`nearby_carcass_count: 0`. Rename all eight to
`carcass_scent_level: 0.0`. The canonical `ctx()` helper at
approximately line 2063 is the primary site. Verify the total with:

```
grep -n "nearby_carcass_count" src/ai/scoring.rs
```

Expect exactly 8 hits, all inside `mod tests`. Update each to
`carcass_scent_level: 0.0`.

---

### Step 4 — Replace the `carcass_count_saturated` entry in `ctx_scalars`

**File:** `src/ai/scoring.rs`, function `ctx_scalars`
(approximately lines 554–558)

Remove the saturating-count block:

```rust
// Saturating-count for Harvest carcass axis — cap at 3 per the
// old inline `min(3)`.
m.insert(
    "carcass_count_saturated",
    (ctx.nearby_carcass_count.min(3) as f32) / 3.0,
);
```

Replace with a direct clamp of the new field:

```rust
// Carcass scent intensity (0.0–1.0) from CarcassScentMap — direct
// map read replaces the per-pair observer_smells_at count
// (ticket 064). Scale is already 0.0–1.0 on both sides.
m.insert(
    "carcass_scent_level",
    ctx.carcass_scent_level.clamp(0.0, 1.0),
);
```

The scalar key changes from `"carcass_count_saturated"` to
`"carcass_scent_level"`. Step 5 renames the matching key in `HarvestDse`
to keep consumer and producer names synchronized.

**Tests:** No additional test changes beyond the struct literal renames
in Step 3. The `ctx_scalars` function is exercised indirectly by the
action-scoring tests; existing tests will continue to pass because the
default test context has `carcass_scent_level: 0.0` (previously
`nearby_carcass_count: 0`), which produces the same 0.0 output for the
Harvest consideration in both forms.

---

### Step 5 — Rename the `HarvestDse` consideration key in `practice_magic.rs`

**File:** `src/ai/dses/practice_magic.rs`, `HarvestDse::new`
(approximately lines 406–409)

In the `considerations` vec, rename the scalar input key:

```rust
// Before
Consideration::Scalar(ScalarConsideration::new("carcass_count_saturated", linear())),

// After
Consideration::Scalar(ScalarConsideration::new("carcass_scent_level", linear())),
```

The `linear()` curve is unchanged — the new scalar is already 0.0–1.0,
matching the old saturated-count's output range exactly.

**Tests — `src/ai/dses/practice_magic.rs`:**
Grep for `"carcass_count_saturated"` within the file:

```
grep -n "carcass_count_saturated" src/ai/dses/practice_magic.rs
```

If any test hardcodes that key in a scalar-input map or assertion, rename
it to `"carcass_scent_level"`. The existing tests (`all_six_practice_magic_ids_stable`,
`every_practice_magic_dse_forbids_incapacitated`, etc.) do not appear
to hardcode the consideration key directly, but confirm after the rename.

---

### Step 6 — Replace the per-pair `want_carcass` scan in `sensing.rs`

**File:** `src/systems/sensing.rs`, function
`update_target_existence_markers`

**ECS param count check:** The function currently has 12 system params.
Removing `carcass_q` (−1) and adding `Res<CarcassScentMap>` (+1) keeps
the count at 12 — no `#[derive(SystemParam)]` bundling required.

**Remove** the `carcass_q` system parameter and its pre-loop
snapshot Vec.

```rust
// Remove this param:
carcass_q: Query<(&crate::components::wildlife::Carcass, &Position), Without<Dead>>,

// Remove this Vec (built from carcass_q inside the function body):
let carcass_positions: Vec<Position> = carcass_q
    .iter()
    .filter(|(c, _)| !c.cleansed || !c.harvested)
    .map(|(_, p)| *p)
    .collect();

// Remove the now-dead range binding:
let carcass_range = sc.carcass_detection_range as f32;
```

Note: `Has<markers::CarcassNearby>` in the `cats` query stays — it is
used by `toggle_target_marker` for idempotency and is unrelated to
`carcass_q`.

**Add** `Res<CarcassScentMap>` as a system parameter (position it near
`constants` for readability):

```rust
carcass_scent_map: Res<crate::resources::CarcassScentMap>,
```

**Replace** the per-pair `want_carcass` block
(approximately lines 909–920):

```rust
// Before
let want_carcass = carcass_positions.iter().any(|cp| {
    observer_smells_at(
        crate::components::SensorySpecies::Cat,
        *pos,
        cat_profile,
        *cp,
        crate::components::SensorySignature::CARCASS,
        carcass_range,
    )
});

// After — §5.6.3 row #6 cutover (ticket 064): sample the map at
// the cat's own tile. Replaces per-pair observer_smells_at scan.
let want_carcass = carcass_scent_map.get(pos.x, pos.y) > 0.0;
```

After the edit, `observer_smells_at` may no longer be called anywhere
inside `update_target_existence_markers`. The function is defined in the
same module and is public API, so its *definition* stays. Check only
whether the `use` statement at the top of the file (if any) pulls it
in; if `observer_smells_at` is still called by other functions in the
file (e.g. test helpers), leave the import unchanged.

**Test changes — `src/systems/sensing.rs` `mod tests`:**

Three targeted changes; all other carcass-adjacent tests
(`dead_cat_excluded_from_authoring`, `target_existence_markers_idempotent`,
`target_existence_markers_clear_when_target_removed`,
`dead_wildlife_excluded`) require no changes.

1. **`target_existence_setup()`** — insert the map resource so the
   system's new `Res<CarcassScentMap>` param resolves. Place it alongside
   the other `insert_resource` calls:

   ```rust
   world.insert_resource(crate::resources::CarcassScentMap::default());
   ```

2. **`carcass_in_range_flags_carcass_nearby`** — remove the
   `spawn_carcass` call; deposit scent on the resource instead. The
   cat is spawned at `(0, 0)`, so deposit at that tile:

   ```rust
   #[test]
   fn carcass_in_range_flags_carcass_nearby() {
       let (mut world, mut schedule) = target_existence_setup();
       let cat = spawn_cat(&mut world, 0, 0);
       // Deposit scent at the cat's tile so the map-threshold fires.
       world
           .resource_mut::<crate::resources::CarcassScentMap>()
           .deposit(0, 0, 0.5);
       schedule.run(&mut world);
       assert!(world
           .entity(cat)
           .contains::<crate::components::markers::CarcassNearby>());
   }
   ```

3. **`fully_processed_carcass_excluded`** — the old test spawned a
   `cleansed=true, harvested=true` carcass and checked that no marker
   fired. The new analog: a default map (all zeros) yields no marker.
   Rename the test to `no_scent_no_carcass_nearby` and rewrite:

   ```rust
   #[test]
   fn no_scent_no_carcass_nearby() {
       // Default CarcassScentMap is all-zero; no marker should fire.
       let (mut world, mut schedule) = target_existence_setup();
       let cat = spawn_cat(&mut world, 0, 0);
       schedule.run(&mut world);
       assert!(!world
           .entity(cat)
           .contains::<crate::components::markers::CarcassNearby>());
   }
   ```

   The `spawn_carcass` helper can be removed from the test module if
   no other test calls it; otherwise leave it in place.

---

### Step 7 — Add `carcass_scent_level` stub to fox scoring infrastructure

**Files:** `src/ai/fox_scoring.rs`, `src/systems/fox_goap.rs`

The fox `Scavenging` disposition does not exist (see Out of scope), but
adding the field stub now prevents a second substrate-layer change when
the disposition lands. This step is purely additive — no behavior
change, no new system param.

**`src/ai/fox_scoring.rs` — `FoxScoringContext` struct:**
Add the field after `ward_nearby` (in the "World perception" block):

```rust
/// Carcass scent intensity at the fox's current tile (0.0–1.0).
/// Populated from `CarcassScentMap::get` once a Scavenging
/// disposition exists. Wired to 0.0 until then — see follow-on
/// ticket for fox Scavenging.
pub carcass_scent_level: f32,
```

**`src/ai/fox_scoring.rs` — `fox_ctx_scalars`:**
Add the scalar entry alongside the other perception scalars:

```rust
m.insert(
    "carcass_scent_level",
    ctx.carcass_scent_level.clamp(0.0, 1.0),
);
```

**`src/systems/fox_goap.rs` — `build_scoring_context`:**
Add `carcass_scent_level: 0.0` to the `FoxScoringContext` struct
literal (the return value of `build_scoring_context`). Do **not** add
`Res<CarcassScentMap>` as a system param here — the 0.0 stub is
intentional until the Scavenging disposition exists to consume it.

**Test changes — `src/ai/fox_scoring.rs`:**
All `FoxScoringContext` struct literals in `mod tests` need
`carcass_scent_level: 0.0` added (the `default_context()` helper and
any inline construction sites). Verify the total count:

```
grep -n "FoxScoringContext {" src/ai/fox_scoring.rs
```

Update every hit with `carcass_scent_level: 0.0`.

---

### Step 8 — Dead-path lint

After all edits compile cleanly, run the following grep checks before
committing to confirm no orphan references remain:

```bash
# Must return zero hits in these files (dead old names):
grep -n "nearby_carcass_count\|carcass_count_saturated" \
    src/ai/scoring.rs \
    src/ai/dses/practice_magic.rs \
    src/systems/goap.rs \
    src/systems/disposition.rs

# WorldStateQueries.carcass_query must be gone from the struct:
grep -n "carcass_query" src/systems/goap.rs
# Expected: only MagicResolverParams.carcass_query + its dispatch_step_action
# call sites. Zero hits in WorldStateQueries or evaluate_and_plan.

# carcass_positions / carcass_range / per-pair loop must be gone:
grep -n "carcass_positions\|carcass_range\|observer_smells_at" \
    src/systems/sensing.rs
# Expected: only the observer_smells_at *definition* (pub fn) and
# any call sites in other functions (not update_target_existence_markers).
```

Any unexpected hit must be resolved before Step 9.

---

### Step 9 — Compile and unit-test

```bash
just check
just test
```

`just check` runs `cargo clippy --all-targets --all-features -D warnings`
plus the step-resolver and time-unit linters. `just test` runs
`cargo nextest run --features all`. Both must be clean before the soak.

Expected compile errors and resolutions in order:
1. **Type mismatch on `nearby_carcass_count`** (Steps 3–4): the compiler
   will flag `usize` vs `f32` at any construction site not yet updated.
   Fix with the `carcass_scent_level: 0.0` renames from Steps 3–4.
2. **Unknown field `carcass_scent_level` on `FoxScoringContext`** (Step 7):
   update every struct literal in `fox_scoring.rs` tests and
   `fox_goap.rs`.
3. **`carcass_q` / `carcass_positions` not found** (Step 6): residual
   references in sensing.rs removed per Step 8 lint.
4. **Unused import warnings**: if `observer_smells_at` is no longer
   called inside `update_target_existence_markers`, any function-level
   `use` statement for it in that scope can be removed. The public
   `fn observer_smells_at` definition in sensing.rs stays.

---

### Step 10 — Soak and verdict

```bash
just soak 42
# writes to logs/tuned-42-<timestamp>/ — never overwrite an existing dir
just verdict logs/tuned-42-<timestamp>/
```

**Hard gates (must pass regardless of hypothesis):**
- `deaths_by_cause.Starvation == 0`
- `deaths_by_cause.ShadowFoxAmbush <= 10`
- Footer line written
- `never_fired_expected_positives == 0`

**Continuity canaries (each ≥ 1 event per soak):**
`grooming` · `play` · `mentoring` · `burial` · `courtship` ·
`mythic-texture`

**Characteristic metrics to watch:**
- `magic_harvest` firing count (events log)
- `cleanse_carcass` firing count
- `CarcassNearby` marker authoring rate (from activation log)

If either harvest or cleanse count drifts **> ±10%** from the
established baseline: run `just hypothesize <spec.yaml>` to generate the
four-artifact bundle (hypothesis · prediction · observation ·
concordance), then append to the relevant `docs/balance/` file. If
drift is **> ±30%**, additional scrutiny is required regardless.

**Focal trace verification:**
```bash
just soak-trace 42 <name-of-magic-eligible-cat>
```
Inspect the L1 records for ticks where the cat scored `magic_harvest`.
`carcass_scent` should appear as a named scalar. Absence indicates
the map read is not reaching the trace emitter.

**On clean `just verdict` (exit 0):** promote the run as the new
balance baseline if the current baseline pre-dates this refactor:
```bash
just promote logs/tuned-42-<timestamp>/ carcass-scent-cutover
```

## Hypothesis

**Predicted direction:** modest **reduction** in `magic_harvest` and
`CarcassNearby` marker authoring frequency relative to the per-pair
baseline.

**Mechanism:** The old `observer_smells_at` check fired whenever any
actionable carcass lay within `carcass_detection_range` tiles of the
cat (tile-exact Manhattan distance, sensing-profile-weighted). The new
check fires when scent intensity at the cat's own 3×3 bucket is `> 0.0`.
A carcass in an *adjacent* bucket from the cat will not trigger the
marker — even if it was within detection range — unless the producer has
deposited scent that diffuses into the cat's bucket. The producer
deposits scent only at the carcass's own tile; no lateral diffusion
occurs per tick.

**Cases where the map fires but the old system would not:** None
expected. The map can only carry non-zero scent where a carcass recently
existed; recent presence there implies the carcass was in range.

**Cases where the old system fired but the map does not:** Cat and
carcass in adjacent 3-tile buckets with a bucket boundary between them.
The maximum mislocation is ~1 bucket radius (~3–4 tiles). This is the
primary reduction source.

**Magnitude estimate:** Small. Carcasses in active playthroughs are
concentrated near kill zones and the colony center, where cats also
congregate. Most cat–carcass pairs will share a bucket. A ±10% shift
would require a majority of encounters to be at bucket boundaries, which
is unlikely given 3-tile bucket size vs. typical detection ranges of
8–12 tiles.

**Prediction:** `magic_harvest` fires within ±8% of the seed-42
baseline. If a shift is observed, the direction should be downward
(fewer fire events). A shift outside ±8% but within ±10% satisfies the
gate; a shift outside ±10% triggers the full `just hypothesize` run.

**Concordance gate:** Direction match (down or flat) + magnitude within
~2× of the ±8% prediction.

## Out of scope

- **Fox `Scavenging` disposition** — `FoxDispositionKind::Scavenging`
  does not exist. Step 7 adds the field stub to `FoxScoringContext` and
  `fox_ctx_scalars` so the disposition can wire directly to the map when
  it lands, but the disposition itself, its DSE, and its GOAP chain are
  out of scope here. See Follow-on (a).
- **`wildlife.rs` store-raiding `observer_smells_at`** — the legacy
  `fox_ai_decision` path uses `observer_smells_at` with
  `SensorySignature::CARCASS` to detect *food stores*, not actual
  carcasses. This is a different signal and a different code path.
  Leave it alone.
- **Herb location map cutover** — the `HasHerbsNearby` marker still uses
  per-pair `observer_sees_at` (deferred per ticket 061 Log; activating
  the producer-side write or projecting through `HerbLocationMap.total`
  collapsed Hunting and Foraging dispositions to zero on the seed-42
  soak). Scope strictly limited to the carcass branch only.
- **`evaluate_dispositions` carcass map wiring** — `evaluate_dispositions`
  receives `carcass_scent_level: 0.0` after the Step 3 rename. Wiring
  the actual map read on this path is optional. See Follow-on (b).

## Verification

Standard three-command gate per CLAUDE.md:

```bash
just check          # clippy + step-resolver + time-unit linters
just test           # cargo nextest run --features all
just soak 42        # 15-min headless soak, seed 42
just verdict logs/tuned-42-<timestamp>/   # one-call gate
```

`just verdict` composes canaries + continuity + constants drift +
footer-vs-baseline. Exit 0 = pass · 1 = concern · 2 = fail.

If `just verdict` exits 2, drill down with:

```bash
just q run-summary logs/tuned-42-<timestamp>/
just q events      logs/tuned-42-<timestamp>/
just q narrative   logs/tuned-42-<timestamp>/
just q deaths      logs/tuned-42-<timestamp>/
```

Drift > ±10% on `magic_harvest` or `cleanse_carcass` counts requires
the four-artifact hypothesis run (`just hypothesize`) before landing.
Drift > ±30% requires additional scrutiny regardless of hypothesis
outcome. Survival canaries are hard gates regardless of drift magnitude.

After a clean `just verdict` (exit 0) with no concerning drift, promote
the soak as the new balance baseline if the current baseline pre-dates
this refactor:

```bash
just promote logs/tuned-42-<timestamp>/ carcass-scent-cutover
```

## Follow-ons

**(a) Fox `Scavenging` disposition with map-read** — The
`carcass_scent_level` field added to `FoxScoringContext` (Step 7) and
the `carcass_scent_map` resource now available on `ColonyContext` (Step
1) give a Scavenging disposition everything it needs on the substrate
side. A follow-on ticket should: define `FoxDispositionKind::Scavenging`,
wire `build_scoring_context` in `fox_goap.rs` to read
`colony.carcass_scent_map.get(fox_pos.x, fox_pos.y)` (requires adding
`Res<CarcassScentMap>` to the `fox_evaluate_and_plan` param list or
threading it through a new bundle), register the DSE, and verify that
`FoxEcologyConstants::satiation_after_scavenge` (currently defined but
unused in GOAP) is consumed by the new disposition's feed step.

**(b) `carcass_scent_level` wiring on the `disposition.rs` path** —
`evaluate_dispositions` currently hardcodes `carcass_scent_level: 0.0`
because it is unregistered in the schedule today. If it is ever
re-registered, the `carcass_scent_map` field on `ColonyContext` (Step 1)
makes the wiring trivial — one field change from `0.0` to
`colony.carcass_scent_map.get(pos.x, pos.y)`. No additional substrate
changes are needed.

## Log
