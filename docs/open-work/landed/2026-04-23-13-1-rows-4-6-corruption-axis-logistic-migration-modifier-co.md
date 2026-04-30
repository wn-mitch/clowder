---
id: 2026-04-23
title: §13.1 rows 4–6 — corruption-axis Logistic migration + modifier/constant retirement
status: done
cluster: null
landed-at: null
landed-on: 2026-04-23
---

# §13.1 rows 4–6 — corruption-axis Logistic migration + modifier/constant retirement

Track B closeout for the AI substrate refactor's §2.3 "Retired
constants" subsection. Retires the three corruption-emergency
`ScoreModifier` impls by absorbing their flat additive
contribution into five axis-level Logistic curves. Behavior-
preserving by construction per the spec's "Logistic curves
absorb modifier contribution" contract. Shipped as Session B of
a three-way parallel agent fan-out; Sessions A and C landed
together as part of the same integrated stack.

**Five axis migrations in `src/ai/dses/`:**

- `herbcraft_gather.rs` — NEW `territory_max_corruption` axis
  with `Logistic(8, 0.1)`; composition widened
  `CompensatedProduct(vec![1.0, 1.0]) → CompensatedProduct(vec![1.0, 1.0, 1.0])`.
- `herbcraft_ward.rs` — NEW `territory_max_corruption` axis with
  `Logistic(8, 0.1)`; same composition widening.
- `practice_magic.rs::DurableWardDse` — NEW
  `nearby_corruption_level` axis with `Logistic(8, 0.1)`;
  composition `CP(3) → CP(4)`.
- `practice_magic.rs::CleanseDse` — SWAP `tile_corruption`
  axis curve from `linear()` to
  `Logistic(8, scoring.magic_cleanse_corruption_threshold)`.
  Factory signature changes: `CleanseDse::new()` → `new(scoring:
  &ScoringConstants)` so the Logistic midpoint reads the
  threshold. Three call-sites updated (`main.rs::build_schedule`
  new + save-load paths + `plugins/simulation.rs`).
- `practice_magic.rs::ColonyCleanseDse` — SWAP
  `territory_max_corruption` axis curve from `linear()` to
  `Logistic(6, 0.3)`.

**Three modifier impl deletions in `src/ai/modifier.rs`:**

- `WardCorruptionEmergency` — absorbed by
  `herbcraft_ward.territory_max_corruption` +
  `herbcraft_gather.territory_max_corruption`.
- `CleanseEmergency` — absorbed by
  `practice_magic::cleanse.tile_corruption` +
  `practice_magic::colony_cleanse.territory_max_corruption`.
- `SensedRotBoost` — absorbed by
  `practice_magic::durable_ward.nearby_corruption_level`.

Their registration in `default_modifier_pipeline` retires too
(pipeline now 7 modifiers — down from 10 after §3.5 remaining-
modifier port's full roster); 4 retired-only scalar-surface keys
removed from `ctx_scalars` (`has_herbs_nearby`, `has_ward_herbs`,
`thornbriar_available`, `maslow_level_2_suppression`); 6 unit
tests that only exercised the retired modifiers removed; 1
`ward_corruption_emergency_boosts_score` scoring.rs test
rewritten as `ward_score_rises_with_territory_corruption` using
relative-monotonic assertions. `modifier.rs` shrank from 1,491
to 1,135 lines.

**Three constant deletions in `src/resources/sim_constants.rs`:**

- `ward_corruption_emergency_bonus`
- `cleanse_corruption_emergency_bonus`
- `corruption_sensed_response_bonus`

Both struct-def entries and `Default` impl entries.

**Non-goals (Session B scope-fence):** did NOT touch rows 1–3
(Incapacitated — Session A), did NOT touch §7 commitment
(Session C), did NOT add new considerations beyond the five
named, did NOT rebalance spec-committed curve parameters, did
NOT update `docs/open-work.md` (plan-maintainer scope).

**Verification:** `just check` clean. `just test` 1093 pass / 0
fail (+~15 new axis-witness tests; net +8 after retired-modifier
test removals). Session-isolated pre-merge soak (commit_dirty=
true): Starvation=3 (within 0–5 noise band), ShadowFoxAmbush=0,
wards_placed_total=216, ward_count_final=3, footer_written.
Integrated post-merge footer below.

**Integrated-stack soak footer (seed 42, `--duration 900`,
release, commit_dirty=false; shared with sibling §13.1 rows 1–3
landing — the two landings are verified as one stack since they
share file-overlap on herbcraft/PM sibling DSEs):**

- `deaths_by_cause`: `{"Starvation": 2}` (within documented
  0–5 Bevy scheduler-variance band per CLAUDE.md; pre-fan-out
  baseline was 0, disabled-gate sibling soak was 0 — the 2 is
  within observed run-to-run variance on same-commit families).
- `shadow_fox_spawn_total: 0`, `shadowfox_ambush_deaths: 0`.
- `wards_placed_total: 200`, `ward_count_final: 2` (pre-fan-out
  baseline 216 / 3; within noise).
- `continuity_tallies`: grooming 129, courtship 2, others 0.
  Grooming + courtship firing where they'd been zero in the
  failed A+B+C integrated soak (Session C's drop-trigger
  regression).
- `positive_features_active: 20 / 44`, `negative_events_total:
  128,641`.
- `never_fired_expected_positives`: `["FoodCooked", "CropTended",
  "CropHarvested", "GroomedOther", "MentoredCat"]` — 5 features,
  all subset of the Phase A1.2 baseline's 11-feature
  never-fired list. **Net +6 features now firing** that weren't
  firing pre-fan-out (KnowledgePromoted, ItemRetrieved,
  KittenBorn, GestationAdvanced, MatingOccurred, KittenFed).
- `footer_written: 1` ✓.

**Specification cross-ref:** `docs/systems/ai-substrate-refactor.md`
§2.3 rows 4–6. Original kickoff:
`docs/systems/a1-4-retired-constants-kickoff.md` (framed the full
§13.1 as one commit; the parallel fan-out naturally split into
this rows-4–6 landing and its sibling rows-1–3 landing).
