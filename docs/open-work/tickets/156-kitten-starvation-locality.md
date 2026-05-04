---
id: 156
title: Kitten starvation localized at (38,22) post-154 cascade — non-parent adults can't perceive distress
status: in-progress
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
Bugfix-shape ticket — ticket 154's land-day verdict surfaced a
hard-gate violation in a brand-new failure mode. CLAUDE.md hard
gate `deaths_by_cause.Starvation == 0` is violated by 2 kitten
deaths at the same tile within one season.

ORIGINAL FRAMING (deprecated 2026-05-03 by re-investigation): the
ticket opened with the premise that caretaking was firing healthy
run-wide ("1631 KittenFed events", "localized scheduling/range
gap"). That premise was wrong on every count — see §Why below for
the actual investigation evidence.
-->

## Why

Ticket 154's land-day soak (`logs/tuned-42`, seed 42, commit
`bb189bc`, 8 sim years) violated the CLAUDE.md hard gate
`deaths_by_cause.Starvation == 0`. Two kittens — `Robinkit-33` (tick
1357232) and `Maplekit-98` (tick 1357784) — starved at the **same
tile (38, 22)**, 552 ticks apart, both deep into year 8. Pre-154
had `kittens_born = 0`, so the feed-kitten flow was untested at
sustained demand. Post-154 the mentoring → bonds → mating cascade
lit up reproduction (`kittens_born = 6`, `bonds_formed = 29`,
+867%); the bottleneck surfaced.

**Re-investigation 2026-05-03 invalidated the original "localized
scheduling/range gap" framing.** The defect is multi-layer and
substrate-shaped, not a tile-local scheduling miss. Evidence:

| Original claim | Actual finding |
|---|---|
| "1631 `KittenFed` events run-wide" | `KittenFed` is **not an event type**. `Caretake` action fired **56 times colony-wide** in 8 sim years (`just q actions logs/tuned-42` → 0.43% of 13,159 CatSnapshots). |
| "Caretaking healthy run-wide" | Caretake is the 4th-rarest colony action; severely under-firing for a colony with hungry kittens. |
| "Localized scheduling defect" | Both kittens lived ~89,000 ticks but each emitted only **1 `PlanCreated`** in their entire life (`just q cat-timeline --summarize`). Action distribution **100% Idle** (891 / 896 snapshots). L2 score breakdown frozen at `Groom 1.13 > Sleep 0.67 > Eat 0.39 > Idle 0.08`. At hunger=0.19 the kitten still scored Groom > Eat (37 violations flagged by `just inspect`). |
| "Localized to (38,22)" | Confirmed both deaths at exact tile (38,22). |

The balance-layer narrative for the cascade lives at
`docs/balance/mentoring-extraction.md` Iter 1; this ticket fixes the
bottleneck the cascade exposed.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L0 substrate | `src/resources/kitten_urgency_map.rs` + `src/systems/growth.rs::update_kitten_urgency_map` | Per-tick stamped influence map painting `(1 - hunger)` weighted discs around every `KittenDependency` cat, falloff 12, channel `Sight`. **Producer-only since landing** — comment claims ticket 052 will land the consumer; 052 went a different way (its commit explicitly retired the `sample_map` consideration shape, "zero production callers"). The map sits stamped per-tick with nobody reading it. | `[verified-defect, no consumer]` |
| L1 marker | `src/components/markers.rs::IsParentOfHungryKitten` (line 409–413) | Fires only on the kitten's *living parent*. Orphans and kittens whose parent is out of range have no perceivable signal for non-parent adults. | `[verified-correct but insufficient]` |
| L1 marker | `src/components/markers.rs::Kitten` (line 74–78) | Lifestage marker authored by `growth.rs::update_life_stage_markers`. | `[verified-correct]` |
| L2 self-state DSE (kitten cohort) | `src/ai/dses/eat.rs` | Hunger-axis curve (`Quadratic(exp=1.5)` shape) on `(1 - hunger)` produces ~0.36 at hunger=0.19, well below the steady ~1.13 from social/grooming DSEs. Even when kittens score, Eat never wins. Kittens are passive feeders by design (no kitten-resolvable Eat action), so winners reduce to Idle anyway, but **the breakdown is dishonest about physiological priorities** — and breakdown honesty is the substrate that future kitten-self-feed work will read. | `[verified-defect, curve too gentle for kitten cohort]` |
| L2 self-state DSE (adult) | `src/ai/dses/caretake.rs` | Three-axis weighted sum: `0.45 kitten_urgency + 0.30 compassion + 0.25 is_parent_of_hungry_kitten`. **No map-perception axis** — adults rely on the (insufficient) `IsParentOfHungryKitten` marker. | `[verified-defect, no map-perception axis]` |
| L2 target-taking DSE | `src/ai/dses/caretake_target.rs` | Per-052 cutover already uses `SpatialConsideration` for the per-target distance axis. `CARETAKE_TARGET_RANGE = 12` candidate-pool gate. | `[verified-correct after R7; range bump revisited only if frame-diff flags it]` |
| L3 softmax | `src/ai/scoring.rs` | Caretaking is in the L3 pool. | `[verified-correct]` |
| Action→Disposition | `src/components/disposition.rs::from_action:131` | `Action::Caretake → Caretaking`. | `[verified-correct]` |
| Plan template | `src/ai/planner/actions.rs::caretaking_actions` | `[RetrieveFoodForKitten, FeedKitten]` two-step chain. | `[verified-correct]` |
| Completion proxy | `src/ai/planner/goals.rs:65` | `TripsAtLeast(N+1)` count-based. | `[verified-correct]` |
| Resolver | `src/steps/disposition/feed_kitten.rs` | Well-formed (10-tick min, witnessed/unwitnessed advance discipline correct). | `[verified-correct]` |

**Kitten passive-feeder design confirmed by user**, so the
"kittens 100% Idle" finding is not a defect at the action layer —
it's expected. The kitten-side work is therefore strictly about
making the L2 breakdown honest (Phase 5 of the fix plan), not about
unlocking new kitten actions.

## Fix direction — R7 (supersedes R1–R6)

**R7 — repurpose `KittenUrgencyMap` as a Hearing-channel cry
broadcast + per-DSE life-stage curves for the kitten cohort.** Five
coordinated changes (see implementation plan for sequencing):

1. **Reshape the existing map to a cry broadcast.** Channel
   `Sight` → `Hearing` (kittens cry, adults hear; this becomes the
   first production caller of `ChannelKind::Hearing`).
   Threshold-gated stamping: stamp only when `hunger <
   KITTEN_CRY_HUNGER_THRESHOLD` (e.g., 0.5), strength `(threshold -
   hunger) / threshold`. Bump `kitten_urgency_sense_range` 12 → ~30
   (sound travels). Rename `KittenUrgencyMap → KittenCryMap`.
2. **Author per-cat `kitten_cry_perceived` perception scalar.** New
   per-tick system samples `KittenCryMap.get(adult.pos)` multiplied
   by `species_sensitivity(Cat, Hearing)` (already 1.0 per
   `sim_constants.rs:4170`). Single-axis discipline — no personality
   folding.
3. **Wire `CaretakeDse` to consume the scalar.** Add fourth axis
   via `ScalarConsideration`, rebalance weights so the cry axis
   dominates when fired but doesn't crowd existing axes when quiet.
4. **Per-DSE life-stage curve branch (kitten Eat).** Inside
   `eat.rs`, branch on `Kitten` marker, substitute a vastly steeper
   curve so at hunger=0.19 kitten Eat exceeds the existing Groom
   1.13. Cosmetic for behavior (kittens still resolve to Idle), but
   makes the breakdown honest and is forward-compatible.
5. **Verification via `just soak 42 → just verdict`.** Hard gate
   restored, cascade preserved, frame-diff Hunting drift ≤ 10%.

R7 satisfies substrate-refactor §4.7: the cry map is externally
authored substrate (no `StateEffect::Set*` writer), the perception
scalar is a single-axis interoception value, and the DSE consumes
it via the standard `ScalarConsideration`. The producer-stamped
substrate that 052 left dead becomes load-bearing.

**Superseded candidates** (kept for historical record; not pursued):

- R1 (range bump alone) — would not address the under-firing or the
  non-parent perception gap.
- R2 (critical-hunger urgency override on Caretaking) — partially
  subsumed by R7's cry-perception axis, which is the more general
  substrate.
- R3 (kitten spawn-locality) — out of scope; the cascade is
  intentional.
- R4 (split `FeedKitten` from `Caretaking`) — Caretaking is still
  single-constituent; not load-bearing yet.
- R5 (`KittenNearby` binary marker) — superseded by the graded
  cry-broadcast map.
- R6 (Care broadcast as new perception channel) — R7 *is* this,
  scoped to the existing `Hearing` channel rather than a new one.

## Verification

- **Hard gate restored:** post-fix `just soak 42` → `just verdict`
  reports `deaths_by_cause.Starvation == 0`.
- **Inspector honesty:** `just inspect <new-kitten>` shows Eat
  dominating at low hunger; "Groom won over Eat" warning count
  drops to 0 for any kitten in the run.
- **Caretake under-firing resolved:** Caretake action count climbs
  meaningfully (rough target ≥ 10× the current 56 if the cascade
  still produces 5–6 kittens).
- **Cascade preserved:** `kittens_born ≥ 5`, `kittens_surviving ≥
  3`, mentoring continuity ≥ ~1000 (within 2× of post-154 baseline
  1614), `MentoredCat` stays off `never_fired_expected_positives`.
- **Hunting drift bounded:** frame-diff focal trace pre/post-fix on
  `Bramble`; Hunting / Foraging unchanged beyond ±10%.

## Out of scope

- **Burial-canary-dark** (separate ticket: 157). burial = 0 has a
  different defect shape (eligibility-predicate match against new
  death distribution).
- **Cross-seed sweep validation.** This ticket's fix targets the
  seed-42 hard-gate violation; a cross-seed sweep should follow as a
  separate post-fix step.
- **Reducing the cascade's intensity.** The post-154 cascade
  (bonds +867%, kittens 0→6) is intentional and tracked by the
  balance doc; this ticket fixes the bottleneck, not the cascade.
- **Kitten-self-feed action.** The Phase 5 curve branch is forward-
  compatible for a future self-feed resolver, but adding the
  resolver itself is a separate scope (would need its own ticket).

## Log

- 2026-05-03: opened by ticket 154's land-day verdict.
  `docs/balance/mentoring-extraction.md` Iter 1 holds the cascade
  narrative.
- 2026-05-03: re-investigated. Original "1631 KittenFed / localized
  scheduling" framing invalidated; actual defect is producer-only
  `KittenUrgencyMap` + non-parent adults blind to kitten distress +
  kitten Eat curve too gentle. Fix direction pivoted to R7
  (Hearing-channel cry broadcast + per-cat perception scalar +
  DSE consumer + kitten curve branch). Implementation plan at
  `~/.claude/plans/work-156-typed-clarke.md`.
