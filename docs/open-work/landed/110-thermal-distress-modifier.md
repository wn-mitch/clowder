---
id: 110
title: ThermalDistress modifier — substrate axis for thermal interrupts (and shelter-seeking)
status: done
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [110-thermal-distress.md]
landed-at: c83de3cd
landed-on: 2026-05-02
---

## Why

`thermal_deficit` is already published in `ctx_scalars` but no modifier currently consumes it directly — it composes into `body_distress_composite` only. A kind-specific lift on Sleep (find shelter) and eventually Build (construct shelter) gives cold cats a behaviorally-distinct response from "generally distressed cats."

Lower priority than 106/107/108 because no current `InterruptReason::ThermalCritical` branch exists to retire; this is purely a perception-richness lever (the "shake the tree" pattern from ticket 047's design — richer cat understanding ⇒ more levers).

## Scope

- New `ThermalDistress` modifier reading `thermal_deficit`.
- Lifts Sleep (find a den / hearth; routes to warm tile) and Build (eventually — construct shelter; out of scope for v1).
- Constants: `thermal_distress_threshold`, `thermal_distress_sleep_lift`. Default 0.0 inert.
- Phase 3 hypothesize predicting cold-weather mortality drops.

## Out of scope

- The Build-lift (deferred — needs a "BuildShelter" disposition variant to make sense).
- Composing with weather forecast (separate spec).

## Log

- 2026-05-01: Opened as the fourth substrate-axis follow-on from ticket 047 — the lower-priority "more levers" application of the doctrine.
- 2026-05-02: **Phase 1 landed** at c83de3cd alongside 106/107 — modifier registered (pipeline +1), 2 ScoringConstants fields with 0.0 lift defaults, 6 unit tests. Phase 3 (hypothesize sweep predicting cold-weather mortality drops) and Build-shelter lift remain.
- 2026-05-02: **Phase 2 verdict — TRIGGER REACHABLE BUT LIFT OUTSCORED BY SIBLING DSE; SHIP INERT.**
  900s focal-trace soak at seed 42 (focal Lark) with proposed Sleep lift (0.40 above
  `thermal_deficit ≥ 0.7`) via `CLOWDER_OVERRIDES`. Run dir
  `logs/tuned-trace-42-110-phase-2`. Survey across all 8 cats: every cat enters the
  modifier window briefly (0.15%–0.66% of cat-ticks); total 24 dips / 9052 snapshots
  = 0.27%. Lark is the deepest case (9 dips, min temp 0.27, max deficit 0.73).
  Mechanical wiring verified: `thermal_distress` fires 52 times in Lark's L2 trace,
  exclusively on `dse:"sleep"`, with delta proportional to ramp × 0.4 lift (sample
  at tick 1234673: deficit ≈ 0.74, delta = +0.054 on Sleep, composing additively
  with `body_distress_promotion`). Override propagation echoed in trace header.
  **Behavioral expression failure (third pattern, distinct from 106/107):** at
  Lark's deepest dip, L2 contest resolves `groom_self`=0.764 > `sleep`=0.488 (with
  lift) > `eat`=0.446 — `groom_self`'s IAUS at `src/ai/dses/groom_self.rs:36`
  already encodes `thermal_deficit` as a `CompensatedProduct` consideration, so
  the existing substrate routes cold cats to grooming/settling. Sleep never wins
  L2 at trigger ticks; L3 picks Groom or Patrol. Footer of Phase 2 run is
  bit-identical to baseline (8× ShadowFoxAmbush, 999 courtship, 219 play, 194
  grooming, 41 mythic-texture, 43017 CriticalHealth, every plan-failure-by-reason
  count identical). Lark's Sleep action share unchanged (49/1369 = 3.6%).
  Phase 3 hypothesize sweep skipped: behavioral metric (Sleep share during
  trigger window) is identical between baseline and treatment by construction.
- 2026-05-02: **No Phase 4.** ThermalDistress has no legacy
  `InterruptReason::Thermal*` arm to retire (unlike 106/107). Wiring stops at
  Phase 1; the modifier ships as perception-richness substrate, dormant in the
  canonical regime, ready to compose if a future regime (winter-bias seeds,
  climate-shift scenarios, Build-shelter DSE) needs the lever. Build-shelter
  lift stays deferred per §Out-of-scope — needs a "BuildShelter" disposition
  variant first.
- 2026-05-02: **Phase 5 landed** — `docs/balance/110-thermal-distress.md` written
  with the full four-artifact (hypothesis · prediction · observation ·
  concordance) record + Decision section. The doc names this ticket as the
  worked example for the **third branch** of the substrate-over-override outcome
  tree at ticket 113's `docs/systems/distress-modifiers.md`: **"L2-lift outscored
  by sibling DSE that already encodes the same scalar"** — distinct from 106's
  "trigger rarely met" and 107's "lift wins L2 blocked by plan-completion
  momentum". Doctrine table at `docs/systems/distress-modifiers.md` updated
  (Status: Ready → Landed inert). Status flipped to `done`; ticket moves to
  `landed/`.
