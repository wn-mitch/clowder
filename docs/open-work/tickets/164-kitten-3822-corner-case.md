---
id: 164
title: Seed-42 (38,22) kitten cohort starves despite KittenCryMap
status: ready
cluster: ai-substrate
added: 2026-05-03
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [mentoring-extraction.md]
landed-at: null
landed-on: null
---

<!--
Bugfix-shape ticket. Successor to 156. Investigation-first — no fix
candidates have been validated yet because the failure mode is
deterministic across all 156 iteration attempts and the cause is
not yet established.
-->

## Why

The seed-42 deep-soak still violates the CLAUDE.md hard gate
`deaths_by_cause.Starvation == 0` after ticket 156 lands the
Hearing-channel cry-broadcast architecture. Two specific kittens —
`Robinkit-33` (tick 1357232) and `Maplekit-98` (tick 1357784) —
starve at the **same map tile (38, 22)** at approximately the same
ticks across every 156 iteration attempt, including:

- pre-156 (`logs/tuned-42-pre-156-fix`): Caretake count 56,
  starvation 2 at (38,22).
- post-156 phase 4 axis attempt (`tuned-42-156-phase4-bad`):
  Caretake count 51 (regression), starvation 4 (got worse).
- post-156 phase 4-rev modifier attempt (`tuned-42-156-rev2-before-rangebump`):
  Caretake count 63 (+13%), starvation 2 (same kittens, same ticks).
- post-156 + range bump (`tuned-42-156-rangebump-bad`): Caretake
  count 51 (regression), starvation 2 (same).
- post-156 + range-bump revert (`logs/tuned-42`): Caretake 63,
  starvation 2 (same).

**Same kittens, same tile, same ticks across all five runs**
strongly suggests the failure is structural and orthogonal to the
cry-broadcast architecture 156 ships. The cry IS firing (constants
verified live in run header) and adults DO pivot to Caretake more
often globally (+13%), but no adult ever reaches (38,22) in time
to feed Robinkit-33 / Maplekit-98.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L0 substrate `KittenCryMap` | `src/resources/kitten_cry_map.rs` + `src/systems/growth.rs::update_kitten_cry_map` | Hearing-channel cry stamped per tick when `hunger < kitten_cry_hunger_threshold = 0.5`, falloff radius 30. Constants live in run header. | `[verified-correct]` |
| L1 marker `IsParentOfHungryKitten` | `src/components/markers.rs:409-413` | Parent-only fire. Robinkit-33 / Maplekit-98 may be orphans or have living parents elsewhere on the map. | `[suspect — investigate]` |
| L1 marker `Kitten` | `src/components/markers.rs:74-78` | Both kittens carry the marker for their full ~89,000-tick life. | `[verified-correct]` |
| L2 self-state DSE `CaretakeDse` | `src/ai/dses/caretake.rs` | Three-axis weighted sum + post-modifier `KittenCryCaretakeLift` lift up to +0.5 when cry perceived. Caretake count colony-wide rose 56 → 63 with 156. | `[verified-correct]` |
| L2 target-taking DSE `CaretakeTargetDse` | `src/ai/dses/caretake_target.rs` | `CARETAKE_TARGET_RANGE = 12`. (38,22) candidate-pool eligibility from any adult > 12 Manhattan tiles is filtered out. Bumping to 30 in 156 iteration was harmful — Caretake throughput dropped because adults locked onto more-distant targets that took longer to reach. | `[suspect — geometric isolation]` |
| L3 softmax | `src/ai/scoring.rs` | Caretake is in the L3 pool and wins enough to fire 63 times colony-wide. | `[verified-correct]` |
| Spawn locality | `src/systems/fertility.rs` (or wherever `KittenDependency` is added on birth) | Kittens may spawn at the parent's bed or birth-tile, which can be peripheral if the parent has wandered. | `[suspect — investigate spawn rule]` |
| Pathfinding | `src/systems/buildings.rs` / movement step resolvers | Adults that score Caretake commit to a target and walk. If (38,22) is path-blocked or terrain-isolated, the chain may abort silently. | `[suspect — investigate path]` |
| Orphan-care substrate | (none — N/A) | No `IsOrphan` marker exists today; orphans inherit no caretake-priority axis. | `[suspect — verify these specific kittens]` |

## Investigation step (do this first, before listing fix candidates)

1. **Establish parents.** Use `just q events logs/tuned-42 --kind Birth --cat Robinkit-33` (or grep `KittenDependency`-shaped events) to find the mother and father of both kittens. Are the parents alive at the death window (tick ~1357000)? If not, when did they die and where?
2. **Spawn position.** Where on the map were Robinkit-33 / Maplekit-98 born? Was the birth tile (38,22) or nearby? Did they migrate to (38,22) or were they always there?
3. **Adult clustering.** Which adults survived to year 8, and where do they cluster? `just q cat-timeline` for several survivors. Is there a colony-wide population centroid that (38,22) is far from?
4. **Caretake target audit at death window.** For every adult that scored Caretake non-trivially in ticks 1356000–1357784, dump their `CaretakeTargetDse` candidate pool. Did (38,22) appear? If yes, why did it lose? If no, was it filtered by `CARETAKE_TARGET_RANGE`, by `KITTEN_HUNGER_THRESHOLD`, or by alive-and-unreserved?
5. **Kitten hunger trajectory.** When exactly did Robinkit-33 / Maplekit-98's hunger first drop below `kitten_cry_hunger_threshold = 0.5`? At that tick, which adults were within 30 Manhattan tiles?
6. **Path reachability.** Is (38,22) reachable from the colony center via the standard movement chain? Run a manual test or check terrain at (38,22).

The investigation step decides which row in the audit moves from
`[suspect]` to `[verified-defect]`.

## Fix candidates (placeholders — finalized after investigation)

**Parameter-level:**

- R1 — extend `kitten_cry_caretake_lift` ceiling above 0.5 (e.g.,
  to 1.0–1.5) so the cry-driven Caretake score completely
  dominates competing actions for the perceiving adult cohort.
  Risk: adults pulled off Hunting / Foraging during peak hunger.

**Structural:**

- R2 (**split**) — `IsOrphan` marker authored when both parents
  are dead. Caretake target ranking gets a kinship-cliff
  override for orphans (currently kinship is 0.6 floor for
  non-parents; orphans should be 1.0). Pulls "no parent
  available" out of an inferred state into substrate.
- R3 (**extend**) — kitten spawn-locality rule: spawn near
  active-adult centroid rather than at the parent's bed
  (revisits ticket 156's R3 candidate, which was deferred to
  this ticket).
- R4 (**rebind**) — kittens with hunger > critical drift toward
  stores or toward the nearest adult, instead of staying
  stationary. Requires a kitten self-state DSE that resolves to
  a movement action (currently kittens have only Idle
  resolution).
- R5 (**rebind**) — adult Caretake target ranking weights
  `kitten_cry_perceived` strength (the loudest crying kitten in
  the candidate pool, not the closest). Pulls "loudness-as-
  priority" into the target-taking DSE, not just the self-state
  DSE.

## Recommended direction

**Defer until the investigation step is complete.** Five iterations
of 156 establish that the cry-broadcast architecture is structurally
sound but doesn't address whatever actually keeps adults away from
(38,22). The investigation step decides whether the fix is in
spawn-locality (R3), orphan-marker substrate (R2), kitten movement
(R4), target ranking (R5), or another axis entirely.

## Out of scope

- **The cry-broadcast architecture itself** is owned by ticket 156
  and shouldn't be re-litigated here.
- **The post-154 reproduction cascade** is intentional and tracked
  by `docs/balance/mentoring-extraction.md`.
- **Cross-seed sweep validation.** Same constraint as 156 — fix
  this seed-42 case first, then sweep.

## Verification

- **Hard gate:** `just soak 42` → `just verdict` reports
  `deaths_by_cause.Starvation == 0` (the canonical 156 + 158
  combined target).
- **Cascade preserved:** `kittens_born ≥ 5`,
  `kittens_surviving ≥ 3`, mentoring continuity within 2× of the
  post-154 baseline of 1614.
- **Caretake throughput preserved:** post-fix Caretake count
  ≥ 63 (the post-156 baseline) — fix must not regress 156's
  +13% improvement.

## Log

- 2026-05-03: opened in the same commit that lands ticket 156's
  partial fix. The same two kittens (Robinkit-33,
  Maplekit-98) at the same tile (38,22) starve at virtually
  identical ticks across all five 156 iteration attempts —
  the failure mode is structural and orthogonal to 156's
  cry-broadcast architecture.

- 2026-05-04: investigation completed. Spatial topology is NOT
  the failure mode (audit row "Pathfinding / Geometric isolation"
  promoted to `[verified-correct]`): adult-position scan during
  the death window (ticks 1340000-1357784) shows Mocha avg
  position (36.3, 21.1), Manhattan **2.6** from her starving
  twins; 99.1% of adult-snapshots within Manhattan 30 cry-disc;
  92.8% within Manhattan 12 `CARETAKE_TARGET_RANGE`. (38, 22) is
  at the colony heart. Both kittens are litter-mates born to
  Mocha at tick 1268164 at (38, 22); Mocha gave birth to three
  subsequent litters elsewhere and never returned to feed her
  firstborn twins. Caretake feeding chain is correct
  (`[RetrieveFoodForKitten@Stores, FeedKitten@Stores]` plan
  template; +0.5 hunger applied via deferred entity-keyed pass at
  `goap.rs:2924-2934`; position-independent on the kitten side).
  H1 verdict promoted to `[verified-defect]`: scoring gate at
  `src/ai/scoring.rs:1308` (`if ctx.hungry_kitten_urgency > 0.0`)
  filters Caretake out of L3 entirely when no kitten meets BOTH
  the per-tick range (Manhattan ≤ 12) AND hunger (< 0.6) gates
  simultaneously for the adult; the cry-lift modifier can't fire
  because of the gated-boost short-circuit at
  `modifier.rs:1649` (`if score <= 0 { return score; }`). The
  orphan marker `IsParentOfHungryKitten` (defined at
  `markers.rs:413` but with zero readers across `src/`, despite
  being load-bearing in `docs/systems/ai-substrate-refactor.md`
  §4.3) is the spec'd substrate bypass — never authored.

- 2026-05-04: structural fix written but does NOT yet land. Authored
  `update_parent_hungry_kitten_markers` in `growth.rs` (sibling
  of `update_parent_markers`); registered in `simulation.rs`
  Chain 2a; added `parent_marker_active: bool` parameter to
  `resolve_caretake_target` with own-kitten-anywhere fallback
  when the per-tick range gate excludes every candidate; threaded
  marker state through 4 production call sites in
  `disposition.rs` / `goap.rs`. 9 new unit tests (5 in `growth.rs`,
  4 in `caretake_target.rs`) all pass; existing 1833 lib tests
  still pass; full `just check` clean. Opened ticket 159
  (parent-grief consumer) and ticket 160 (substrate stub catalogue).
- 2026-05-04: post-fix seed-42 deep-soak fails the hard gate in
  an unexpected way: `Starvation == 2` remains, but the failure
  mode is totally different. **0 kittens born** (the fix's bypass
  never even fires); 6/8 adult cohort wiped by shadow-fox
  ambush in a 57k-tick window 1250-1307k; 2 surviving cats
  (Nettle, Heron) starve as orphans. Same-seed position
  comparison shows the trajectory diverges at **tick 1201300**
  with Mocha and Simba's positions swapped — well before any
  kittens exist. The new system's `&Needs` reader perturbs Bevy's
  parallel-execution order. Ticket 161 (seed-42 fox-attrition
  perturbation cascade) is the active blocker.

- 2026-05-04: structural fix lands on main alongside the 161
  investigation ticket. Code (the new
  `update_parent_hungry_kitten_markers` system + `parent_marker_active`
  fallback through `resolve_caretake_target` + four production
  call-site reads) ships in the same commit as the doc updates so
  the perturbation is reproducible from `main` while 161 is being
  diagnosed. Status stays `blocked` on 161 because the seed-42
  hard gate is still red — landing the code does not close 158's
  acceptance criteria, only stages the substrate so 161 can
  investigate from a single shared workspace state.
- 2026-05-04: ticket 161 lands the perturbation fix (merged the
  marker authoring into `update_kitten_cry_map` to avoid the new
  schedule conflict edge). Post-161 seed-42 soak: Starvation=0,
  ShadowFoxAmbush=3, kittens_born=5 — the original (38,22)
  twin-starvation hard gate now passes. Cross-seed sanity (seed
  43): Starvation=0, ShadowFoxAmbush=6. Both seeds clear the hard
  gate. Status flipped to `ready`; remaining acceptance question is
  whether `kittens_surviving=0` (vs the ≥3 target) reflects a soak
  that ended before the late-born cohort matured (kittens take 4
  seasons), or whether a separate kitten-attrition issue needs
  follow-up. Will close pending that read.

- 2026-05-04: closeout investigation halts. Renumbered 158 → 164
  to resolve the frontmatter id-collision with
  `landed/158-groomed-other-structural.md` (both carried `id: 158`,
  same `added: 2026-05-03`). Fresh `just soak 42` against current
  HEAD (`2b6b49fb6054`, post-d1722a33) is **deterministically RED**:
  `Starvation=3` (Thymekit-19 @ (24,19), Wispkit-21 + Emberkit-3 @
  (41,22)), `kittens_born=3` (down from the post-161 baseline of 5),
  `kittens_surviving=0`, `continuity_tallies.burial=0`. Verdict:
  `fail`. The pre-d1722a33 `logs/tuned-42-e9d9ac1d` reproduces the
  same 3/3/3 starvation pattern at the same tiles — the regression
  was already present in the dirty post-158 run referenced by
  `mentoring-extraction.md` Iter 2. The `kittens_surviving=0` half
  of the prior acceptance question turns out to be a double artifact
  (truncation + unimplemented metric: `colony_score.rs:56` is
  declared and `headless_io.rs:577` emits, but **zero increment-sites
  exist in `src/`** — same substrate-bypass shape as the
  `IsParentOfHungryKitten` defect this ticket itself fixed; tracked
  by ticket 166).

  Critically, the (38,22) structural fix shipped by this ticket is
  **intact on main** — verified by code-walk: `parent_marker_active:
  bool` parameter at `caretake_target.rs:215` + empty-pool fallback
  at `:257`, plus the `IsParentOfHungryKitten` author at
  `growth.rs:182,187` (inside `update_kitten_cry_map` per 161's
  merge, comment at `:124`). The starved kittens are at (24,19) and
  (41,22) — **not** the (38,22) Robinkit-33 / Maplekit-98 cohort
  this ticket targeted. So this is a *new* attrition pattern
  introduced by the GroomedOther 158 affiliative redistribution
  (mentoring-extraction.md Iter 2 records grooming 499 → 1,279, a
  +156% jump; Caretake share almost certainly fell with it). Opening
  ticket 165 to track the post-d1722a33 regression. Status stays
  `blocked` on 165 — same precedent as the original 161-block: a
  structural fix is shipped, sibling change introduces a new
  perturbation, hard-gate acceptance waits for the new defect to
  close. Will not ship a closeout against a red canonical run.
