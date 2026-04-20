# eat_from_inventory_threshold — restore on-the-fly feeding

**Status:** landed; concordance mostly direction-correct, magnitudes much
smaller than predicted because the baseline was healthier than the
premise assumed. Two larger issues surfaced as follow-on plans.

## Context

Auditing the food economy surfaced a constant that contradicted its own
function's docstring:

- `src/systems/needs.rs::eat_from_inventory` exists to **"keep cats alive
  during long hunts"** — a supply-chain shortener that lets cats consume a
  fresh kill in-transit rather than carrying it all the way back to stores.
- The gating constant `eat_from_inventory_threshold` was set to `0.05`, which
  only triggers consumption at near-death (5% hunger). That's post-hoc
  starvation relief, not on-the-fly feeding.
- The 2026-04-11 tuning memory had this value at `0.4`. It was lowered during
  the 2026-04-16 Stores-construction work (which wanted cats depositing, not
  snacking). The docstring wasn't updated to match — the tell that the
  revert was an overcorrection.

## Hypothesis

> Real predators eat fresh kills when hungry; they don't cache a catch and
> grow hungrier while walking home. Current `eat_from_inventory_threshold: 0.05`
> models the latter. Raising to `0.4` lets cats consume catches when
> moderately hungry — a correction that aligns with the function's own
> stated purpose and with predator ecology.

## Implementation

One line. `src/resources/sim_constants.rs:128`:

```diff
- eat_from_inventory_threshold: 0.05,
+ eat_from_inventory_threshold: 0.4,
```

Constants diff between baseline and treatment headers confirms exactly one
field changed.

## Prediction

Per `docs/balance/eat-inventory-threshold.predictions.json`. Summary:

| Metric | Direction | Magnitude |
| --- | --- | --- |
| Mean cat hunger | up | +20% |
| Below-0.5 hunger fraction | down | −50% |
| Leisure action count | up | ≥3× (≥200%) |
| Stores food level mean | down | −30% |
| Starvation deaths | flat (bar: ≤ baseline) | 0 |
| Catches per cat per day | down | −15% |

## Observation

Both runs: seed 42, `--duration 900`, release build, commit 8d8fb85 (dirty
working copy for both — internally comparable per methodology). Constants
diff shows exactly `needs.eat_from_inventory_threshold: 0.05 → 0.4` and
nothing else.

| Metric | Baseline (0.05) | Treatment (0.4) | Δ |
| --- | ---: | ---: | ---: |
| `eat_from_inventory_threshold` | 0.05 | 0.40 | +0.35 |
| CatSnapshot samples | 9,040 | 10,500 | +16% (more sim ran) |
| **Hunger mean** | 0.630 | 0.632 | **+0.3%** |
| Hunger stddev | 0.134 | 0.128 | −4.6% |
| **Below-0.5 fraction** | 7.62% | 7.48% | **−1.9%** |
| **Below-0.3 fraction** | 1.06% | 0.50% | **−53%** |
| **Stores food mean** | 0.850 | 0.922 | **+8.5%** (wrong direction) |
| **Prey catches total** | 235 | 219 | −6.8% |
| Hunt plans created | 2,045 | 2,288 | +11.9% |
| Search timeouts | 10 | 9 | −10% |
| **Leisure action time** | 6,008 | 7,088 | **+18%** |
| Survival action time | 2,301 | 2,810 | +22% |
| Work action time | 731 | 602 | −18% |
| **Starvation deaths** | 2 | 1 | **−50%** |
| Shadowfox ambush deaths | 0 | 0 | 0 |

### Catches per week trajectory (findability diagnostic)

| Relative week | Baseline | Treatment |
| ---: | ---: | ---: |
| 0 | 66 | 66 |
| 1 | 22 | 22 |
| 2 | 9 | 9 |
| 3 | 18 | 18 |
| 4 | 15 | 8 |
| 5 | 13 | 9 |
| 6 | 7 | 10 |
| 7 | 11 | 3 |
| 8 | 7 | 2 |
| 9 | 7 | 7 |
| 10 | 1 | 9 |
| 11 | 15 | 15 |
| 12 | 10 | 10 |
| 13 | 10 | 4 |
| 14 | 10 | 5 |
| 15 | 9 | 4 |
| 16 | 5 | 4 |
| 17 | — | 7 |
| 18 | — | 7 |

Both runs show the week-0 boom (66 catches) followed by a ~5–22-per-week
settling range with occasional dips to 1–3. Treatment ran ~2 more sim-weeks
because the colony was marginally healthier. The findability hypothesis is
**partially implicated but not decisive**: catches do decline after the
local boom but don't flatline, and the primary hunt failure mode is "lost
prey during approach" (1,366 baseline / 1,774 treatment), not "no scent
found" (10 / 9). Cats locate prey fine; they lose it during stalk/approach.

### Action distribution (from CatSnapshot.current_action)

| Action | Baseline | Treatment | Δ |
| --- | ---: | ---: | ---: |
| Explore | 4,024 | 4,942 | +23% |
| Socialize | 1,704 | 1,895 | +11% |
| Forage | 1,084 | 1,456 | +34% |
| PracticeMagic | 615 | 520 | −15% |
| Hunt | 556 | 622 | +12% |
| Eat | 358 | 380 | +6% |
| Sleep | 303 | 352 | +16% |
| Idle | 143 | 99 | −31% |
| Herbcraft | 82 | 79 | −4% |
| Wander | 76 | 79 | +4% |
| **Groom** | 33 | 46 | +39% |
| Coordinate | 24 | 27 | +13% |
| **Mentor** | 0 | 0 | 0 |
| **Caretake** | 0 | 0 | 0 |
| **Mate** | 4 | 0 | — |
| **Cook** | 0 | 0 | 0 |
| Build | 3 | 3 | 0 |
| Patrol | 31 | 0 | — |
| Fight | 0 | 0 | 0 |
| Flee | 0 | 0 | 0 |

Mentor / Caretake / Cook remain at zero. Groom moved 33 → 46 — noticeable
in relative terms (+39%) but still trace-level. Mate went 4 → 0 and Patrol
went 31 → 0 — within sample noise given tiny absolute counts.

## Concordance

| Metric | Prediction | Observed | Verdict |
| --- | --- | --- | --- |
| Hunger mean | up, +20% | +0.3% | **direction ✓, magnitude rejected** — baseline was already at 0.63, not the 0.55-ish I assumed |
| Below-0.5 fraction | down, −50% | −1.9% | **direction ✓, magnitude rejected** — baseline was 7.6%, not the 40–50% the auditor's narrative implied |
| Below-0.3 fraction | (unnamed prediction) | −53% | **strong unintended win** — the deeply-hungry tail really did halve |
| Leisure action count | up, +200% | +18% | **direction ✓, magnitude rejected** — baseline leisure was already 66% of all action-time |
| Stores food mean | down, −30% | **+8.5%** | **direction wrong** — cats eat inventory → fewer store-trip withdrawals → stores accumulate instead of drain. Second-order effect I didn't model. |
| Catches/day | down, −15% | −6.8% | direction ✓, magnitude within band |
| Starvation | ≤ baseline | 2 → 1 | **canary pass per plan** (absolute canary in `check_canaries.sh` wants 0 and fails; see Canaries section) |
| Shadowfox ambush | flat, ±20% | 0 → 0 | pass |
| Wipeout | flat | colony survives; treatment ran 2 more weeks | pass |
| Findability diagnostic | (measurement only) | catches ~66→{3–15}/week, "lost prey during approach" dominates failure modes | **findability indicated but diagnosis refined** — cats locate prey (10 timeouts), lose it during approach (1,774 failures) |

## Canaries

Run: `just check-canaries logs/tuned-42/events.jsonl`:

```
[FAIL] starvation_deaths                1 (target == 0)
[pass] shadowfox_ambush_deaths          0 (target <= 5)
[pass] footer_written                   1 (target >= 1)
[pass] features_at_zero                 0 (target informational)
```

Starvation canary's absolute threshold fails with 1 death. However:

- **Baseline already failed** with 2 deaths under the same absolute threshold.
- This plan's explicit success criterion (per the plan file) is `starvation ≤
  baseline`, not `starvation == 0`. Treatment halves starvation (2 → 1),
  which satisfies this plan's criterion.
- The absolute 0 canary reflects a pre-existing colony-level issue that this
  plan doesn't claim to fix. A separate plan is required for the remaining
  starvation (likely linked to the `Explore`-dominance or approach-failure
  issues described below).

`just ci` passes (no test changes; constant value only).

## Overall verdict

**Accept and ship.** Direction matches across the major metrics, starvation
halves, stores stay positive (actually improve), and the specific "deeply
hungry tail" (below-0.3 fraction) is cut in half. The magnitude misses are
because the baseline was healthier than the motivating narrative implied,
not because the mechanism is wrong. The docstring/constant inconsistency
is resolved.

The small absolute movement on aggregate hunger (+0.3%) reflects that only
a minority of cats were ever hitting the inventory-eating gate at all —
precisely the starving-tail cats. Those cats are meaningfully better off;
the healthy majority's behavior is unchanged.

## Follow-on work surfaced

1. **`Explore`-dominance over targeted leisure.** In both runs, `Explore`
   accounts for ~45% of action-time while `Groom` is ~0.5%, and `Mentor` /
   `Caretake` / `Cook` sit at exactly 0. The user's observation that
   "narrative leisure isn't happening" is real — it's just that it's being
   out-competed by Explore, not crushed by survival-floor suppression.
   This is a scoring/weighting issue, probably in `src/ai/scoring.rs`
   (Explore's base score and/or curiosity drive). Own plan.
2. **Approach-pipeline hunt failures.** 1,774 "lost prey during approach"
   failures vs. 9 "no scent found" — cats locate prey via scent fine, then
   lose it during stalk/approach. Candidate levers: stalk speed, approach
   speed under stealth, prey detection-of-cat during approach phase.
   Refines the findability hypothesis from "can't find" to "can't close."
   Own plan.
3. **Remaining single starvation death.** One cat still starves in the
   15-minute soak. Likely downstream of #1 (spends time exploring instead
   of eating) and/or #2 (hunts but can't convert). Not an independent issue
   — follows from the other two.
4. **Mentoring snapshot bug** (`src/steps/disposition/mentor_cat.rs:23–25`).
   Mentor fires 0 times in both runs, but when it *does* fire the
   snapshot of the mentor's skills is never applied to the apprentice —
   meaning even if we lift Mentor's scoring, the action teaches nothing.
   Orthogonal to this plan but primes #1's fix to actually have an effect.
5. **Magic scoring gate** (`src/ai/scoring.rs:483`). 60% of cats can't
   attempt magic because of the affinity/skill threshold. `PracticeMagic`
   fires 520–615× in the soak entirely from the qualified minority. Own
   plan.

## Baseline reference

Pre-change baseline preserved at `logs/tuned-42-baseline-eat-threshold/`.
Treatment run at `logs/tuned-42/`.

## Lessons for the auditor

The pre-soak auditor estimated baseline `below-0.5 fraction` at 40–50%;
actual is 7.6%. That was the wrong-premise that made every magnitude
prediction here too aggressive. The auditor read the compression stack
correctly as code *topology* but overestimated how often it was triggered
at current tuning. The user's meta-correction ("tune the supply, not the
sensor") was right on principle; the magnitude-of-problem wasn't as large
as I'd feared.

Future economy-audit passes should sample CatSnapshot hunger distributions
from a real soak before guessing at below-threshold fractions from
aggregate catch numbers.
