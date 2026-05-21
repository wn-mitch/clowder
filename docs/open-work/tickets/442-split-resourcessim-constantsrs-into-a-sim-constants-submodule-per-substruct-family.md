---
id: 442
title: Split resources/sim_constants.rs into a sim_constants/ submodule per substruct family
status: blocked
cluster: ai-substrate
initiative: []
orchestration: substrate-sensitive
added: 2026-05-21
parked: null
blocked-by: [441]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

`src/resources/sim_constants.rs` is 7,895 LOC. The pain is the **file size**, not the **struct shape**: `SimConstants` is already domain-grouped at the type level into 47 substructs (`needs`, `buildings`, `combat`, `magic`, `social`, …), and 71 source files (404 references) read those substructs by name. Splitting the file per substruct family — each family living in `sim_constants/<family>.rs` — preserves the serialized JSON header byte-for-byte (no `#[serde(flatten)]`, no header-version bump), keeps every existing `use crate::resources::sim_constants::FooConstants;` path working via `pub use` re-exports, and reduces single-file edit-conflict and compile-time surface. Header line 1 of every `events.jsonl` carries the full `SimConstants` serialized via `serde_json::to_value` (`src/plugins/headless_io.rs:279`); preserving that shape exactly is the load-bearing invariant.

## Scope

- Convert `src/resources/sim_constants.rs` → `src/resources/sim_constants/` directory with `mod.rs` + one file per substruct family.
- Each per-family file owns the struct definition, its `Default` impl, any other `impl <Family>Constants { ... }` blocks, and the free `fn default_*` helper functions referenced from that struct's `#[serde(default = "...")]` attributes.
- `SimConstants` top-level struct remains in `mod.rs` unchanged. `mod.rs` re-exports every previously-public substruct type.
- For the two oversized families (`ScoringConstants`, `DispositionConstants` — see notes below), additionally split into `<family>.rs` + `<family>_defaults.rs` so the `struct` body and the `Default` impl + serde-default free functions live in separate files. This is the maximum decomposition possible without changing the serialized JSON shape.
- Header JSON shape **must be byte-identical** before and after.
- Field names, field order, field types, and serde rename annotations preserved exactly.

## Out of scope

- **Struct-shape changes.** Grouping leaf fields into nested substructs with `#[serde(flatten)]`, or promoting families to separate `#[derive(Resource)]`s, or bumping a `header_version` — all rejected for this ticket. These cross the comparability invariant and warrant their own dedicated tickets if ever desired. See "Further decomposition beyond this ticket" below.
- **Default-value tuning.** Any field whose default value changes — even by one digit — is a balance change requiring the four-artifact methodology, not a refactor.
- **Dead-field retirement.** A field that turns out to have zero read-sites still stays in place for this ticket.
- **`just explain` changes.** If the recipe relies on the source layout, this ticket lands a fix to it in the same commit sequence; but no feature additions.

## Current state

`sim_constants.rs` at 7,895 LOC, 47 substruct types, ~1,209 transitive leaf fields, top-level `SimConstants` with ~33 fields (each a substruct). Read-site fan-out: 71 files, 404 references across `src/plugins/`, `src/resources/`, `src/components/`, `src/steps/disposition/`, `src/steps/magic/`, `src/steps/fox/`, `src/scenarios/`. Serialization: structural via `#[derive(Serialize, Deserialize)]` on every substruct; no manual `Serialize` impls discovered in a spot check.

**Family scattering wrinkle:** each substruct family is *not* contiguous in the file. Example for `ScoringConstants`:

- `pub struct ScoringConstants { ... }` at 1582–2839
- `impl Default for ScoringConstants` at 2840+
- Free `fn default_sleep_health_deficit_midpoint() -> f32` at 3893
- Free `fn default_sleep_*` group at 4677–4689
- Free `fn default_fox_hunt_*` and `default_fox_patrol_*` group at 4697–4718
- `impl ScoringConstants` at 4813

The free `default_*` functions are referenced via `#[serde(default = "default_xyz")]` attributes inside the struct body — they're load-bearing and have to move with the struct, not be left behind in the original file. Stage 1 must inventory every `#[serde(default = "...")]` callback and move the named function into the same family file.

Blocked by **441** (goap decomposition) because (a) reviewer/implementer focus is the binding constraint and serial execution makes each ticket's bit-identical diff cleaner to verify, and (b) any unexpected interaction with the substrate-stub CI scripts surfaces in the leaner ticket first.

## Approach

### Proposed module layout

`src/resources/sim_constants.rs` → `src/resources/sim_constants/`:

| File | Contents | Notes |
|---|---|---|
| `mod.rs` | `SimConstants` top-level struct (unchanged); `pub use` re-exports of every substruct | sole entry point |
| `needs.rs` | `NeedsConstants` + Default impl + helpers | |
| `buildings.rs` | `BuildingConstants` + Default + helpers | |
| `combat.rs` | `CombatConstants`, `BodyZoneHealing` + Defaults + helpers | |
| `magic.rs` | `MagicConstants` + Default + helpers | |
| `social.rs` | `SocialConstants` + Default + helpers | |
| `mood.rs` | `MoodConstants` + Default + helpers | |
| `death.rs` | `DeathConstants`, `FounderAgeConstants` + Defaults + helpers | |
| `prey.rs` | `PreyConstants`, `SpeciesProfile`, `SpeciesConstants` + Defaults + helpers | |
| `scoring.rs` | `pub struct ScoringConstants { ... }` body | **struct-only**; ~1,255 LOC |
| `scoring_defaults.rs` | `impl Default for ScoringConstants`, `impl ScoringConstants { ... }`, all `fn default_*` helpers referenced by ScoringConstants serde attributes | ~1,250 LOC |
| `disposition.rs` | `pub struct DispositionConstants { ... }` body | **struct-only**; ~605 LOC |
| `disposition_defaults.rs` | `impl Default for DispositionConstants` + helpers | |
| `colony_score.rs` | `ColonyScoreConstants` + Default + helpers | |
| `wildlife.rs` | `WildlifeConstants`, `FoxEcologyConstants`, `HawkEcologyConstants`, `SnakeEcologyConstants` + Defaults + helpers | |
| `fate.rs` | `FateConstants` + Default + helpers | |
| `coordination.rs` | `CoordinationConstants` + Default + helpers | |
| `aspirations.rs` | `AspirationConstants` + Default + helpers | |
| `fertility.rs` | `FertilityConstants` + Default + helpers | |
| `kitten_rearing.rs` | `KittenRearingConstants`, `ParentingActivityConstants` + Defaults + helpers | |
| `crafting.rs` | `CraftingConstants` + Default + helpers | |
| `knowledge.rs` | `KnowledgeConstants`, `PersonalityFrictionConstants` + Defaults + helpers | |
| `world_gen.rs` | `WorldGenConstants` + Default + helpers | |
| `sensory.rs` | `SensoryConstants` + Default + helpers | |
| `fulfillment.rs` | `FulfillmentConstants` + Default + helpers | |
| `influence_maps.rs` | `InfluenceMapConstants` + Default + helpers | |
| `practices.rs` | `PracticeConstants`, `CourtshipPracticeConstants` + Defaults + helpers | |
| `planning_substrate.rs` | `PlanningSubstrateConstants` + Default + helpers | |
| `escape_viability.rs` | `EscapeViabilityConstants` + Default + helpers | |
| `beliefs.rs` | `BeliefAxisTunables`, `SpeciesViolencePriors`, `BeliefsConstants` + Defaults + helpers | |
| `affordances.rs` | The seven affordance substructs + Defaults + helpers | |

Family-to-file groupings above are starting estimates from `pub struct` grep output; the implementer re-verifies at Stage 1 and adjusts (e.g., if a struct in `affordances.rs` turns out to be referenced by a serde attribute on a different family, it has to live with whatever family owns that reference).

### Structural-option menu

- **(D1) Pure file split with `<family>.rs` + `<family>_defaults.rs` for the two oversized families (chosen)** — preserves serialized JSON shape exactly. Zero read-site migration. Zero risk to the header comparability invariant. Maximum decomposition feasible without crossing the invariant.
- **(D2) `#[serde(flatten)]` field grouping (rejected — premise-error)** — fields are already in substructs; nothing left to flatten that isn't.
- **(D3) `header_version: u32` bump + compatibility shim (rejected)** — would only be needed if D2 were chosen.
- **(D4) Promote families to separate `#[derive(Resource)]`s (rejected — out of scope)** — would force migration of every read-site from `Res<SimConstants>` to per-family resources; header schema changes; logdb/explain/verdict all need migration. Resources are read-only at runtime, so disjoint-mutability is not a payoff. **If we ever want this, it's a separate epic, not part of this ticket.**

### Further decomposition beyond this ticket

The `ScoringConstants` struct body (~1,255 LOC) and `DispositionConstants` struct body (~605 LOC) are the hard floors of this refactor — they can't be shrunk further without changing the serialized JSON shape. If after landing this ticket those files are still pain-causing, the natural follow-on is:

- **(potential follow-on A)** — group `ScoringConstants` leaf fields by name prefix (e.g., `sleep_*` → `SleepScoring` substruct, `fox_*` → `FoxScoring` substruct) using `#[serde(flatten)]` to preserve header JSON shape. Migrates every read-site to the new path. Substantial but bounded.
- **(potential follow-on B)** — same for `DispositionConstants`.

These are **not opened as blocked-by tickets now** — they're speculative scope per the "close the clade, don't open another follow-on" rule. Open only if real pain persists after 442 lands.

### Commit sequence

| # | Stage | What changes | Gate |
|---|---|---|---|
| 1 | **Baseline capture** | `just soak 42` on current main → `logs/tuned-42-pre442` (skip if 441's baseline is still current) | run completes; footer present |
| 2 | **Scaffold** | `sim_constants/mod.rs` + per-family files; original `sim_constants.rs` deleted; every substruct + its Default + its serde-default helpers moved verbatim into the per-family file; the two oversized families split additionally into `<family>.rs` + `<family>_defaults.rs` | `just check && just test` green; **header JSON shape diff**: `jq -c '._header.constants' logs/tuned-42-pre442/events.jsonl | head -1` vs the same on a fresh post-refactor `just headless` run must be byte-identical |
| 3 | **Determinism gate** | (no code — `## Log` entry recording the diff result) | run `just soak 42` post-refactor; diff `events.jsonl` body (lines 2+) byte-for-byte against pre-baseline; **must be byte-identical**; `just verdict <run-dir>` belt-and-braces |
| 4 *(optional)* | **`just explain` recipe fix** | only if Stage 2/3 surfaced a recipe break from the path change | recipe produces identical output for three sample constants paths before and after |
| 5 *(optional)* | **Visibility audit** | demote re-exports the rest of the codebase doesn't actually consume | `just check && just test` green |

## Verification

- `just check && just test` at every commit.
- **Header JSON shape diff** at Stage 2 — instant; structural invariant test. **Failure here means the move dropped or renamed a field — stop, investigate, do not paper over.**
- **Byte-for-byte seed-42 event-log diff** at Stage 3 — `diff <(tail +2 logs/tuned-42-pre442/events.jsonl) <(tail +2 logs/tuned-42-post442/events.jsonl)` must be empty.
- `just verdict <run-dir>` at Stage 3 — belt-and-braces.
- `just explain <constants.path>` on three sample paths (one each from `needs`, `scoring`, `coordination`) before and after — output must be substantively unchanged.
- `rg 'use crate::resources::sim_constants::' src/` returns identical results before and after.
- No new lines added to `scripts/substrate_stubs.allowlist`; no `scripts/check_*` script changes.

## Log

- 2026-05-21: opened; blocked on 441. Plan derived from same session as 441. Reframe from initial gut-call: `SimConstants` is not a god-struct (33 direct fields, already domain-grouped); it's a god-file (7,895 LOC of struct-body + Default-impls + helpers). D1 (pure file split + split struct-from-Default for the two oversized families) is the maximum decomposition feasible without crossing the `events.jsonl` header comparability invariant. Struct-shape changes (D2/D3/D4) are explicitly out of scope and would each warrant their own dedicated ticket if desired.
