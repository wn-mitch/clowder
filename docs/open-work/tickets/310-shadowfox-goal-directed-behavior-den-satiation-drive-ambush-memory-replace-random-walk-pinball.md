---
id: 310
title: ShadowFox goal-directed behavior — den + satiation drive + ambush memory replace random-walk pinball
status: ready
cluster: wildlife
orchestration: substrate-sensitive
initiative: [predator-prey-dynamics]
added: 2026-05-13
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

ShadowFoxes are random-walking threats with a stalk trigger, not goal-directed predators. The behavior in `wildlife.rs::wildlife_ai` + `predator_stalk_cats` is: patrol with jitter → bounce off wards → if a cat is visible and not behind a ward, 5%/tick chance to flip into Stalking → walk straight at the target → adjacent ambush. No den, no satiation drive, no memory of prior hunts, no pack coordination, no time-of-day preference. Pinball with a kill rule.

Pre-260 this gap was masked by the `shadow_fox_ward_repel_multiplier = 3.0` constant: ShadowFoxes were repelled from a 27-tile zone around each ward, vastly wider than the ward's actual coverage radius (~9 tiles). The colony's safety came not from intelligent predator avoidance but from a hardcoded radius that kept ShadowFoxes pressed against the map edges. Ticket 260's substrate-honesty work surfaced this — when fox avoidance reads `WardCoverageMap` (the substrate-visible cat-perceivable radius) instead of the multiplied snapshot, ShadowFoxes can wander into the 9-27 tile band freely. Combined with the 5%/tick stalk trigger and an unwarded pocket in the colony (separate failure mode tracked in ticket 308 / 309), the result is a 7-cat ambush wave once the conditions align.

The fix isn't to re-paper over the gap with `WardMagicMap` (the half-built exploration deleted from 260). It's to give ShadowFoxes their own BDI-shaped substrate: a den as a Belief, satiation/hunger as a Desire-layer drive, ambush-and-retreat as an Intention with its own commitment strategy. Once predators have goal-directedness, the colony's defense story shifts from "wards stop them at unrealistic distance" to "predator and prey both reason; cats out-think predators through layered defense (wards + scent + patrol cooperation + intelligence about predator behavior)."

## Scope

- ShadowFox-specific den entity + spawning behavior (analogous to FoxDen but distinguished — supernatural origin, may emerge from corruption-saturated tiles).
- Satiation drive on `WildAnimal` for the ShadowFox case (or species-keyed extension) — successful ambush satiates; satiation decays; high satiation suppresses stalk eligibility.
- Ambush memory: `RecentAmbushMap` already exists (ticket 219) as colony-shared, but ShadowFoxes themselves don't read it. Each ShadowFox should retain a per-entity "last successful hunt site" and a cooldown that biases AWAY from that site (predators-don't-hunt-fished-out-ponds).
- Post-ambush retreat: instead of cooldown-and-resume-patrol, a successful ambush triggers retreat-to-den with a state machine (Fleeing back along their entry corridor).
- Time-of-day preference: ShadowFoxes are corruption-born — twilight/night should weight their patrol vs rest cycle distinctly from prey-shape foxes.
- Substrate visibility: each of these decisions must be trace-visible (DSE consideration or marker), not hidden in `wildlife.rs` movement-layer side channels.

## Out of scope

- Pack coordination / JointIntention-shape behavior for ShadowFoxes (deferred — interesting follow-on; not load-bearing for the immediate gap).
- Hawk / snake parallel work (each has its own ecological shape; this ticket is ShadowFox-specific).
- The fox-side ward-perception substrate-honesty work parked in 260 — that residue work *unblocks* once 310 lands, because predator AI then justifies a substrate-honest avoidance gradient instead of a safety blanket.
- Regular Fox behavior (`behavior_loop`) — already has more structure (FoxState, FoxAiPhase, fox_evaluate_and_plan). Improvements there are separate work.

## Current state

- `wildlife.rs::wildlife_ai` (lines 72-291) handles ShadowFox patrol-state movement. After ticket 260: ward avoidance reads `WardCoverageMap` (9-tile coverage) AND `CatScentMap` (cat-scent avoidance, additive). Both substrate-visible. Stalk-cancel uses same `WardCoverageMap` read.
- `wildlife.rs::predator_stalk_cats` (875-1050) handles ShadowFox stalk initiation + ambush. Still uses the pre-260 hardcoded `ward_positions × multiplier` snapshot for ward avoidance + cat-detection filter. This is the residual inconsistency 260 didn't close — should resolve as part of the predator-AI substrate work here.
- `RecentAmbushMap` (219) tracks colony-shared ambush memory but ShadowFoxes don't read it.
- No den / satiation / per-entity memory on ShadowFox. The species ships with `BehaviorType::Patrol` and the patrol logic is the same shape as regular Fox / Hawk patrolling.

## Approach

Apply the BDI mapping to ShadowFoxes:
- **Belief**: lightweight MentalModel (or simpler per-entity state) carrying `den_position`, `last_kill_site`, `last_ward_encounter_tick`. Influences stalk-target selection and retreat geometry.
- **Desire**: satiation drive (low satiation → high Hunt-equivalent score), territory drive (lifts patrol-toward-den when far away), avoidance drive (lifts retreat when sated or recently ambushed at this site).
- **Intention**: explicit `ShadowFoxIntention` enum (`Hunt`, `Retreat`, `Patrol`, `EncircleWard`) with commitment strategies — Hunt is `SingleMinded` until satiation or target loss; Retreat is `SingleMinded` until at den.

Then the wildlife_ai vs predator_stalk_cats inconsistency dissolves — both systems read the same substrate-visible signals.

Order of work likely: (1) lift the satiation drive (smallest mechanism change, biggest behavior change), (2) add den + retreat geometry, (3) ambush memory + per-entity belief, (4) intention-layer integration, (5) retire the hardcoded ward snapshot in predator_stalk_cats.

## Verification

- Scenario: spawn ShadowFox + 4 cats, observe the predator hunt-retreat-hunt cycle over 200 ticks; assert satiation rises after ambush and the next stalk waits until satiation decays.
- Scenario: spawn 2 ShadowFoxes near a kill site; second ShadowFox should NOT re-stalk at the same site immediately (per-entity memory).
- Soak: ShadowFoxAmbush count distribution should shift from "occasional wave when conditions align" to "spread across run" — predators reason about prey-availability, leading to more sustainable predator-prey dynamics.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- · **245** (blocked, wildlife, score 0.90) — Ambient predator/prey behavior-observation enrichment
- · **100** (ready, wildlife, score 0.88) — Tremor map, Action::Stalk, and personality-driven hunt approach
- · **294** (ready, belief-perception, score 0.87 (cross-cluster)) — RecentAmbushMap retirement — colony Resource moves to per-cat LocationBeliefs.r…

<!-- linkages:end -->
## Log

- 2026-05-13: opened from ticket 260's verification soak discovery. The substrate-honesty work on ward perception revealed that ShadowFox behavior beneath was already shallow (random walk + 5%/tick stalk trigger + no memory). 7-cat ambush wave at (25-39, 20-23) over ticks 1309038-1314568 confirmed the gap. Pre-260's 27-tile ward radius was hiding this; the right response is predator AI substrate, not re-papering with a multiplied ward map.
- 2026-05-19: accuracy audit pass — 260 (context event) is landed; all file paths verified; BDI-shape mapping is sound; RecentAmbushMap (294 dependency) is live; approach is predator-ecology grounded.
- 2026-07-09: **S1 landed (satiation drive)** — release-plan step 23; commits 12c5eea9 (drive: fifth motivation-softmax input, ambush/prey-kill gains, cadence decay, stalk-suppression gate, `shadowfox_hunger_hunt_cycle` scenario, `ShadowFoxHungerHuntEntered` feature+event) → 55c75f5c (hunger eligibility gate) → ef08d805 (escalation satiation gate) → 1effd660 (KnowledgePromoted per-soak gate demotion, user-approved). Full four-artifact record: `docs/balance/310-s1-satiation-activation.md`. Two soak-caught structural defects fixed mid-gate: (1) the weight-gated hunger candidate was electable via softmax temperature spread at satiation 0.98 → eligibility-before-scoring (pressure ≥ motivation floor); (2) the 023 Haunting-escalation loop was satiation-blind and formed a positive feedback loop (ambush → victim mood/safety tank → Dread → Haunting → 30-tick escalation → re-ambush; 71 ambushes, ~45-tick same-cat trains) → satiation now gates all THREE physical-predation entries (legacy roll, hunger election, escalation); fed foxes keep haunting without striking. Accepted artifact `tuned-42-1effd660` (concern-band: survival/continuity PASS, never-fired clean); 1800s window shows the genuine cycle (hunger elections at satiation 0.055–0.078, one ambush per ~150k ticks). Rolled to S4: hunger pricing prey-hunts; near-zero-drive election sibling on the four 023 drives. Also surfaced: soak-harness frame-hitch double-emission → ticket 517.
- 2026-07-09: **S2 landed (den + post-ambush retreat)** — commit 571815fd; record `docs/balance/310-s2-den-retreat.md`. `ShadowFoxDen` world entity at the corruption origin (reuse radius 8 bounds accumulation), `den_position` on drives (serde-default None; S3 migrates it into `ShadowFoxBeliefs`), SingleMinded `WildlifeAiState::Retreating` entered on a landed ambush (motivation-guard held, `steering::arrive`, released at `shadow_fox_retreat_arrival_radius`). Gate `tuned-42-571815fd`: concern-band, survival/continuity PASS; mechanism exact — one ambush with paired `ShadowFoxRetreatEntered` same tick. Scenario extended to the full hunt-feed-retreat cycle and de-raced (Waiting start — the S1 pass had been trajectory-family luck; patrol drift could beat the first motivation cadence into legacy detection range). Watch-item sharpened: fulfillment −13..−16% across four consecutive families — trend-shaped, check explicitly at step 24.
- 2026-07-09: **S3 landed (ambush memory)** — commit aa365199; record `docs/balance/310-s3-kill-site-memory.md`. `ShadowFoxBeliefs` place-memory component (den migrated from drives; last_kill_site/tick on ambush; MentalModel migration path documented in rustdoc; last_ward_encounter deferred to S5 with its reader). Kill-site consideration at ALL THREE target selections — the scenario caught that the active-stalk retarget was a third, unfiltered selection silently overriding elections (movement-layer selection, pillar violation; now filtered, empty pool holds the committed target). `ShadowFoxKillSiteAvoided` names memory-reshaped choices; `shadowfox_kill_site_avoidance` scenario hosts. Gate `tuned-42-aa365199`: concern-band, survival/continuity PASS, zero drift flags — the fulfillment −13..−16% streak breaks at four families. Interpretation call logged: per-entity memory semantics (killer avoids its own site) per the ticket's design text.
- 2026-07-09: **S4 landed (DSE-shaped scoring + time-of-day)** — commits 2484bdb8 → 344e6d39 → 1d181030 → 246153e0 → 910a1cb7 (accepted gate `tuned-42-910a1cb7`); record `docs/balance/310-s4-dse-scoring.md`. `shadowfox_{hunt,retreat,patrol}` in DseRegistry via `shadowfox_scoring.rs`, standing as eligibility-gated candidates in the SINGLE 023 motivation softmax (pillar 4); legacy 5%/tick roll retired; 265 affordance slice absorbed with the Ambush estimator re-keyed ward-cover → tile-corruption concealment; night_scalar day-phase texture. Five-iteration gate: (1) hunt-pool ward parity (sieges +1375% — third occurrence of the carry-ALL-legacy-filters class); (2–4) the retreat oscillator (WS conjunction trap; fed foxes shuttled 184–299×/900s under every home-range shape) → retreat election closed DORMANT per close-the-clade (`shadow_fox_retreat_election_scale` 0.0; S2 event path owns retreat; den-rest arrival stays) → **ticket 518** (rest drive). Accepted posture: 13 deliberate hunts at satiation 0.54–0.70, 12 spread ambushes, retreats = event path; predation engagement ~6× the pinball era with chronic Haunting pressure (health −20.8%) — designed direction, intensity flagged as a named step-24/25 posture item.
