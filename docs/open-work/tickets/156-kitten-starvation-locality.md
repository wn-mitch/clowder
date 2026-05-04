---
id: 156
title: Kitten starvation localized at (38,22) post-154 cascade — Caretaking range/scheduling miss
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
Bugfix-shape ticket — ticket 154's land-day verdict surfaced a
hard-gate violation in a brand-new failure mode. CLAUDE.md hard
gate `deaths_by_cause.Starvation == 0` is violated by 2 kitten
deaths at the same tile within one season. Caretaking is firing
healthy run-wide (1631 KittenFed events) — this is a localized
defect, not a systemic break.
-->

## Why

Ticket 154's land-day soak (`logs/tuned-42`, seed 42, commit
`bb189bc`, 8 sim years) violated the CLAUDE.md hard gate
`deaths_by_cause.Starvation == 0`. Two kittens — `Robinkit-33` (tick
1357232) and `Maplekit-98` (tick 1357784) — starved at the **same
tile (38, 22)**, 552 ticks apart, both deep into year 8. This is a
brand-new failure mode: pre-154 had `kittens_born = 0`, so the
feed-kitten flow was untested at sustained demand. Post-154 the
mentoring → bonds → mating cascade lit up reproduction (`kittens_born
= 6`, `bonds_formed = 29`, +867%), and the feed-kitten path
bottlenecked locally.

Caretaking is **not** systemically broken: the run logged 1631
`KittenFed` events. Both starvations are at the same map tile,
suggesting a localized scheduling / range gap rather than a missing
DSE or broken step resolver. Investigation needed before the fix
shape is clear. The balance-layer narrative for this regression
lives at `docs/balance/mentoring-extraction.md` Iter 1.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/ai/markers/kitten_*.rs` | unverified — does a `KittenNeedsFood` marker correctly flag (38,22)-kitten as eligible? | `[suspect]` |
| L2 DSE scores | `src/ai/dses/caretake.rs` + `src/ai/dses/caretake_target.rs` | unverified — does `CARETAKE_TARGET_RANGE = 12` reach (38,22) from where adults are clustering? | `[suspect]` |
| L3 softmax | `src/ai/scoring.rs` | Caretaking is in the L3 pool and *does* fire (1631 KittenFed events) | `[verified-correct]` |
| Action→Disposition mapping | `src/components/disposition.rs::from_action:131` | `Action::Caretake → Caretaking` | `[verified-correct]` |
| Plan template | `src/ai/planner/actions.rs::caretaking_actions` | `[RetrieveFoodForKitten, FeedKitten]` two-step chain | `[verified-correct]` |
| Completion proxy | `src/ai/planner/goals.rs:65` | `TripsAtLeast(N+1)` count-based | `[verified-correct]` |
| Resolver | `src/steps/disposition/feed_kitten.rs` | unverified — does the chain time out before kitten hunger drains, given new sustained-demand cadence? | `[suspect]` |

## Investigation step (do this first, before listing fix candidates)

1. **Cat-timeline both kittens.** `just q cat-timeline logs/tuned-42 Robinkit-33` and `Maplekit-98` for the 5000 ticks pre-death. Establish: (a) where on the map they spawned, (b) whether they migrated to (38,22) or were born there, (c) which adults were within `CARETAKE_TARGET_RANGE = 12` of them in the death window.
2. **Caretake-target audit.** For each adult that scored Caretaking ≥ ε in the death window, dump their `caretake_target_dse` evaluation: did (38,22) appear in the candidate pool? If yes but lost on score, why? If no, was it filtered by range, by the `require_alive_filter`, or by an eligibility marker?
3. **Compare the survivor cohort.** 4 of 6 kittens matured pre-run-end (kittens_born=6, kittens_surviving=0 because 4 aged out, 2 starved). Where did the survivors spawn? Same cluster or distributed? Distance from adult population centroid?

The investigation step decides which row in the audit moves from
`[suspect]` to the defect site.

## Fix candidates

**Parameter-level:**

- R1 — bump `CARETAKE_TARGET_RANGE` from 12 to ~15-20. Trivially
  enlarges the candidate pool. Risk: at 60-tile colonies this is
  almost map-wide; could over-aggressively pin one adult onto a
  far-off kitten over a closer task.
- R2 — add a hunger-urgency boost to the Caretaking DSE when any
  reachable kitten's hunger drops below a critical threshold (mirror
  the Hunting/Foraging critical-hunger override at `goap.rs:644`).
- R3 — adjust kitten-spawn locality so newborns spawn closer to
  active-adult centroid rather than at the parent's death-spot or
  birth-bed.

**Structural:**

- R4 (**split**) — extract `DispositionKind::FeedKitten` out of
  `Caretaking` if Caretaking grows additional sub-actions
  (grooming-kitten, sheltering-kitten). Caretaking today is single-
  constituent (`[Caretake]`) so this only matters if scope grows.
  Currently low-value.
- R5 (**extend**) — add a `KittenNearby` proximity marker to the
  adult that fires when *any* kitten is within Manhattan-N of them,
  authored by a system pass; gate Caretaking-DSE eligibility on it.
  Pulls "is there a kitten close enough to feed" out of the L2 search-
  state and into substrate. Aligns with substrate-refactor §4.7.
- R6 (**rebind**) — remap kitten-feeding to a "Care broadcast" the
  kitten emits as substrate (PainOf-style perceivable signal); adults
  score against the broadcast strength. Larger refactor; requires a
  new perception channel.

## Recommended direction

**Defer the recommendation until the investigation step is complete.**
The defect-shape isn't yet established — we know it's localized at
(38,22), but not whether the bottleneck is range, scheduling, scoring,
or kitten spawn-position. Listing R1 as the recommendation now would
violate CLAUDE.md's "audit L3 before listing fixes" discipline.

After the investigation, expect either:
- R1 (range bump) if the candidate pool lacked the kitten — small param fix.
- R2 (urgency boost) if the candidate pool had it but Caretaking lost
  the L3 race — small balance change.
- R5 (KittenNearby marker) if R1+R2 are insufficient and the issue is
  an absence of perceivable substrate — structural change, separate
  PR.

## Verification

- **Hard gate restored:** post-fix `just soak 42` → `just verdict`
  reports `deaths_by_cause.Starvation == 0`.
- **Cascade preserved:** `kittens_born ≥ 5`, `kittens_surviving ≥ 3`
  (the cascade isn't smothered by the fix).
- **Mentoring undisturbed:** `MentoredCat` stays off
  `never_fired_expected_positives`; `mentoring` continuity tally
  ≥ ~1000 (within 2× of the post-154 baseline of 1614).

## Out of scope

- **Burial-canary-dark** (separate ticket: 157). burial = 0 has a
  different defect shape (eligibility-predicate match against new
  death distribution).
- **Cross-seed sweep validation.** This ticket's fix targets the
  seed-42 hard-gate violation; a cross-seed sweep should follow as a
  separate post-fix step, not as part of investigation.
- **Reducing the cascade's intensity.** The post-154 cascade
  (bonds +867%, kittens 0→6) is intentional and tracked by the balance
  doc; this ticket fixes the bottleneck, not the cascade.

## Log

- 2026-05-03: opened by ticket 154's land-day verdict.
  `docs/balance/mentoring-extraction.md` Iter 1 holds the cascade
  narrative.
