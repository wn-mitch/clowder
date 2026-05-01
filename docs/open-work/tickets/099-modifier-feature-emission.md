---
id: 099
title: Feature emission for §3.5.1 Modifiers — colony-wide canary surface for substrate-lift signals
status: ready
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 088 (Body-distress Modifier) deferred `Feature::BodyDistressPromotionApplied`
because no existing Modifier emits a Feature — the trace's `ModifierDelta`
(`src/ai/eval.rs:330`) already records every firing for diagnostic purposes,
and adding emission for a single modifier requires either trait extension
(touching all 10 modifiers) or single-modifier carve-out at the pipeline call
site. Both larger than 088 warranted as a substrate-quality change.

If 047's CriticalHealth-interrupt retirement surfaces a need for an always-on
colony-wide canary that the body-distress lift fired during a soak (rather than
a per-cat focal-trace inspection), this ticket is where that work lands.
Designed once, applied uniformly to every Modifier in `default_modifier_pipeline`.

## Design

Two viable approaches, decide at unblock-time based on what 047 actually needs:

### A. `ScoreModifier` trait extension

Add an optional `feature_on_apply(&self) -> Option<Feature>` method to the
`ScoreModifier` trait (`src/ai/eval.rs:235-248`). Default impl returns `None`.
`BodyDistressPromotion` (and any other modifier opting in) returns
`Some(Feature::*)`. The `apply_with_trace` loop at `src/ai/eval.rs:289-311`
emits the feature exactly when the delta is non-zero (mirrors the existing
`if (score - pre).abs() > f32::EPSILON` gate). The activation increment
piggybacks on the trace-sink None/Some pattern: in production runs (sink=None),
features are still incremented through the canonical activation path; in trace
runs they're recorded both ways.

Pros: substrate-quality; symmetric with the `name()` method; every modifier
can opt in or stay silent.
Cons: requires resource access at the pipeline call site (the activation
counter is a Resource; the call site currently runs without one). Threading
that resource through `EvalCtx` or as a parameter is ~2 hours of plumbing.

### B. Pipeline call-site name match

In `apply_with_trace`, when a modifier's `pre != post` and its `name()`
matches a small lookup table (`"body_distress_promotion" → Feature::BodyDistressPromotionApplied`),
increment the activation counter. Hard-coded per modifier; no trait change.

Pros: ~10 LOC; no trait churn.
Cons: drift risk — adding a new modifier requires also editing the lookup
table; the name-string match is implicit coupling.

### Recommended: A

Trait extension is the substrate-correct version of the same change and
matches the §3.5.1 modifier-pipeline aesthetic. Defer until unblocked.

## Scope

- Pick A or B based on 047's verification needs at unblock-time.
- Add `Feature::BodyDistressPromotionApplied` to `Feature` enum
  (`src/resources/system_activation.rs`).
- Classify in `Feature::expected_to_fire_per_soak()` — likely `true` (a
  positive canary; if body distress is high enough to lift self-care, the
  cat is recovering, and the colony should produce non-zero counts per soak).
- Wire one or more Modifiers (start with `BodyDistressPromotion`) to emit.
- Test: focal cat at full distress emits the feature; colony soak at seed-42
  produces non-zero counts.

## Verification

- Unit test: trait method returns the right `Option<Feature>` per modifier.
- Pipeline test: synthetic soak with one body-distressed cat increments the
  activation counter exactly once per tick the lift was non-zero.
- Real soak: `just soak 42 && just verdict` — no canary regression; new
  feature shows up in the activation tracker.

## Out of scope

- Retiring 087's `BodyDistressed` ZST marker — it serves a different purpose
  (perception-event surface for outer DSE gates), not a canary signal.

## Log

- 2026-05-01: Opened as the substrate-quality follow-on to 088. Blocked-by 047
  because emission is only worth designing once we know whether 047's
  retirement actually needs colony-wide canary visibility or whether focal-
  trace inspection suffices.
