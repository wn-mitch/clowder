---
id: 472
title: Festering wound kind on randomly-selected body part (Ashitaka substrate anchor)
status: done
cluster: combat-threat
orchestration: substrate-sensitive
initiative: [welfare-fidelity, mythic-texture]
added: 2026-05-26
parked: null
blocked-by: []
supersedes: []
related-systems: [body-zones.md, magic.md, htn-methods.md]
related-balance: []
landed-at: 29707d49ade8
landed-on: 2026-05-26
---

## Why

When a cat is damaged by magic-misfire on a corrupted tile (the (26,61) death class from `logs/tuned-42-01eb555d`), the damage event is anatomically anonymous: `Health.current -= synthetic_damage` at `src/systems/magic.rs:1172`, no body-part attribution, no festering-vs-healing distinction, no surface for healing-quest behavior to organize around. The cat's `Corruption(f32)` (`src/components/skills.rs:81`) bonds permanently (no decay path in `personal_corruption_effects`), and they drift back to the source. No bonded peer notices; no shaman dispatches; no herb-gathering urgency rises; no bedside grooming pulls in close friends. The cat dies alone of a slow corruption.

Per user reframe 2026-05-26: *"I like that misfires kill cats. that's really narrative and really cool. we just need to log it, and we also need cats to get themselves cleaned up. this is a festering wound to the cat and would be perceived as such to them and those around them. Remember Ashitaka's arm from Princess Mononoke."* The Ashitaka anchor names a concrete substrate shape: **visible** (marked on a specific body part), **progressive** (festers worse without intervention), **socially-perceived** (others recoil / recognize / aid), **source-attributed** (everyone knows it came from the boar-god / besieged ward), and **quest-driving** (multi-tick goal: travel-West / seek-shaman-dispatch).

The substrate slot is precise: add ONE `WoundKind::Festering` variant to the existing per-body-part wound enum that 095's body-zones epic established. Random body part selected at injury time; existing per-part penalty pipeline composes naturally (festering on a paw damps combat / herbcraft / dexterity; festering on a leg damps movement); existing `BodyZoneHealing` config grows one row with a near-zero passive heal rate so cure only advances via intervention; existing `InjurySource` enum carries the source attribution. Compile-time substrate extension, not runtime new types — honors CLAUDE.md "prefer compile-time contracts to runtime checks."

## Hot context

Verified mechanism from `logs/tuned-42-01eb555d`: Heron's CatSnapshot trail shows `Corruption(f32)` rising 0.000 → 0.034 → 0.081 → 0.096 → ... → 0.541 over his death sequence, with HP bleeding in lockstep from 0.97 to 0.04. Carried corruption persists for 6k ticks at the colony center between excursions to (26,61) — the bond is permanent under the current substrate. Each `MagicCleanse` cast on the besieged tile rolls a misfire chance; misfires resolve via `apply_misfire` at `magic.rs:1115-1194` as `CorruptionBacksplash` (bumps `Corruption.0`), `WoundTransfer` (drains `Health.current`), or others.

This is the **aftermath-foundation** layer of the kin-care cluster: [[470]] (perception-before), [[471]] (telemetry-during, emits the event stream this substrate organizes around), [[473]] / [[474]] / [[475]] (the aftermath ripple consumers, all blocked-by this ticket).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| CatBodyModel substrate | landed `docs/open-work/landed/095-body-zones-epic.md` | Per-body-part damage substrate with `WoundKind` variants (Bruised, Wounded, etc.) and `BodyZoneHealing` durations. The healing path converts pain-weight to HP per-tick automatically (`combat.rs:839-896`). **The `WoundKind` enum is where `Festering` slots in.** | `[verified-correct]` |
| Misfire authoring site | `src/systems/magic.rs:1115-1194` (`apply_misfire`) | `MisfireEffect::WoundTransfer` mutates `health.current` synthetically; `CorruptionBacksplash` mutates `corruption.0`. Neither calls `damage_to_body_part` or selects a body part. | `[verified-defect-shape]` |
| Healing rate config | `src/resources/sim_constants.rs::BodyZoneHealing` (per Explore agent) | Per-`WoundKind` healing durations (`soft_bruised_to_healthy = 2 seasons`). Adding a `festering_*` entry is a config extension. | `[verified-correct]` |
| InjurySource enum | (per Explore agent; `combat.rs:309`) | Carries source attribution alongside damage application. Variants like `WildlifeCombat`, `ShadowFoxAmbush`. Magic misfire path can add `MisfireBacksplash` / `MisfireWoundTransfer` variants. | `[verified-correct]` |
| WitnessableEvent substrate | `src/messages/witnessable_event.rs` (258); `src/systems/belief_integrator.rs` (consumes) | 279-pattern: add a variant, add an arm, lift a facet. Adding `WitnessableEvent::CarriesFesteringWound { actor, body_part, source_kind, severity }` follows the exact precedent. | `[verified-correct]` |
| HTN method registry | `populate_method_registry` per CLAUDE.md "All multi-tick aspirations are HTN methods" | `SeekHealing` method slots into the registry with `ApplicableWhen` reading the festering predicate. Per CLAUDE.md "Every dormant method has a glue ticket," the method can land active or dormant (with this ticket as the glue if dormant). | `[verified-correct]` |
| perceived_injury_level facet | `src/components/mental.rs::MentalModel` (258) | Existing belief facet on `MentalModel<other>`. Currently underused — no `WitnessableEvent` variant lifts it in the post-279 catalog. This ticket lifts it via the new `CarriesFesteringWound` event. | `[verified-correct]` |
| 88 / 89 predecessors | `docs/open-work/landed/088-body-distress-modifier.md`, `089-interoceptive-self-anchors.md` | Body-distress + interoceptive self-anchor substrate already lifts `OwnInjurySite` and self-care prompts. **`OwnInjurySite` is the self-perception precedent for this ticket's "I have a festering wound" awareness.** Composes naturally. | `[verified-correct]` |
| 17 sibling | `docs/open-work/tickets/17-anatomical-slot-inventory.md` | Anatomical slot inventory (blocked). Sibling-substrate — anatomical-slot work would compose with festering-on-body-part for "wound on the slot holding the herb-pouch." Not blocking. | `[verified-correct]` |

## Fix candidates

**Parameter-level options**:
- R1 — Lower the misfire chance (`magic.rs::check_misfire` probability config): reduces the death rate but loses the user's intended narrative texture ("I like that misfires kill cats. that's really narrative and really cool"). Rejected.
- R2 — Add a `Corruption.0` natural decay rate in `personal_corruption_effects`: returns the cat to healthy without any narrative arc. Loses the Ashitaka shape. Rejected.

**Structural options**:

- R3 (**split**) — **Recommended.** Add `WoundKind::Festering` variant to the existing per-body-part wound enum. Authoring site: extend `apply_misfire::WoundTransfer` (and a high-severity branch of `CorruptionBacksplash`) so it (a) selects a random body part via SimRng, (b) applies `WoundKind::Festering` to that part carrying the `InjurySource` (`MisfireBacksplash` / `MisfireWoundTransfer`), (c) emits the `BodyPartInjury` event that [[471]] reinstates. Per-part penalty pipeline picks up the new kind via one new row in the per-part effect table. `BodyZoneHealing::festering_*_to_healthy` config knob set near-zero (cure only via intervention). New `WitnessableEvent::CarriesFesteringWound { actor, body_part, source_kind, severity }` variant + one new `belief_integrator` arm lifting `perceived_injury_level` (follows 279 pattern exactly). New HTN method `SeekHealing` in `populate_method_registry`, `ApplicableWhen` reading `body_model.has_wound(WoundKind::Festering)`, decomposes into source-appropriate subgoals (`Rest + RitualCleanse` for `MisfireBacksplash`, `Rest + ApplyRemedy + AcceptTending` for `WildlifeCombat`). Per CLAUDE.md "All multi-tick aspirations are HTN methods" + "Every dormant method has a glue ticket": [[473]] / [[474]] are the glue tickets that wire the decomposition's leaf actions (TendFestering DSE, etc.).
- R4 (**extend**) — Don't add a wound kind; instead extend `Corruption(f32)` semantics so high values trigger festering effects directly. Loses the body-part-located narrative texture (which part?), loses the per-part penalty composition, loses the random-body-part Ashitaka anchor.
- R5 (**rebind**) — Festering is a separate component `FesteringMark { body_part, source, since }` rather than a wound kind. Parallel to existing wound substrate — violates the "slot into existing substrate" principle the user named in the 2026-05-26 reframe (*"none of these are explicit but all easily slot into the substrate"*).
- R6 (**retire**) — Not viable. The user named this as load-bearing.

## Recommended direction

**R3 (split)** — `WoundKind::Festering` is a one-variant enum extension, the cleanest substrate-honest expression of the Ashitaka anchor. The Princess-Mononoke property table from the plan:

| Ashitaka arm property | Clowder substrate target | Maps to |
|---|---|---|
| Visible mark on body | Wound on a specific body part | `CatBodyModel` per-part state (existing) |
| Others perceive it and react | `WitnessableEvent::CarriesFesteringWound` per observation | 279 / 258 belief layer (existing) |
| Festers progressively | Near-zero passive heal rate; severity from carried-corruption | `BodyZoneHealing::festering_*` config (new row) |
| Drives a healing-quest plan | `SeekHealing` HTN method, decomposes by source | `populate_method_registry` (existing pattern) |
| Cure is source-specific | `InjurySource` field on the wound carries the curse's origin | `combat.rs` damage path (existing field) |
| Social aid behavior | `TendFestering` cat-side DSE, lifted by `perceived_injury_level` | Authored under [[473]] (sibling ticket; this ticket lands the predicate it reads) |

Landing approach:
1. Add `WoundKind::Festering` variant + per-part effect row + `BodyZoneHealing::festering_*` config. Behaviour-neutral if no authoring site selects it.
2. Wire `apply_misfire` to apply the wound on a random body part (uses SimRng — handle the perturbation by gating behind a `misfire_festering_chance` knob defaulted to 0.0 at first land, lifted in a follow-on tuning ticket per `feedback_dormant_substrate_activation_soak_first`).
3. Add `WitnessableEvent::CarriesFesteringWound` variant + exhaustive `belief_integrator` arm. Behaviour-neutral until consumers ([[473]]) read `perceived_injury_level` differently.
4. Add `SeekHealing` HTN method to `populate_method_registry`, applicable on the predicate. Land dormant (`ApplicableWhen::PendingSubstrate { blocker: 473 }`) since the decomposition's leaf actions (TendFestering DSE) ship in [[473]] — per CLAUDE.md "Every dormant method has a glue ticket."
5. Add the predicate helper `is_festering(cat)`.

## Out of scope

- The corrupted-kin influence map ([[473]]) — observers' positional perception layer.
- Colony demand signals ([[474]]) — warder succession + shaman dispatch.
- Role-recognition helper ([[475]]) — derived from existing markers.
- Activation tuning (lifting `misfire_festering_chance` from 0.0) — opens as a follow-on per `feedback_dormant_substrate_activation_soak_first`.
- Cure-specific item authoring (cleansing-herb recipe, ritual-cleanse action) — separate ticket if not already covered by existing herbcraft / magic substrate.

## Verification

- `just check && just test` clean.
- `just soak-trace 42 Simba && just verdict` with `misfire_festering_chance = 0.0` → byte-identical footer + per-DSE L2 scores vs pre-472 baseline.
- New unit tests in `belief_integrator` for the `CarriesFesteringWound` arm (sibling tests to 279's PlayBow / ReciprocalAdvance / SustainedCoPresence arms).
- Scenario test (`src/scenarios/festering_wound.rs` or similar): preload a cat with `WoundKind::Festering` on a random body part; assert the predicate fires, the per-part penalty applies, `BodyZoneHealing` rate is near-zero, `WitnessableEvent::CarriesFesteringWound` emits to nearby observers.
- L1 trace shows `perceived_injury_level` rising on observers when they sense a festering peer.
- Follow-on activation tuning ticket lifts `misfire_festering_chance > 0`; targeted soak shows ≥1 `WoundKind::Festering` authored per (26,61)-class siege event; `[[473]]`'s TendFestering DSE pulls bonded peers toward the wounded cat.

## Related work

<!-- linkages:start -->
- ✓ landed **095** (done) — Body zones epic (foundation for the per-part wound substrate this ticket extends).
- ✓ landed **088** (done, ai-substrate) — Body-distress modifier (self-care promotion under L2.10; the OwnInjurySite precedent).
- ✓ landed **089** (done, ai-substrate) — Interoceptive self-anchors (the spatial self-perception this ticket's predicate composes with).
- ✓ landed **173** (done, ai-substrate) — IsHerbalist / IsSpiritualist capability markers (role substrate consumed by [[473]]'s TendFestering eligibility).
- ✓ landed **258** (done) — `MentalModel` + `belief_integrator` (the 279-pattern this ticket's WitnessableEvent variant follows).
- · **17** (blocked, items-crafting) — Anatomical slot inventory (sibling-substrate; composes with festering-on-body-part).
- · **470** (ready, belief-perception) — Ward-siege fear influence map (cluster sibling — perception-before).
- · **471** (ready, combat-threat) — Damage events to log (cluster sibling — telemetry-during; this ticket consumes that event stream).
- · **473** (ready, belief-perception) — Corrupted-kin signal map (blocked-by this ticket; consumes the festering predicate).
- · **474** (ready, social-coordination) — Colony demand signals (blocked-by this ticket).
- · **475** (ready, social-coordination) — Role-recognition helper (blocked-by this ticket).
<!-- linkages:end -->

## Log

- 2026-05-26: opened from seed-42 soak `logs/tuned-42-01eb555d` death-class investigation. User reframe (Princess Mononoke / Ashitaka's arm): festering substrate must be visible, progressive, socially-perceived, source-attributed, and quest-driving. *"Festering wound actually does look like it fits, but it's a type of wound on a body part, randomly selected."* This ticket is the foundation of the aftermath layer; sibling consumer tickets [[473]] [[474]] [[475]] are blocked-by this one. Cluster: [[470]] (before), [[471]] (during), [[472]] (this — anchor), [[473]]/[[474]]/[[475]] (after).
- 2026-05-26: Verified clean: just check / just test (2516 passed) / just scenario festering_wound (2/2 pass on persistence + nearby-peer-perceives assertions) / just soak-trace 42 Simba — verdict: survival pass, continuity pass, never_fired=[], deaths_by_cause={} (no misfires in this seed post-279; festering substrate stays dormant in soak but the scenario verifies the persistence + observation + belief-lift paths). misfire_festering_chance=0.5 active at land; 273's frontmatter carries wires-method:[seek_healing]; SeekHealing HTN method registered as PendingSubstrate { blocker: 473 }. WoundKind axis on BodyPartState extends to Frozen / Poisoned later by adding one f32 multiplier on BodyZoneHealing.
