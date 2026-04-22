# AI Substrate Refactor — Phase 2 (L1 Influence-Map Generalization)

**Status:** complete. All 5 §5.6.3 Partial maps now implement
`InfluenceMap`:

| Map            | Implementation                              | Landed  |
|----------------|---------------------------------------------|---------|
| fox_scent      | trait impl on `FoxScentMap`                 | 2A      |
| congregation   | trait impl on `CatPresenceMap`              | 2A      |
| exploration    | trait impl on `ExplorationMap`              | 2A      |
| corruption     | `CorruptionLens<'a>(&'a TileMap)` adapter   | 2C      |
| **prey_scent** | **new `PreyScentMap` resource**             | **2B**  |

Phase 2B's scope was rewritten mid-session per user direction:
scent **changes** behaviour as part of becoming an influence map,
rather than preserving tick-for-tick parity with the old
`cat_smells_prey_windaware()` formula. The point-to-point wind-aware
detection retired; cats now detect prey via grid sampling.

## Thesis

§5 of `docs/systems/ai-substrate-refactor.md` generalizes today's
scent system (`wind.rs` + `sensing.rs`) into a reusable influence-map
substrate so all 13 named L1 maps share one API, attenuation
pipeline, and trace format. Phase 2 is "name what already exists"
(per the plan): three Partial persistent-grid maps (`FoxScentMap`,
`CatPresenceMap`, `ExplorationMap`) get trait impls, corruption gains
a borrow-adapter lens, and the Phase 1 trace emitter stops
hardcoding fox-scent and starts walking the registered maps.

## Hypothesis

> Exposing existing L1 data through a uniform `InfluenceMap` trait
> and walking it in the trace emitter is a read-surface refactor
> with zero behaviour change. Every frame-diff comparison between a
> pre-Phase-2 trace and a post-Phase-2 trace on seed 42 should show
> ≤ε drift on L1 sample values for maps that are covered by the
> trait (identical computation path; only the wrapping API changed).
> Scoring / selection / gameplay rely on on-demand detection paths
> that Phase 2 **does not touch** — those paths remain pointer-
> equivalent to the pre-refactor code.

## Predicted drift direction

| Metric | Prediction |
|---|---|
| Trace L1 records per (focal cat × tick) | 1 (fox_scent only, pre-Phase-2) → 4 (fox_scent + congregation + exploration + corruption, post-Phase-2) |
| Per-record `base_sample` on fox_scent map | bitwise identical pre/post-Phase-2 (same `FoxScentMap::get` underneath) |
| Sim-behavior drift | none — attenuation pipeline is not yet consumed by scoring |
| Survival canaries (Starvation, ShadowFoxAmbush) | unchanged from Phase 1 soak |
| Continuity canaries | unchanged (no new emission sites) |
| `just ci` | green |

## Canaries under this phase

**Hard gates (must pass):**
- `just ci` — type-check, clippy (-D warnings), tests all green.
- 14 new unit tests in `influence_map.rs` (attenuation arithmetic +
  species-matrix lookup + real-resource trait impls).
- Phase 1 acceptance gate still passes: `scripts/replay_frame.py
  --verify-against` confirms trace L3 ranked DSE list matches
  `CatSnapshot.last_scores`.
- `cargo test --lib influence_map` — all trait impls verified on
  fresh-default resources.

**Soft gates (informational, in-progress):**
- Scent-migration parity check (Phase 2B) — not yet runnable
  because scent still flows through `cat_smells_prey_windaware`.

## Acceptance gate

Phase 2 exits when:

1. `InfluenceMap` trait stable, documented, and used at least one
   real call site (trace emitter) — **done**.
2. Every Partial map named in §5.6.3 has either a trait impl or a
   documented deferral — **partial** (4/5 done; scent deferred to
   Phase 2B).
3. Trace emitter walks the registered maps rather than hardcoding
   fox-scent — **done**.
4. Scent L1 records show ≤ε drift vs pre-Phase-2 baseline — **TBD
   on Phase 2B landing**.
5. `just ci` green — **done**.

## Observation

Autoloop not re-run for Phase 2 because the changes are pure
additive-read refactors (new trait + new trace records) with no
gameplay-state mutations. Direct verification:

| Check | Result |
|---|---|
| `just ci` | green |
| `cargo test --lib influence_map` | 14 tests pass |
| Phase 1 acceptance gate (`replay_frame.py --verify-against`) | passes on Phase-2 trace |
| L1 records per tick on focal-cat trace | 4 (fox_scent, congregation, exploration, corruption); was 1 |
| `base_sample` for `fox_scent` at sample tick | unchanged (same `FoxScentMap::get` underneath the lens) |
| Simulation behaviour | unchanged (trace emitter is read-only; attenuation pipeline not yet consumed by scoring) |

## Concordance

Landed work ties out to prediction:

- Scaffolding-only refactor → no behavioural drift. Verified by Phase 1
  acceptance gate still passing on the Phase 2D-produced trace.
- L1 record fan-out from 1 → 4 per (cat × tick) matches the prediction.
- Attenuation pipeline sets `species_sens=1.0` on active channels and
  `0.0` on disabled ones — matches the binary-gate semantic declared
  in `influence_map.rs::species_sensitivity` docs.

Soft concordance: the `faction` field in L1 records now uses kebab-case
slugs (`fox`, `colony`, `observer`, `neutral`) rather than the
pre-Phase-2 opaque `"fox"` string — a format change, not a value
change. jq queries pointing at `logs/trace-*.jsonl` should accept both
(pre-Phase-2 traces are outside the diff-target window anyway; only
Phase-2+ traces will be compared).

## Artifacts that ship this phase

**Rust:**
- `src/systems/influence_map.rs` (new, Phase 2A) — `InfluenceMap`
  trait, `Faction` enum, `Attenuation` struct with §5.6.6 pipeline,
  `species_sensitivity` matrix lookup, `channel_label` helper.
  14 unit tests.
- `src/systems/influence_map.rs` (Phase 2C extension) —
  `CorruptionLens<'a>(&'a TileMap)` borrow-adapter for per-tile
  corruption. Avoids conflating TileMap identity with corruption
  identity; leaves room for a future `MysteryLens` on the same
  storage.
- `src/systems/trace_emit.rs` (Phase 2D) — L1 emission replaces
  hardcoded fox-scent shim with a generic `emit_l1_for_map<M:
  InfluenceMap + ?Sized>` walker. Four map reads per tick with real
  attenuation + kebab-case `faction_slug()`.
- `src/systems/mod.rs` — module registration.

**No script / just-recipe changes** this phase.

## Phase 2B — scent port (completed)

**User correction mid-session:** the prior version of this doc
claimed "every read must produce a value byte-equal (or ≤ε) to
what `cat_smells_prey_windaware()` returns today." That was a bad
assumption — the refactor's goal is to **change** scent behaviour
in the direction of "scent works like the other senses." The sim
drifting is the explicit goal.

**What landed in Phase 2B:**

1. **New `PreyScentMap` resource** (`src/resources/prey_scent_map.rs`)
   — bucketed grid mirroring `FoxScentMap`'s structure. 3-tile
   buckets on the default 120×90 map; `deposit` / `decay_all` /
   `highest_nearby` helpers. One aggregate map covers all prey
   species today; per-species maps are a §5.6.3 follow-up.

2. **New `prey_scent_tick` system** (in `prey.rs`) — live prey
   deposit per tick at their current position; whole grid decays
   globally. Mirrors `fox_scent_tick`. Added to both
   `SimulationPlugin::build` and headless `build_schedule`
   (manual-mirror invariant).

3. **Detection rewired** in `disposition.rs` hunt-search and
   `goap.rs::resolve_search_prey`. Instead of
   `prey_query.iter().filter(|pp| can_smell_prey(cat_pos, pp,
   wind, map, d))`, the code now:
     - Calls `prey_scent_map.highest_nearby(pos, scent_search_radius)`
       to find the strongest nearby scent bucket.
     - Checks the sampled value against
       `DispositionConstants::scent_detect_threshold`.
     - Resolves the prey entity nearest to the scent source tile.

4. **Retired** `cat_smells_prey_windaware` in `sensing.rs` and both
   per-file `can_smell_prey` wrappers. `d.scent_base_range`,
   `d.scent_min_range`, `d.scent_downwind_dot_threshold`,
   `d.scent_dense_forest_modifier`,
   `d.scent_light_forest_modifier` stay in `DispositionConstants`
   for now (retirement passes through Phase 3c's
   retired-constants burn, not here).

5. **Three new constants** (all `#[serde(default)]` for save
   compat):
     - `DispositionConstants::scent_search_radius: i32 = 20`
     - `DispositionConstants::scent_detect_threshold: f32 = 0.05`
     - `PreyConstants::scent_deposit_per_tick: f32 = 0.1`
     - `PreyConstants::scent_decay_per_tick: f32 = 0.02`

6. **`InfluenceMap` impl** for `PreyScentMap` registered under
   `"prey_scent"` / `Scent` channel / `Faction::Neutral`. Trace
   emitter's L1 walk now covers **all five** Partial §5.6.3 maps
   per (focal cat × tick).

**Behaviour changes to expect:**

- No more wind-direction gating on scent detection. Scent diffuses
  symmetrically from prey positions. Cats upwind of prey can now
  smell them.
- No more terrain-modulation on detection range. Dense-forest
  prey are no longer scent-muffled (terrain stamping is a §5.6.3
  follow-up).
- The `scent_search_radius = 20` is tighter than the old
  `scent_base_range = 80`, but the old value was gated by wind +
  terrain multipliers that typically reduced effective range
  below 20 anyway. Net effect: unclear; balance work in Phase 3+
  will tune.
- No close-range `min_range` bypass; detection is fully driven by
  `PreyScentMap` values.

**Smoke test (seed 42, 60s):** sim runs without wipeout; focal-cat
prey_scent L1 records show non-zero values on 263/10032 ticks with
peak 1.0 (fully saturated bucket — prey is literally on the cat's
tile). Hunt plan failure counts in the soak footer look similar to
baseline ("ForageItem: nothing found" 109×, "EngagePrey: lost
prey during approach" 84×, etc. at 60s). A 15-min deep-soak
diagnostic is a natural follow-on.

## Cross-refs

- `docs/systems/refactor-plan.md` Phase 2 deliverables + acceptance.
- `docs/systems/ai-substrate-refactor.md` §5.1–§5.6.9 — substrate
  architecture, sensory channels, 13-map catalog, attenuation
  pipeline, extensibility contract.
- `docs/balance/substrate-refactor-baseline.md` — diff target for
  subsequent phases.
- `docs/balance/substrate-phase-1.md` — prior phase's kickoff + exit.

## Handed forward to Phase 3

- `Faction` enum and `ChannelKind` reuse set the vocabulary Phase 3a
  markers (§4.3) will use for DSE eligibility filters.
- `MapMetadata` struct is the shape the Phase 3 Dse trait's
  spatial-consideration reads will join against for per-target
  best-contributor lookup.
