---
id: 203
title: CriticalHealth interrupt drives hunt-to-starvation plan churn — concrete reproducer for ticket 119
status: done
cluster: ai-substrate
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, needs.md]
related-balance: [047-acute-health-adrenaline.md]
landed-at: pending
landed-on: 2026-05-11
---

<!--
Verification / reproducer ticket. The structural fix lives in 119 (retire
the CriticalHealth interrupt); 119 itself is blocked on 118 (modifier-lift
vs plan-completion momentum). This ticket exists so the post-119 soak
has a concrete, named pass criterion to check against, separate from
119's own implementation iteration log.
-->

## Why

Surfaced by the post-ticket-193 canonical seed-42 soak (`logs/tuned-42`,
commit `6273c669`). The 193 fix closed the
`PickingUp:GoalUnreachable` cascade (3302 → 0); the colony now survives
2.3× longer (24,145 → 55,671 ticks) and nourishment recovers from 0.22
to 0.92 (back above the 0.82 healthy baseline). But two adult founders
— **Cedar** (death tick 1,240,147) and **Heron** (death tick 1,246,625)
— starve under a different mechanism that was previously masked by 193's
earlier collapse. The mechanism is the **CriticalHealth interrupt
treadmill** that ticket 119 already documents structurally; the post-193
run provides the first clean production reproducer because the colony
finally lives long enough for slow-onset starvation to surface behind
the (now closed) plan-creation cascade.

**Diagnostic data (Cedar, the cleanest exemplar):**

| Tick | hunger | health | current_action |
|---:|---:|---:|---|
| 1,230,000 | 0.995 | 0.277 | Forage |
| 1,239,000 | 0.095 | 0.097 | **Hunt** |
| 1,239,100 | 0.085 | 0.097 | **Hunt** |
| 1,239,200 | 0.075 | 0.097 | **Hunt** |
| 1,240,147 | 0.000 | — | **Death (Starvation)** |

Health pinned at ≈0.097 (right at `CriticalHealth` threshold) for the
entire ~10kt decay window. Action distribution in the final 6kt: 52%
Hunt, 35% Forage, 8% PickUp, 4% Wander, 2% Sleep — **0% Eat**. Cedar
created **9,760 plans / 10,000 ticks (1.04 ticks per plan)**, of which
8,711 were `PlanInterrupted`. Heron shows the identical pattern at a
different tick offset.

Colony-wide: `interrupts_by_reason.CriticalHealth = 29,089` (sole
non-trivial interrupt reason in the run).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Substrate (modifier) | `src/ai/modifiers/acute_health_adrenaline.rs` | `AcuteHealthAdrenalineFlee` lifts Sleep score from ≈0.30 to ≈0.87 when health is critical (047). | `[verified-correct]` (per 047 phase-2 trace) |
| L3 softmax | `src/ai/scoring.rs` | Sleep wins L3 softmax in 99.3% of injured-window ticks once the modifier is composed. | `[verified-correct]` (per 047 phase-2 trace) |
| Plan-completion momentum | `src/components/commitment.rs` + `src/systems/goap.rs` | Mid-plan Hunt/Forage steps complete naturally before next softmax fires; cat re-engages same DSE class because Hunt/Forage scores haven't dropped. **Behavioral expression of the substrate is gated by this gap (118).** | `[verified-defect]` (118 documents this) |
| Override (legacy) | `src/systems/disposition.rs:301-302` + `src/systems/goap.rs:493-498::check_anxiety_interrupts` | `CriticalHealth` interrupt fires every tick the cat is below threshold and tries to plan; force-Flee path was doing 64% of life-saving in 047 verification. With 193's PickingUp cascade closed, the override now expresses as the dominant tick-by-tick replan signal (29,089 interrupts in 55kt across 8 cats). | `[verified-defect]` (this is what 119 retires) |
| Eligibility | `EatAtStores` requires `HasStoredFood::KEY` colony marker | `HasStoredFood = false` in famine windows (no produced food has reached Stores yet). Eat is rejected at the eligibility filter, so the only food-recovery loop is Hunt → DepositPrey → Cook → Eat. | `[verified-correct]` (091/092 wiring) |
| Resolver | `src/steps/disposition/eat_at_stores.rs` etc. | When given a target and a non-empty store, Eat resolves cleanly. The resolver is not the problem — the cat never reaches it. | `[verified-correct]` |

## Fix candidates

**This ticket does NOT carry a fresh fix candidate slate.** 119 is the
structural fix (retire the `CriticalHealth` interrupt); 118 is its
prerequisite (close the modifier-lift vs plan-completion momentum gap so
the substrate's Sleep lift actually expresses as the chosen action).
Re-litigating the structural slate here would just duplicate 119's
"## Why" and "## Scope" sections and risk drift between two files.

The brief reasonable-alternative audit is:

- **R1 (parameter, rejected)** — lower the `CriticalHealth` threshold so
  fewer cats hit the override. Doesn't fix the treadmill — moves it; cats
  starving from an unrecoverable hunger arc still do, they just hit
  it later.
- **R2 (parameter, rejected)** — give `EatAtStores` a fallback path that
  consults `Carrying` (eat from inventory if a cat is carrying prey).
  Reasonable on its own merits but **does not** address the underlying
  treadmill: Cedar's plan churn was 1.04 ticks/plan; the cat never holds
  a prey item long enough for an inventory-Eat fallback to fire either.
- **Structural (winning, lives in 119)** — retire the interrupt
  override; let the `AcuteHealthAdrenalineFlee` modifier's Sleep lift
  drive recovery via the substrate, once 118 closes the momentum gap so
  the modifier's score lift actually manifests as the chosen action.

## Recommended direction

Track 118 → 119 landing in the existing tickets. **This ticket's job is
the verification artifact**, not a competing fix.

## Out of scope

- The structural fix itself (lives in 119; 119 blocks on 118).
- The mating-loop / `kittens_born=0` issue (separate; 188 wave / 192
  follow-on).
- The post-193 `Guarding:GoalUnreachable=810` spike (separate
  investigation — wards/patrol substrate, possibly its own ticket).
- Tuning the `CriticalHealth` threshold itself (parameter-level
  workaround — 119's structural retirement supersedes it).

## Verification

Pass criteria when 119 lands. Re-run `just soak 42` post-119 and verify:

- `interrupts_by_reason.CriticalHealth = 0` in `events.jsonl` footer
  (true zero per 119's own gate).
- Cedar / Heron equivalent founders complete a full Hunt → Cook → Eat
  cycle without 9,000+ plan-create cadence in any 10kt window.
  (`just q cat-timeline logs/tuned-42 <name> --tick-range=N..M --summarize`
  auto-flags plan-churn under 5 ticks/plan; the post-119 soak should
  not flag.)
- `deaths_by_cause.Starvation` ≤ 1 in the canonical 15-min seed-42 soak
  (matches the pre-185 baseline; 0 is the hard target but seed-42 has
  known endemic starvation noise).
- The action distribution for any cat that dipped below the
  `CriticalHealth` threshold during the run includes ≥ 1 `Eat`,
  measured by `just q actions logs/tuned-42 --cat=<name>`. (Today
  Cedar's distribution in the dying window was 0% Eat across 52
  CatSnapshot rows — that's the regression signal.)

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-08 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **175** (done, ai-substrate, score 0.90) — GoalUnreachable plan-failure root-cause investigation (172 follow-on)
- ✓ landed **106** (done, ai-substrate, score 0.90) — HungerUrgency modifier — substrate axis for Starvation interrupt retirement
- ✓ landed **184** (done, ai-substrate, score 0.89) — Hunt kill→stockpile pipeline regressed under L3 bandwidth pressure (root cause…

<!-- linkages:end -->
## Log

- 2026-05-06: opened. Post-193 soak surfaced the 119 pattern as a
  concrete production reproducer (Cedar & Heron starvation with 1.04
  ticks/plan churn, hunger 0.99 → 0 over 10kt with 0% Eat actions,
  action distribution dominated by Hunt). 193's PickingUp cascade had
  previously masked this by killing cats via injury before slow-onset
  starvation could surface. Blocked on 119 (which itself blocks on 118).
- 2026-05-11: verified against logs/tuned-42 (commit 81e555db). Footer interrupts_by_reason_top=[], anxiety_interrupt_total=0 (the 29,089 CriticalHealth treadmill is gone); deaths_by_cause.Starvation=1 (PASS, target ≤1); no cat at 9k+ plans/10kt. The lone Starvation (Maplekit-92 @ tick 1,299,298, 210 total events across 20,898 ticks — no plan-churn signature) is a separate kitten-care mechanism owned by ticket 273, not a 203 regression.
