---
id: 410
title: L2 ParentingActivity — HandoffItem cascade follow-on (400 verdict concern)
status: done
cluster: social-coordination
orchestration: substrate-sensitive
initiative: [smarter-cats]
added: 2026-05-18
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-18
---

## Why

Ticket 400's seed-42 deep-soak verdict landed at **concern** with the
`plan_failures_by_reason["HandoffItem: handoff: no recipient on disposition
(no kittens in colony)"]` rate at **26.9× baseline** (0.07459/tick vs
0.00277/tick on `tuned-42-095-phase-1a-shadow`). The plan's hard gate was
"do NOT regress this canary" — the gate is violated.

The failure mode is **"no kittens in colony"** — cats elect Caretake and the
planner finds no living dependent kitten to feed. With 400's substrate
landed, the volume increases because:

1. Fathers now adopt `RAISE_OFFSPRING_ASPIRATION` (398's `is_mother` gate
   was widened in 400 Step 4).
2. `ParentingActivity.relationships[i].parental_engagement` decays toward
   a residual (not zero) when the target kitten dies or matures —
   intentional, per §7.7.b grief substrate foundation. The
   `ParentingActivityModifier` therefore continues lifting Caretake even
   after kittens are gone.
3. The Caretake DSE WeightedSum's `caretake_compassion` and
   `parental_engagement` axes produce non-zero raw scores even when no
   hungry kitten exists; the gated-boost contract (`score > 0` ⇒ modifier
   fires) is satisfied and the lift compounds.

400's existing JointIntention-aware suppression mechanic (parent A holding
Caretake → parent B's `caretake_suppression_factor *= 0.3`) is **target-
specific** (verified by `suppression_target_specific_no_cross_litter_yield`
unit test) — multi-litter colonies don't over-suppress. But the
suppression CAN'T address the "no kittens at all" cascade: when zero
kittens exist, no parent is holding Caretake, so suppression doesn't fire,
and every parent independently elects Caretake → fails → repeats.

Failing run: `logs/tuned-42` at commit `06a2034a` (Steps 1-9, dirty with
Steps 10-12 + suppression fix).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/components/markers.rs::Parent` | `Has<Parent>` tracks "≥1 living dependent kitten." When the marker turns off, the cat is no longer "currently a parent" but `ParentingActivity` Component persists per 400 design. | `[verified-correct]` |
| L1 sync | `src/systems/parenting_activity.rs::update_parenting_activity_biological` | Only inserts; never removes Biological-kind RelationshipTo entries. The entries persist past kitten death/maturity. | `[verified-correct]` |
| L1 engagement | `src/systems/parenting_activity.rs::tick_parental_engagement` | When target is dead/despawned, asymptote drops to `matured_residual_factor × full` (≈ 0.15× ≈ 0.07 for typical asymptote). Engagement decays toward this residual, not zero. | `[verified-correct]` |
| L2 Caretake DSE | `src/ai/dses/caretake.rs` | WeightedSum of `kitten_urgency` (0 when no kitten cry), `caretake_compassion` (non-zero from personality), `parental_engagement` (gradient, non-zero from residual). Raw score = 0.30×compassion + 0.25×residual ≈ 0.10 for a typical low-compassion adult with grief residual. | `[verified-defect]` — non-zero raw score in the absence of kittens |
| L2 modifier | `src/ai/modifier.rs::ParentingActivityModifier` | Adds `caretake_bias_sum * suppression_factor` on top. With no partner holding Caretake (because no kitten exists), suppression = 1.0 (no dampening). bias_sum = `scale_presence × bond × engagement` ≈ 0.04-0.20 depending on personality. | `[verified-defect]` — fires regardless of kitten existence |
| L2 modifier | `src/ai/modifier.rs::KittenCryCaretakeLift` | Reads `kitten_cry_perceived` from `KittenCryMap`. Zero when no kitten is crying. So this modifier doesn't fire when no kittens. | `[verified-correct]` |
| L3 softmax | `src/systems/goap.rs` | Caretake's final score (~0.15-0.30 in no-kitten state) wins softmax against lower-scored DSEs roughly 5-15% of ticks per cat. With 10 cats and ~12k ticks of no-kitten windows, this produces thousands of failed Caretake plans. | `[verified-defect]` |
| Plan template | `src/ai/methods/caretake_kitten.rs` | The planner walks Caretake's HTN method, can't find a target kitten, emits `PlanningFailureReason::HandoffItem("no recipient on disposition (no kittens in colony)")`. | `[verified-correct]` (planner correctly reports the failure) |
| Suppression mechanic | `src/systems/parenting_activity.rs::populate_parenting_scalars` | Suppression fires when partner has `HeldIntention::Caretake` with `target ∈ my dependents`. Inactive when no partner is caretaking (the failure case). | `[verified-correct]` (works as designed; doesn't address this failure mode) |

## Fix candidates

**Parameter-level options:**
- R1 (**threshold**) — set `parenting_caretake_bias_sum` to zero when
  `kitten_urgency == 0`. The bias only fires when a hungry kitten exists.
  Simple, targeted; addresses the failure but blunts the design's "grief
  substrate" semantics (grieving parents' Caretake-axis goes to zero).
  Single-line change to `populate_parenting_scalars` or to the modifier's
  CARETAKE arm: `bias_sum = if kitten_urgency > 0 then bias_sum else 0.0`.
- R2 (**residual zero**) — set `matured_residual_factor = 0.0` so
  engagement decays fully to zero when no living kitten exists. The
  Caretake-axis on the DSE WeightedSum collapses to zero in the no-kitten
  case (no `parental_engagement` lift). Side effect: kills the §7.7.b
  grief-as-frustrated-target-taking foundation that 400 explicitly preserves.
- R3 (**eligibility filter**) — add a hard `EligibilityFilter` requirement
  to Caretake DSE: requires `HasHungryKitten` marker (or equivalent). When
  no hungry kitten exists, Caretake scores 0 (eligibility-gated). Cleanest
  parameter-level fix; preserves grief substrate (parents still feel the
  pull) but the DSE doesn't actively elect. Requires authoring the marker
  in a per-tick system.

**Structural options** (at least one drafted, per CLAUDE.md):
- R4 (**split**) — give grief-state-parental a separate Disposition or
  DSE variant. Caretake retains "active feeding of a hungry kitten"
  semantics with hard eligibility on kitten presence. A new
  `GrievingParent` disposition fires the residual axes (mourning, lingering
  at last-seen locations) without competing for Caretake plans. Honest
  separation of "actively parenting" vs "carrying parental state" — moves
  the §7.7.b cascade onto its own substrate slice rather than overloading
  Caretake. Most aligned with the design pillar's "split when the layer-
  walk shows two jobs." Heavier lift; coordinates with ticket 407.
- R5 (**extend**) — keep the umbrella Caretake DSE, but branch its
  `emit()` to produce two different Goal labels: `kitten_fed` (active
  feeding, requires hungry kitten) and `grieving_parental_witness` (silent
  pull, no target). The planner can immediately drop the second goal at
  resolution time without enrolling a HandoffItem plan failure.

## Recommended direction

**R3 (eligibility filter)** as the immediate fix — it cleanly closes the
canary regression without sacrificing 400's grief substrate (the
gradient persists on the ParentingActivity Component; it just doesn't
elect Caretake when no kitten exists). Add a `HasHungryKittenInColony`
colony-scoped marker (or per-cat marker keyed on "any dependent in my
RelationshipTo list is hungry"); gate Caretake on it.

If R3 doesn't fully close the gap (some failures might come from
mid-resolver kitten-death timing), escalate to **R4 (split)** which moves
the grief expression onto its own substrate slice. R4 dovetails with
ticket 407 (§7.7.b grief cascade proper) and may belong there rather than
here.

## Out of scope

- Tuning the ParentingActivityModifier's asymptote weights (W_N, W_D, etc.)
  — that's ticket 408's scope.
- The §7.7.b grief cascade proper (mourning DSE, vigil behaviors, decay
  rates) — ticket 407.
- Renaming the `Parent` marker to disambiguate from the lifelong
  ParentingActivity — design parked in 399's design plan as a future
  cleanup.

## Verification

Hard gates restored:
- `plan_failure_canary["HandoffItem: no recipient on disposition (no
  kittens in colony)"]` returns to baseline (≤ 2× rate vs
  `tuned-42-095-phase-1a-shadow`).
- All 400 unit tests continue to pass, especially:
  - `suppression_fires_when_partner_holds_caretake`
  - `suppression_target_specific_no_cross_litter_yield`
- `just verdict logs/tuned-42` returns `pass` (or `concern` only on the
  pre-existing mythic-texture canary).

Soak recipe:
```bash
just soak-trace 42 Pebblekit 900
just verdict logs/tuned-42
just frame-diff logs/baselines/current/trace-Pebblekit.jsonl \
                 logs/tuned-42/trace-Pebblekit.jsonl
```

## Log

- 2026-05-18: opened as 400's verdict-concern follow-on. The HandoffItem
  cascade fix requires a kitten-presence gate on Caretake; the
  partner-suppression mechanism (target-specific via `HeldIntention.target`
  plumbing) works correctly but only addresses the "two parents racing for
  the same kitten" case, not the "no kittens at all" case. R3 is the
  recommended direction; R4 may consolidate into ticket 407.
- 2026-05-18: plan review surfaced two refinements over the original R3 —
  (1) the existing `HasHandoffRecipient` colony marker (188's wave-closeout)
  already encodes "≥1 kitten in colony" and gates the Handing DSE; reusing
  it on Caretake's `EligibilityFilter` is a single-line change vs. authoring
  a new `HasHungryKittenInColony` marker; (2) the marker's name encodes the
  *delivery mechanic* ("slot to receive a handoff"), not the *narrative*
  ("creature who needs care") — the mechanic-named marker would equally
  apply to a construction-kitty waiting on reeds. Renamed
  `HasHandoffRecipient` → `HasDependentCat` across the substrate
  (markers.rs · buildings.rs populator · disposition.rs / goap.rs writers
  · handing.rs require). Populator stays `!kittens.is_empty()` for this
  PR; the narrative-named marker trivially accommodates a future
  incapacitated-adult-recipient union without consumer changes. Per
  "mechanics are the narrative" design pillar.
- 2026-05-18: plan review also surfaced a *footer rate-arithmetic*
  recurrence pattern. During fact-checking, an ad-hoc subagent computed
  the failure rate as 7,090 / 1,295,058 = 0.00547/tick (≈ 2× baseline)
  and reported the ticket was overstating by 10×. That was wrong: runs
  start at absolute `start_tick ≈ 1,200,000`, so elapsed ticks are
  `final_tick − start_tick = 95,058` and the correct rate is 0.0746/tick
  = 26.9× as originally claimed. `verdict.py` does this correctly; the
  footer itself did not carry `start_tick` / `final_tick` / `elapsed_ticks`,
  forcing consumers to pair footer with header to get the right
  denominator. Per the user's framing, "isn't the first time we've seen
  misaligned footers cause freakouts." Closed in this PR by surfacing
  the three fields directly in the footer (`emit_headless_footer`) and
  updating `scripts/logq/logq.py` to report a `rate_per_tick` column
  alongside `plan_failures_by_reason_top`. Documented invariant:
  `rate = count / elapsed_ticks`, never `count / final_tick`.
- 2026-05-18: landed in tandem with the rename and footer-substrate
  commits. Caretake DSE now carries
  `.require(HasDependentCat::KEY)`; new scenarios
  `parenting_caretake_kitten_absent` (asserts the gate suppresses
  Caretake when the marker is false) and `parenting_caretake_kitten_present`
  (asserts the gate passes when a Kitten exists) close the corpus's
  missing parent→kitten succeeding-handoff scenario gap. Closes the
  364 → 394 → 395 → 397 → 398 → 399 → 400 → 410 kitten-arc cascade —
  no follow-on tickets opened.
