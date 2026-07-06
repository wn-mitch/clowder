---
id: 511
title: Weaned-kitten starvation complex: no self-feeding pathway past the care bands, feed-targeting preempted by a HandoffItem livelock (733 plans per 10k ticks), exposed at 900s soaks by the 0.4.0 throughput gains
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-07-05
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
Both 140 step-7 verification soaks failed the `Starvation == 0` hard
gate on DEPENDENT KITTENS (Duskkit-45 t1297266, Sparkkit-34/
Nettlekit-64 in the two runs; `logs/tuned-42-60ab5916`,
`logs/tuned-42-53213797`). A three-agent + manual drill established
this is a PRE-EXISTING defect complex, not a step-7 regression: the
healthy step-6 run carries the identical latent shape and its kittens
simply never aged into it (max kitten age 24.7k ticks vs 95k of
kit-exposure in the failing runs). The 0.4.0 perf recovery (67.7 →
143.6 t/s) doubled the tick-depth of every 900s soak — kittens now
age past the care-band cliff inside the soak window, and EVERY future
landing gate will trip this until it is fixed. **This blocks the
140 step-7 landing (parked, committed locally) and all subsequent
Phase II landings.**

## Current architecture (layer-walk audit — evidence from logs/tuned-42-53213797)

| Layer | Load-bearing fact | Status |
|---|---|---|
| Feed cadence | Duskkit-45 fed 12× (2/10kt sawtooth, +0.4-0.5 per feed) until age 50,797 ticks; then ZERO feeds for 11,966 ticks → monotone decay to death, with mother Mocha 1.0-1.4 tiles away. Nettlekit identical, last feed at age 50,815 | `[verified-hostile]` |
| Kitten self-feeding | NO pathway exists: no Eat-family action ever appears in kitten action menus; `BegForFood` appears only at the spawn tick (hunger spawns 0.4999, beg gate never re-opens); kitten Foraging plans END IN DepositFood (deposits to stores, never eats) | `[verified-hostile]` — the structural gap |
| Care-band thresholds | weaned 0.33 / teach_done 0.66 / release 0.95 at maturity rate 1/80k per tick. Age 50.8k = maturity 0.635 ≈ teach_done edge — feeds stop near the TEACH band boundary, not wean or release | `[verified]` (exact band mechanism unconfirmed — next drill below) |
| caretake_target DSE | Candidate filter is CORRECT: hunger < 0.6 gate, range 12, deficit-quadratic weight 0.40 dominant, `(1.0 - k.hunger)` input (caretake_target.rs:229-242, 291-294). With Nettlekit sated, Duskkit was the ONLY eligible candidate for its last ~5,300 ticks | `[verified-correct]` |
| Plan livelock | Mocha, window 1288k-1298k: **733 PlanCreated in 10k ticks**, dominated by `Handing [TravelTo(SocialTarget), HandoffItem]` recreated every ~3 ticks in bursts; Caretaking plans (`[TravelTo(Stores)/DropItem, RetrieveFoodForKitten, FeedKitten]`) created ~80× but repeatedly displaced pre-completion; only 10 PlanStepFailed total — plans END without failing (silent-completion or replan churn) | `[verified-hostile]` — why FeedKitten never executes |
| KittenFed counter | 165 firings but only 31 restored hunger — counter increments on full-kitten top-ups; NOT a health signal (red herring during diagnosis) | `[verified]` |
| Commitment layer | `momentum.commitment_strength == 0.0` across entire runs (ticket 509) — nothing damps the 3-tick Handing re-election | `[verified]` (cross-ref 509 R3) |

## Fix candidates
- R1 (**livelock**) — HandoffItem: find why a completed/no-op handoff
  leaves its election pressure intact (silent no-op transfer to a
  full partner? score input not cleared?). A 3-tick create loop with
  no step failures is a silent-canary-shaped defect. Fixing this
  alone may restore feeding (Caretaking regains execution time).
- R2 (**self-feeding pathway, structural**) — weaned-but-dependent
  kittens need to eat: either (a) extend parental FeedKitten
  candidacy to `maturity < release_threshold` (queens provision
  juveniles — smallest change), or (b) unlock Eat/BegForFood for
  weaned kittens (juvenile self-feeding — the ethologically-correct
  end state). (b) is a design decision — surface to the user before
  building.
- R3 (**feed-target hunger check at execution**) — resolve_feed_kitten
  executes on sated targets (top-ups burn the caretaker trip);
  cheap guard: skip/re-target when target hunger > threshold.
- R4 — cross-ref 509 R3 (commitment first-light) — would damp the
  re-election churn class globally.

## Recommended next drill (fresh session)
1. Drill the Handing livelock: pick one 3-tick burst
   (Mocha 1288336-1288354), find the HandoffItem resolver outcome per
   plan (events/narrative + resolver trace if a focal soak is run
   with Mocha as focal), and identify the un-cleared election input.
2. Then decide R1-vs-R1+R2 scope with the user (R2b is design-shape).

## Out of scope
- The 140 step-7 landing itself — its code is sound (travel timeouts
  0, +28% tps, zero adult deaths, all suites green); it lands as soon
  as this gate unblocks.

## Verification
900s seed-42 soak: Starvation == 0 with ≥1 kitten aging past 50.8k
ticks in-window; Handing plan-creation rate < 1/100 ticks per cat;
step-7 landing soak re-run green.

## Log
- 2026-07-05: opened from the 140 step-7 landing gates. Full evidence
  chain above; step-7 parked locally (commit on top of main 9b56ea38).
- 2026-07-06: user decision — R2(b), juvenile self-feeding is the
  ethological shape. Implemented: Eat life-stages widened to
  `juvenile_and_up()` (Stage-3 kittens are ambulant; the 451 hazard
  applies only to Stage 1/2, unchanged) + fourth BegForFood sibling
  `juvenile()` (JuvenileKitten ∧ ¬HasFoodInInventory; incapacitated
  sibling's set now excludes JuvenileKitten — coverage test updated +
  new kitten-completeness regression test). R3 (sated-target guard)
  DEFERRED: the goap-side kitten snapshot is intentionally empty (no
  Needs access at that arm) — needs the caretake-pass query work;
  waste-only post-R2b, not lethal. R1 (Handing churn: measured 8,972
  plans → 155 handoffs even in the HEALTHY baseline — chronic,
  pre-dates Phase II) re-homed to ticket 509's commitment-layer
  first-light (R3 there), evidence appended.
