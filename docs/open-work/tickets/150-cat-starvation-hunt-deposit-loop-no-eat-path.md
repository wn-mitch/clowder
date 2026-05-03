---
id: 150
title: Cat starvation despite active food production — hunt-deposit loop has no eat path
status: in-progress
cluster: ai-substrate
added: 2026-05-03
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

While diagnosing courtship fragility under ticket 148, a focal-cat trace
of `Bramble` (last survivor in `logs/sweep-courtship-fondness-148-quota-wardswap/314-1/`)
revealed a structural hole in the food loop: cats can starve to death while
**actively and successfully producing food for the colony stockpile**.

Bramble's final 5,830 ticks of life (from the second-to-last colony death
at tick 1,288,000 to their starvation death at tick 1,293,830):

| disposition | plans created |
|---|---|
| Hunting   | **385** |
| Foraging  | **221** |
| Building  | 10 |
| Exploring | 2 |
| **Resting** | **2** |

Plans were exclusively production-shaped:

- `Hunting`: `TravelTo(HuntingGround) → SearchPrey → EngagePrey → TravelTo(Stores) → DepositPrey`
- `Foraging`: `TravelTo(ForagingGround) → ForageItem → TravelTo(Stores) → DepositFood`

**Neither plan template contains an `Eat` step.** Bramble caught prey,
foraged berries, walked back to stores, deposited the food, and re-planned
another hunt — never eating. Hunger drained from 0.53 to 0 over those
ticks. The cat starved at the door of a stockpile they had been depositing
into all along.

This is a structural fault, not a balance miss. The user's framing was
correct: this is a zero-sum sim, and "cats doing one thing necessarily
means they aren't doing something else." The thing they aren't doing
is **eating**, even when the food they need is sitting in their own
inventory or in the stockpile they just walked away from.

This problem hides during normal multi-cat operation: with 8 cats, *some*
cat will pick `Resting → EatAtStores` while others hunt, and the colony
muddles through. It only surfaces dramatically when a cat is solo —
either as a last survivor or in a sparse-population edge case. But the
underlying architectural defect is present every tick: every cat is
choosing Hunt/Forage over Eat at moderate hunger levels, and only
luck-of-the-scoring-jitter rescues most of them before they crash.

## Current architecture (load-bearing facts to verify before fixing)

1. **`resolve_engage_prey`** (`src/systems/goap.rs:4985`) puts the catch
   into the cat's inventory (`inventory.slots.push(ItemSlot::Item(...))`)
   and **does not touch `needs.hunger`**. The `needs` parameter is
   currently `&Needs` (read-only).
2. **`resolve_forage_item`** (`src/systems/goap.rs:5248`) — same shape:
   forage yields go to `inventory.slots`; hunger is not consumed.
3. **`resolve_eat_at_stores`** (`src/steps/disposition/eat_at_stores.rs:41`)
   is the **only** step that adds to `needs.hunger`. It pulls a food item
   from `StoredItems`, applies `food_value × freshness × cooked_mult`,
   destroys the item.
4. **`Action::Eat` maps to `DispositionKind::Resting`**
   (`src/components/disposition.rs:94`). The Resting disposition's plan
   contains `EatAtStores → Sleep → SelfGroom`. So a cat must pick Eat at
   the Action-scoring layer (L3), which then commits Resting, which then
   sequences EatAtStores via GOAP.
5. **`EatDse`** (`src/ai/dses/eat.rs`) is gated by the
   `HasStoredFood` colony marker. It scores
   `hunger_urgency × stores_distance` under CompensatedProduct,
   maslow tier 1.
6. **`HuntDse`** is also tier 1, also driven by hunger urgency, but
   composes additional positive multipliers (hunting skill, prey
   proximity). At moderate hunger (≈ 0.5), Hunt typically outscores
   Eat because of those extra multiplicative axes.
7. **Hunger semantic**: `Needs.hunger` is `1.0 = full belly`, `0.0 =
   starving and dying` (`src/systems/needs.rs:116` — `hunger == 0.0`
   triggers starvation damage; `:289` — eating ADDS to hunger up to 1.0;
   `:206` — spawn hunger is 1.0).

## Suspected root cause(s)

The investigation needs to confirm which of these is load-bearing
(possibly multiple):

### (R1) Hunt/Forage plans omit a self-feed step

Real cats eat their kill on the spot when hungry, then bring leftovers
home. The current GOAP templates skip the eat-the-catch beat entirely.
A hungry cat who succeeds at `EngagePrey` walks past every opportunity
to consume the prey they're carrying.

**Fix shape:** `resolve_engage_prey` and `resolve_forage_item` accept
`&mut Needs` and, on a successful catch/forage, branch on hunger:
hungry cats consume the catch (apply `food_value`-based hunger gain,
do not push to inventory); satiated cats deposit as today. Threshold
candidate: `hunger < ~0.5`.

### (R2) `eat_goal_achieved` predicate has inverted polarity — but is dead code

`src/ai/dses/eat.rs:171` returns `hunger < 0.3 → true`. By every other
semantic in the codebase, `hunger=1.0` is sated and `hunger=0.0` is
starving (`src/systems/needs.rs:116, :289`; spawn at 1.0 in
`physical.rs:206`). So this predicate returns true when the cat is
*dangerously hungry*, not when sated. The doc comment at line 165
("Goal predicate: hunger has dropped below the satiation threshold")
and `HUNGER_GOAL_THRESHOLD`'s comment at line 65 ("Below this, the
cat is sated") are also inverted — they were written against the
wrong semantic.

**Status: confirmed dead code.** `eat_goal_achieved` is the only DSE
in the codebase with a non-`|_, _| false` `achieved` field
(2026-05-03 grep). Phase 3a's promised consumer ("§7.2's reconsideration
gate") was never wired — runtime grep finds **zero call sites** that
invoke `state.achieved(world, cat)`. The only `Intention::Goal { state,
.. }` destructures live in `#[test]` blocks and `trace_emit.rs:411`,
which reads `state.label` only. So the polarity bug is latent: harmless
today, dangerous if anyone ever wires §7.2 up against the current
predicate body.

**Disposition:** R2 is **not** Bramble's load-bearing root cause. Pivot
to R3/R1. Fix the polarity + comment as a hygiene cleanup separately —
either in this ticket's eventual landing PR or as a tiny standalone fix.
The fix is a one-liner: `needs.hunger > HUNGER_GOAL_THRESHOLD` with
the threshold re-anchored upward (~0.7 means "above this you're full
enough to stop").

### (R3) Eat scores too low against Hunt at moderate hunger

Even with `HasStoredFood` set, Eat's CompensatedProduct (2 axes:
hunger_urgency × stores_distance) loses to Hunt's CompensatedProduct
(typically 3+ axes including positive skill/density bonuses).

**Investigation:** dump action-scoring for a focal cat at hunger ≈ 0.5
in seed-314 (or any seed with the failure). Compare Eat vs Hunt vs
Forage scores tick-by-tick. Determine whether Eat is *almost* winning
or *never* winning. If never: Eat needs an additional boost axis (e.g.,
"already-near-stores", or a stockpile-depth multiplier).

### (R4) `Carrying-food → switch-to-Eat` interrupt missing

A cat with food in inventory and rising hunger should interrupt the
current production plan and consume what they're carrying. The anxiety-
interrupt system (`src/systems/anxiety_interrupts.rs` or similar) might
be the right place for a `HungerInterrupt` that fires when
`Carrying::Prey | Carrying::ForagedFood` AND `hunger < threshold`.

**Investigation:** find the existing interrupt plumbing and see if
hunger-while-carrying is already a registered interrupt class.

## Investigation steps

Each of the following should be a discrete artifact in this ticket's
log before any code changes are proposed.

1. **Confirm Bramble's failure mode is structural, not seed-specific.**
   Re-trace seeds 7 and 99 (which had near-deaths or starvations under
   different configs) — do their dying cats also show all-Hunting/Foraging
   plans and no Resting plans? Use `just q trace` or the focal-cat
   subagent.

2. **Verify `eat_goal_achieved` polarity by reading GOAP planner.**
   Find where `GoalState::achieved` is consulted. If `true` short-
   circuits, the body is inverted. Quote the exact lines.

3. **Score-dump Eat vs Hunt at multiple hunger levels.** Add a temporary
   `ActionChosen`-style scoring instrumentation OR write a focal-cat
   trace and read the L2 DSE scores. Goal: determine the hunger value
   at which Eat finally outscores Hunt.

4. **Check if `HasStoredFood` is correctly set during Bramble's life.**
   Is the marker authored every tick the colony has food? Or does it
   flicker (set after deposit, cleared as food gets eaten)? If it's
   flickering with millisecond windows, Eat may have no opportunity
   to score even when it would win.

5. **Survey other cats' production-vs-consumption ratios.** In the
   diagnostic 5-seed sweep (`logs/sweep-courtship-fondness-diag/`),
   count `EngagePrey + ForageItem` successes vs `EatAtStores` successes
   per cat. The ratio should be roughly 1:N where N = food_value /
   hunger_drain_per_day. Departures from this ratio on the wrong side
   indicate cats over-producing relative to consumption (the same
   defect as Bramble, just survived through luck).

6. **Audit Maslow-tier coherence for production actions.** The current
   Hunt/Forage tier (1, physiological) puts them in the same suppression
   class as Eat. The user's question — "why isn't Eat winning?" —
   reduces to "why does Hunt's score stay above Eat's?" Confirm the
   hangry curve and per-DSE composition exhibit the right shape.

## Proposed fix order (after investigation lands)

If the investigation confirms (R1) + (R2) are both real:

1. **Fix (R2) first** (`eat_goal_achieved` polarity). Lowest-risk
   correctness fix. If the predicate has been short-circuiting Eat
   plans, this single change may resolve most of the issue.
2. **Re-measure the diagnostic sweep** with R2 fixed. If cats now
   pick Resting at meaningful rates, R1/R3/R4 may not be needed.
3. **If starvation still happens**, ship (R1) — eat-the-catch in
   `resolve_engage_prey` / `resolve_forage_item`. Structural,
   biologically correct, narrow blast radius.
4. **(R3) and (R4) only if needed.**

## Out of scope

- Any change to courtship/bond/mating mechanics (ticket 148's domain).
- Any change to ward labor / magic (ticket 148 ward-swap).
- Any tuning of `socialize_fondness_per_tick`, `fondness_grooming_floor`,
  or related social-rate constants.
- Founder-distribution changes (ticket 148's quota).

## Log

- 2026-05-03: Opened from ticket 148's diagnostic-sweep investigation.
  Bramble's focal trace at `logs/sweep-courtship-fondness-148-quota-wardswap/314-1/`
  is the reference evidence (deaths_by_cause includes 1 Starvation; cat
  was last survivor for 5,830 ticks with 385 Hunting + 221 Foraging +
  2 Resting plans). Investigation paused before any code changes — full
  audit per the steps above before drafting the fix.
- 2026-05-03: Plan revised to surface **R5** as a co-equal root cause
  after the user flagged that `Action::Eat` mapping to
  `DispositionKind::Resting` is itself part of the defect. The L3
  softmax sees Eat and Hunt as peer Actions but doesn't see that
  picking Eat commits the cat to a multi-need Resting plan
  (Sleep + SelfGroom too) while picking Hunt commits to a
  catch+deposit cycle that terminates much sooner. R5 splits Eat into
  its own DispositionKind so the L3 layer's plan-duration cost is
  symmetric with Hunt's. Plan and decision tree updated
  (`/Users/will.mitchell/.claude/plans/working-dir-users-will-mitchell-clowder-tidy-hopcroft.md`).
- 2026-05-03: Sequence 1 landed — R1 (eat-the-catch in
  `resolve_engage_prey` and `resolve_forage_item`, gated on the new
  `production_self_eat_threshold` SimConstants knob, default 0.5) +
  R5a (new `DispositionKind::Eating` with single-action plan template
  `[TravelTo(Stores), EatAtStores]`, hunger-only completion proxy,
  Blind strategy, Maslow tier 1; Resting drops EatAtStores from its
  plan template and hunger from its completion gate) + R2 (polarity
  fix on the dead-code `eat_goal_achieved` predicate, comment hygiene
  on `cook_hunger_gate`). 1806 lib tests / 20 integration tests pass;
  `just check` clean. Polarity sweep ran on every `needs.hunger`
  comparison in `src/`; no other inversions. Follow-on tickets 151
  (bugfix-discipline doctrine in CLAUDE.md) and 152 (tier-1
  disposition-collapse audit) opened as 150-landing siblings per the
  antipattern-migration discipline.
- 2026-05-03: Empty-plan guard added — when `make_plan` returns an
  empty Vec (start state already meets the goal — common for
  Eating-when-not-hungry because hunger>resting_complete_hunger
  satisfies HungerOk(true) at planning time), `goap.rs:1779` now
  `continue`s instead of committing to a 0-step plan. Without the
  guard, cats picked Eating ~34,570× per soak with empty plans,
  thrashing the L3 layer and starving longer-form activities of
  their tick budgets. Post-guard count drops to ~193 — the genuinely-
  hungry cases.
- 2026-05-03: **Balance regression observed; balance follow-on opened.**
  Clean seed-42 deep-soak passes the load-bearing gates
  (`deaths_starvation=0`, ShadowFoxAmbush 6/8, footer written), but
  shows a behavioral-mix shift: cats over-allocate to Hunting/Foraging/
  Exploring at the expense of Resting (441 plans vs ~1900 in
  baseline). Less Resting → less ambient fondness accumulation →
  `continuity_tallies.courtship` collapses 999 → 0; never-fired list
  gains `CourtshipInteraction` and `PairingIntentionEmitted`. Pre-
  existing baseline failures (`mentoring=0`, `burial=0`) are
  unaffected. Aggregate colony_score -17%. The structural fix is
  correct (load-bearing gate passes); the L3 score-mass needs
  rebalancing for Resting-class. Opened ticket 153 to track the
  balance thread.
