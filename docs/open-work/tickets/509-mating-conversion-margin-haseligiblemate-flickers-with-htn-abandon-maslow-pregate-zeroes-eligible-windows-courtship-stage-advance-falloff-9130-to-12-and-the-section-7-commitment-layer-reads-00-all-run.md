---
id: 509
title: Mating conversion margin: HasEligibleMate flickers with HTN Abandon, Maslow pregate zeroes eligible windows, courtship stage-advance falloff 9130-to-12, and the section-7 commitment layer reads 0.0 all run
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
The 493 collapse diagnosis (three-agent sweep over
`logs/tuned-42-07acc090` trace + events) found the mating pipeline
converting at a knife-edge margin — the reason seed-42 fertility zeroes
under ANY trajectory perturbation (observed twice this session: the
502 lex-pin run and the 493 run). The colony courts constantly and
almost never converts. This ticket consolidates four verified findings
so the Phase IV mate-activation step (plan.md step 20, 027 Mate-cadence
canary) starts from evidence, not a blank audit.

## Current architecture (layer-walk audit — all `[verified-*]` from the 07acc090 trace)

| Layer | Finding | Evidence |
|---|---|---|
| Eligibility marker | `HasEligibleMate` true **20 of 9117 ticks (0.22%)** for Simba, in two short bursts, with 1-tick flickers inside the bursts | L2 records; bursts 1231719-1231737, 1236051-1236208 |
| Maslow pregate | In eligible window 1, `maslow_pregate = 0.0` zeroes Mate's final score despite raw composition 0.69 — Mate never contends | L2 records 1231719-1231737 |
| HTN failure strategy | `mate_with_goal` uses `MethodFailure::Abandon` — any 1-tick `HasEligibleMate` drop silently kills the in-flight Mating plan (no PlanStepFailed, no retry); `TravelTo(SocialTarget)` never gets multi-tick runway | src/ai/methods/mating.rs doc + trace: 1 real election at 1236166, plan superseded ~12 ticks later |
| Commitment layer | `momentum.commitment_strength == 0.0` and `active_intention == null` on ALL 9117 L3 records — the pillar-4 softmax+persistence layer is inert for this cat/run; nothing holds Mate (or anything) against re-scoring churn | L3 trace, whole run |
| Stage advance | Colony-wide cumulative: JointIntentionEmitted{Courtship}=9130, JointBiasApplied=5071, **JointStageAdvanced=12**, MatingOccurred=1 | SystemActivation counters at 1313400 |
| Bonded-pair thrash | max+Basil (fondness/familiarity/romantic all 1.0, Partners): 12 Mating PlanCreated in a 200-tick window, each displaced by Exploring/Witchcraft/Cooking before TravelTo closes | events 1217828-1218012 |
| Trace artifact | L3 `chosen:"Mate"` persists 7 ticks after the plan was superseded (stale echo — ranked list lacks Mate, L2 shows ineligible) | ticks 1236178-1236184; fold into 163's trace-fidelity scope |

## Fix candidates (NOT to be shipped from this ticket blind — each needs its own pass)
- R1 (marker hysteresis) — `HasEligibleMate` gets a hold-band (like
  BefriendedAlly's hysteresis) so 1-tick flickers don't kill plans.
- R2 (failure strategy) — `mate_with_goal` Abandon → a bounded retry /
  backtrack while the pair's bond state still qualifies.
- R3 (**commitment layer first-light**) — investigate why
  `commitment_strength` is 0.0 across an entire run. If the §7
  persistence bonus never engages, EVERY multi-tick social plan is
  fighting per-tick re-scoring unaided — the thrash is general, mating
  is just where it kills a canary. Pillar 4 says the L2 trace must
  show the held Intention's persistence-bonus offset.
- R4 (Maslow interaction) — audit why tier-pressure zeroes Mate inside
  an eligibility window (correct suppression or a tier misread?).

## Recommended direction
R3 first (it may subsume R1/R2's symptoms), then re-measure the
margin. Fold into / sequence with plan.md Phase IV step 20 (mate
activation + 027 canary) — do NOT tune mate constants before the
commitment layer question is answered (park-demographic-tuning
discipline).

## Out of scope
- The ambush-hotspot deaths that zeroed the 07acc090 breeder pool —
  ticket 508 (landed fix: ThreatBeliefOverlay).

## Verification
Post-fix soak: MatingOccurred >= 2 per 900s at seed 42 with kittens
born; JointStageAdvanced/JointIntentionEmitted ratio up an order of
magnitude; L2 trace shows persistence-bonus offsets on held intentions.

## Log
- 2026-07-05: opened from the 493 /diagnose-collapse sweep. All rows
  verified against logs/tuned-42-07acc090.
