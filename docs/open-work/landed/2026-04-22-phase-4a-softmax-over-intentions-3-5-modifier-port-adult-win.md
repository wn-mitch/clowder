---
id: 2026-04-22
title: Phase 4a — softmax-over-Intentions + §3.5 modifier port + Adult-window retune
status: done
cluster: null
landed-at: c4552dc
landed-on: 2026-04-22
---

# Phase 4a — softmax-over-Intentions + §3.5 modifier port + Adult-window retune

Three Phase 4 deliverables landed together on the
`docs/balance/substrate-phase-4.md` balance thread. Each addresses one
of the three Phase 3 exit-soak regressions that prompted open-work #14:

- **§L2.10.6 softmax-over-Intentions** (`src/ai/eval.rs`,
  `src/ai/scoring.rs`, `src/ai/fox_scoring.rs`, `src/systems/goap.rs`,
  `src/systems/disposition.rs`, `src/systems/fox_goap.rs`). Replaced
  the `aggregate_to_dispositions → select_disposition_softmax`
  two-step and the fox-side argmax with direct softmax over the flat
  Intention pool. New `select_intention_softmax` in `eval.rs` consumes
  `&[ScoredDse]` per §L2.10.6; bridge helper
  `select_disposition_via_intention_softmax` in `scoring.rs` operates
  on the legacy `(Action, f32)` pool and maps via
  `DispositionKind::from_action`. New
  `ScoringConstants::intention_softmax_temperature` (default 0.15).
- **§3.5 modifier-pipeline port** — new `src/ai/modifier.rs` with
  three `ScoreModifier` impls (`WardCorruptionEmergency`,
  `CleanseEmergency`, `SensedRotBoost`). `ScoreModifier::apply`
  extended to take a `fetch_scalar` closure so modifiers read
  trigger inputs through the same canonical scalar surface as DSE
  considerations. `ctx_scalars` gained `nearby_corruption_level`,
  `maslow_level_2_suppression`, `has_herbs_nearby`, `has_ward_herbs`,
  `thornbriar_available`. The three emergency-bonus additions at
  `scoring.rs:576–712` are retired; pipeline registered at all four
  mirror sites (`plugins/simulation.rs` + `main.rs` setup_world /
  run_new_game + test infra in scoring.rs).
- **Adult life-stage window retune** — `Age::stage` Adult upper
  bound 47 → 59, Elder 60+. Paired update: `DeathConstants::
  elder_entry_seasons` 48 → 60 and `FounderAgeConstants::
  elder_{min,max}_seasons` 48/50 → 60/62 to keep the stage /
  old-age-mortality coupling and founder-runway invariants intact.
  Marker doc comments updated; `age_stages_at_boundaries` test
  updated to the new thresholds.

**Concordance on seed-42 `--duration 900` re-soak (landed commit `c4552dc`, log `logs/phase4a-c4552dc/events.jsonl`):**

| Metric | Baseline (`562c575`) | Phase 4a | Direction |
|---|---|---|---|
| deaths_by_cause.Starvation | 8 | 0 | ✅ canary passes |
| MatingOccurred | 0 | 0 | flat (substrate gate opens but density is a follow-on tune — dirty-commit run hit 1 on seed noise) |
| BondFormed | 16 | 34 | +112% |
| ScryCompleted | 256 | 615 | +140% |
| WardPlaced | 89 | 264 | +197% |
| ward_avg_strength_final | 0.0 | 0.39 | wards persisted |
| Grooming (continuity) | 30 | 213 | +610% |
| KnowledgePromoted | 35 | 92 | +163% |

Canonical `scripts/check_canaries.sh` passes all four survival
canaries (Starvation == 0, ShadowFoxAmbush ≤ 5, footer written,
features_at_zero informational). Generational-continuity canary still
fails (0 kittens matured) but that tracks with the MatingOccurred
density gap, not the substrate mechanisms shipped.

**Remaining Phase 4 work** moved to open-work #14 (outstanding):
target-taking DSE registration, §4 marker-eligibility authoring
systems, §7.M.7.4 `resolve_mate_with` gender fix, and the
MatingOccurred density + Cleanse/Harvest/Commune/Farming dormancy
balance gaps unblocked by §4 marker authoring.

Balance thread: `docs/balance/substrate-phase-4.md`.

---
