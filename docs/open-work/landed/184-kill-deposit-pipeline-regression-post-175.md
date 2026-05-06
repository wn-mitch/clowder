---
id: 184
title: Hunt kill→stockpile pipeline regressed under L3 bandwidth pressure (root cause - CanHunt over-gated on Injured)
status: done
cluster: ai-substrate
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [181-hunt-forage-saturation-tune.md]
landed-at: 4db67313
landed-on: 2026-05-06
---

## Why

Ticket 181's iteration-1 soak (`logs/tuned-42`, seed 42, weights
`hunt_food_security_weight=0.20` / `forage_food_security_weight=0.15`,
since reverted) revealed a concerning pattern: cats killed 374 prey
in a 15-minute soak yet **the food stockpile stayed flat at 0/50 for
the entire run**. Pre-181 baseline at the same commit (weights at
0.0) had 383 kills and the stockpile cycled 0% → 36% → 48% → 26% →
0% — confirming the deposit pathway *can* work at this commit.

The collapse is **broader than food**: the same soak shows
`WardPlaced 6 → 0`, `ShadowFoxBanished 11 → 0`, `RemedyApplied 209 →
0`, `FoodCooked 154 → 8`, `MentoredCat 121 → 21`. Multiple
dispositions are being *elected* (Patrol surged +15 pp) but their
**tail plan-steps are not delivering productive outputs**. This
pattern suggests a defect in late-stage step completion that's
gated by L3 bandwidth distribution — pre-181's distribution doesn't
trigger it; post-181's does.

Hard-gate impact:
- `deaths_by_cause.Starvation == 0` — failed in both runs (1 each)
- Continuity canary `mythic-texture` — `11 → 0` (collapse)
- Continuity canary `mentoring` — `121 → 21` (-83%)
- colony_score `nourishment` axis crashed because the colony went
  extinct ~30k ticks earlier (post-extinction footer artifact).
- Run-dir surfacing this: `logs/tuned-42` (post-181 iteration 1) vs
  `logs/tuned-42-pre-181` (baseline at same commit).

## Current architecture (layer-walk audit, verified 2026-05-06)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/components/markers.rs` | Storehouse / Stores building markers fire correctly; the carcass-pickup `PickingUp` DSE eligibility filter (`forbid(markers::Incapacitated::KEY)`) is the only marker check on pickup | `[verified-correct]` |
| L2 DSE scores | `src/ai/dses/picking_up.rs:34-39` | `PickingUp` ships at `Curve::Linear { slope: 0.0, intercept: 0.0 }` — dormant per its own doc-comment ("Stage 3 ships dormant; balance-tuning lifts the score") | `[verified-correct]` |
| L2 DSE scores | `src/ai/dses/hunt.rs`, `forage.rs` | Hunt / Forage RtEO axes work as wired; weight 0.20/0.15 on the saturation axis cascaded share to Patrol | `[verified-correct]` (181 iteration 1) |
| L3 softmax | `src/ai/scoring.rs:2140` | Filter `*s > 0.0` excludes zero-scoring actions, including PickingUp / disposal DSEs | `[verified-correct]` |
| Action→Disposition mapping | `src/components/goap_plan.rs:362` | `RetrieveRawFood \| Cook \| DepositCookedFood => Action::Cook` — RetrieveRawFood is a **Cook plan step**, not the carcass-pickup pathway | `[verified-correct]` |
| Action→Disposition mapping | `src/ai/mod.rs:124-128` | `Action::PickUp` rides `DispositionKind::PickingUp`; carcass-pickup is gated entirely behind PickingUp's L3 election | `[verified-correct]` |
| Plan template | `src/ai/planner/actions.rs:35-76` | Hunt plan synthesis (with travel decoration): `TravelTo(HuntingGround) → SearchPrey → EngagePrey → TravelTo(Stores) → DepositPrey`. `DepositPrey` requires `CarryingIs(Carrying::Prey)`, which `EngagePrey` sets symbolically when the kill resolver returns Advance. | `[verified-correct]` |
| Plan template | `src/ai/planner/actions.rs:274-280` | `picking_up_actions()` plan template: single step `PickUpItemFromGround` | `[verified-correct]` |
| Completion proxy | `src/components/commitment.rs:235` | `DispositionKind::PickingUp => SingleMinded` strategy | `[verified-correct]` |
| Resolver | `src/systems/goap.rs:5380-5530` | Three kill-outcome branches: (a) consumed_in_place (`hunger < production_self_eat_threshold = 0.5`) → emit `FoodEaten`, plan Fail (line 5496); (b) `!inventory.is_full()` AFTER push, "still has room" → record `KilledAndReplanned`, plan Fail; (c) `inventory.is_full()` AFTER push (or already-full + ground spawn) → record `Killed`, plan Advance to TravelTo(Stores) | `[verified-correct]` |
| Resolver | `src/steps/disposition/deposit_at_stores.rs:114-184` | Deferred-removal contract (post-175): items removed from inventory **after** `add_effective` returns true, so capacity rejection leaves the slot intact. Pre-175 removed upfront and silently destroyed overflow on capacity-fail. | `[verified-correct]` (post-175 is strictly more correct than pre-175; not a regression vector) |
| Pre-175 baseline | `git show da92888b -- src/systems/goap.rs src/steps/disposition/deposit_at_stores.rs` | [HYPOTHESIS — please verify] Pre-175, the kill resolver teleported the catch directly into the killer's inventory. **Refuted:** the kill resolver was unchanged by 175 — branch (b) `!inventory.is_full() → inventory.slots.push(ItemSlot::Item(...))` is identical pre-175 and post-175. What 175 changed was `deposit_at_stores.rs` (deferred removal, see row above). | `[verified-correct]` (refuted; no regression vector here) |
| Bevy command-buffer race | `src/components/building.rs:388-422` | `add_effective` calls `is_effectively_full` → `effective_capacity_with_items` which queries `items_q.get(entity)` on items in `self.items`. A just-spawned item via `commands.spawn(Item).id()` is buffered until next system flush; its `capacity_bonus` is invisible during this tick's check. **Real defect** — but only affects items with `capacity_bonus() > 0` (storage-upgrade baskets). Foods have `capacity_bonus = 0`, so the race is orthogonal to ticket 184's stockpile observation. Open as ticket 186. | `[verified-defect]` (orthogonal; tracked under 186) |
| L3 commitment → tail-step | (cross-disposition collapse) | Pattern hypothesized as "tail-steps silent fail across multiple dispositions" was actually downstream of L3 selection-share collapse: cats elect Hunt/Forage when hungry (food pressure stays high in post-181 because stockpile equilibrium is low), spend less time on Herbalism / Cooking / Mentoring → wards not placed → fox kills → injury deaths → 30K-tick earlier extinction. | `[verified-correct]` (no tail-step defect; one cascade, multiple symptoms) |

## Verdict — pipeline fine, but a real over-gating defect surfaced

The ticket's premise — "tail plan-steps not delivering productive
outputs" — is true at the symptom level but false at the
mechanism level. Phase 1 logq queries plus the
`hunt_deposit_chain` scenario together established that the
kill→deposit pipeline works in isolation. **However**, deeper
focal-trace L2 analysis (Wren's `final_score` means: Hunt 1.07
when eligible, Patrol 0.52, Hunt ineligible 9.7% of the time)
surfaced a real structural over-gating that the pipeline
verdict alone obscured.

1. **Pipeline is structurally fine** (locked by
   `cargo test --lib scenarios::hunt_deposit_chain`). One cat,
   five prey, one Stores → 9 food deposited over 400 ticks.
   Future regressions to `goap.rs:5380-5530` or
   `deposit_at_stores.rs:114-184` will fail this test.

2. **The actual defect: `CanHunt` over-gated on `!Injured`.**
   `update_capability_markers` (`src/ai/capabilities.rs:75-82`)
   removed the `CanHunt` marker from any cat with the `Injured`
   marker, making Hunt's L2 eligibility filter reject 9.7% of
   eligibility checks in the post-181 trace. **Combined with
   Patrol's `Blind` commitment** (`src/ai/dses/patrol.rs:121` —
   "Territory defense shouldn't flinch mid-patrol") **and
   Patrol's longer plan duration**, those 9.7% windows
   converted to a +15pp action-share gain for Patrol. The user's
   diagnostic intuition: "a mangy one-eyed cat still hunts
   rats" — injury should dissuade, not disable. Hunt's L2
   scoring already dampens via skill / health interoception
   signals; the eligibility gate double-counted.

3. **Stockpile equilibrium gap (peak 50/50 → 9/50) was
   amplified by the Patrol lock-in.** Without the
   over-gating, the brief moments stockpile rose under post-181
   weights would have triggered the saturation axis's intended
   suppression — but cats would have stayed available to
   resume Hunt when stockpile dropped back. With the
   over-gating, injured cats migrated to Patrol's Blind
   commitment and stayed there for full plan durations,
   compounding the throughput shortfall.

4. **Cross-disposition collapse** (`WardPlaced 6→0`,
   `RemedyApplied 209→0`, `FoodCooked 154→8`) is downstream of
   the colony dying ~30K ticks earlier (1,254,648 vs 1,285,262).
   Same root cause (Patrol lock-in starves food + reduces
   Herbalism time → no wards → fox kills succeed → injury
   cascade), multiple visible symptoms.

5. **The user's gut about items-are-real (`da92888b`)
   refuted.** Kill resolver unchanged by 175.
   `deposit_at_stores.rs` is *more* correct post-175 (rejected
   items stay in inventory; pre-175 silently destroyed them).
   The `add_effective` command-buffer race is real but
   orthogonal (capacity-bonus items only) — tracked as
   ticket 186.

## Fix landed

`src/ai/capabilities.rs:75-82` — removed `!is_injured` from
`want_hunt`. CanHunt now gates on `(Adult ∨ Young) ∧
¬InCombat ∧ forest nearby` only. Injury affects Hunt via the
L2 scoring layer (skill + health-interoception signals dampen
the score, not the eligibility). The `injured_adult_no_can_hunt`
test flipped to `injured_adult_keeps_can_hunt`. The
`heal_transition` and `injury_transition` tests updated to
assert CanHunt persists across injury (CanWard / CanCook still
gate on `¬Injured` — those are separate design calls).

The other three capability markers (`CanForage`, `CanWard`,
`CanCook`) retain their `¬Injured` gate by design — the user's
"dissuade not disable" call was specifically for hunting
("a mangy one-eyed cat still hunts rats"). If a similar
action-share cascade surfaces for Forage / Ward / Cook, revisit
under separate tickets.

## Follow-ons opened

- **Ticket 185** — extend `PickingUp` DSE on a new
  `HasGroundCarcass` colony marker for emergent scavenging.
  Addresses the 6071 `OverflowToGround` items per soak that
  rot uncollected because PickingUp ships dormant. The user
  flagged this as appealing during 184's investigation.
- **Ticket 186** — fix the `add_effective` Bevy
  command-buffer race for capacity-bonus items
  (`src/components/building.rs:388-393`). Real silent-loss
  path for storage-upgrade items; orthogonal to 184's food
  observation.

## Diagnostic gaps to close before fix candidates

These must be answered first; don't list a fix that depends on a `[suspect]` row:

1. **Does the items-are-real refactor (`da92888b`) actually change the kill resolver's deposit semantics?** Read the diff of `src/systems/goap.rs` around what is now line 5318-5524, and `src/steps/disposition/deposit_at_stores.rs`, in `da92888b` vs its parent. Confirm or refute the "kills used to teleport into inventory" gut.
2. **What fraction of post-181 kills hit each of the three resolver branches** (consumed_in_place / inventory-room / inventory-full)? Pre-181 had 168 `FoodEaten` / 322 `CarcassSpawned` / 383 kills; post-181 had 128 / 208 / 374. Per-tick rates of `CarcassSpawned` are identical (3.78/k). Need to know how many post-181 kills *should* have produced a successful DepositPrey — i.e., went via the inventory-room branch.
3. **Why did `WardPlaced`, `ShadowFoxBanished`, `RemedyApplied` all go to zero** in post-181 despite their parent dispositions being elected? Same generic pattern as DepositPrey: tail-step not firing. Walk one of them (e.g., RemedyApplied) end-to-end to see whether the failure is plan-interrupt-rate or something marker-state.
4. **Plan-completion-rate per disposition**: of the 751 post-181 Hunt elections, how many completed all three steps (SearchPrey + EngagePrey + DepositPrey) vs. how many got interrupted partway? `PlanInterrupted` event is logged but not bucketed by disposition.
5. **Bevy command-buffer / query-visibility check on `add_effective`**: Agent 3 (Theory 3 explore) claimed `commands.spawn().id()` followed by `items_q.get(item_entity)` returns None because the spawn is deferred until the next system flush. Pre-181 functioning suggests the bug isn't unconditional, but it may have a conditional path. Read `add_effective` in `src/components/building.rs` (around line 379-394) carefully.

## Fix candidates

_None proposed yet — see "Diagnostic gaps" above. Per memory rule "Promote audit-table rows before listing fix candidates", fix candidates that depend on `[suspect]` rows are not evidence._

The structural-option menu (per CLAUDE.md "Bugfix discipline"):
- **split** — could give carcass-pickup its own `DispositionKind` separate from PickingUp (e.g., `RetrieveCarcass`) so it's gated by ground-carcass-presence rather than competing in the general PickingUp election.
- **extend** — keep PickingUp, but branch its scoring on a `HasGroundCarcass` marker so it lifts above zero only when carcasses are present.
- **rebind** — chain `PickUpItemFromGround` directly into the Hunt plan template (after the inventory-full ground-spawn branch) so the killer immediately re-plans to retrieve their own kill, rather than waiting for a separate L3 election.
- **retire** — N/A; the carcass mechanism is load-bearing for the items-are-real philosophy.

These get drafted concretely once the diagnostic gaps are closed.

## Out of scope

- Ticket 181's saturation weight balance (parked behind 183 / this ticket).
- Ticket 183's paired-axis lift on higher-tier DSEs — that hypothesis was about *why bandwidth flows to Patrol*, this ticket is about *why Patrol's tail-steps don't deliver*. Different problem; don't conflate.
- The 182 courtship/burial regression — independent.
- Re-running the 0.20/0.15 weight test — pointless until this regression is understood.

## Verification

- Soak gates: a fresh seed-42 deep-soak with the fix lands a non-zero food stockpile peak (≥ pre-181's 48% / 24-of-50, ideally) and `WardPlaced ≥ 6`, `ShadowFoxBanished ≥ 11`, `RemedyApplied ≥ 200` per the pre-181 reference.
- Focal-cat trace via `just soak-trace 42 Wren` to confirm Hunt plans are completing all three steps to DepositPrey, not just the first two.
- `just q footer logs/tuned-42 --field plan_failures_by_reason` should NOT show a "DepositPrey: …" failure as a top-10 item if the fix is correct (or alternatively, should show it if the fix exposes a previously-silent failure that needs separate handling).
- Once the regression is fixed, re-attempt ticket 181's saturation-weight tuning under the now-stable deposit baseline.

## Log

- 2026-05-06: opened from ticket 181 iteration-1 closeout. Sibling
  ticket 183 (paired-axis / Patrol-collision investigation) was
  opened first under a different theory; this ticket is the more
  urgent root-cause investigation surfaced by deeper drilling. The
  user's gut: items-are-real refactor broke kill→inventory teleport
  ([HYPOTHESIS — please verify]).
- 2026-05-06: pipeline verdict locked by scenario
  `hunt_deposit_chain` (one cat, five prey, one Stores → 9
  food deposited; `cargo test --lib
  scenarios::hunt_deposit_chain` passes). Initial close as
  "no fresh defect" was too narrow.
- 2026-05-06: user pushed back on the verdict citing the gap
  between Hunt-Advance count (362) and stockpile peak (9).
  Focal-trace L2 analysis on Wren surfaced **the actual defect**:
  `CanHunt` was over-gating on `Injured`, and Patrol's `Blind`
  commitment + long plans amplified the 9.7% Hunt-ineligibility
  window into a +15pp action-share gain. User's design call:
  "Cats shouldn't be hunt ineligible if injured, just dissuaded.
  I've seen a mangy coon with 1 eye hunting rats."
- 2026-05-06: fix landed in `src/ai/capabilities.rs:75-82` —
  removed `!is_injured` from `want_hunt`. CanHunt now gates on
  `(Adult ∨ Young) ∧ ¬InCombat ∧ forest nearby`. Tests updated
  (`injured_adult_keeps_can_hunt`,
  `injury_transition_keeps_can_hunt_removes_can_ward`). Hunt
  L2 scoring dampens via skill + health-interoception signals
  rather than via eligibility gate. The other three capability
  markers (CanForage, CanWard, CanCook) retain their
  `¬Injured` gate — separate design calls. Follow-ons opened:
  185 (extend PickingUp on HasGroundCarcass for emergent
  scavenging — user-flagged appealing) and 186 (`add_effective`
  Bevy command-buffer race for capacity-bonus items).
