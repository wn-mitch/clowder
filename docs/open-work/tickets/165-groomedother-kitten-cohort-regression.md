---
id: 165
title: Post-d1722a33 GroomedOther affiliative redistribution starves entire kitten cohort on seed-42
status: ready
cluster: ai-substrate
added: 2026-05-04
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [mentoring-extraction.md]
landed-at: null
landed-on: null
---

<!--
Bugfix-shape ticket. Investigation-first — fix candidates are
placeholders pending the layer-walk and a scenario-microexperiment.
This ticket BLOCKS ticket 164 (kitten-3822 corner case) — 164's
structural fix is intact on main, but 164's seed-42 hard-gate
acceptance can't pass while this regression is live.
-->

## Why

The post-d1722a33 (GroomedOther 158 — `feat: 158 — split Action::Groom +
extract DispositionKind::Grooming`) seed-42 deep-soak deterministically
violates the CLAUDE.md hard gate `deaths_by_cause.Starvation == 0`:

| Field | Pre-d1722a33 (`tuned-42-pre158`) | Post-d1722a33 (`tuned-42`, header `2b6b49fb`) |
|---|---|---|
| `kittens_born` | 5 | **3** |
| `deaths_by_cause.Starvation` | 0 | **3** (all kittens, 100% mortality) |
| `continuity_tallies.grooming` | 499 | **1,279** (+156%) |
| `continuity_tallies.mentoring` | 445 | 165 (-63%) |
| `continuity_tallies.courtship` | 3,613 | 1,330 (-63%) |

Reproduction: `just soak 42` against current HEAD `2b6b49fb` produced
identical numbers (deterministic). Both `logs/tuned-42-e9d9ac1d` (the
pre-rerun artifact, dirty post-158) and `logs/tuned-42` (fresh against
`2b6b49fb`) show the same three starvations:

- tick 1,309,257 — Thymekit-19 at (24, 19)
- tick 1,309,386 — Wispkit-21 at (41, 22)
- tick 1,309,441 — Emberkit-3 at (41, 22)

These are NOT the (38,22) Robinkit-33 / Maplekit-98 cohort that ticket 164
fixed structurally. The (164) `IsParentOfHungryKitten` author + empty-pool
fallback in `resolve_caretake_target` are still on main (verified by
code-walk: `src/ai/dses/caretake_target.rs:215,257` +
`src/systems/growth.rs:124,182,187`). What changed is the affiliative
time-share: GroomedOther became a first-class peer affiliative action
(per ticket 158-Groom), and `mentoring-extraction.md` Iter 2 documents the
+156% grooming jump — but the doc never records what happened to Caretake
or to kitten-feeding throughput.

Hypothesis: the L3 softmax now distributes affiliative time across
{`SocializeWith`, `GroomOther`, `Mentor`, `Caretake`} where it previously
distributed across {`SocializeWith`, `Mentor`, `Caretake`}. With one extra
peer competitor and `GroomOther` now strongly contesting (since the
pre-158 A* prune at `planner/mod.rs:437` no longer fires), Caretake's
softmax share drops below the level needed to feed kittens before they
starve. Adults choose to groom each other instead of feeding kittens.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L0 substrate | `KittenCryMap` + `IsParentOfHungryKitten` | Cry stamped, marker authored. 164's structural fix in place. | `[verified-correct]` |
| L1 markers | `src/components/markers.rs` | `Kitten`, `IsParentOfHungryKitten` markers fire correctly. | `[verified-correct]` |
| L2 self-state DSE `CaretakeDse` | `src/ai/dses/caretake.rs` | Three-axis weighted sum + `KittenCryCaretakeLift` modifier up to +0.5. Pre-158 score-distribution unchanged structurally; what changed is the *softmax pool* it competes against. | `[suspect — needs Caretake-share comparison]` |
| L2 self-state DSE `GroomOtherDse` | `src/ai/dses/groom_other.rs` (or wherever 158-Groom landed it) | New first-class peer DSE post-158-Groom; competes for affiliative time-share. | `[suspect — needs share comparison]` |
| L3 softmax | `src/ai/scoring.rs` | Caretake's competitive position weakened by the addition of `GroomOther` to the pool. Continuity grooming +156% confirms `GroomOther` is dominating its share. | `[suspect — quantify]` |
| Action → Disposition mapping | `src/components/disposition.rs::from_action` | `GroomOther → Grooming` (per 158-Groom split). `Caretake → Caretaking`. Mappings independent. | `[verified-correct]` |
| Caretake plan template | `src/ai/planner/...` (`[RetrieveFoodForKitten@Stores, FeedKitten@Stores]`) | Same as 164 — feeding chain itself works when adults choose Caretake. | `[verified-correct]` |
| Cohort-size regression | Reproduction sub-system (`fertility.rs` / mating chain) | `kittens_born` dropped 5 → 3 in same window; suggests `Mate` / `Courtship` action share also fell as a side-effect of the affiliative redistribution. Continuity courtship -63% supports. | `[suspect — separate sub-defect or downstream of same redistribution]` |

## Investigation step (do this first, before listing fix candidates)

1. **Caretake-share comparison.** Use `just frame-diff <pre-158-trace>
   <post-158-trace>` to rank all DSEs by |Δ mean(final_score)| between
   commits. Need focal-cat traces from the same seed (e.g., Mocha) on
   both sides of d1722a33. Likely the pre-158 trace is stale — may need
   to `just soak-trace` on a pre-d1722a33 commit and on HEAD.
2. **Scenario microexperiment** (`just scenario`). Per CLAUDE.md
   "Bugfix discipline", isolate "given this state, which DSE wins?" with
   a preset adult + hungry kitten + nearby adult-peer-to-groom. Does the
   post-158 adult choose `GroomOther` over `Caretake` when both are
   eligible? Define a scenario under `src/scenarios/` if no existing one
   covers it.
3. **Cohort-size regression.** Why `kittens_born` 5 → 3? Is it the same
   "affiliative-share-stole-from-Mating" mechanism, or a separate path?
   Run `just q events logs/tuned-42 --kind MatingOccurred` and compare
   to the pre-158 cohort.
4. **Fingerprint vs healthy baseline.** `just fingerprint logs/tuned-42`
   to confirm which characteristic metrics are out of band.

## Fix candidates (placeholders — finalize after investigation)

**Parameter-level:**

- R1 — Re-tune `KittenCryCaretakeLift` ceiling above 0.5 (perhaps to
  1.0–1.5) so the cry-driven Caretake score dominates the softmax pool
  even with `GroomOther` added. Risk: same as ticket 164's R1 — adults
  pulled off Hunting / Foraging during peak hunger.
- R2 — Decay `GroomOtherDse` score under starvation-pressure: when
  any `Kitten` within hearing-disc has hunger > critical, suppress
  `GroomOther` for `IsParentOfHungryKitten`-bearing adults (and
  possibly all adults).

**Structural:**

- R3 (**rebind**) — Re-bind `GroomOther` Action under `Socializing`
  rather than its own `DispositionKind::Grooming`. Effectively reverts
  158-Groom's extraction. Risk: re-introduces the `[GroomedOther
  never-fires]` defect that 158-Groom shipped to fix.
- R4 (**extend**) — keep `Grooming` as a sibling disposition, but
  branch its scoring template on `IsParentOfHungryKitten`: when the
  marker is present, `GroomOtherDse` returns 0 regardless of base
  score. Pulls "kittens-take-priority" into substrate at the
  scoring layer instead of relying on softmax balance.
- R5 (**split**) — split `IsParentOfHungryKitten` semantics into
  `IsParentOfHungryKitten` (feeding need) and a new
  `KittenCohortInDistress` (broadcast: any kitten in colony hungry
  beyond critical). Use the latter to suppress non-Caretake
  affiliative DSEs colony-wide during cohort-distress windows.

## Recommended direction

**Investigation step first.** The defect could be (a) softmax-share
drift, (b) priority-inversion needing structural suppression, or (c)
cohort-size regression with a separate root cause. Don't draft a fix
until the layer-walk promotes a `[suspect]` row to `[verified-defect]`.

## Out of scope

- **The (38,22) defect.** Owned by ticket 164 (the renumbered
  kitten-3822 ticket) — its structural fix is intact and orthogonal to
  this regression.
- **GroomedOther never-fired baseline.** Owned by landed ticket 158-Groom
  (the GroomedOther split) — that's the change that introduced this
  regression, but the fix to the never-fired defect was correct on its
  own terms; the question is what the redistributed time-share broke
  downstream.
- **`kittens_surviving` metric wiring.** Owned by ticket 166 — the
  unimplemented footer field surfaced during 164's closeout
  investigation, orthogonal to this regression.

## Verification

- **Hard gate:** `just soak 42` → `just verdict` reports
  `deaths_by_cause.Starvation == 0`. This unblocks ticket 164.
- **Cohort restored:** `kittens_born ≥ 5` (post-161 baseline).
- **Affiliative balance:** `continuity_tallies.grooming` does not
  regress below the post-158-Groom baseline of ~1,279, AND
  `continuity_tallies.mentoring` does not fall further below post-154
  (~445). Caretake throughput recovers to ≥ 63 (the post-156 baseline).
- **Cross-seed sanity:** seed 43 also clears `Starvation == 0`.

## Log

- 2026-05-04: opened. Surfaced during ticket 164's closeout
  investigation. Re-soak of seed-42 against current HEAD `2b6b49fb`
  reproduced the post-d1722a33 regression deterministically. The
  (38,22) structural fix from 164 is verified intact on main —
  this is a separate, GroomedOther-introduced regression. Blocks 164
  closeout per the same precedent as 161 (sibling-change perturbation
  re-blocks an otherwise-fixed substrate ticket).
