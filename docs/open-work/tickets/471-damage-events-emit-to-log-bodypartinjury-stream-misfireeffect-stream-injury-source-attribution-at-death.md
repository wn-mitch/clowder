---
id: 471
title: Damage events emit to log (BodyPartInjury stream + MisfireEffect stream + injury_source attribution at death)
status: ready
cluster: combat-threat
orchestration: substrate-sensitive
initiative: [welfare-fidelity]
added: 2026-05-26
parked: null
blocked-by: []
supersedes: []
related-systems: [body-zones.md, magic.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The entire damage stream is structurally invisible to diagnostics: in `logs/tuned-42-01eb555d`, `just q events --kind BodyPartInjury` returns 0 results with "no similar events anywhere in the log." Two cats died "from wounds" at tile (26,61) with `injury_source: null` on both Death events — and **the source-erasure isn't just at death-discriminator time, it's the entire damage stream that was retired by 095 Phase 1 Stage B**. The code comment at `src/systems/magic.rs:1164-1170` names this exactly: *"would require threading `&mut MessageWriter<BodyPartInjury>` through 4 step resolvers × 2 Bevy systems — separate ticket since misfire is rare and synthetic damage barely exceeds the negligible threshold."* This investigation falsified the "misfires are rare" assumption — Heron and Simba each took dozens of misfire ticks during MagicCleanse on a besieged-corruption hotspot, but every one is invisible to the log.

The user's directive 2026-05-26: *"damage events absolutely have to go into the logs."* Load-bearing because: (a) without per-tick damage events, the (26,61) diagnostic took 90 minutes of CatSnapshot trail reconstruction instead of 5 minutes of `just q events --kind BodyPartInjury --tick-range A..B`; (b) every future bugfix touching combat / corruption / magic / wound healing inherits the same diagnostic blindness; (c) the festering-wound substrate ([[472]]) needs the event stream to carry source attribution forward; (d) the perception scalars in [[234]] (damage recency) need the events to trigger their decay-based ramp.

## Hot context

Failing run: `logs/tuned-42-01eb555d`. Two `DeathCause::Injury` events (Heron 1229681, Simba 1242756) both with `injury_source: null` per `src/systems/death.rs:106` (hardcoded `None`). The injury_source field exists on the EventLog::Death variant; the discriminator just doesn't populate it. Behind that: zero `BodyPartInjury` events for the entire run (`scanned 0 / returned 0`) — the upstream emitters were retired.

Verified damage paths that should emit but don't:
- `src/systems/magic.rs:1147-1153` `MisfireEffect::CorruptionBacksplash` — `corruption.0 += misfire_corruption_backsplash_amount`. No event emit.
- `src/systems/magic.rs:1163-1178` `MisfireEffect::WoundTransfer` — `health.current -= synthetic_damage`. No event emit. Code comment explicitly names the deferred work.
- `src/systems/combat.rs:204-211, 283-364` cat-vs-wildlife combat — `damage_to_body_part()` called with an `InjurySource` argument, but no event written to the log.
- `src/systems/combat.rs:300` wildlife-attacks-cat — same.
- `src/systems/magic.rs:377` corruption tile health drain — no event.
- `src/systems/needs.rs:153` starvation damage — no event (starvation has its own attribution path via `total_starvation_damage`, but still inaudible per-tick).
- `src/systems/incapacitation.rs:245` severe-injury kill — no event.
- `src/systems/wildlife.rs:1767, 3169, 3176` standoff escalation damage — no event.

This is the **telemetry-during** layer of the kin-care cluster: see [[470]] (perception-before), [[472]] (festering wound substrate that the event stream populates).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Death-cause discriminator | `src/systems/death.rs:59-98, 106` | When `health.current <= 0.0`, branches on starvation_attribution_threshold; else `DeathCause::Injury`. `let injury_source: Option<String> = None;` at line 106 is hardcoded — the field exists, just never populated. | `[verified-defect]` |
| EventLog Death variant | `src/resources/event_log.rs` (Death push at `src/systems/death.rs:149-158`) | Variant carries `injury_source: Option<String>` field; would deserialize correctly if populated. | `[verified-correct]` |
| Damage-application sites | `src/systems/combat.rs:204-211, 283-364, 300, 1404`; `src/systems/magic.rs:266, 377, 1148, 1172`; `src/systems/needs.rs:153`; `src/systems/incapacitation.rs:245`; `src/systems/wildlife.rs:1767, 3169, 3176`; `src/systems/prey.rs:1060` | ~10 distinct call sites mutate `Health.current` or `Corruption.0`. None emit a `BodyPartInjury` / `MisfireEffect` event. | `[verified-defect-shape]` |
| InjurySource enum | (per Explore agent report; `combat.rs:309` `InjurySource::WildlifeCombat`) | Already exists with variants like `WildlifeCombat`, `ShadowFoxAmbush`, `CatCombat`, etc. The enum the missing event would carry is in-tree. | `[verified-correct]` |
| BodyPartInjury event type | referenced in `src/systems/magic.rs:1166-1167` comment | Comment says "would require threading `&mut MessageWriter<BodyPartInjury>` through 4 step resolvers × 2 Bevy systems." Implies the message type exists or did exist. Verify via `grep -rn "BodyPartInjury" src/`. | `[suspect — verify whether the type exists today or needs re-introduction]` |
| 095 Phase 1 Stage B retirement | `docs/open-work/landed/095-body-zones-epic.md` | Retired `Health.injuries` field + the per-tick `Injury.source` history. Substituted body-zone-per-part damage. The event emission was deferred at that landing per the magic.rs:1164 comment. | `[verified-correct]` |
| EventLog header / footer tallies | run footer in events.jsonl | `deaths_by_cause` is keyed by `DeathCause` enum variants; without source attribution, every Injury death goes to the same bucket. | `[verified-defect]` |
| 234 consumer | `docs/open-work/tickets/234-damage-recency-perception-scalar-couple-acutehealthadrenalineflee-to-felt-danger.md` | Proposes `LastDamageEvent { tick, severity }` consumed by an `AcuteHealthAdrenalineFlee` recency scalar. **Consumer of this ticket's emitters.** | `[verified-correct]` |

## Fix candidates

**Parameter-level options**:
- R1 — Only populate `injury_source` at death-discriminator time from the body model's most-recent wound: closes the death-footer hole but leaves the per-tick damage stream invisible. Not sufficient for [[472]] / [[234]] consumers.

**Structural options**:

- R2 (**split**) — **Recommended.** Author two new message types: `BodyPartInjury { entity, body_part, source: InjurySource, severity: f32, tick: u64 }` (combat / wildlife / corruption-tile / starvation paths) and `MisfireEffect { entity, effect: MisfireEffectKind, severity: f32, tick: u64 }` (magic misfire path). Register via `app.add_message::<...>()` in `SimulationPlugin::build()`. Thread `&mut MessageWriter<...>` through the ~10 damage-application sites (per the magic.rs:1164 comment estimate of "4 step resolvers × 2 Bevy systems," extended to the full damage surface). EventLog consumes both via `event_log::on_body_part_injury` / `event_log::on_misfire_effect` systems that push EventKind variants. Death discriminator at `death.rs:106` reads the most-recent BodyPartInjury for the dying entity (via a small last-injury cache or per-cat component) and populates `injury_source`.
- R3 (**extend**) — Single `BodyPartInjury` event variant covers misfires too (with a synthetic `InjurySource::MagicMisfire`). Fewer message types but loses the misfire-effect-kind distinction (CorruptionBacksplash vs WoundTransfer vs LocationReveal) that future narrative templates may want.
- R4 (**rebind**) — Route through the existing `WitnessableEvent` substrate: extend with `WitnessableEvent::DamageObserved { actor, source, severity }`. Substrate-honest under the 258 belief-layer convention but loses the diagnostic-friendly raw event stream — `WitnessableEvent` is per-observer-witness-tick, not per-damage-tick.
- R5 (**retire**) — Not viable. The user named this as load-bearing.

## Recommended direction

**R2 (split)** — distinct event types for distinct damage shapes preserves the diagnostic clarity that the (26,61) investigation needed. Distinct types also let downstream consumers (festering substrate [[472]], damage-recency scalar [[234]]) gate on shape without enum-matching every variant.

Landing approach (per CLAUDE.md "incremental implementation"):
1. Author the message types + register in `SimulationPlugin::build()`. Behaviour-neutral if no consumer reads them yet.
2. Wire emit at the magic-misfire path first (smallest surface — 1 file, 2 sites). Verify L1 trace shows `MisfireEffect` rows on a soak with magic activity.
3. Wire emit at combat / wildlife paths. Verify `BodyPartInjury` rows on a soak with predator combat.
4. Wire emit at corruption-tile / starvation / incapacitation paths.
5. Populate `injury_source` at `death.rs:106` from the most-recent BodyPartInjury for the dying entity (small last-injury cache or per-cat component).
6. Update footer tally to key on source — `deaths_by_cause.Injury.WildlifeCombat`, `.MagicMisfire`, etc.

## Out of scope

- Downstream consumers — [[472]] (festering wound substrate reads the event stream), [[234]] (damage-recency scalar reads it). Both effectively blocked-by this ticket but tracked separately.
- Narrative template authoring for the new events — handled by the narrative system when the events start firing; this ticket just emits.
- Body-model tissue-damage / pain-weight changes — pre-existing substrate from 095; this ticket emits events alongside the existing per-part state mutation.
- Per-source death-cause enum variants (`DeathCause::WildlifeAttack(BodyPartInjurySource)`) — opens as a follow-on once the event stream is verified active; preferable to extend `DeathCause::Injury` with a populated `injury_source` field first.

## Verification

- `just check && just test` clean.
- `just soak-trace 42 Simba && just verdict` — `just q events logs/tuned-42-<new-commit> --kind BodyPartInjury` returns non-zero count (target: ≥1 row per cat-vs-wildlife combat tick in the run; ≥1 row per starvation-damage tick).
- `just q events --kind MisfireEffect` returns non-zero count on any soak with magic activity (typically 100+ misfire events per 60k-tick soak per the corruption_tile_effects threshold path).
- `just q deaths` with the new column shows `injury_source` populated on Injury deaths (replay (26,61) class: expect `injury_source: "CorruptionBacksplash"` or `"WoundTransfer"` for the misfire deaths).
- Behavior-neutral at land: no DSE / scoring / planner read the new events yet, so footer survival metrics + frame-diff stay within ±5% of baseline (the events are diagnostic-only).
- Footer regression: `deaths_by_cause` keys change shape from flat enum to per-source-flagged. Update verdict canary to accept the new shape.

## Related work

<!-- linkages:start -->
- · **234** (ready, belief-perception) — Damage-recency perception scalar (consumer of this ticket's event stream).
- ✓ landed **095** (done) — Body zones epic — retired per-tick `Injury.source` history; this ticket re-establishes the event stream over the new body-zone substrate.
- · **180** (ready, belief-perception) — Death-stamp / scent-anchor at kill sites (consumer pattern; same event-driven shape).
- ✓ landed **295** (done, belief-perception) — WitnessableEvent emit sites (sibling event-emission pattern at the belief layer).
- · **470** (ready, belief-perception) — Ward-siege fear influence map (cluster sibling — perception-before).
- · **472** (ready, combat-threat) — Festering wound substrate (cluster sibling — aftermath; consumer of this ticket's events).
- · **473** (ready, belief-perception) — Corrupted-kin signal map (cluster sibling).
- · **474** (ready, social-coordination) — Colony demand signals (cluster sibling).
- · **475** (ready, social-coordination) — Role-recognition helper (cluster sibling).
<!-- linkages:end -->

## Log

- 2026-05-26: opened from seed-42 soak `logs/tuned-42-01eb555d` diagnostic gap. The (26,61) deaths investigation took 90 minutes of CatSnapshot trail reconstruction because no per-tick damage events exist in the log. User directive: *"damage events absolutely have to go into the logs."* The magic.rs:1164-1170 code comment already names this as deferred work; this ticket cashes it in. Cluster siblings [[470]] [[472]] [[473]] [[474]] [[475]].
