---
id: 111
title: Retire 088 BodyDistressPromotion once kind-specific modifiers cover its surface
status: ready
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [088-body-distress-modifier.md]
landed-at: null
landed-on: null
---

## Why

088's `BodyDistressPromotion` is the original undifferentiated lift — `body_distress_composite = max(deficits)` flattens which-kind-of-distress into one scalar and lifts six self-care DSEs uniformly. The kind-specific modifier program (047, 106, 107, 110) replaces this surface with discriminating modifiers — each reading the source axis directly with a curve and DSE-targeting that match the actual phenomenon.

Once HungerUrgency (106), ExhaustionPressure (107), and ThermalDistress (110) land — joining 047's AcuteHealthAdrenaline as the fourth axis-specific modifier — 088's surface is fully replaced. Retire it.

## Scope

- Remove `BodyDistressPromotion` struct + impl from `src/ai/modifier.rs`.
- Remove pipeline registration in `default_modifier_pipeline`.
- Remove constants `body_distress_promotion_threshold` + `_lift` from `sim_constants.rs`.
- Bump pipeline-count assertion in `default_pipeline_registers_*_modifiers` test.
- Deprecate `BODY_DISTRESS_COMPOSITE` constant in `modifier.rs` if no remaining consumers (087's perception layer publishes the scalar; if no modifier reads it, the publication wastes per-tick work — `interoception::body_distress_composite` may also be removable, depending on 087-marker dependencies).

## Verification

- Hypothesize: re-run 088's soak with the modifier removed; expect no behavioral change relative to (047 + 106 + 107 + 110) all active. If there IS behavioral change, 088 was load-bearing somewhere the kind-specific modifiers don't cover — investigate.
- `just verdict` post-removal: survival canaries hold.

## Out of scope

- Retiring the 087 `BodyDistressed` ZST marker if its consumers are downstream of the modifier (separate analysis once this lands).

## Log

- 2026-05-01: Opened as cleanup follow-on from ticket 047. Blocked by the three remaining substrate-axis tickets that complete 088's replacement.
- 2026-05-02: **Unblocked.** 106 + 107 + 110 all landed inert by today (each in its own
  branch of the substrate-over-override outcome tree per
  `docs/systems/distress-modifiers.md`). Status flipped `blocked` → `ready`,
  blocked-by cleared. Note for the implementer: with all four kind-specific
  modifiers shipping inert by default (047/106/107/110), 088's runtime effect IS
  still load-bearing in the canonical regime — retirement should be paired with
  a same-commit verification soak that confirms 088's removal doesn't shift any
  footer metric (or, if it does, surface the lifts on the kind-specific modifiers
  to recover whatever 088 was contributing).
