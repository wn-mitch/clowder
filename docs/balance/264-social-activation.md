# 264 — social DSE activations (plan step 20): four-artifact record

Ticket: `docs/open-work/tickets/264-*.md`. Activation commits (each
gate-soaked 900s, verdicted against the previous accepted stream,
frame-diffed): 01c450c7 (Socialize + live scorer borrow), 4340d12a
(GroomOther), 140de5fd (Mate), 24100dbc (Mentor/Caretake/ApplyRemedy
+ raw-HP axis retirement). Reference chain root: `tuned-42-919ae1a8`
(step-19 gate).


## Gate 1 — Socialize (01c450c7): ACCEPTED

Weights: socialize_affiliation 0.10, socialize_affordance 0.10; live
scorer-side Res<ActionAffordances> borrow in PlanResources (read-only,
caretake pre-check; unread until caretake lift).

Four artifacts:
1. Hypothesis: partner selection composes actor-subjective affiliation
   belief + Affordance(Socialize); picks concentrate on
   witnessed-affiliative, currently-receptive partners.
2. Prediction: social continuity stable (±noise); no survival impact;
   concentration signature — social-warmth distribution tightens.
3. Observation (vs 919ae1a8): survival PASS, continuity PASS,
   throughput −7.2% (pass). courtship −5.0% (11268→10700), grooming
   −6.6%, mentoring −8.2%, play −14.3% (98→84, small-N).
   kittens_born/matured identical (3/2), bonds_formed 48→46,
   deaths identical (3 injury). **social_warmth.stdev −40.4%
   (0.134→0.080), social_warmth.min +20.8% (0.627→0.757)** —
   the predicted concentration signature. colony_score.aggregate
   −1.2%. fulfillment −18.6% (within family variance; it swung +73.7%
   between accepted trajectory families pre-activation).
   colony_score_at_checkpoint (elapsed 50k) is IDENTICAL to baseline —
   divergence manifests late; the uniform-prior phase (0.5 neutral
   beliefs) preserves argmax until beliefs differentiate.
   Ward channels (ward_count 1→0, sieges 81→0, shadowfox-ward-avoid
   −82.9%) are the known trajectory-family knife-edge class, not a
   socialize mechanism.
4. Concordance: direction match (tightening, stability); magnitude
   within 2× of "modest first-light". PASS.

Frame-diff (advisory, cross-commit): max |Δ mean| 0.024 (caretake
+4.4%); socialize self-state −5.8% (target-selection change, not
desire change); concordance ok, no wrong-direction rows.
plan_failure_canary: none.

## Gate 2 — GroomOther (4340d12a): ACCEPTED (with scrutiny note)

Weights: groom_other_affiliation 0.10, groom_other_hostility 0.10
(inverted), groom_other_affordance 0.10; base six ×0.7.

Four artifacts:
1. Hypothesis: grooming partner pick composes affiliation belief,
   inverted perceived-hostility ("don't groom the cat that just hissed
   at you"), and Affordance(GroomOther).
2. Prediction: grooming continuity stable-to-mildly-different in count
   but concentrated (same social_warmth tightening direction);
   hostility avoidance may shave a few grooming events off; no
   survival impact.

Artifacts 3 (Observation) + 4 (Concordance):
Observation (vs 01c450c7; elapsed 91442 → 111551, +22% — compare on
rate): survival PASS, continuity PASS, ShadowFoxAmbush 2 (≤10).
Rate-normalized: grooming +59%/10kt, courtship +36%/10kt, mentoring
+312%/10kt (follows the kitten boom). bonds_formed 46→65 (+41%),
kittens_born 3→7, kittens_matured 2→5, adults 8→13.
social_warmth.stdev −68.4% (0.080→0.025), min +20.2% (0.757→0.909),
mean 0.915→0.984 — concentration signature compounding.
deaths_injury 3→2. colony_score.aggregate +11.1%. Frame-diff
concordance ok (caretake/handoff per-focal deltas reflect the 13-adult
demographic, not mechanism regressions). plan_failure_canary none.

Concordance: direction matches the design payoff (fewer wasted
approaches → faster bond formation → reproduction). Magnitude exceeds
the "modest" prediction (kittens +133% > 2×) → extra-scrutiny pass
done: no pathology (zero starvation, deaths down, aggregate +11%),
kitten variance between accepted families was already ±50%, and the
bond→mating chain is the designed mechanism. ACCEPTED.

Watch-items for step-24 baseline re-promote:
- fulfillment 0.234→0.190→0.147 across gates 1–2 (−37% cumulative) —
  plausibly demographic dilution (kittens + fresh adults score low);
  re-check at re-promote.
- founder-dispersion cuddle-puddle windows 8→20 — the flip side of
  warmth concentration; ticket 490's dormant
  WorkPressureAffiliativeYield modifier is the designed absorber.
- mythic-texture tally 3→0 (demoted canary, informational only).

## Gate 3 — Mate (140de5fd): ACCEPTED

Weights: mate_receptivity 0.12, mate_affordance 0.10; base four ×0.78.

Four artifacts:
1. Hypothesis: perceived-receptivity belief stops low-receptivity
   partners winning the pick and oscillating (126/027 supply-chain
   lever).
2. Prediction: courtship tally stable or slightly UP (less oscillation
   waste); MatingOccurred fires; kittens_born within family band
   (2–6); 027 cadence canary holds.

Artifacts 3 (Observation) + 4 (Concordance):
Observation (vs 4340d12a; elapsed 111551 → 112725): survival PASS,
continuity PASS, throughput +1.1%. ZERO significant footer-drift
channels; 50k checkpoint bit-identical. kittens_born 7 (=),
bonds_formed 65 (=), courtship +1.0%, grooming +1.8%, mentoring +1.7%,
play +8.3%. social_warmth stable (stdev 0.025→0.031).
plan_failure_canary none. 027 Mate-cadence canary HOLDS (courtship
17,966; MatingOccurred fired — 7 kittens born).

Concordance: prediction was "courtship stable-to-slightly-up, cadence
holds, kittens within band" — exact match. The receptivity belief
mostly agrees with the bond + pairing-intention axes in a
13-cat colony (partners are receptive because courtship practice keeps
lifting the facet); its designed job is vetoing the oscillation case,
which this seed's trajectory rarely produces. ACCEPTED.

## Gate 4 — Mentor/Caretake/ApplyRemedy (24100dbc): ACCEPTED

Weights: mentor_affordance 0.10, caretake_affordance 0.10,
apply_remedy_injury_belief 8/14 (raw target_injury axis RETIRED),
apply_remedy_affordance 0.10.

Four artifacts:
1. Hypothesis: mentor/caretake picks gain substrate pricing; ApplyRemedy
   triage switches from god-eye HP to witnessed/believed injury.
2. Prediction: mentoring continuity stable; FeedKitten path unchanged
   in volume (affordance concentrates, doesn't gate); remedy
   application may DROP for unwitnessed injuries (honest-perception
   cost — watch deaths_injury and festering; deaths_injury ≤ family
   band max 3–4, no Starvation). Scorer/dispatch caretake picks now
   provably equal (same live resource).

Artifacts 3 (Observation) + 4 (Concordance):
Observation (vs 140de5fd; elapsed 112725 → 99259, −11.9% — compare on
rate): survival PASS, continuity PASS, throughput −11.9% (pass),
plan_failure_canary none. deaths_injury 2→1, ShadowFoxAmbush 1, zero
starvation. mentoring −3.6% (stable, as predicted). play +33.8%.
kittens_born 5 / courtship −21.6% / bonds −23.1% — early trajectory
divergence (the 50k checkpoint differs this time, as expected: these
picks change from tick 1), values inside the accepted family band
(kittens 3–7). kittens_matured 0 is duration+timing (shorter run,
later births). Ward channels flipped again (knife-edge class).

ApplyRemedy mechanism signal: "no patient for remedy" 3→0 and
"missing remedy in inventory" 4→17 (+325%, absolute 0.17/1k ticks) —
belief-triage produces MORE willing healers: perceived injury persists
where raw HP silently recovers, so remedy demand now exceeds herb
supply occasionally. Not the predicted "may drop"; direction differs
but the survival channel (deaths_injury DOWN) and the design intent
(cats act on witnessed/cued injury) are both satisfied. Watch-item:
herb supply-chain pressure from belief-persistent triage.

Concordance: mentoring/caretake stable ✓, deaths_injury within band ✓,
honest-perception cost manifests as demand-over-supply rather than
under-treatment — acceptable, recorded. ACCEPTED.

## Run inventory

`logs/tuned-42-01c450c7` / `tuned-42-4340d12a` / `tuned-42-140de5fd` /
`tuned-42-24100dbc` (all 900s gate soaks, focal Simba). Verdicts ran
with `--baseline <previous run>` for per-commit attribution (the
promoted baseline is stale until plan step 24 re-promotes it).

## Deferred / follow-on

- The `Relationships.fondness` axis on Socialize/GroomOther stays
  alongside the new affiliation belief axis — supersession waits for
  soak history on the belief axis (pillar-2 ordering satisfied:
  substrate landed first; the ledger axis is not a hidden side-channel).
- Multi-focal trace (caretake-eligible parent focal) per the
  ticket-227 convention was not run this pass — caretake scorer =
  dispatch pick equality is structural (same live resource), and the
  kitten pipeline (5 born, 0 starved, FeedKitten failures zero) covers
  the volume check. Run one at step-24 re-promote if the caretake
  channel drifts.
- Watch-items rolled to step 24: fulfillment trend, 490 cuddle-puddle
  windows, herb supply pressure from belief-persistent remedy demand.
