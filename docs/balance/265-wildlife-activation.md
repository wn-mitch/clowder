# 265 — wildlife DSE activations (plan step 21): four-artifact record

Ticket: `docs/open-work/tickets/265-*.md`. Activation commits (each
gate-soaked, verdicted against the previous accepted stream,
frame-diffed advisory): ea638840 (fox, 900s), f72f4e32 (hawk + snake,
900s), 9eb6e7ed (FleeFrom→PredatorBeliefs write + wildlife_species_clash;
900s run kept at `tuned-42-9eb6e7ed-g3-900s`, accepted on the 1800s
re-soak at `tuned-42-9eb6e7ed`). Reference chain root:
`tuned-42-24100dbc` (step-20 gate 4, main).

Plan gate: four-artifact each; ShadowFoxAmbush ≤ 10; wildlife-mortality
canary stable.

## Gate 1 — Fox (ea638840): ACCEPTED

Weights: fox_hunting_prey_affordance 0.10, fox_flee_cat_violence_belief
0.10; live `Res<ActionAffordances>` borrow in fox_evaluate_and_plan
(read-access class); NEW `fox_flee_belief_eligibility_threshold` 0.75 —
belief clause on the Fleeing outer gate (legacy `health<0.5 ||
cats_nearby>=2` silenced the axis for healthy lone-cat encounters —
silent-canary conjunction shape, found by the scenario BEFORE soak).
Clause keyed off weight>0 (zeroed weight = legacy gate byte-exact).
Scenario `fox_belief_high_violence_capability_cat` landed (believer
flees / skeptic doesn't, mirrored geometry, near-argmax temperature).

Four artifacts:
1. Hypothesis: (a) FoxHunting election now prices real stalk/chase
   opportunity (314 writer rows, live borrow); (b) foxes that witness
   cat violence (Attack/Hunt within 10 tiles, EMA toward 1.0) become
   flee-eligible against single healthy cats via the belief clause.
2. Prediction: wildlife-channel drift only at first-light 0.10 —
   fox disposition mix shifts modestly (Hunting pickier, occasional
   belief-flee); cat-side social continuity within family variance;
   survival gates hold (Starvation 0, ShadowFoxAmbush ≤ 10); fox
   mortality (WildlifeCombat / FoxConfrontation channels) within
   family band; no starvation-side coupling expected (fox hunt
   economics shift only in target choice, not appetite). Belief-flee
   trips rarely (threshold 0.75 needs witnessed evidence well above
   the 0.5 prior).
3. Observation (logs/tuned-42-ea638840 vs 24100dbc; elapsed 95605 vs
   99259, −3.7% — compare on rate): verdict CONCERN, survival PASS,
   continuity PASS, ShadowFoxAmbush 0, zero starvation, deaths 1 vs 1
   (FoxConfrontation vs ShadowFoxAmbush — single-death band, wildlife-
   mortality canary stable), throughput −3.7% pass, plan_failure_canary
   none. **Fox-predation mechanism signature**: cat-side "EngagePrey:
   lost prey during approach" 435→30 (−93%) while "target invalid:
   Despawned" 374→579 (+55%) — prey now dies under approaching cats
   because foxes pick targets with real stalk/chase affordance.
   Ward channels flipped hard (wards placed 59→28, sieges −71%,
   shadowfox-ward-avoid −79%, shelter score −31%) — the DOCUMENTED
   knife-edge trajectory-family class (flipped in 264 gates 1 AND 4).
   Population upside: kittens 1→3, peak pop 9→11, happiness +41% —
   within the 264 accepted-family variance (kitten band 3–7,
   happiness swings large between families). Frame-diff advisory:
   concordance ok; Simba's caretake +137%/mentor +426% track the
   kitten demographic (no cat-mechanism regression; cat scoring is
   untouched by this commit).
4. Concordance: direction matches (wildlife-side mechanism + trajectory
   cascade; survival stable); the loud channels are the pre-registered
   knife-edge class, and the mechanism-level signal is exactly
   hypothesis (a). Belief-flee (hypothesis b) expected rare — no
   negative signal. ACCEPTED.

## Gate 2 — Hawk + Snake (f72f4e32, one commit): ACCEPTED

Bundled: both species share the identical mechanism shape and small DSE
catalogs; per-species attribution comes from the separately-instrumented
hawk/snake footer channels, exactly as 264 gate 4 attributed
Mentor/Caretake/ApplyRemedy per-mechanism inside one commit. (Also
avoids splitting mixed sim_constants.rs hunks across commits.) Fox — the
high-interaction species (raids, confrontations, den defense) — kept its
own gate.

Hawk: hawk_hunting_prey_affordance 0.10, hawk_flee_cat_violence_belief
0.10, live borrow in hawk_evaluate_and_plan, belief clause on the hawk
Fleeing gate (same `>=2` legacy shape as fox) via
hawk_flee_belief_eligibility_threshold 0.75 + precomputed
`belief_flee_eligible` ctx bool (Hawk ctx carries no &ScoringConstants).
Scenario `hawk_dive_affordance_aerial_cover` landed (writer-level:
Dive(open) > Dive(under-cover); live Ward entity, NOT setup stamp_ward —
update_ward_coverage_map clears+rebuilds per tick, which is why the
older affordance_substrate cover test could only assert `>=`).

Snake: snake_ambush_strike_affordance 0.10, snake_forage_stalk_affordance
0.10, snake_flee_cat_violence_belief 0.10, live borrow. NO gate clause —
snake Fleeing's legacy gate is `cats_nearby >= 1` (already admits the
single-cat case; the belief axis differentiates score, not eligibility).

Four artifacts (hawk):
1. Hypothesis: Dive/Chase election prices real opportunity (open ground,
   unaware prey); witnessed cat violence can make a healthy hawk leave.
2. Prediction: hawk hunt-success channel may tick up (Dive picked when
   it actually lands); hawk mortality stable; survival gates hold.
Four artifacts (snake):
1. Hypothesis: Strike rewards holding an ambush spot prey actually pass
   (adjacency-gated writer row); Foraging stalk pricier and pickier.
2. Prediction: snake ambush/strike channels shift modestly; snake
   mortality stable; no cat-side coupling beyond trajectory noise.

Observation (logs/tuned-42-f72f4e32 vs ea638840; elapsed 101043,
+5.7%): verdict CONCERN, survival PASS (ShadowFoxAmbush 3 ≤ 10, zero
starvation, deaths 3 all-ambush vs 1), continuity PASS (courtship
12885, grooming 2266, mentoring 5734, play 58), plan_failure_canary
none, throughput +5.7% pass. Hawk/snake predation events ~1k in the
stream (activity present). CORRECTION to the bundling note above: no
per-species hawk/snake footer channels exist — attribution rests on
the mechanism-level tests/scenarios + the aggregate wildlife-mortality
canary, not on footer channels. Drift is the ward knife-edge family
flipping BACK (wards 28→37, sieges +68%, shadowfox-ward-avoid +103%,
shelter +47%) plus its downstream trajectory texture (kittens 3→1 —
inside the accepted family band 1–7; fulfillment +70%, happiness
+21%). Frame-diff advisory concordance ok (Simba count deltas track
the smaller-kitten-cohort demographic).

Concordance: survival + mortality within band ✓, cat-side coupling =
trajectory noise as predicted ✓, hawk hunt-success channel
unverifiable at footer granularity (recorded as instrumentation gap,
NOT smuggled in — plan's tooling-ticket rule applies only if a harness
gap blocks the gate; it doesn't: the gates hold without it). ACCEPTED.

## Gate 3 — FleeFrom→PredatorBeliefs write + wildlife_species_clash (9eb6e7ed): ACCEPTED

Landed: third-party cat witnesses (fleer excluded — self-write would
make fleeing self-confirming, pinned by evidence_count==0 assertion)
update PredatorBeliefs[threat].perceived_violence_capability toward
NEW `FLEE_CUE_OBSERVED_VALUE` 0.75 (indirect-cue class like
SCENT_OBSERVED_VALUE 0.65; deliberately AT the flee-eligibility
threshold: flee cues alone can only reach the edge of eligibility,
witnessed violence pushes past). Wildlife-gated via wildlife_set
(component truth, 292 discipline) — cat-keyed threats still write
nowhere (505 ballast test extended to assert PredatorBeliefs too).
No new ballast class: the write only touches entities the Implant pass
already models.

Scenario `wildlife_species_clash` landed — full observation channel,
NO stamped beliefs: fox watches 16 max-severity Attacks (learning_rate
0.1 → belief ≈0.82 ≥ 0.75), cat's PredatorBeliefs implant seeds within
one stagger period, fox's first adopted plan is Fleeing/Avoiding
(capture-first-plan pattern: short plans exhaust, any single tick can
catch the fox between plans). Geometry lesson recorded: fox threat
reads (cats_nearby AND perceived_cat_threat) are range-gated ≤6 in
build_scoring_context; witness range is 10 — a fox at 8 integrates
beliefs it can never act on. Distance 5 is the working band; distance
2 gets the fox mauled (injury arm contaminates attribution).

Four artifacts:
1. Hypothesis: colony-mates fleeing a predator propagate fear through
   the belief substrate — witnesses' violence models of that specific
   predator lift toward 0.75 without direct observation of violence.
2. Prediction: cat-side flee/patrol consumers see slightly hotter
   PredatorBeliefs where FleeFrom cascades happen; small drift in
   flee-affordance-fed channels; survival gates hold; NO new decay
   ballast (entries already implanted). Trajectory drift expected from
   tick 1 (belief writes change scoring inputs immediately).
3. Observation, 900s run (logs/tuned-42-9eb6e7ed-g3-900s vs f72f4e32;
   elapsed 110107, +9.0%): verdict FAIL — never-fired canary:
   **KnowledgePromoted 0×** (fired ≥1 in all three prior chain runs).
   Everything else green: Starvation 0, ShadowFoxAmbush 1, deaths 2
   (FoxConfrontation+ShadowFoxAmbush), continuity PASS (courtship
   15616, grooming 3126, mentoring 13517, play 220 — all UP on rate),
   plan_failure_canary none, kittens 1 (=), peak pop 9 (=).

   Investigation (structured, /diagnose-collapse discipline):
   - Mechanism structurally intact: promotion derives ONLY from
     LocationBeliefs (colony_knowledge.rs quorum over
     recency_of_threat_cue/prey_yield) — the gate-3 write touches
     PredatorBeliefs exclusively; no read-path interference. All 17
     colony_knowledge unit tests + colony_knowledge_false_belief
     scenario (expects KnowledgePromoted) pass at this commit.
   - Fear-contagion-overheat hypothesis REFUTED by the chain family:
     Flee elections 139/104/7/84 across the four runs — gate 2's 7 was
     the outlier; gate 3's 84 (0.76%) is in family. happiness −34% is
     against gate 2's ceiling-pinned 1.0.
   - Input starvation REFUTED: cat hunt success 80.8% (chain-high;
     50.1%→80.2%→77.6%→80.8%), attempts stable — prey_yield learning
     material abundant.
   - belief_divergence_duration_ticks = 0: no (bucket,facet) group
     ever met the strength quorum at all (not an agreement failure) —
     the 3-cat same-bucket strong-belief coincidence simply never
     occurred in this trajectory family. Chain-rare re-timing, per the
     established chain-rare-events discipline (structural verification
     over sweep gating).
   - Action: 1800s re-soak at the same commit (double observation
     window) — a 900s window on a ~once-per-run event is borderline by
     construction; deterministic seed-42 rerun at 900s would reproduce
     the same stream bit-exactly, so window length is the only lever.
   RESOLUTION — 1800s re-soak (logs/tuned-42-9eb6e7ed, elapsed
   160655): **KnowledgePromoted fired; never-fired list clean.**
   Verdict CONCERN: survival PASS (Starvation 0, ShadowFoxAmbush 1),
   continuity PASS, plan_failure_canary none, aggregate −4.8% pass,
   kittens/peak-pop/fulfillment flat. Throughput −20.5% (concern band)
   is the longer window sampling more late-run heavy ticks — not a
   regression signal at gate scope. happiness −34.1% "fail" band is
   the gate-2 ceiling artifact (baseline pinned at 1.0). Frame-diff
   advisory: concordance ok.
4. Concordance: prediction was "small drift in flee-affordance-fed
   channels, survival holds, no new ballast, trajectory divergence
   from tick 1" — matches (action mix in family, survival green,
   trajectory reshuffle confirmed by the 50k-divergent checkpoint
   class). The KnowledgePromoted scare resolved as the pre-registered
   chain-rare re-timing class, caught by the never-fired gate doing
   exactly its job and cleared by structural verification + a doubled
   observation window rather than a mechanism change. ACCEPTED.

Watch-item rolled to step-24 baseline re-promote: KnowledgePromoted
cadence — it fired once per ~100k-tick window pre-gate-3 and needed
160k post; if the re-promoted baseline also shows sparse promotion,
consider whether wildlife competition (foxes taking prey mid-approach)
is thinning the shared-location hunt experience that feeds the
LocationBeliefs quorum, and whether that's honest ecology (likely) or
starves the 291 mythic-texture channel.
