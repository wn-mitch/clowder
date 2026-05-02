---
id: 095
title: Body zones — anatomical injury model for all animal species
status: ready
cluster: null
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [body-zones.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The current health model is a single `f32` for cats and a flat `defense: f32` for
predators. Cats need anatomical weight; so do foxes, hawks, and snakes — they are
active AI agents that persist across encounters. A fox that retreated with a
wounded haunch should return to the next raid slower. A grounded hawk is a
different threat than one at altitude. A snake whose head has been pinned and
bitten has lost its primary weapon.

Prey (Mouse, Rat, Rabbit, Fish, Bird) get a simpler 3-zone model that drives
hunt difficulty and carcass yield rather than identity or narrative.

Spec: `docs/systems/body-zones.md`.

## Scope

### Phase 1 — Cat zones
- Replace `Health.current: f32` / `Health.injuries: Vec<Injury>` with a
  13-part `CatBodyModel` component.
- Each part carries `tissue_damage: f32` and a derived `PartCondition`
  (Healthy / Bruised / Wounded / Mangled / Destroyed).
- `total_pain` computed from part damage × pain weight; replaces
  `is_incapacitated` check in `ai.rs`.
- `health_derived` replaces the raw `Health.current` float for all systems
  that read it (display, starvation gates, `CriticalHealth` interrupt).
- `damage_to_body_part()` in `combat.rs` replaces `damage_to_injury()` —
  selects target part via weighted random per attacker species.
- Healing tick per part using per-category rates. Permanent-injury flag on
  `Destroyed` parts that don't regrow.
- Narrative templates wired per part (see spec §Narrative Integration).
- TUI inspect view shows permanent injuries as identity traits.
- New `SimConstants` knobs: pain weights, condition thresholds, healing rates,
  incapacitation threshold.

### Phase 2 — Predator zones
- `FoxBodyModel` (8 parts): Muzzle/Jaw, Ears, Throat, Flanks, Belly, Front
  Paws, Haunches, Tail. Applies to both Fox and ShadowFox.
- `HawkBodyModel` (8 parts): Beak, Eyes, Breast/Keel, Left Wing, Right Wing,
  Left Talon, Right Talon, Tail Feathers. Bilateral wings/talons tracked
  separately — both Mangled+ grounds the hawk.
- `SnakeBodyModel` (3 parts): Head, Body, Tail.
- `WildAnimal.defense` derived from `total_pain / max_possible_pain` instead
  of a flat float.
- `WildAnimal.threat_power` scales with key-part condition (muzzle → Fox bite
  damage; talon → Hawk grab damage; head → Snake venom delivery).
- Pain-threshold retreat check: `total_pain > retreat_threshold[species]`
  emits `FleeMessage` and sets `WildlifeAiState::Fleeing`. Replaces any
  ad-hoc HP retreat logic.
- Predator healing while resting (Fox: den resting 2× rate; Hawk: roosting
  baseline rate; Snake: passive).
- ShadowFox extension: wounded parts emit corruption patches; destroyed parts
  reform at Wounded after `shadow_fox_part_reform_ticks`.
- Encounter narrative templates for predator wounds (spec §Narrative Integration).
- `cats → predators` targeting weights wired (spec §Combat Targeting Weights).

### Phase 3 — Prey zones
- `PreyState.wound_zones: [Option<WoundTier>; 3]` (Head / Body / Legs).
  `WoundTier` is `Wounded` or `Dead`; Healthy = None.
- Head hit above threshold → kill (existing despawn + carcass spawn path).
- Legs wound: flee speed × `wounded_prey_flee_speed_multiplier`; alertness
  ceiling × 0.7.
- Body wound: `body_wound_yield_penalty` applied to spawned carcass.
- Wounded prey persists; heals passively after `prey_wound_recovery_ticks`.
- Wounded-prey scent tag (opt-in integration point for ticket 062 per-species
  scent maps — if 062 has landed, tag the cell; otherwise no-op).

## Out of scope

- Cat treatment / medicine system — healing rates are passive; treatment
  acceleration is a separate feature.
- Body zones on fox cubs — cubs don't enter combat.
- Prey identity or narrative — prey wounds drive mechanics only.
- Balance tuning of any downstream metric until the substrate is stable
  (see §Verification).

## Current state

Not started. Spec is complete in `docs/systems/body-zones.md`.

## Approach

Phase 1 is self-contained and can ship first. Phase 2 should wait on
ticket 025 (Hawk/Snake GOAP domains) being at least partially landed so
hawks and snakes can act on their own injury state (e.g. a grounded hawk
should try to flee rather than continue circling). Fox body zones can ship
before 025 lands since fox AI already has a `Fleeing` phase.

Phase 3 is independent of Phase 2 but benefits from ticket 062 (prey scent
maps) for the wounded-prey scent tag.

Suggested landing order: Phase 1 → Fox zones (Phase 2 partial) → Phase 3
→ Hawk + Snake zones (Phase 2 complete, after 025).

## Verification

- **Phase 1:** `just soak 42 && just verdict`. Expect no change to survival
  canaries (grooming, play, mentoring, burial, courtship, mythic-texture).
  Deaths-by-starvation == 0 must hold. Focal trace a cat that takes damage:
  per-part injury should appear in L1 records.
- **Phase 2:** Focal trace a fox raid — verify fox departs with wounded parts
  after a cat posse defence. Verify `WildlifeAiState::Fleeing` fires when
  fox pain exceeds threshold. Hawk grounding test: both wings Mangled → hawk
  no longer executes altitude-dive attack code path.
- **Phase 3:** Hunt a rabbit with claw-rake attack → verify Legs wound; verify
  flee speed penalty; verify prey entity persists (not despawned); verify
  carcass yield penalty if Body is also Wounded.
- All phases: `just verdict` gate must exit 0 on seed-42 post-landing soak.
  Drift > ±10% on any characteristic metric requires a hypothesis.

## Log

- 2026-05-01: Opened. Spec expanded from cat-only to full species coverage.
  Phased implementation plan added to decouple Phase 1 from ticket 025.
- 2026-05-02: Cross-link to ticket 046 (retired in this commit). Phase 1's
  `combat_advantage_normalized` rewiring (spec §IAUS Integration §2 — read
  `health_derived` instead of `Health.current`) carries the 046-Layer-1
  partial fix and is load-bearing for 046's retirement, not optional.
  Phase 2's dynamic `threat_power` per key-part condition (§IAUS §1)
  reduces opening-engagement urgency against wounded predators across
  encounters. The full time-to-kill rebuild (046-Layer-1 in its
  substrate-over-override form) lands as ticket **133**'s
  `combat_winnability` scalar's consumer wiring (separate follow-on
  tickets per 133's §Out of scope).
