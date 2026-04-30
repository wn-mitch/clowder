---
id: 2026-04-22
title: "Phase 5a — silent-advance audit: `StepOutcome<W>` + contract + never-fired canary"
status: done
cluster: null
landed-at: null
landed-on: 2026-04-22
---

# Phase 5a — silent-advance audit: `StepOutcome<W>` + contract + never-fired canary

Follow-on from the Phase 4c.3 / 4c.4 silent-advance pair (feed-
kitten and tend-crops). Those two bugs shared a shape — step
resolver silently returns `Advance` without producing its real-
world effect, caller emits `Feature::*` unconditionally or not at
all — and the Activation canary had no way to see the gap. Phase
5a turns that class of bug into a type error.

**Type contract.** New `src/steps/outcome.rs` defines
`StepOutcome<W>` (return type of every `pub fn resolve_*`) with a
`Witnessed` trait impl'd for `bool` and `Option<T>` but not for
`()`. The `record_if_witnessed(activation, Feature)` helper is
only callable on witness-carrying outcomes, so a resolver that
wants a Positive Feature must declare a witness type at its
signature. `#[must_use]` on the struct + clippy warnings catch
discarded returns.

**Documentation contract.** CLAUDE.md §"GOAP Step Resolver
Contract" specifies the 5-heading rustdoc preamble required on
every `resolve_*` (Real-world effect / Plan-level preconditions /
Runtime preconditions / Witness / Feature emission).
`scripts/check_step_contracts.sh` enforces it via `just check`.

**Migrations.** All 30+ step resolvers now return
`StepOutcome<_>` with the 5-heading docstring:
- Exemplars (already correctly gated, 3 files):
  `cook.rs`, `feed_kitten.rs`, `tend.rs`.
- High-severity silent-advance fixes with new Features (7):
  `eat_at_stores` → `FoodEaten`; `socialize` → `Socialized`;
  `groom_other` → `GroomedOther`; `mentor_cat` → `MentoredCat`;
  `fight_threat` → `ThreatEngaged`; `deliver` →
  `MaterialsDelivered`; `retrieve_raw_food_from_stores` wires
  existing `ItemRetrieved`.
- Medium-severity gating fixes (3): `harvest` (Fail instead of
  silent-reset on missing Stores; `CropHarvested` now gated on
  items placed); `mate_with` (+ new `CourtshipInteraction` for
  tom×tom); `deliver_directive` (gates existing
  `DirectiveDelivered`).
- Witness-less docs/uniformization: `sleep`, `self_groom`,
  `survey`, `patrol_to`, `move_to`, `gather`, `construct`,
  `repair` (+ new `BuildingRepaired`), `deposit_at_stores`,
  `retrieve_from_stores`, `retrieve_any_food_from_stores`, plus
  magic/* and fox/* resolvers (kept their plain `StepResult`
  returns where Feature emission was already correctly gated
  elsewhere; added contract preambles).

**Never-fired canary.** New `Feature::expected_to_fire_per_soak()`
predicate plus `SystemActivation::never_fired_expected_positives()`
→ footer field `never_fired_expected_positives`. `scripts/
check_canaries.sh` fails on non-empty list. Rare-legend features
(`ShadowFoxBanished`, `FateAwakened`, `ScryCompleted`, etc.) are
exempted. This is the canary that would have caught the farming
bug in the first soak after it broke.

**New Feature variants (8):** `FoodEaten`, `Socialized`,
`GroomedOther`, `MentoredCat`, `ThreatEngaged`,
`MaterialsDelivered`, `BuildingRepaired`, `CourtshipInteraction`.
All Positive. Total Positive features: 44 (up from 36).

**Drive-by:** fixed 10 pre-existing clippy warnings in
`target_dse.rs`, `modifier.rs`, `practice_magic.rs` so `just
check` comes up green with the new lint wired in.

**Verification:** `just check` green; `cargo test --lib` 948
passing (up from 945, +3 canary tests); canonical seed-42 900s
soak TK.

---
