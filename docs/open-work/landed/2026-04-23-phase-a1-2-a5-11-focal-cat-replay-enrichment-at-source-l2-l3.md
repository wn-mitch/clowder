---
id: 2026-04-23
title: "Phase A1.2 (A5) — §11 focal-cat replay enrichment: at-source L2/L3 trace capture"
status: done
cluster: null
landed-at: null
landed-on: 2026-04-23
---

# Phase A1.2 (A5) — §11 focal-cat replay enrichment: at-source L2/L3 trace capture

Closes out the last cluster-A entry. Phase 1/2 of §11 shipped the
sidecar file, three record variants, replay script, and
influence-map L1 walk as a *shim* — L2 records emitted empty
`considerations`/`modifiers` arrays and L3 records emitted empty
softmax probabilities, so the joinability invariant (§11.4) was
vacuous in practice. A1.2 fills those fields with real data pulled
from the live scoring pass, routed through three new `_with_trace`
variants on `evaluate_single` / `ModifierPipeline::apply` /
`select_disposition_via_intention_softmax`.

**At-source capture pattern — observational, not behavior-altering.**
Each `_with_trace` variant takes `Option<&mut Sink>`; the plain
entry points delegate with `None`. When the sink is absent the cost
is a single `Option` check per pass, matching §11.5's zero-
overhead-when-dormant contract. New types in `src/ai/eval.rs`:

- `EvalTrace` — per-DSE `{considerations: Vec<ConsiderationTraceRow>,
  composition_{mode,weights,compensation_strength}, maslow_pregate,
  modifier_deltas: Vec<ModifierDelta>}`. `ConsiderationTraceRow`
  carries `{name, kind, input, curve_label, score, weight,
  spatial_map_key}` per §11.3 L2 record schema.
- `ModifierDelta` — `{name, pre, post}`. `apply_with_trace` pushes
  rows only when `pre != post` so no-op passes stay out of replay
  frames.

New `SoftmaxCapture` in `src/ai/scoring.rs` parallels the §11.3 L3
record: `{pool, weights, probabilities, temperature, raw_roll,
chosen_idx, chosen_action, empty_pool}`. The empty-pool flag
distinguishes "softmax ran and picked" from "fallthrough to
`DispositionKind::Resting`" — an ambiguity the shim couldn't
express.

**Focal-cat detection threaded through `EvalInputs`.** Two new
fields (`focal_cat: Option<Entity>`,
`focal_capture: Option<&FocalScoreCapture>`) populated from the new
`PlanResources::focal_target` / `focal_capture` Bevy resources.
`score_dse_by_id` routes through the traced variant when
`ctx.cat == focal_cat`; non-focal cats take the untraced path
unchanged. Interior-mutex on `FocalScoreCapture` lets `EvalInputs`
carry a shared reference while `score_dse_by_id` accumulates rich
captures — matching the existing pattern where `EvalInputs` is
passed by `&EvalInputs` through ~30 call sites in `score_actions`.

**`emit_focal_trace` rewritten as a drain.** The old shim read
`CurrentAction::last_scores` post-hoc and emitted empty-body
records every tick. The new system drains `FocalScoreCapture`
populated by the scoring pass and emits L2/L3 records **only on
planning ticks** — `evaluate_and_plan` fires when plans expire or
need replanning (~every 200 ticks for a mid-plan cat), not every
tick, so 90%+ of ticks had no softmax data for the shim to emit.
L1 continues to emit every tick from the influence-map walk
because senses don't gate on the GOAP cadence.

**Verification.** `just check` + `cargo test --lib` green (1092
tests pass, +7 new A1.2 tests: `evaluate_single_with_trace_captures_consideration_input_and_score`,
`evaluate_single_without_trace_is_zero_cost_path`,
`modifier_pipeline_apply_with_trace_records_nonzero_deltas`,
`softmax_capture_records_probabilities_sum_to_one`,
`softmax_capture_flags_empty_pool_fallthrough`,
`softmax_without_capture_matches_capture_variant`,
`focal_capture_accumulates_and_drains`).

**Seed-42 15min release soak** (`just soak-trace 42 Simba` →
`logs/a5-focal/`): 491,545 trace records, 713 planning ticks for
Simba, 80,033 event entries, sim day 1297. Survival canaries hold:
Starvation = 0, ShadowFoxAmbush = 0, footer written. Sample replay
at mid-run tick 1,249,409 (saved as `logs/a5-focal/sample-frame.txt`)
shows:

- **L1** — 5 registered influence maps sampled at Simba's position
  (29, 10) with per-channel attenuation breakdowns for each.
- **L2** — 12 DSE evaluations with real per-consideration rows
  (e.g. `forage.hunger_urgency = 0.408 → Logistic(8, 0.75) →
  score 0.061, weight 0.30`), composition mode / raw / Maslow
  pre-gate, and modifier deltas when they fired
  (`hunt.pride: +0.075`, `explore.independence_solo: +0.080`).
- **L3** — ranked pool of 9 Intentions with real softmax
  probabilities summing to 1.0 (Explore, Sleep, Forage, Hunt, …),
  temperature 0.15, chosen = Explore.

`never_fired_expected_positives` footer lists 11 features
(`KnowledgePromoted`, `ItemRetrieved`, `FoodCooked`, `KittenBorn`,
`GestationAdvanced`, `MatingOccurred`, `KittenFed`, `CropTended`,
`CropHarvested`, `GroomedOther`, `MentoredCat`). None attributable
to A1.2: parity-guard tests (`evaluate_single_without_trace_is_zero_cost_path`,
`softmax_without_capture_matches_capture_variant`) enforce at
compile+test time that the traced variants match the untraced
variants bit-for-bit, so the capture path is observation-only.
Tracked separately against the balance-deferred cluster.

**Out of scope — deferred to §11.6 follow-ons.** Per-modifier
catalog expansion, GUI frame-scrubber, event-triggered records,
aggregate-distribution footer, L1 lazy emission from
`Consideration::Spatial` (no live DSE has spatial considerations
yet), top-N losing-axis + apophenia schema slots.

**Specification cross-ref:** `docs/systems/ai-substrate-refactor.md`
§11.1–§11.7, `docs/systems/a1-2-focal-cat-replay-kickoff.md`.
