---
id: 321
title: Aspirations milestones gain emits table + L1→L2 picker
status: ready
cluster: ai-substrate
initiative: [smarter-cats]
added: 2026-05-14
parked: null
blocked-by: []
supersedes: []
related-systems: [htn-methods.md, ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic infrastructure. Closes the loop §7.7 of the master spec
left open: aspirations are "long-horizon Intentions that emit
short-horizon Intentions," but §7.7 only names what makes an
aspiration **drop**, not what makes it **emit** a specific Goal
label at any given tick.

This ticket lands the per-milestone `emits[]` table extension on
`Milestone` and the per-tick picker system that walks each active
aspiration's current milestone, finds the first applicable Goal
label whose method exists and is live, and emits it into the L2
DSE scoring pool.

## Scope

- Extend `Milestone` shape in `src/systems/aspirations.rs` per
  [`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
  §L1→L2 emission picker:
  - Add `emits: &'static [Emit]` field.
  - `Emit` struct with `label`, `applicable_when`, `strategy`,
    `priority` (enum: `Primary | Secondary | Tertiary`).
- Per-tick picker system: walks each cat's active aspirations,
  applies the four-step contract (already-in-flight check →
  emits walk → domain-affinity fallback → silent quiet).
- Wire `IntentionSource::AspirationEmitted(AspirationId)` (the
  variant itself lands in #320, but the picker is the producer).
- Emissions enter the L2 scoring pool as `Intention::Goal`
  candidates.
- Open the new L1Aspiration trace record schema (full
  registry-walked record lands in #338; this ticket commits the
  schema + emits an empty record per tick per cat).

## Out of scope

- Filling in per-chain `emits[]` tables (those are #325-#331).
- Authoring methods that catch the emitted labels (those are
  #320 / #323-#324).
- Trace surface rendering (#338).

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #3 of 25, blocked on #319. Parallel with #320 in Batch A.
Critical-path predecessor for Batch C (chain wrappers).

## Approach

Per htn-methods.md §H. The picker is per-cat per-tick per active
aspiration. At #321 land, all chains' `emits[]` tables are empty
arrays — no emission yet. The picker silent-quiets through all
chains until #325-#331 each fill their chain's data.

Composition with §7.W axis-capture: multiple aspirations emit
per tick; L2 softmax selects; losers stay as active-but-losing
axes per §7.W.2. No mutual-exclusion enforcement at the picker
layer.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes.
- `just soak-trace 42 <focal>` produces a trace with the new
  L1Aspiration record present per tick per aspiration, all rows
  reporting `fallback_used: false` and empty emit-walk (no
  data yet).

## Log

- 2026-05-14: opened as 128 epic child #3 (Batch A infrastructure).
