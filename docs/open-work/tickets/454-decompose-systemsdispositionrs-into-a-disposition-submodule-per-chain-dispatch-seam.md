---
id: 454
title: Decompose systems/disposition.rs into a disposition/ submodule per chain-dispatch seam
status: blocked
cluster: ai-substrate
initiative: []
orchestration: substrate-sensitive
added: 2026-05-23
parked: null
blocked-by: [441]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

`src/systems/disposition.rs` is the second-largest file in the repo (4,991 LOC, 35 fns, ~138 lines/fn average) and the largest outside the `goap.rs` hotspot that 441 already addresses. Four functions in this file are 500+ lines — including `dispatch_chain_step` at **1,124 lines**, the single longest function in the codebase. It owns the L2→L3→resolver bridge (`evaluate_dispositions`, `disposition_to_chain`, `resolve_disposition_chains`, `dispatch_chain_step`), making it the per-tick path companion to `goap.rs::evaluate_and_plan`. Same failure shape as 441: edit-conflict surface, compile-time cost, and per-session cognitive load all scale with the line count, and the section seams to split along already exist as the four named functions.

Same shape, same precedent (ticket 072 `plan_substrate` extraction; ticket 441 `goap.rs` decomposition). Blocked on 441 landing first so the pattern is locked in and the byte-identical-footer determinism gate is demonstrated on the harder file before reapplying to this one.

## Scope

- Convert `src/systems/disposition.rs` → `src/systems/disposition/` directory with `mod.rs` + one file per natural seam.
- `mod.rs` re-exports every previously-public symbol so callers (primarily `src/plugins/simulation.rs`) need **zero edits** to import paths.
- Function signatures, visibilities, and bodies preserved bit-for-bit. Pure code motion.
- Tests (if present as inline `#[cfg(test)] mod tests`) move into `disposition/tests.rs`.

## Out of scope

- Behavior changes of any kind. Schedule-edge ordering, `.after(...)/.before(...)` constraints, system params, and chain composition preserved verbatim.
- Crate extraction. Stay inside the single-crate workspace.
- Visibility tightening / dead-code retirement. Bundling a demote pass with a move pass makes review harder; defer to optional follow-on.
- Splitting `dispatch_chain_step` itself into smaller functions. Code motion only — the 1,124-line function moves intact into `disposition/dispatch.rs`. Function-level decomposition is a separate ticket if the LOC count proves to still be a problem post-split.
- Promoting helpers into `src/steps/`. The chain-step dispatch helpers aren't `pub fn resolve_*` per the step-contract convention (`scripts/check_step_contracts.sh`).

## Current state

`disposition.rs` ships at 4,991 LOC. The top-of-file structure follows `goap.rs`'s convention — SystemParam bundles up top, scheduled functions middle, helpers below. The natural seams (verified against the function-size scan):

| Function | Line | LOC | Role |
|---|---|---|---|
| `evaluate_dispositions` | 374 | 917 | L2 disposition scoring (scheduled in `simulation.rs`) |
| `disposition_to_chain` | 1300 | 552 | L2→chain translation (the per-disposition fan-out helper) |
| `try_crafting_sub_mode` | 2539 | 206 | crafting-specific sub-mode helper |
| `resolve_disposition_chains` | 3088 | 525 | per-tick chain execution (scheduled) |
| `dispatch_chain_step` | 3623 | 1124 | the step-by-step action dispatcher (private helper of `resolve_disposition_chains`) |

Together these five functions = 3,324 LOC = ~67% of the file. The remaining 33% is SystemParam bundles and small helpers. Line numbers above are starting estimates; the implementer re-verifies before Stage 1.

## Approach

### Proposed module layout

| File | What lives there |
|---|---|
| `disposition/mod.rs` | `pub use` re-exports; nothing else |
| `disposition/system_params.rs` | All `SystemParam` bundles (top of file, ~lines 1–373) |
| `disposition/evaluate.rs` | `evaluate_dispositions` *(scheduled)* + its private helpers |
| `disposition/to_chain.rs` | `disposition_to_chain` + the `try_crafting_sub_mode` helper |
| `disposition/resolve.rs` | `resolve_disposition_chains` *(scheduled)* |
| `disposition/dispatch.rs` | `dispatch_chain_step` (the 1,124-line function moves intact) |
| `disposition/tests.rs` | inline `#[cfg(test)] mod tests` block |

### Structural-option menu

- **split (chosen)** — pure module split inside `src/systems/disposition/`. No codegen change. Annotations and signatures preserved verbatim. Module re-exports keep every existing caller path valid.
- **split-and-decompose-dispatch (rejected for this ticket)** — split the 1,124-line `dispatch_chain_step` into per-DispositionKind sub-dispatchers in the same pass. Rejected: too much change in one ticket; the move pass is the structural test (byte-identical determinism), function decomposition is judgment-laden refactor. Reconsider as a follow-on once the move is landed and we can see whether the 1,124-line function still feels load-bearing.
- **rebind (rejected)** — promote helpers into `src/steps/...`. Same reason as 441: not `pub fn resolve_*` per the step-contract convention.
- **retire (rejected)** — nothing inert was found in the audit scan; bundling a dead-code pass with a move pass makes review harder.

### Commit sequence

| # | Stage | What changes | Gate |
|---|---|---|---|
| 1 | **Baseline capture** | After 441 lands, capture a fresh `logs/tuned-42-pre454` on current main HEAD via `just soak 42` | run completes; footer present |
| 2 | **Scaffold** | Create `disposition/mod.rs` + per-section files; delete `disposition.rs`; `mod.rs` re-exports every previously-public symbol | `just check && just test` green; `cargo build --release` succeeds; `rg 'crate::systems::disposition::' src/` returns identical symbol set |
| 3 | **Determinism gate** | (no code change — `## Log` entry recording the diff result) | run `just soak 42` post-refactor; diff `events.jsonl` body (lines 2+) against baseline byte-for-byte; **must be byte-identical**; `just verdict <run-dir>` belt-and-braces |
| 4 *(optional)* | **Visibility audit** | demote re-exports that no caller outside `disposition/` actually consumes | `just check && just test` green; no scheduled symbol demoted |

## Verification

- `just check && just test` at every commit boundary.
- **Byte-for-byte seed-42 event-log diff** at Stage 3: `diff <(tail +2 logs/tuned-42-pre454/events.jsonl) <(tail +2 logs/tuned-42-post454/events.jsonl)` must be empty. (Header line 1 differs on commit hash and is excluded.)
- `just verdict <run-dir>` at Stage 3 as a belt-and-braces check.
- `rg 'crate::systems::disposition::' src/` produces the same symbol set before and after.
- No new lines added to `scripts/substrate_stubs.allowlist`; no `scripts/check_*` script changes.

## Log

- 2026-05-23: opened as sibling of 441. Audit surfaced `disposition.rs` as the second-largest file in the repo and the home of the longest function (`dispatch_chain_step` at 1,124 lines). Blocked on 441 landing first so the byte-identical determinism gate is demonstrated on the harder file before reapplying here.
