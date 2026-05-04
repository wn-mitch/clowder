---
id: 161
title: Seed-42 colony collapse after ticket 158 — Bevy scheduler perturbation cascade
status: done
cluster: ai-substrate
added: 2026-05-04
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 1ee08b8c
landed-on: 2026-05-04
---

<!--
Bugfix-shape ticket. Investigation-first — the failure mode is real
but not understood, and the user explicitly rejected the
short-circuit fix (revert the new system) in favor of a full
root-cause investigation.
-->

## Why

Ticket 158 lands the structural fix for the (38, 22) twin-starvation
bug: authors `IsParentOfHungryKitten` in
`growth.rs::update_parent_hungry_kitten_markers`, registers it in
Chain 2a after `update_parent_markers`, and adds an own-kitten-anywhere
fallback in `resolve_caretake_target`. All 9 unit tests pass. Full
`just check` clean. 1833 lib tests still pass.

But the post-fix seed-42 deep-soak (`logs/tuned-42`, commit b43bc0e
+ uncommitted 158 fix) **fails the hard gate** — `Starvation == 2`
remains, but in a totally different failure mode than ticket 158
addressed:

- **Pre-fix run** (`logs/tuned-42-pre158-b43bc0e`): 5 deaths total
  (3 ShadowFoxAmbush, 2 Starvation). Robinkit-33 and Maplekit-98
  starved at (38, 22) — the original 158 bug.
- **Post-fix run** (`logs/tuned-42`): 8 deaths total
  (6 ShadowFoxAmbush, 2 Starvation). **0 kittens born.** The 6
  ShadowFox deaths cluster in a 57k-tick window
  (Bramble at 1250152, then Calcifer/Simba/Wren/Cedar/Mocha
  in a tight bunch around 1303-1307k), wiping 6/8 of the
  starting cohort. The 2 starvations are downstream survivors
  (Nettle at 1307222, Heron at 1307349) — orphaned cats unable
  to sustain themselves after the colony lost its hunters.

**Critical: the fix's bypass logic NEVER FIRED in the post-fix run.**
With 0 kittens born, the marker never authored, the fallback never
triggered. The fix was structurally inert in this run. Yet the
trajectory diverged.

## The divergence point

Position-based same-seed comparison shows the runs diverge
**at tick 1201300** — only 1300 ticks (~1 game-day) into the
simulation, well before any kitten has been conceived. Specifically:

- Pre-fix tick 1201300: Mocha at (27, 13), Simba at (26, 12).
- Post-fix tick 1201300: Mocha at (26, 12), Simba at (27, 13).

**Same set of positions — different cat-to-position mapping.** This
is a parallel-execution-order perturbation: Bevy's scheduler put
Mocha's movement step before Simba's in the pre-fix run and the
opposite in the post-fix run. The cats took each other's destinations.

That single inversion cascades over thousands of ticks into:
1. Different patrol patterns → shadow-foxes find different
   undefended approaches.
2. Different mating routines → Mocha never gets pregnant in the
   post-fix run.
3. By tick 1303k, six adults are dead and the colony has no
   reproductive capacity.

## Diagnostic question

**Why did adding a new system that's a no-op (no kittens, no marker
authoring done, just the empty author-loop pass) shift Bevy's
parallel-execution order at tick 1201300?**

The most likely culprit: the new system's signature adds a
schedule conflict edge that didn't exist before:

```rust
pub fn update_parent_hungry_kitten_markers(
    mut commands: Commands,
    kittens: Query<(&Needs, &KittenDependency), Without<Dead>>,
    cats: Query<(Entity, Has<markers::IsParentOfHungryKitten>), (With<Species>, Without<Dead>)>,
    constants: Res<SimConstants>,
)
```

vs the sibling `update_parent_markers`:

```rust
pub fn update_parent_markers(
    mut commands: Commands,
    kittens: Query<&KittenDependency, Without<Dead>>,
    cats: Query<(Entity, Has<markers::Parent>), (With<Species>, Without<Dead>)>,
)
```

The new system reads `&Needs`. Bevy's parallel scheduler treats
`&Needs` as a shared (immutable) access; any system that takes
`&mut Needs` cannot run concurrently. This **adds a new edge in
the system-conflict graph** — pre-fix, certain `&mut Needs`
writers could run in parallel with our chain block; post-fix, they
must wait for our chain to release `&Needs`. The shift in
when-which-system-runs cascades into per-tick-decision ordering
for cat movements.

The new system being inside a `.chain()` block doesn't help —
chains enforce intra-chain order, not lock-out of parallel
non-chained sibling systems.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Schedule registration | `src/plugins/simulation.rs:354-361` (Chain 2a, after `update_parent_markers`) | `update_parent_hungry_kitten_markers` was added inside the existing `.chain()` block. Sibling parallel systems still race the chain. | `[suspect — primary]` |
| System signature | `src/systems/growth.rs::update_parent_hungry_kitten_markers` | Adds `&Needs` reader and `Res<SimConstants>` borrow that didn't exist on the sibling `update_parent_markers`. | `[suspect — primary]` |
| Bevy 0.18 parallel scheduler | (external) | Parallel scheduler uses constraint-graph topological sort. New conflict edges shift ordering. | `[verified-correct]` |
| RNG consumption order | `src/resources/...` (search for `SimRng` users) | If RNG is consumed by the new system or by re-ordered systems, RNG state diverges. **Not yet investigated.** | `[suspect]` |
| Movement step resolver | `src/steps/...` movement steps | The (Mocha, Simba) tile-swap suggests two cats targeting the same tile and a tie-break flipping. Tie-break may be RNG-dependent or order-of-iteration dependent. | `[suspect]` |
| Function modification | `src/ai/dses/caretake_target.rs::resolve_caretake_target` | Added `parent_marker_active: bool` parameter and own-kitten-anywhere fallback. **Inert** when `kittens.is_empty()` (no kittens). | `[verified-correct — inert in early ticks]` |

## Investigation steps

Strict order — each step decides whether the next is needed.

1. **Confirm the divergence is purely scheduling, not RNG.** Run the
   post-fix code WITH the system registered but with `.chain()` lifted
   to a `.before(every_other_chain_2a)` style explicit pin. If the
   trajectory matches pre-fix, scheduling is the cause. If it still
   diverges, look at RNG.
2. **Identify the conflicting writer system.** Grep for `Query<&mut
   Needs>` or `ResMut<Needs>`-like patterns in `src/systems/` and
   list every system that conflicts with the new system's `&Needs`
   reader. The set of newly-blocked-from-parallelism systems is the
   suspect set.
3. **Eliminate the `&Needs` read.** Restructure the marker authoring
   to use the same query shape as `update_parent_markers` —
   `Query<&KittenDependency, Without<Dead>>` only — and read hunger
   via a SEPARATE pre-pass that builds a HashMap<Entity, f32> of
   kitten hunger. The pre-pass can run inside an existing system
   that already reads `&Needs` on kittens (e.g., `update_kitten_cry_map`
   already does this). If this preserves the pre-fix trajectory, ship.
4. **Failing 1-3, fall back to function-only fix.** Delete the new
   system; let the fallback in `resolve_caretake_target` always run
   based on the kittens slice's parent-pointers (the function has
   all the data it needs). The marker `IsParentOfHungryKitten`
   stays orphan and rolls into ticket 160's catalogue.
5. **Cross-seed sanity check.** Whatever fix lands must run on at
   least seed 43 in addition to 42 to confirm the perturbation
   didn't simply migrate to a different seed boundary.

## Fix candidates (drafted, finalized after investigation)

**Parameter / scheduling-level**:

- R1 — Pin the system with explicit `.after(specific_system)` /
  `.before(specific_system)` constraints to fix its slot in the
  topological sort. Doesn't eliminate the conflict edges but makes
  ordering deterministic.
- R2 — Move the marker authoring INTO `update_kitten_cry_map`
  (which already reads `&Needs` on kittens — same query shape, no
  new conflict edge). Author both the cry map AND the parent-marker
  in one pass.

**Structural** (CLAUDE.md "every fix-shape includes a structural
candidate"):

- R3 (**retire**) — delete the marker authoring system. Make the
  fallback in `resolve_caretake_target` unconditional (always check
  for out-of-range own-kittens when in-range pool empty). The
  function has all the data it needs from the kittens slice; no
  marker required. The marker `IsParentOfHungryKitten` reverts to
  orphan substrate per ticket 160. **Trade-off**: the substrate-doc
  §4.3 row stays unimplemented, but with much simpler scheduling.
- R4 (**rebind**) — re-author `IsParentOfHungryKitten` as a derived
  query at the populate site (no separate authoring system at all),
  consuming `Has<markers::Parent> + Q<&KittenDependency, Without<Dead>>`
  inside the existing `disposition.rs` / `goap.rs` populate code.
  Trades scheduler purity for derived-on-demand state.

## Recommended direction

**R2 (move into `update_kitten_cry_map`)** is the cleanest
substrate-aligned fix: the cry map and the parent-marker share the
same trigger condition (kitten hunger below threshold), so authoring
them in one pass over the same query is semantically clean. No new
schedule conflict edge.

If R2 still produces divergence, fall through to R3 (function-only).

## Verification

1. **Same-seed reproducibility check**: post-fix `logs/tuned-42`
   trajectory at tick 1201300 must match pre-fix
   `logs/tuned-42-pre158-b43bc0e` exactly (Mocha at (27, 13),
   Simba at (26, 12)).
2. **Hard gate**: `just soak 42 && just verdict logs/tuned-42-<sha>`
   reports `deaths_by_cause.Starvation == 0`.
3. **Cross-seed sanity**: same gate on seed 43.
4. **Cascade preservation**: `kittens_born ≥ 5`,
   `kittens_surviving ≥ 3`, mentoring continuity within 2× of
   post-154 baseline.
5. **No new test regressions**: 1833 lib tests + the 9 ticket-158
   tests still pass.

## Out of scope

- Rewriting Bevy's parallel scheduler (obviously).
- Any change to ticket 158's function modification in
  `resolve_caretake_target` — that's correct and stays.
- Multi-seed sweep validation — defer to baseline-dataset rebuild
  once 161 lands.
- Fox-attrition / ward-coverage tuning — the post-fix collapse
  exposes a separate balance question (why does seed 42 routinely
  produce 3+ ShadowFox deaths, why does ward placement fall to 0?)
  but that's not 161's scope.

## Log

- 2026-05-04: opened in the same commit that lands ticket 158's
  structural fix. Surfaced when the post-fix soak hit
  `Starvation == 2` again with a totally different failure mode
  (colony collapse via shadow-fox attrition) and the user
  rejected the short-circuit fix (revert the new system). Same-seed
  position-comparison narrows the divergence to tick 1201300
  Mocha/Simba position-swap, before any kittens exist. Most likely
  cause: the new system's `&Needs` reader adds a schedule conflict
  edge that re-orders parallel sibling systems.
- 2026-05-04: landed R2 (merge into `update_kitten_cry_map`). The
  cry-map already reads `&Needs` on kittens with the same
  hunger-threshold predicate, so co-locating the marker authoring
  there introduces no new schedule conflict edge. Standalone
  `update_parent_hungry_kitten_markers` deleted; Chain 2a
  registration removed; doc references updated in markers.rs,
  goap.rs, disposition.rs, ai/dses/caretake_target.rs, and the
  substrate-refactor §4.3 row. Verification: `just check` clean,
  1842 lib tests pass, `just soak 42` → Starvation=0,
  ShadowFoxAmbush=3, kittens_born=5; `just soak 43` cross-seed →
  Starvation=0, ShadowFoxAmbush=6. Both hard survival gates pass on
  both seeds. (`GroomedOther` non-firing is pre-existing on both
  baselines; not 161-related.) Unblocks ticket 158.
