---
id: 088
title: Body-distress Modifier — uniform self-care promotion under §L2.10 Modifier substrate
status: blocked
cluster: ai-substrate
added: 2026-04-30
parked: null
blocked-by: [087]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Once interoceptive perception (087) publishes `body_distress_composite`, the natural next layer is a §L2.10 IAUS Modifier that uniformly promotes the *class* of self-care DSEs (Flee, Rest, Sleep, Eat, Hunt, Forage, GroomSelf) when composite distress exceeds a high band. Strictly stronger than per-DSE scoring because it can lift the whole self-care class above a single competitor that scores well on one axis but ignores the body — exactly the failure mode 047's treadmill exhibits when Guarding scores high on threat axes but ignores the cat's collapsing health.

Reframes 076 (last-resort-promotion-modifier, parked). 076 frames promotion as *post-failure* (after N recovery attempts fail); 088 frames it as *proactive* (when distress is high, before failures accumulate). The two are complementary: 088 is the early-warning lift, 076 is the panic-fallback. Both can ship.

Blocked-by 087 (the perception module that publishes the signal) and the §L2.10 Modifier substrate work which is in flight under ticket 014 / 060 epic.

## Scope

- New `BodyDistressPromotion` Modifier in `src/ai/modifier.rs` (or wherever §L2.10 Modifier substrate lands by then).
- Reads `body_distress_composite` sensor from interoceptive perception (087).
- Effect: additive lift on every self-care DSE's score when `body_distress_composite > body_distress_promotion_threshold`. Lift magnitude scaled by how far past threshold; tunable via `body_distress_promotion_lift` constant.
- Self-care DSE class: `Flee`, `Rest`, `Sleep`, `Eat`, `Hunt`, `Forage`, `GroomSelf`. Authoritatively listed as a `&[DseId]` constant in the Modifier source so the class is grep-discoverable.
- New `Feature::BodyDistressPromotionApplied` (Negative or Neutral category — TBD; depends on whether it's a distress signal worth canary-monitoring or just a routine score nudge).
- New `PlanningSubstrateConstants` knobs: `body_distress_promotion_threshold` (default ~0.7), `body_distress_promotion_lift` (default ~0.20).

## Verification

- Unit test: `BodyDistressPromotion::compute` returns the configured lift when `body_distress_composite >= threshold`, 0.0 otherwise.
- Integration test: a cat with high hunger + low health *not* in immediate threat range — Flee/Rest/Eat scores rise as a class, not individually.
- `just soak 42 && just verdict` post-080-stable-baseline. Expect interrupts_by_reason further reduced from 087's baseline; expect the new feature to fire non-zero counts on the seed-42 stuck-pattern recovery.

## Out of scope

- Modifying the per-DSE scoring curves themselves — this Modifier sits outside the per-axis composition and lifts the aggregate.
- L4/L5 distress (mastery / purpose) — handled by 090 once those scalars exist.

## Log

- 2026-04-30: Opened alongside 087. Blocked-by 087 (perception substrate) until that lands.
