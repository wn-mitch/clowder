---
id: 470
title: Ward-siege fear influence map (spirituality-modulated perception of besieged tiles)
status: done
cluster: belief-perception
orchestration: substrate-sensitive
initiative: [full-sensory-perception, welfare-fidelity]
added: 2026-05-26
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, magic.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-26
---

## Why

A cat standing on a besieged-ward tile reads its position as **`safety=1.00`** while bleeding to death from magic misfires. In the seed-42 deep-soak `logs/tuned-42-01eb555d`, Heron (warder, died tick 1229681 at HP=0.04) and Simba (focal, died tick 1242756 at HP=0.03), both at tile (26,61), reported `safety=1.00` and `safety=0.90` respectively in their final pre-death snapshots. The substrate gap: `WardCoverageMap` is consumed as a "cover" / "safety" proxy by `affordance_writer::cover_at` (`src/systems/affordance_writer.rs:219, 257`) but conflates "ward present" with "tile is safe" — there is no orthogonal siege signal for consumers to compose with. Meanwhile the existing `WardsUnderSiege` marker (`src/components/markers.rs:1071`) is a **colony-level boolean** populated into `MarkerSnapshot.ColonyState` at `src/systems/goap.rs:1666` — neither positional nor per-perceiver. A cat at the besieged tile and a cat across the map see the same scalar.

Per user reframe 2026-05-26: *"wards under siege should instead be a sensory input for the cats modulated on their magical perception — ie influence maps."* The substrate-honest shape is a per-tile `InfluenceMap` for siege-pressure, modulated at the consideration layer by the cat's `spirituality` scalar (existing personality axis) — high-spirituality cats perceive siege at low intensity, mundane cats only at high intensity. The signal composes with the existing `WardCoverageMap` at the consideration layer; consumers (Flee, Wander, Explore, HerbcraftWard placement, affordance_writer cover semantics) compose cover-vs-siege-fear independently per action.

## Hot context

Failing run: `logs/tuned-42-01eb555d` (commit `01eb555d`, seed 42, dirty header from uncommitted 279 changes).

Footer survival gate violations:
- `never_fired_expected_positives: ['MatingOccurred']` — orthogonal concern, not this ticket.
- `deaths_by_cause.Injury: 2` (Heron 1229681, Simba 1242756, both at (26,61)). `deaths_by_cause.ShadowFoxAmbush: 0` — but per `src/systems/death.rs:106` `injury_source` is hardcoded `None`, so the footer ShadowFoxAmbush==0 is meaningless (see [[471]] / Defect B for the source-erasure defect).

Verified mechanism: magic-misfire damage from `MagicCleanse` on the besieged-corruption hotspot. Heron's CatSnapshot trail at (35,60)→(31,61)→(26,61) shows HP 0.97→0.93→0.89→0.71 over 4 ticks of first contact; carried `Corruption` 0.000→0.034→0.081→0.096 in parallel. After 6k ticks of stable HP=0.55 at the colony center, Heron returned to the hot zone, MagicCleanse'd repeatedly, and HP bled from 0.42 to 0.04 over 8 ticks. The `safety=1.00` reading throughout the death sequence is the perception failure this ticket addresses. (Simba's stair is identical 13k ticks later.)

This is the **perception-before** layer of the kin-care cluster surfaced 2026-05-26: see [[471]] (damage-events-to-log telemetry), [[472]] (festering wound substrate), [[473]] (corrupted-kin perception map), [[474]] (colony demand signals), [[475]] (role-recognition helper).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 influence maps | `src/systems/influence_map.rs:426-497` | 12 `InfluenceMap` impls exist (`WardCoverageMap`, `WardIntentMap`, `GraveAuraMap`, `ColonyDistrictMap`, etc.) following a uniform trait shape. No `WardSiegeFear`-shaped sibling exists. | `[verified-correct]` |
| L1 markers (siege) | `src/components/markers.rs:1071`; author `src/systems/magic.rs:1294-1309`; reader `src/systems/goap.rs:1666, 2495` | `WardsUnderSiege` is a colony-bool marker. Not per-tile, not per-perceiver, not spirituality-modulated. | `[verified-wrong-shape]` |
| L1 wildlife substrate | `src/systems/wildlife.rs:190-256` | `WildlifeAiState::EncirclingWard { ward_x, ward_y, angle, ticks }` carries siege state per fox-vs-ward pair. This IS the per-tile signal — but it's only readable from the wildlife query, not exposed to cat perception. | `[verified-correct]` |
| Cover affordance read | `src/systems/affordance_writer.rs:219, 257` | `cover_at(perceiver.position, ward)` reads `WardCoverageMap.get(pos)` directly. No siege-overlay. This drives the `safety=1.00` inflation. | `[verified-defect-cause]` |
| MagicCleanse DSE scoring | `src/ai/dses/practice_magic.rs` (cleanse path) + `src/systems/magic.rs::corruption_tile_effects` | DSE scores on corruption-presence; no opposing siege-fear signal. Pulls cats INTO besieged hotspots (Simba's L3 trace: MagicCleanse 355/475 ticks = 74.7% in his final 4.75k ticks). | `[verified-defect-cause]` |
| HerbcraftWard placement | `src/ai/dses/herbcraft_ward.rs`; scorer `src/systems/coordination.rs::compute_ward_placement` (~line 1913) | Placement scorer reads threat / corruption / coverage / cat_value but has no defensibility / siege-awareness axis. | `[verified-defect-shape]` |
| Flee DSE eligibility | `src/ai/dses/flee*.rs` | Eligibility gates on threat-distance, not siege state. No `WardsUnderSiege` marker consumer found in any DSE eligibility (only the GOAP context boolean is read). | `[verified-defect-shape]` |
| Spirituality scalar (perception modulator) | `src/components/personality.rs::spirituality` | Already exists as a personality scalar; already consumed by `HerbcraftWardDse` as a `ScalarConsideration` (`src/ai/dses/herbcraft_ward.rs:91`). Precedent for spirituality-gated perception. | `[verified-correct]` |
| 312 precedent (perception axis on ward placement) | `docs/open-work/landed/312-fox-approach-corridor-perception-axis-for-ward-placement-301-fo-2.md` | Landed precedent for adding a perception axis to ward placement scoring; same composition pattern. | `[verified-correct]` |

## Fix candidates

**Parameter-level options** (none viable — the defect is structural: a missing perception axis):
- R1 — Raise `WardsUnderSiege` colony-bool's influence on Flee DSE: not viable, the bool is not consumed by any flee DSE and adding the consumer doesn't fix the positional / per-perceiver gap.
- R2 — Mask `WardCoverageMap.get()` to return 0 on besieged tiles: not viable, conflates two orthogonal signals (cover, siege) and breaks consumers that legitimately want raw ward intensity.

**Structural options**:

- R3 (**split**) — **Recommended.** Introduce `WardSiegeFearMap : InfluenceMap` alongside the 12 existing maps in `src/systems/influence_map.rs`. Producer: a new per-tick system (or event-driven on `WildlifeAi` transitions to/from `EncirclingWard`) reads wildlife state and stamps siege-pressure intensity at each besieged ward's position (intensity scales with siege duration and/or fox count). Channel: `ChannelKind::Sight` matching the existing ward-substrate convention. Faction: `Colony`. Consumers compose via `SpatialConsideration` at the cat's position, modulated by the existing `spirituality` scalar (curve steepness scales with spirituality — high-spirituality cats perceive siege at low intensity, mundane cats only at high intensity). Affected DSEs: `FleeDse` (lift when on-tile siege-fear is high), `WanderDse` / `ExploreDse` (suppress destinations on high-siege-fear tiles), `HerbcraftWardDse` (suppress placement on high-siege-fear tiles), `affordance_writer::cover_at` (subtract scaled siege-fear from raw cover for Stalk/Pounce/Hide composites). Follows 312's precedent for perception-axis-on-ward-placement.
- R4 (**extend**) — Keep `WardsUnderSiege` colony-bool, add a sibling `BesiegedWards: Vec<Entity>` or `BesiegedTiles: HashSet<Position>` resource that consumers query directly. Positional but not an influence map; loses the spirituality-modulation and the spatial-curve composition. Weaker substrate.
- R5 (**rebind**) — Ward-siege transitions emit `WitnessableEvent::WardUnderSiege { position }` that lifts `perceived_threat_proximity` via `belief_integrator` (258). Substrate-honest under the belief-layer convention, but beliefs are per-target-entity, not per-tile, and the threat-proximity facet doesn't compose with `cover_at` naturally. Mismatches the user's "influence maps" framing.
- R6 (**retire**) — Drop `WardsUnderSiege` entirely. Not viable — `compute_ward_placement` and downstream consumers need to distinguish "fox patrolling" from "fox actively besieging."

## Recommended direction

**R3 (split)** — the user-named substrate shape ("influence maps modulated by magical perception") is exactly what the 312 precedent + the existing influence-map registry + the spirituality consideration pattern supports. R4/R5 lose load-bearing properties (spatial-curve, spirituality-modulation, composition-with-cover). R6 not viable.

Default behavior at land: the new map producer ships, but consumers read with weight 0.0 (per the 301-style conditional-axis pattern from `herbcraft_ward.rs:112-122` — byte-identical pre-landing). Activation lands as a follow-on tuning ticket once the producer is verified active.

## Out of scope

- The colony-bool `WardsUnderSiege` consumer retirement at `goap.rs:2495` — keep as a separate colony-stress signal for now; revisit after R3 lands and downstream consumers migrate.
- Per-DSE modifier weights for siege-fear consumption — initial defaults at 0.0 (dormant); tune as a follow-on per `feedback_dormant_substrate_activation_soak_first`.
- The `compute_ward_placement` defensibility axis (friendly-neighbor density / open-frontier distance) — narrower scope; opens as a follow-on if R3 doesn't close the placement gap.
- The aftermath / festering / kin-care substrate — [[472]] / [[473]] / [[474]] / [[475]].

## Verification

- `just check && just test` clean.
- `just soak-trace 42 Simba && just verdict` with the new map producer ACTIVE but consumers at weight 0.0 → byte-identical footer + per-DSE L2 scores vs pre-470 baseline (the 301 dormancy precedent).
- L1 trace shows `ward_siege_fear` channel populated (non-zero stamps when wildlife state is `EncirclingWard`).
- Follow-on tuning ticket lifts consumer weights and re-verifies: targeted soak shows reduced visits to besieged-ward tiles by non-warder cats; if Simba's MagicCleanse-on-hotspot pattern persists, the modulation curve needs steepening (low-spirituality / mundane cats should still detect siege at high intensity).

## Related work

<!-- linkages:start -->
- ✓ landed **312** (done, belief-perception) — fox-approach-corridor perception axis for ward placement (precedent for perception-axis-on-ward-placement composition pattern).
- ✓ landed **258** (done, belief-perception) — `MentalModel` + `belief_integrator` + evidence typology (sibling perception substrate).
- · **234** (ready, belief-perception) — Damage-recency perception scalar (sibling perception scalar; not blocking).
- · **124** (ready, belief-perception) — `LandmarkAnchor::OwnTerritoryCenter` (sibling interoceptive anchor).
- · **471** (ready, combat-threat) — damage events to log (telemetry sibling of this cluster).
- · **472** (ready, combat-threat) — festering wound substrate (aftermath sibling).
- · **473** (ready, belief-perception) — corrupted-kin signal map (sibling influence map; same registry slot pattern).
- · **474** (ready, social-coordination) — colony demand signals (aftermath sibling).
- · **475** (ready, social-coordination) — role-recognition helper (aftermath sibling).
<!-- linkages:end -->

## Log

- 2026-05-26: opened from seed-42 soak `logs/tuned-42-01eb555d` (Heron + Simba deaths at besieged-ward tile (26,61)). User reframe: "wards under siege should instead be a sensory input for the cats modulated on their magical perception — ie influence maps." Sibling tickets [[471]] [[472]] [[473]] [[474]] [[475]] opened in the same session.
- 2026-05-26: Verified clean: just check / just test (2522 passed) / 3 new unit tests (ward_siege_fear_map_stamps_besieged_wards, _clears_when_no_siege, stamp_paints_falloff) / just soak-trace 42 Simba — verdict survival pass, continuity pass, never_fired=[], deaths_by_cause={}. L1 trace shows ward_siege_fear channel populated 62780x (producer active every tick). Consumer DSE weights ship dormant (ward_siege_fear_weight=0.0) per the 301 byte-identical-at-land precedent; activation + the 5 consumer-site conditional considerations (Flee / Wander / Explore / HerbcraftWard / cover_at) are deferred to a follow-on tuning ticket. WardSiegeFearMap registered in populate_influence_map_registry (25 impls now). Verdict concern band is constants-drift vs stale 1799e798 baseline, not a 470 regression.
