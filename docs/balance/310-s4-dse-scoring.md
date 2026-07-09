# 310 S4 — DSE-shaped shadow-fox scoring: four-artifact record (5 iterations)

Ticket: `docs/open-work/tickets/310-*.md` (S4; release-plan step 23).
Commits: 2484bdb8 (DSEs + dispatcher + election integration + roll
retirement + affordance slice) → 344e6d39 (hunt-pool ward parity) →
1d181030 (retreat fed-eligibility) → 246153e0 (home range + den-rest) →
910a1cb7 (retreat election closed dormant — ACCEPTED gate artifact).
Baseline throughout: `tuned-42-aa365199` (S3 accepted artifact).

## Shape (pillar 4 — one election)

`shadowfox_{hunt,retreat,patrol}` DSEs in the DseRegistry, scored via
`shadowfox_scoring.rs`'s hand-written dispatcher (silent-inert rule) as
eligibility-gated CANDIDATES inside the single 023 motivation softmax,
next to the four hand-scored corruption drives. No second elector. The
legacy 5%/tick stalk roll retired in the same commit its substrate
replacement landed (pillar 2). Day-phase texture via `night_scalar`
(1.0 night / 0.7 twilight / 0.2 day) on the hunt and patrol axes. 265's
shadowfox affordance slice absorbed: the Ambush-vs-cat estimator was
re-keyed from ward coverage (anti-cover for a shadow-fox — inverted
ecology) to tile corruption, feeding the hunt DSE's conditional
first-light-0.10 axis; deterministic concealment-ordering test.

## What the five iterations taught

1. **Iteration 1 — ward parity.** The retired roll had an unwarded
   filter its DSE replacement lacked: foxes elected hunts on protected
   cats, stalks cancelled at the perimeter, and the foxes were left
   patrolling in ward coverage rolling sieges — WardSiegeStarted
   +1375% per 10kt. Every predator selection point needs ALL the
   legacy filters carried over (the third occurrence of this class
   after S1's eligibility and S3's retarget).
2. **Iterations 2–4 — the retreat oscillator.** FED-and-far is a
   conjunction a WeightedSum cannot express (den-distance alone scored
   ~0.54); with the fed half moved to eligibility, prey kills held
   foxes fed for ~4.8k-tick stretches and they shuttled their home
   range under every shape tried (299 → 184 elections per 900s;
   arrival-radius eligibility, home-range radius 6.0, den-rest arrival
   all landed but did not close the churn). Two consecutive discordant
   iterations on one channel → close-the-clade: the retreat candidacy
   ships DORMANT (`shadow_fox_retreat_election_scale` 0.0). S2's
   mechanism-exact event-driven post-ambush retreat remains THE
   retreat; den-rest arrival (Waiting on corrupted ground, coherence
   recovering) stays; activation waits on a rest-drive design
   (follow-on ticket).
3. **Iteration 5 — accepted.** Retreats 14 ≈ ambushes 12 (event path
   only); hunts 13 at satiation 0.538–0.699 — the deliberate
   near-threshold regime; sieges at family magnitude; survival and
   continuity PASS; throughput −6.7%.

## The posture shift (explicit, rolled to steps 24–25)

Goal-directed hunting raised predation engagement ~6× over the
pinball era (12 spread ambushes + 261 Haunting entries per 900s vs the
S3 family's 2 + 16). The colony visibly pays: health −20.8%, welfare
−9.4%, fulfillment −19.8%, MentorCat-incapacitated plan-failures 11×.
Ambushes are SPREAD (no waves — the satiation gates hold); the cost is
chronic psychological pressure from proximity. This is the ticket's
designed direction — the defense story belongs to the cats' substrate
(263/264 beliefs, wards, patrol cooperation) — landed at a first-light
intensity stronger than predicted. Predation-pressure posture is now a
named step-24 re-baseline item and feeds step 25's hunt-success band
tuning; the first knobs are the hunt eligibility threshold (shared
`shadow_fox_stalk_satiation_threshold` 0.7), the night weighting, and
Dread's group-suppression — all trace-visible.

## Escape hatches

- `shadowfox_hunt_cat_ambush_affordance_weight = 0.0` → three-axis hunt
  composition byte-exact.
- `shadow_fox_retreat_election_scale` stays 0.0 (dormant) — lifting it
  is the follow-on's four-artifact, not a config toggle.
- The 023 corruption drives and their softmax are untouched at zeroed
  DSE participation only insofar as candidates simply don't stand;
  there is no byte-exact legacy hatch for S4 as a whole (the roll
  retirement is a substrate swap, priced by this gate).

## Verdict

S4 **accepted** at 910a1cb7. Rolled forward: rest-drive follow-on
(retreat election activation); predation-posture at step 24/25; the
near-zero-drive election sibling on the four 023 drives (from S1)
remains open.
