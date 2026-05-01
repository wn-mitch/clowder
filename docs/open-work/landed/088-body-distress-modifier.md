---
id: 088
title: Body-distress Modifier — uniform self-care promotion under §L2.10 Modifier substrate
status: done
cluster: ai-substrate
added: 2026-04-30
landed-at: null
landed-on: 2026-05-01
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
---

# Body-distress Modifier — uniform self-care promotion under §L2.10 Modifier substrate

**Why:** Once 087's interoceptive perception substrate (landed `fc4e1ab`)
published `body_distress_composite` — the unified "I am unwell" scalar (max of
hunger/energy/thermal/health deficit) — the natural next layer was a §L2.10
Modifier that lifts the *class* of self-care DSEs as a unit when distress
exceeds a high band. Strictly stronger than per-DSE scoring because it can lift
the whole class above a single competitor that scores well on one axis but
ignores the cat's body — exactly the failure mode 047's CriticalHealth
treadmill exhibits when Guarding scores high on threat axes while the cat
takes 0.18 damage/tick to death. **088 is the substrate prerequisite for 047**:
without class-level lift on Flee + Eat + Sleep, retiring 047's interrupt branch
would re-create the Mallow-shape collapse.

**What landed:**

1. **`src/ai/modifier.rs`** — new `BodyDistressPromotion` struct in §3.5.1,
   mirroring `StockpileSatiation`'s shape. Trigger:
   `body_distress_composite > body_distress_promotion_threshold`; transform:
   additive lift on Flee / Sleep / Eat / Hunt / Forage / GroomSelf, where
   `lift = ((distress − threshold) / (1 − threshold)) × lift_scale`. Roster
   authored as `SELF_CARE_DSES: &[&str]` constant for grep-discoverability;
   `apply` matches the same set inline via `matches!` for compile-time
   efficiency. Registers in `default_modifier_pipeline` between `Tradition`
   and `FoxTerritorySuppression` — additive lifts before multiplicative damps,
   so under combined high stockpile + high body distress the lift on Eat
   fires before `StockpileSatiation` damps Hunt/Forage (composition pre-
   described in the 094 doc-comment).
2. **`src/resources/sim_constants.rs`** — `ScoringConstants::body_distress_promotion_threshold`
   (default 0.7) and `body_distress_promotion_lift` (default 0.20). Threshold
   set deliberately *above* 087's `body_distress_threshold` (0.6, the marker-
   insertion gate) so the marker fires first as a perception event and the
   modifier engages later as a stronger response. Both serialize into the
   `events.jsonl` header per the comparability invariant.
3. **Seven new unit tests** under `mod tests`: no-lift-below-threshold,
   zero-lift-at-threshold (boundary), lifts-above-threshold (linear ramp),
   max-lift-at-full-distress, targets-only-self-care-class (14 non-target
   DSEs asserted unchanged at full distress), does-not-resurrect-zero-score
   (gated-boost contract), composes-with-stockpile-satiation (full-pipeline
   regression: high stockpile + high distress → Eat 0.27 → 0.47 wins over
   Hunt 0.85 → ~0.23). Pipeline-length assertion bumped from 9 to 10.

**Implementation deviations from plan.** Two:

- **Self-care class is 6 DSEs, not 7.** The ticket scope listed `Flee, Rest,
  Sleep, Eat, Hunt, Forage, GroomSelf`. There is no `Rest` DSE in the catalog
  — only `Sleep` exists for energy recovery (087's "implementation deviation
  from plan" already covered the same observation: a distinct Rest DSE would
  require resolver wiring + id propagation across many sites and is high
  risk). Sleep covers the role; the actual class is six. Documented inline
  in `SELF_CARE_DSES`'s doc-comment so the deviation doesn't re-emerge.
- **Feature emission deferred.** The ticket scope mentioned a
  `Feature::BodyDistressPromotionApplied` "(Negative or Neutral category —
  TBD)." No existing Modifier emits a Feature — modifiers are pure `&self`
  score transformers and the trace's `ModifierDelta` (`src/ai/eval.rs:330`)
  already records every firing for diagnostic purposes. Adding emission
  requires either trait extension (touching all 10 modifiers) or single-
  modifier carve-out at the pipeline call site; both larger than this ticket
  warrants. Diagnostic need (047's verification that the lift fires) is
  served by `just soak-trace 42 <cat>` + grep `body_distress_promotion` in
  the trace's `modifier_deltas` rows. Follow-on ticket NNN opened for the
  substrate-quality version covering all modifiers uniformly, in case 047's
  verification surfaces a need for an always-on canary.

**Substrate-over-override discipline.** The modifier respects the established
"gated-boost contract" — short-circuits on `score <= 0.0` so it doesn't
resurrect a DSE the Maslow pre-gate or outer scoring layer suppressed.
Ecologically: high body-distress doesn't conjure food into existence or create
a safe sleep spot; the modifier only re-ranks already-accessible
considerations. Matches the convention every other additive modifier (Pride /
IndependenceSolo / Patience / CommitmentTenure / Tradition) follows.

**Verification:**

- `just check && cargo test --lib` — both green; **1659/1659 tests passing**
  including 7 new `BodyDistressPromotion` tests.
- Empirical magnitude tuning (whether default 0.20 is enough to suppress
  non-self-care DSEs through the IAUS contest alone) is **deferred to ticket
  047**, since 047 is the consumer that needs the magnitude to be sufficient.
  Ship at 0.20; let 047 verify via focal trace and tune as part of its
  CriticalHealth-interrupt retirement.

**Landed at:** TBD (this commit).
