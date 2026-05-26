---
id: 279
title: Body-cue-driven joint adoption (compose 127 with 242 + 243)
status: done
cluster: social-coordination
orchestration: substrate-sensitive
initiative: [generational-continuity, full-sensory-perception]
added: 2026-05-11
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 3f7bbad4c5e3
landed-on: 2026-05-26
---

## Why

The `JointIntention` module rustdoc (`src/components/joint_intention.rs:14-20`) names the perception channels the substrate stands in for: *"Real cats perceive each other's persistent practice-engagement through scent, posture, mounting tolerance, repeated proximity. We don't have those perception channels yet, so the substrate stands in."* Today's `belief_integrator` consumes `WitnessableEvent::Groom` / `Mate` / `Care` / `FleeFrom` / `Attack` / `Hunt` / `SelfPlanFailed` (`src/messages/witnessable_event.rs:25-155`) — the affiliative-practice and threat-response cues — but has no variants for the play-engagement-specific body cues (play-bow, sustained orientation, reciprocal advance, mounting tolerance) that real cats read to register a peer as engagement-eligible.

The behavioral consequence shows up in PlayBout (276 Commit A): the matchmaker emits 8247 JIs per 60k ticks because `MentalModel<other>.perceived_intent_clarity` has no PlayBout-relevant evidence source to accrue from — only generic socialize / groom witnessable events whose signal is too weak and too broad to discriminate "this cat wants to play with me right now" from "this cat is generally amiable." 279 wires the play-engagement cue variants into `belief_integrator` so 280's matchmaker rebind has the perception substrate it needs.

This is the upstream wiring for 280's matchmaker rebind; the two land in sequence. The cluster also subsumes a portion of the retired 469's "candidacy signal" scope — 469's audit identified the spec-honest home as `MentalModel.perceived_intent_clarity`; this ticket authors the witnessable events that update that facet from play-engagement cues.

## Scope

- **Add `WitnessableEvent` variants** in `src/messages/witnessable_event.rs`:
  - `PlayBow { actor: Entity, position: Position, tick: u64 }` — observable play-solicitation posture. Strongest play-engagement signal.
  - `ReciprocalAdvance { actor: Entity, target: Entity, position: Position, tick: u64 }` — actor moved into engagement range of target after a prior advance from target (or directly approached an actively-soliciting target). Mutual-engagement signal.
  - `SustainedCoPresence { actor: Entity, target: Entity, ticks_held: u32, position: Position, tick: u64 }` — actor and target remained within sensing range for ≥ `sustained_copresence_threshold_ticks` consecutive ticks. **Renamed from `SustainedOrientation` at implementation** (see 2026-05-26 Log entry): cats have no `Heading`/facing substrate (`Position` is grid-only), so the cue records continuous *co-presence* rather than directional facing — substrate-honest. Generic engagement signal (used by Courtship and PlayBout consumers).
- **Wire `belief_integrator`** (`src/systems/belief_integrator.rs`) to consume the new variants and update `MentalModel<actor>` facets on the witness:
  - `PlayBow` → `perceived_intent_clarity` (full lift) + `perceived_receptivity` (half lift).
  - `ReciprocalAdvance` → `perceived_intent_clarity`. When `target == witness`, full lift ("they advanced toward *me*"); third-party witnesses get half.
  - `SustainedCoPresence` → `perceived_intent_clarity`, scaled by `ticks_held / sustained_copresence_saturation_ticks`. When `target == witness`, full scaled value; third-party witnesses get half.
- **Emit the variants from resolver / system code**:
  - `PlayBow` — `src/systems/playbow_emitter.rs::emit_play_bows`: per-tick probabilistic emit for cats in `Action::Idle`/`Wander`/`Socialize`/`Explore` with `playfulness >= threshold`, `mood.valence >= threshold`, a peer in candidate range, and cooldown elapsed (`PlayBowCooldown` component).
  - `ReciprocalAdvance` — `src/systems/playbow_emitter.rs::emit_reciprocal_advances`: `CatMoved`-driven; when a moved cat lands within engagement range of a peer whose `PlayBowCooldown` shows a recent solicitation/reciprocation inside `reciprocal_window_ticks`, emit and stamp `last_reciprocal_advance_tick` to chain reciprocity.
  - `SustainedCoPresence` — `src/systems/sustained_copresence.rs::track_sustained_copresence`: per-tick tracker consuming `NearPairCache`; accumulates per-pair consecutive-tick counts in a `BTreeMap`, emits both directions at threshold with a per-pair cooldown, resets.
- **Per-cue tunables in `SimConstants`** — `PlayCueEmissionConstants` (emit thresholds / ranges / chance / cooldowns) holds the emit-cadence knobs; `BeliefsConstants.sustained_copresence_saturation_ticks` holds the belief-lift-shape knob (the integrator reads `BeliefsConstants`). Facet lifts reuse existing `perceived_intent_clarity` / `perceived_receptivity` `BeliefAxisTunables`.

## Out of scope

- The matchmaker emission gate that consumes the resulting `MentalModel` facets — that's 280's scope.
- `MountingTolerance` (a fourth cue named in JI's rustdoc) — Courtship-specific; gated on Body Zones epic for the integrity model. Open as a follow-on if 280's Courtship rebind needs it.
- `Scent` channel — broader sensory-perception epic; tracked under `initiative: full-sensory-perception` separately.
- Replacing the existing affiliative `WitnessableEvent` variants (`Groom`, `Mate`, `Care`) — those stay; this ticket adds play-engagement-specific siblings.
- Tuning per-facet `learning_rate` / `decay_rate_to_prior` for `perceived_intent_clarity` and `perceived_receptivity` from the new evidence sources. Initial defaults match the existing `Groom`/`Mate` lifts; balance-thread work tunes them post-land if 280's verdict shows over- or under-accrual.

## Current state

- **258 landed 2026-05-11.** `MentalModel<Cat>`, `belief_integrator`, `WitnessableEvent` consumer path. Existing affiliative variants (`Groom`, `Mate`, `Care`) drive `perceived_intent_clarity` and `perceived_receptivity` today.
- **127 landed.** `JointIntention` substrate; rustdoc explicitly names the perception channels this ticket wires.
- **242 / 243 blocked.** Body-cue marker substrate (`HeadDownCurled`, `TailPosture`, etc.); cited as related but not strictly required — 279 emits `WitnessableEvent` variants from system-level observations (action transitions, sustained pair-facing windows), not from marker reads. If 242 / 243 land later they refine the emit sites; they don't block this ticket.
- **280 ready, blocked-by 279.** The downstream consumer; reads the facets this ticket populates.
- **469 retired 2026-05-26** as substrate-duplicating. Its "candidacy signal source" framing folded here; its "mutual-perception emission gate" framing folded into 280.

## Approach

**Pre-flight.** Read `src/systems/belief_integrator.rs` to confirm the `match` on `WitnessableEvent` variants is exhaustive (per CLAUDE.md silent-canary discipline — adding a variant should be a compile error until classified). If a catch-all arm exists, retire it as part of this ticket's substrate-stub fix; otherwise the new variants land cleanly with three new match arms.

**Implementation order:**

1. Add the three new `WitnessableEvent` variants (no consumers yet — emit-only land). Run `just check` to confirm the integrator's match becomes a compile error or matches exhaustively as designed.
2. Wire `belief_integrator` arms for the three new variants. Update unit tests.
3. Author the `SustainedOrientation` per-tick tracker system (the most novel piece — needs a `HashMap<(Entity, Entity), u32>` tick counter). Add to `SimulationPlugin::build()`. Per CLAUDE.md ECS rules: this is `Message`-emit but the tracker itself is per-tick (event-driven would miss the sustained-facing accumulation). Justify per-tick in the system doc-comment.
4. Author the `PlayBow` emitter (small system reading `Personality.playfulness`, `Mood.valence`, `CurrentAction`, with candidate-range peer scan). The emission predicate roughly mirrors `is_playbout_eligible` but fires on individual cats irrespective of partner-perception — it's the *source* of the signal that the matchmaker (280) will then gate on.
5. Author the `ReciprocalAdvance` emitter — likely a small companion to `dispatch_step_action` or a per-tick scan over recent MoveTo completions joined against a per-cat `last_play_bow_tick` / `last_reciprocal_advance_tick` map.
6. Run `just check && just test`. Run `just soak-trace 42 Simba && just verdict` to confirm survival + continuity hard gates hold under behaviour-neutral landing (no consumers wired this tick).

**Behaviour-neutral at land.** Like 261, 279 lands the variants + integrator wiring + emit sites with no consumer change. `MentalModel<partner>.perceived_intent_clarity` accrues differently after land, but no DSE or matchmaker reads the changed values yet; `just verdict` against a pre-279 baseline shows null behavioral drift. The behavioral payoff lands with 280.

## Verification

- `just check && just test` clean.
- `just soak-trace 42 Simba && just verdict` — hard gates hold (Starvation 0, ShadowFox ≤10, all continuity canaries ≥1). Frame-diff against pre-279 baseline within ±5% on every DSE row.
- Focal-cat trace shows `MentalModel<partner>.perceived_intent_clarity` rising during sustained-orientation windows and after observed play-bows; decays under silence per existing `decay_rate_to_prior`.
- New unit tests in `src/systems/belief_integrator.rs` cover each variant's facet-update path.
- `belief_integrator`'s `match` over `WitnessableEvent` is exhaustive (no catch-all).

## Related work

<!-- linkages:start -->
- · **280** (ready, social-coordination) — Mental model of partner JointIntention — downstream matchmaker rebind that consumes the facets this ticket populates.
- · **276** (in-progress, social-coordination) — Play-bout practice on JointIntention substrate — the consumer whose matchmaker churn motivated this wiring.
- · **258** (done) — `MentalModel` + `belief_integrator` + evidence typology.
- · **127** (done) — `JointIntention` substrate (rustdoc names the perception channels this ticket wires).
- · **242** / **243** (blocked, belief-perception) — body-cue marker substrate; not blocking, but if they land later they refine emit sites.
- · **469** (retired 2026-05-26) — Ground JointIntention emission in mutual perception — retired as substrate-duplicating; its "candidacy signal source" framing folded here.
<!-- linkages:end -->

## Log

- 2026-05-11: opened as joint-adoption body-cue composition stub.
- 2026-05-19: accuracy audit — ticket body incomplete (template boilerplate with no substantive content). [needs-review] on scope/approach/verification sections before work commences.
- 2026-05-26: body reshape after 469 retirement audit. Scope concretized to three `WitnessableEvent` variants (`PlayBow`, `ReciprocalAdvance`, `SustainedOrientation`) + `belief_integrator` arms + per-tick `SustainedOrientation` tracker. Behaviour-neutral at land; 280 is the downstream consumer. Removed `[blocked-by: 242/243]` framing — emit sites are system-level observations, not marker reads.
- 2026-05-26: implemented. Three variants + integrator arms (`OBSERVED_HALF` for third-party/receptivity lifts) + `PlayCueEmissionConstants` + `SustainedCoPresenceTracker` (consumes `NearPairCache`) + `emit_play_bows` / `emit_reciprocal_advances` + `PlayBowCooldown` component. Registered after `update_near_pair_cache` in Chain 4 (one-tick integrator latency, matching existing emitters). **`SustainedOrientation` → `SustainedCoPresence` rename**: no `Heading` substrate exists, so the cue records continuous co-presence, not directional facing (substrate-honest; a `SustainedFacing` sibling can be added if 242/243 or a `Heading` component lands later). Not strictly behaviour-neutral: `affordance_writer::write_cat_vs_cat` reads `perceived_intent_clarity` (was decay-only / always 0 pre-279) to discount cat→cat Stalk/Chase/Pounce — expected small drift, verified at soak. 6 new `belief_integrator` unit tests + `play_engagement_cues` scenario all green; `just check`/`just test` clean.
- 2026-05-26: seed-42 deep-soak `logs/tuned-42-01eb555d` failed survival hard-gate (`never_fired_expected_positives=[MatingOccurred]`) + 2 new `Injury` deaths at tile (26,61) — Heron 1229681, Simba 1242756. Diagnostic investigation traced the death class to **magic-misfire damage on a corrupted besieged-ward tile**: verified mechanism `apply_misfire` at `magic.rs:1163-1178` resolving `WoundTransfer` / `CorruptionBacksplash` repeatedly during `MagicCleanse`-on-hotspot, with `safety=1.00` perception throughout the death sequence because `cover_at(ward)` reads warded tile as safe. **Pre-existing pathology**, not 279-introduced — the misfire damage path is untouched by `perceived_intent_clarity` (which only enters cat-vs-cat Stalk/Chase/Pounce/Freeze via `affordance_writer::write_cat_vs_cat`, not cat-vs-environment corruption-tile bleed or cat-vs-wildlife siege combat). 279 may have shifted *which* cats fell into the trap in seed-42, but the death class is structurally pre-existing. Surfaced six bugfix tickets (the kin-care cluster, with Princess Mononoke / Ashitaka's-arm reframe from user): [[470]] (ward-siege fear influence map, perception-before), [[471]] (damage events to log, telemetry-during), [[472]] (festering wound kind on body part, aftermath foundation), [[473]] (corrupted-kin signal map, aftermath perception), [[474]] (warder succession + shaman dispatch, aftermath ripple), [[475]] (current-role query helper). 279 **lands as-is**; the `MatingOccurred` miss is a separate concern deferred to a follow-on per `feedback_chain_rare_events` if it persists after the cluster lands.
