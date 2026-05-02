---
id: 107
title: ExhaustionPressure modifier — substrate axis for Exhaustion interrupt retirement
status: in-progress
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

`InterruptReason::Exhaustion` (`src/systems/disposition.rs:315`) — same per-tick override pattern as 047's CriticalHealth. Substrate-over-override wants a kind-specific modifier reading `energy_deficit` (already in `ctx_scalars`) that lifts Sleep + GroomSelf so a tired cat selects rest unprompted.

Pressure modifier (graded ramp). Sibling to ticket 106 (HungerUrgency).

## Scope

- New `ExhaustionPressure` modifier reading `energy_deficit`.
- Lifts Sleep (largest), GroomSelf (smaller — exhausted cats sometimes groom-then-sleep as a settling ritual).
- Constants: `exhaustion_pressure_threshold`, `exhaustion_pressure_sleep_lift`, `exhaustion_pressure_groom_lift`. Default 0.0; enable via hypothesize patch.
- Phase 3 hypothesize predicting `interrupts_by_reason.Exhaustion` decreases.
- Phase 4 retire `InterruptReason::Exhaustion` branch (`disposition.rs:314-316`).
- **Wrapper cleanup (per landed-112's supersession Log):** if 106 has already
  landed at the time 107's Phase 4 ships, also delete the wrapping
  `if !matches!(disposition.kind, Resting | Hunting | Foraging)` block at
  `disposition.rs:305-317` — both arms inside it are gone, the wrapper is
  dead code. If 106 hasn't landed yet, leave the wrapper for 106's Phase 4
  to remove.

## Verification

- Same playbook as 047 / 106.

## Out of scope

- HungerUrgency (106), ThreatProximityAdrenaline (108), ThermalDistress (110).

## Log

- 2026-05-01: Opened as substrate-axis follow-on from ticket 047, applying the playbook to the energy axis.
- 2026-05-02: **Phase 1 landed** at c83de3cd alongside 106/110 — modifier registered (pipeline +1), 3 ScoringConstants fields with 0.0 lift defaults (ships inert), 7 unit tests pass. Phases 2-5 remain.
- 2026-05-02: **Phase 2 verdict — LIFT WINS L2, BLOCKED BY PLAN-COMPLETION
  MOMENTUM (047 PATTERN ON THE ENERGY AXIS); SHIP INERT.** 900s focal-trace
  soak at seed 42 (focal Simba) with proposed lifts (0.40 sleep / 0.10
  groom_self above deficit 0.7) AND doubled energy decay (0.2/day) via
  `CLOWDER_OVERRIDES`; run dir `logs/tuned-trace-42-107-phase-2`. Footer:
  `interrupts_by_reason.Exhaustion == 0` regardless. Energy-deficit
  survey across all 8 cats: every cat enters the modifier window at
  least briefly (range 0.3% – 8.1% of ticks); Nettle is the deep case
  with 79 in-window ticks AND 33 ticks at energy ≤ 0.10 (legacy
  threshold). Mechanical wiring verified: at Simba's tick 1202395
  (deficit just above 0.7), L2 row shows `exhaustion_pressure` modifier
  firing on Sleep (+0.0005 micro-delta from the ramp's threshold edge)
  and on GroomSelf (+0.0001) — the gated-boost contract is honored.
  At deeper deficits the lift would be larger: at deficit 1.0 (Nettle),
  full ramp = (1.0-0.7)/(1.0-0.7) = 1.0 × 0.40 = +0.40 on Sleep, which
  is mechanically sufficient to win L2 against most competing DSEs.
  **Behavioral expression failure:** Nettle at deficit 1.0 spends 15+
  consecutive snapshots in `action=Forage` despite Sleep winning L2 —
  the 047/118 plan-completion-momentum gap, expressed on the energy
  axis. The Foraging disposition holds GOAP commitment so Sleep DSE
  wins the score contest but GOAP never executes the rest plan.
  **Why the legacy interrupt can't catch this:** Foraging is in the
  exemption list at `disposition.rs:315-318`. Even at energy 0.0,
  Nettle is shielded from the Exhaustion interrupt branch — the
  interrupt arm is structurally vestigial. Phase 3 hypothesize sweep
  skipped: `Exhaustion == 0` for both baseline AND treatment is the
  noise floor; `direction: decrease` is unmeasurable.
- 2026-05-02: Phase 4 reframed — **interrupt retirement becomes structural
  cleanup, not behavior change.** Same shape as 106's verdict. Removing
  the `Exhaustion` arm at `disposition.rs:322-324` is zero-risk: the
  arm cannot fire today because either (a) the cat is exempt
  (Hunting/Foraging/Resting), or (b) the cat's energy is healthy
  enough to never reach the threshold. Phase 4 may proceed in a
  follow-up session as a code-debt commit; canary suite confirms.
- 2026-05-02: Energy-axis plan-completion-momentum gap surfaced. Suggest
  expanding ticket 118 (originally health-axis-only) to cover all
  pressure modifiers, OR opening a sibling 118-shape ticket for the
  energy axis. Until that gap is fixed, ExhaustionPressure activation
  (lifts > 0.0) won't translate to behavioral change at L3 — same
  outcome as 047 on health.
