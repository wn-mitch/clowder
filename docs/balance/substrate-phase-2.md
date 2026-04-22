# AI Substrate Refactor — Phase 2 (L1 Influence-Map Generalization)

**Status:** partial landing — scaffolding + 4-of-5 map ports + L1
trace emitter complete. Scent-from-on-demand port (Phase 2B) remains
open; see **Deferred** below.

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

## Deferred — Phase 2B (scent port)

Scent-from-on-demand remains open. §5.6.3 row #1 describes scent as
"sparse per-emitter today; persistent bucketed grid at end-state"
with tick-for-tick parity required on migration. The hard invariant:
every read must produce a value byte-equal (or ≤ε) to what
`cat_smells_prey_windaware()` returns today, for every (observer,
target) pair the sim queries.

Porting approaches considered and their tradeoffs:

1. **Persistent bucketed scent grid (spec end-state).** Ticking
   emitters stamp templates; cats sample the grid. Requires Phase
   5.1 template machinery, emitter registry, and decay-per-tick
   system. Weeks of work; the Phase 2B invariant (tick-for-tick
   parity vs on-demand) is strictly easier than the end-state
   invariant (grid-equivalent behaviour).
2. **Borrow adapter over live queries.** `ScentLens<'a>(&'a World,
   …)` implements `InfluenceMap` by re-running the detection
   algorithm at sample time. Preserves tick-for-tick parity
   trivially (same code path), but `base_sample(pos)` returns a
   bool-as-f32, which is semantically wrong (influence maps carry
   continuous magnitude).
3. **Hybrid — typed continuous value.** Introduce a `scent_strength`
   f32 that combines emitter proximity, wind vector, and terrain
   modulation into a `[0, 1]` scalar. Trace records carry the
   scalar; detection callers compare to a threshold for the bool.
   This is what §5.6.3 ultimately wants; it requires re-authoring
   the detection formula, which is its own balance-thread work
   (detection-threshold tuning vs `cat_smells_prey_windaware`
   baseline).

Resolution: Phase 2B lands its own focused session. Open work entry
tracks the specific parity test — a same-seed, same-commit run pair
with traces diffed on `scent` L1 records and `PreyKilled` event
counts within ≤ε of the Phase 2A baseline.

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
