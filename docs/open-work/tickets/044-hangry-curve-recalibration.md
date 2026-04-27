---
id: 044
title: Recalibrate hangry curve from Logistic(8, 0.75) to Logistic(8, 0.5) — cats now eat at half-hungry, not just emergency-hungry
status: in-progress
cluster: null
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [needs.md]
related-balance: [hangry-recalibration.md, healthy-colony.md]
landed-at: null
landed-on: null
---

## Why

The 1-hour collapse-probe soak (`logs/collapse-probe-42/`, seed 42, 17 in-game years, full extinction) surfaced 4 starvation deaths with food stockpile at ≥ 0.90. Three of the four (Ivy, Simba, Lark) showed regular plan-creation activity — they were *not* locked out of `evaluate_and_plan` (that's ticket 043's pattern, Calcifer specifically). They re-evaluated normally; Eat just never won.

Snapshot evidence at tick 1,250,500 (52 ticks before Calcifer's death):

| Cat | hunger | Eat score | Eat rank | Top disposition |
|---|---|---|---|---|
| Calcifer (dying) | 0.0 | 0.053 | 12th of 14 | Sleep (0.606) |
| Ivy (will die in 14k ticks) | 0.61 | 0.046 | 10th of 12 | Socialize (0.605) |
| Lark (healthy, will die last) | 0.58 | 0.105 | 7th of 12 | Hunt (0.686) |

The Eat scores match `hangry()` exactly at the given urgencies — the curve is doing what it says, the curve says the wrong thing. `Logistic(8, 0.75)` evaluated at urgency=0.4 (hunger=0.6) is 0.057; at urgency=0.5 (hunger=0.5) is 0.018. The cat has to drop to hunger=0.3 (urgency=0.7, score 0.5) before Eat starts winning — and the gap from there to hunger=0 is a narrow window in which any plan-failure (the run shows 76 `TravelTo(SocialTarget): no reachable zone target` and 68 `HarvestCarcass: no carcass nearby`) starves the cat.

Real cats nibble continuously — they don't panic-eat. The curve was a "threshold not ramp" by spec design, but the threshold is set so high that the ramp from "starting to want food" to "cliff death" is shorter than any other behavioral cycle in the sim.

## Fix

Single-line change in `src/ai/curves.rs::hangry()`: midpoint `0.75 → 0.5`. Steepness stays at 8. Family stays Logistic. The named-anchor pattern (introduced by the AI substrate refactor specifically to make exactly this kind of swap easy) propagates the change to every consumer:

- `Eat` (`src/ai/dses/eat.rs`)
- `Hunt` (`src/ai/dses/hunt.rs`)
- `Forage` (`src/ai/dses/forage.rs`)
- fox `Hunting` (`src/ai/dses/fox_hunting.rs`)
- fox `Raiding` (`src/ai/dses/fox_raiding.rs`)

All five DSEs use `hangry()` directly via `ScalarConsideration::new("hunger_urgency", hangry())` — no per-call overrides. One edit lands on all five.

## Predicted effects

(Full table in `docs/balance/hangry-recalibration.md`.)

- **Starvation deaths drop sharply** (the direct prediction). Healthy-colony band currently `1.2 ± 1.7`; expect post-recalibration `< 0.5`.
- **`FoodEaten` fires more often** (cats top up at half-hungry).
- **Mating gate satisfied more often** — hunger now oscillates closer to [0.5, 0.7] than [0.3, 0.7]; `breeding_hunger_floor=0.6` is gate-passed more frequently.
- **Hunt / Forage cadence rises** — same anchor, same effect at the predator level.
- **Fox raids become more frequent but less crisis-shaped** — same anchor on fox-side.

## Verification

Re-run the collapse probe head-to-head:

```bash
cargo run --release -- --headless --seed 42 --duration 3600 \
  --log logs/collapse-probe-42-fix-044/narrative.jsonl \
  --event-log logs/collapse-probe-42-fix-044/events.jsonl
just verdict logs/collapse-probe-42-fix-044
just fingerprint logs/collapse-probe-42-fix-044
```

Then a healthy-colony comparison:
```bash
just soak 42  # writes logs/tuned-42/
just verdict logs/tuned-42
```

**Acceptance:**
- `cargo test` passes (curves.rs and eat.rs tests already updated).
- 15-min soak: `deaths_by_cause.Starvation` ≤ 1 (down from 1.2±1.7 mean — most of the time we should hit 0).
- 17-year collapse probe: colony survives, OR if it still dies, the cause shifts away from starvation (e.g. predator attrition without reproduction). Either way, the *Eat-never-wins* signature should be gone — `last_scores` should show Eat in the top 3 at hunger ≤ 0.5.
- Continuity canaries: `courtship` and `MatingOccurred` should fire at least once (currently both at zero in the collapse probe).
- ShadowFoxAmbush stays ≤ 10 in 15-min soak (no expected blast radius here, but verify).

## Out of scope

- Spec doc sweep — `docs/systems/ai-substrate-refactor.md` references `Logistic(8, 0.75)` in 9 places (§2.3 calibration table, retired-constants discussion, JSON snapshot example). Code is source of truth; spec update tracked as a follow-on commit after the verification soak validates the new value.
- Other curve recalibrations (sleep_dep, loneliness, scarcity) — leave alone for now; tackle only if a similar evidentiary pattern emerges for those axes.
- The `if needs.hunger == 0.0` starvation cliff (ticket 032 item #1). Still relevant for edge cases — separate ticket, separate methodology.
- The `breeding_hunger_floor=0.6` gate (ticket 032 item #3). Independent.

## Log

- 2026-04-27: Ticket opened during collapse-probe analysis. Direct evidence: cats with full pantry choose Sleep / Groom / Socialize over Eat at hunger ∈ [0.3, 0.6]; the curve is the cause.
