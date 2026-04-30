---
id: 2026-04-25
title: Respect restoration — iteration 2 (witness magnitude tune)
status: done
cluster: null
landed-at: da51270
landed-on: 2026-04-25
---

# Respect restoration — iteration 2 (witness magnitude tune)

**Landed:** 2026-04-25 | **Balance:** `docs/balance/respect-restoration.md` iter 2

**Diagnosis.** Iter 1 of respect-restoration shipped a witness-multiplier
on plan completion at `respect_per_witness = 0.005` against a 0.5–0.7
prediction band. Post-relocation seed-42 soak (`logs/tuned-42-iter2/`,
commit `da51270`) measured **mean = 0.998, =0% = 0.0%** — direction
match but magnitude 3.5× off, with respect pinned at the `.min(1.0)`
ceiling and seed-conditional dynamics gone (Thistle seed at
0.997 too). Per CLAUDE.md balance methodology, magnitude > 2× off
requires an iter-2 magnitude correction before acceptance.

**Fix.** `default_respect_per_witness` (`src/resources/sim_constants.rs:~2692`)
**0.005 → 0.0001** (50× cut).

The handoff predicted a 3.3× cut would suffice, but bisection through
the saturation cliff was much steeper than that:

| `respect_per_witness` | Mean | sd | =0% |
|---|---|---|---|
| 0.005 (iter 1) | 0.998 | — | 0% |
| 0.0015 (3.3× cut) | 0.996 | 0.032 | 0% |
| 0.0003 (16× cut) | 0.982 | 0.060 | 0% |
| **0.0001 (50× cut, landed)** | **0.566** | **0.463** | **18.3%** |

The cliff lives between 0.0003 and 0.0001. Above it, disposition
baselines (`respect_gain_*` = 0.01–0.15 per chain) plus any nonzero
witness contribution stay above drain and `.min(1.0)` clips the gain
rate, so 16× cuts look identical at the colony-mean level. Below the
cliff, equilibrium falls naturally and the colony spreads.

**Hypothesis.** Per-completion gain at the typical witness count must
fall below the drain accumulated between completions, or `.min(1.0)`
saturates. Iter-1 had gain >> drain at saturation; iter 2 finds the
magnitude where gain ≈ drain at the desired equilibrium.

**Prediction.** Mean ∈ 0.5–0.7, zero% < 20%, survival canaries
unchanged.

**Observation** (seed-42 15-min release soak, `logs/tuned-42-iter2-batch3/`):

- Respect mean = **0.566** (in band ✓)
- Respect zero% = **18.3%** (under 20% target ✓)
- Respect sd = **0.463** (wide variation; not flat ✓)
- Survival canaries: starvation = 0, ambush = 0, footer written ✓
- Per-cat distribution is **bimodal-by-role** — colony-centre cats
  (Birch, Mocha, Nettle, Ivy) saturate at ~0.98; solitary hunters
  (Simba, Mallow, Lark, Calcifer) sit at 0.08–0.19. Matches iter-1's
  design intent ("isolation gets only the baseline"). Saturation at 1.0
  for social cats is cosmetically unsatisfying — iter-3 candidate is a
  logistic update (`+= (1 - respect) × …`) replacing the additive-with-
  clamp pattern.
- Pre-existing failures unchanged: 10 never-fired-expected positives
  (BondFormed, Socialized, GroomedOther, MentoredCat, …) and 5/6 zero
  continuity tallies. Out of scope per
  `docs/balance/acceptance-restoration.md` iter-2 deferral.

**Concordance.** Direction match, magnitude inside the strict band.
**Accept.**

**Files:** `src/resources/sim_constants.rs` (1-line magnitude),
`docs/balance/respect-restoration.md` (backfill iter-1 observation +
concordance, append iter 2),
`docs/open-work/landed/2026-04.md` (this entry).

---
