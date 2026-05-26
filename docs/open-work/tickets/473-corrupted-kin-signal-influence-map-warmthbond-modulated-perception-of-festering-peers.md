---
id: 473
title: Corrupted-kin signal influence map (warmth/bond-modulated perception of festering peers)
status: ready
cluster: belief-perception
orchestration: substrate-sensitive
initiative: [full-sensory-perception, welfare-fidelity]
added: 2026-05-26
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, htn-methods.md]
related-balance: []
wires-method: [seek_healing]
landed-at: null
landed-on: null
---

## Why

Once [[472]] lands the `WoundKind::Festering` substrate on a random body part, the wound exists per-cat — but it isn't yet *positional* at colony scale. A bonded peer two tiles away should "feel" their friend's festering more strongly than a stranger at the same distance does (Ashitaka's arm shape: *others perceive it and react*). The 258 belief layer already carries `MentalModel<other>.perceived_injury_level` as a per-pairwise facet — but that's only updated when an observer actively senses the target, and it doesn't compose with positional considerations (Wander destinations, GroomOther target selection, Coordinate eligibility) without an influence-map producer that stamps the festering cat's position so consumers sample it spatially.

Per user reframe 2026-05-26: *"wards under siege should instead be a sensory input for the cats modulated on their magical perception — ie influence maps."* That framing extends naturally to corrupted-kin perception: the festering peer becomes a positional, intensity-stamped signal in the existing `InfluenceMap` registry at `src/systems/influence_map.rs:426-497`, modulated at consumption time by the perceiver's bond-strength + warmth scalars (the existing affordance-modifier pipeline handles this — no new modulation substrate needed). Sibling to [[470]]'s `WardSiegeFear` map; same trait shape, same registration pattern.

## Hot context

The (26,61) death class showed cats die alone of a slow corruption: Heron's bonded peers (Mocha, Calcifer, Bramble — fondness +1.00 / familiarity 1.00) at the colony center never visited him in his final 280 ticks. Simba's bonded peers (Bramble, Calcifer, Mocha — same triple) likewise. Either:
- (a) the peers literally didn't perceive the festering (perception layer gap — what this ticket addresses), OR
- (b) the peers perceived it but had no DSE that responded (DSE-layer gap — addressed by the new `TendFestering` DSE this ticket also authors).

Both are present in current substrate. This ticket closes both with the influence-map producer + the consumer DSE.

This is the **aftermath-perception** layer of the kin-care cluster: [[470]] (perception-before), [[471]] (telemetry-during), [[472]] (festering anchor — this ticket's blocker), [[474]] [[475]] (colony-economy ripple).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Influence-map registry | `src/systems/influence_map.rs:426-497` | 12 `InfluenceMap` impls follow uniform trait shape. Adding one more sibling (this ticket + [[470]]) follows the precedent. | `[verified-correct]` |
| Festering predicate | [[472]] (blocked-by) | `is_festering(cat) = cat.body_model.has_wound(WoundKind::Festering)` — authored by [[472]]. This ticket consumes the predicate. | `[verified-correct]` (post-[[472]]) |
| MentalModel facet `perceived_injury_level` | `src/components/mental.rs::MentalModel` + `src/systems/belief_integrator.rs` | Per-target facet, already exists. Currently lifted only by witnessable damage events (post-[[472]]). This ticket adds a *positional* sibling so non-pairwise consumers (Wander destinations, Coordinate eligibility) can sample without targeted belief lookup. | `[verified-correct]` |
| Relationships substrate | `src/resources/relationships.rs` (or similar — `Relationships` Res with `fondness` / `familiarity` / `romantic` / `bond`) | Per-pairwise relationship state already exists. Bond-strength is queryable for modulating influence-map sampling. | `[verified-correct]` |
| Personality scalars (warmth, compassion) | `src/components/personality.rs` | `warmth`, `compassion` already exist as personality axes. Existing affordance-modifier pipeline already composes per-cat scalars at consideration time. | `[verified-correct]` |
| Affordance modifier pipeline | `src/ai/modifier.rs` + per-DSE composition | Already supports per-cat scalar lift / damp at consideration evaluation. The bond-strength modulation slots in here, no new modulation substrate. | `[verified-correct]` |
| Existing DSEs that should respond | `src/ai/dses/groom*.rs` (GroomOther), `src/ai/dses/coordinate*.rs` (Coordinate) | These DSEs already exist; they need a new consideration that reads the festering-kin signal. Eligibility already gates on relationship presence. | `[verified-correct]` |
| Missing consumer DSE | (new) | No DSE today specifically lifts toward a festering peer for tending. A new `TendFestering` cat-side DSE under `src/ai/dses/tend_festering.rs` follows the existing DSE registration pattern via `linkme::distributed_slice` (per CLAUDE.md's CatDseRegistration shape). | `[verified-defect-shape]` |
| 312 precedent | `docs/open-work/landed/312-fox-approach-corridor-perception-axis-for-ward-placement-301-fo-2.md` | Landed precedent for adding a perception axis + consumer in the same ticket. | `[verified-correct]` |

## Fix candidates

**Parameter-level options**:
- R1 — Tune the existing `Caretake` DSE eligibility to fire on `target.health < threshold`: doesn't compose with `WoundKind::Festering` specifically, fires on any wounded cat including those healing normally. Loses the source-specific cure shape from [[472]]'s HTN method decomposition.

**Structural options**:

- R2 (**split**) — **Recommended.** Add `CorruptedKinSignalMap : InfluenceMap` (or `FesteringKinMap`) as a sibling to [[470]]'s `WardSiegeFear` map in `src/systems/influence_map.rs`. Producer: per-tick system stamps the position of every cat with `WoundKind::Festering` (predicate from [[472]]), intensity = wound severity from the body-model field. Channel: `Sight` (or new `Empathic` if a future ticket separates social-perception). Faction: `Colony`. Consumer modulation via the existing affordance-modifier pipeline: bond-strength + warmth + compassion compose at the sampling consideration. Affected DSEs: `GroomOther` (existing, lifts toward festering bonded peer — bedside grooming), `Coordinate` (existing, lifts when colony has any festering peer — coordinator notices), and a new `TendFestering` cat-side DSE for high-warmth observers (one new file under `src/ai/dses/`, `linkme` registration). The new DSE provides the leaf action for [[472]]'s `SeekHealing` HTN method's `AcceptTending` step.
- R3 (**extend**) — Don't add a positional map; instead extend `belief_integrator` so per-target `perceived_injury_level` lifts more aggressively from `WitnessableEvent::CarriesFesteringWound`, then rely on per-target belief queries from consumers. Loses the spatial-curve composition (a Wander destination can't easily query per-target beliefs).
- R4 (**rebind**) — Festering perception flows through the existing `KittenCryMap` (auditory-channel cry broadcast). Wrong channel (festering isn't audible-by-default; kitten-cry is its own substrate); muddies semantics.
- R5 (**retire**) — Not viable. User reframe makes the influence-map shape explicit.

## Recommended direction

**R2 (split)** — direct expression of the user's "influence-map for perception" framing. The new map + new DSE land together because the DSE provides the consumer that proves the producer is wired correctly. Default behavior at land: producer ships active (stamps festering peers); consumers (`GroomOther` consideration, `Coordinate` consideration, `TendFestering` DSE) read with default weights that may need tuning.

Landing approach:
1. Add `CorruptedKinSignalMap : InfluenceMap` impl in `src/systems/influence_map.rs`. Producer system reads `WoundKind::Festering` predicate (post-[[472]]) and stamps positions per tick.
2. Add `TendFestering` cat-side DSE under `src/ai/dses/tend_festering.rs`, registered via `linkme::distributed_slice(CAT_DSE_REGISTRY)`. Eligibility: high `warmth` + bonded-peer-with-festering present. Action: `TendFestering` (new `Action` variant or repurposed `GroomOther` target). Per CLAUDE.md `Dse::action() -> Action` is mandatory.
3. Add the leaf-action resolver under `src/steps/` for `TendFestering` — applies a healing tick to the target's festering wound; mutates body model.
4. Add the new `Action::TendFestering` to the existing `score_actions` dispatcher in `src/ai/scoring.rs` (per memory `project_score_actions_dispatch_antipattern` — registration alone isn't enough).
5. Wire `GroomOther` and `Coordinate` to lift via a new consideration sampling the influence map at the cat's position.

Per CLAUDE.md "Every dormant method has a glue ticket": [[472]]'s `SeekHealing` HTN method's `AcceptTending` leaf decomposes to `Action::TendFestering` — this ticket IS the glue (frontmatter `wires-method: [...]` if [[472]] lands the method dormant referencing this ticket as the blocker).

## Out of scope

- The colony-economy ripple ([[474]]) — warder succession + shaman dispatch demand signals.
- Role-recognition helper ([[475]]).
- Cure-specific item recipes (cleansing-herb production) — out of scope for E-cluster; would compose with [[309]]'s reserve-deficit consideration if a follow-on extends that path.
- Activation tuning for the new DSE's weights — separate balance ticket once the producer is verified active.
- Per-source TendFestering decomposition (different action for `MisfireBacksplash` source vs `WildlifeCombat` source) — opens as a follow-on; this ticket lands generic TendFestering, [[472]]'s HTN method handles the source-routing at decomposition time.

## Verification

- `just check && just test` clean.
- `just soak-trace 42 Simba && just verdict` — once [[472]] lands and a festering wound authoring fires, the new map populates non-zero. Verify via L1 trace inspection.
- New unit tests on the producer system + the TendFestering DSE.
- Scenario test: preload cat A with `WoundKind::Festering`, cat B (bonded, high warmth) within sensing range. Assert B's L2 trace shows `TendFestering` scoring high; resolver advances A's wound severity downward.
- Behavior-neutral at land if [[472]]'s `misfire_festering_chance = 0.0` (no festering wounds = no signal stamps = no consumer lift). Activation requires both [[472]]'s tuning and this ticket's consumer-weight tuning.
- Follow-on tuning ticket lifts both knobs; targeted soak shows bonded peers visiting a festering cat (compare to the (26,61)-class where Heron / Simba died alone).

## Related work

<!-- linkages:start -->
- · **472** (ready, combat-threat) — Festering wound substrate (BLOCKER — provides the predicate this ticket samples).
- · **470** (ready, belief-perception) — Ward-siege fear influence map (sibling influence-map ticket; same registry-slot pattern).
- ✓ landed **312** (done, belief-perception) — fox-approach-corridor perception axis (precedent for producer + consumer same ticket).
- ✓ landed **258** (done) — MentalModel + belief_integrator (per-target belief facet substrate this ticket complements positionally).
- · **279** (ready, social-coordination) — Body-cue-driven joint adoption (sibling perception-substrate; play-engagement cues).
- · **280** (ready, social-coordination) — Mental model of partner JointIntention (sibling per-target belief).
- · **474** (ready, social-coordination) — Colony demand signals (cluster sibling — aftermath ripple).
- · **475** (ready, social-coordination) — Role-recognition helper (cluster sibling).
<!-- linkages:end -->

## Log

- 2026-05-26: opened from seed-42 soak `logs/tuned-42-01eb555d` kin-care cluster. The (26,61) deaths showed bonded peers never visited the dying cats; this ticket closes that gap via positional perception + a new TendFestering DSE. Blocked-by [[472]] for the festering predicate. Cluster: [[470]] [[471]] [[472]] (blocker) [[473]] (this) [[474]] [[475]].
