---
id: 441
title: Decompose systems/goap.rs into a goap/ submodule per section seam
status: ready
cluster: ai-substrate
initiative: []
orchestration: substrate-sensitive
added: 2026-05-21
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

`src/systems/goap.rs` is the repo's hottest file: 10,495 LOC, 153 changes in the last 30 days, owner of the L2/L3 hot path (`evaluate_and_plan`, `resolve_goap_plans`, `dispatch_step_action`). Single-file edit-conflict surface, compile-time cost, and per-session cognitive load all scale with this number. The file already carries clear section banners (`// =====` blocks at lines ~50, 870, 1105, 1277, 1359, 3459, 5398, 8095, 8217, 8327, 8622, 9273, 9468, 9540, 9670, 10154) that mark natural module seams. Decomposing along those seams into `src/systems/goap/{mod.rs, ...}` reduces the cognitive surface without changing any behavior. Precedent: ticket 072 (`plan_substrate` module extraction) landed the same refactor shape with a bit-identical-footer gate.

## Scope

- Convert `src/systems/goap.rs` → `src/systems/goap/` directory with `mod.rs` + one file per section banner.
- `mod.rs` re-exports every previously-public symbol so callers (`src/plugins/simulation.rs` and others) need **zero edits** to import paths.
- `#[inline(never)]` annotation on `dispatch_step_action` moves verbatim into `goap/dispatch.rs`.
- Function signatures, visibilities, and bodies are preserved bit-for-bit. Pure code motion.
- Tests file (`#[cfg(test)] mod tests`) moves into `goap/tests.rs`.

## Out of scope

- Crate extraction (`crates/clowder-goap`). Deferred to a possible follow-on; current ticket stays inside the single-crate workspace.
- Step-resolver migration into `src/steps/`. The four `resolve_*` helpers in goap.rs are not `pub fn resolve_*` per the step-contract convention (`scripts/check_step_contracts.sh`); promoting them is a separate ticket.
- Dead-code retirement. Any `pub fn` that turns out to have no external caller stays `pub` for this ticket; visibility tightening is the optional Stage 3 audit.
- Behavior changes of any kind. Schedule-edge ordering, `.after(...)/.before(...)` constraints, system params, and chain composition are preserved verbatim.

## Current state

`goap.rs` ships at 10,495 LOC with section banners already in place from prior refactor passes (most recently 367 and 431). The five scheduled-from-`simulation.rs` functions are:

- `check_modifier_preemption` (~870)
- `evaluate_and_plan` (~1359)
- `resolve_goap_plans` (~3459)
- `emit_plan_narrative` (~9468)
- `check_anxiety_interrupts` (~1113, post-230 status uncertain — verify before move)

The LLVM optimization cliff is held at bay by `#[inline(never)]` on `dispatch_step_action` at line 5408 (see `docs/systems/phase-6a-commitment-gate-attempt.md` §"LLVM optimization cliff"). Module location in a single-crate build does **not** affect codegen — the annotation is what does the work and must move with the function.

## Approach

### Proposed module layout

| File | Source lines | Public exports (re-exported via `mod.rs`) |
|---|---|---|
| `goap/mod.rs` | (new) | `pub use` of everything below; nothing else |
| `goap/system_params.rs` | ~50–869 | `PreyHuntParams`, `NarrativeEmitter`, `SystemParam` bundles, `stance_overlays_from_query`, `ec_is_focal` |
| `goap/preemption.rs` | ~870–1112 | `check_modifier_preemption` *(scheduled)* |
| `goap/anxiety_interrupts.rs` | ~1113–1275 | `check_anxiety_interrupts` *(scheduled — confirm post-230)* |
| `goap/threat_context.rs` | ~1277–1358 | `evaluate_threat_context` (crate-private) |
| `goap/planner_entry.rs` | ~1359–3458 | `evaluate_and_plan` *(scheduled)* |
| `goap/executor.rs` | ~3459–5397 | `resolve_goap_plans` *(scheduled)*, `htn_advance_or_pop`, `htn_abandon_or_pop`, `StepSnapshots`, `StepAccumulators`, `MentorEffect` |
| `goap/dispatch.rs` | ~5398–8094 | `dispatch_step_action` (crate-private, **`#[inline(never)]` preserved**), `dispatch_htn_kitten_primitive`, `build_dependent_kitten_snapshot` |
| `goap/narrative.rs` | ~8095–8216, ~9468–9538 | `emit_plan_narrative` *(scheduled)*, `emit_hunt_narrative` |
| `goap/resolvers.rs` | ~8217–9272, ~9273–9467 | `resolve_travel_to`, `resolve_search_prey`, `resolve_engage_prey`, `record_hunt_attempt`, `resolve_forage_item` (all crate-private) |
| `goap/spatial.rs` | ~9539–9668 | `patrol_move`, `has_nearby_tile`, `mix_hash`, `find_nearest_tile`, `find_random_nearby_tile`, `respect_for_disposition` |
| `goap/zone_resolution.rs` | ~9669–10153 | `resolve_zone_position`, `nearest_corrupted_tile`, `build_planner_state`, `materials_available_for`, `herb_stash_accessible_for`, `classify_zone`, `build_zone_distances` |
| `goap/tests.rs` | ~10154–end | unchanged |

Line numbers above are starting estimates; the implementer re-verifies each section transition before Stage 1.

### Structural-option menu

- **split (chosen)** — pure module split inside `src/systems/goap/`. No codegen change. Annotations and signatures preserved verbatim. Module re-exports keep every existing caller path valid.
- **split with prejudice (rejected)** — extract `goap` into a workspace crate. Would create real LLVM optimization boundaries. Rejected: single-crate workspace currently, no precedent, circular-dependency risk between `goap`/`resources`/`components`/`ai/planner` unsurveyed, slower confidence-building. Possible follow-on ticket if (chosen) doesn't recover enough relief.
- **rebind (rejected)** — promote `resolve_*` helpers into `src/steps/...`. Rejected: they aren't `pub fn resolve_*` and don't carry the five-heading rustdoc contract `check_step_contracts.sh` enforces. Separate ticket.
- **retire (rejected)** — nothing inert was found in a quick scan; bundling a dead-code pass with a move pass makes review harder.

### Commit sequence

| # | Stage | What changes | Gate |
|---|---|---|---|
| 1 | **Baseline capture** | Capture a fresh `logs/tuned-42-pre441` on current main HEAD via `just soak 42` | run completes; footer present |
| 2 | **Scaffold** | Create `goap/mod.rs` + per-section files; delete `goap.rs`; `mod.rs` re-exports every previously-public symbol | `just check && just test` green; `cargo build --release` succeeds; `rg 'crate::systems::goap::' src/` returns identical symbol set |
| 3 | **Determinism gate** | (no code change — `## Log` entry recording the diff result) | run `just soak 42` post-refactor; diff `events.jsonl` body (lines 2+) against baseline byte-for-byte; **must be byte-identical**; `just verdict <run-dir>` belt-and-braces |
| 4 *(optional)* | **Visibility audit** | demote re-exports that no caller outside `goap/` actually consumes | `just check && just test` green; no scheduled symbol demoted |

Single-implementer focus. Each commit compiles and tests green; the seed-42 byte-identical check is the structural test.

## Verification

- `just check && just test` at every commit boundary.
- **Byte-for-byte seed-42 event-log diff** at Stage 3: `diff <(tail +2 logs/tuned-42-pre441/events.jsonl) <(tail +2 logs/tuned-42-post441/events.jsonl)` must be empty. (Header line 1 differs on commit hash and is excluded.)
- `just verdict <run-dir>` at Stage 3 as a belt-and-braces check.
- `rg 'crate::systems::goap::' src/` produces the same symbol set before and after.
- No new lines added to `scripts/substrate_stubs.allowlist`; no `scripts/check_*` script changes.

## Log

- 2026-05-21: opened; plan derived from session investigation into "project complexity limit" framing. The repo isn't at a limit, but `goap.rs` is the single file most likely to trigger the "small edit balloons" symptom — split is the structural mitigation.
