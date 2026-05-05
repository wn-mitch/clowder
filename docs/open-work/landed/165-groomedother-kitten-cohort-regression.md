---
id: 165
title: Post-d1722a33 GroomedOther affiliative redistribution starves entire kitten cohort on seed-42 (wontfix — bereavement-orphan, working as intended)
status: done
cluster: ai-substrate
added: 2026-05-04
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [mentoring-extraction.md]
landed-at: null
landed-on: 2026-05-04
---

# Post-d1722a33 GroomedOther affiliative redistribution starves entire kitten cohort on seed-42 (wontfix — bereavement-orphan, working as intended)

**Landed:** 2026-05-04 | **Parent:** `2b6b49fb` (post-158) | **Code change:** none

**Verdict — wontfix-by-design.** The diagnosis "GroomedOther stole softmax share from Caretake → adults groomed peers instead of feeding kittens" is **falsified.** The seed-42 starvations are downstream of a colony-collapse cascade that left a single mother (Mocha) responsible for the only surviving kittens; when she was killed by a shadow fox 104+ ticks before the first kitten starved, there was literally no other adult alive to feed them. Bereavement-orphan starvation under terminal-decline conditions is working as intended.

## Investigation evidence

**Death-and-birth timeline (post-158 seed-42, against HEAD `2b6b49fb`):**

| Tick | Event | Cause / parent | Location |
|---|---|---|---|
| 1,208,121 | Calcifer dies | WildlifeCombat | (27, 25) |
| 1,208,208 | Cedar dies | WildlifeCombat | (27, 31) |
| 1,208,247 | Nettle dies | ShadowFoxAmbush | (27, 25) |
| 1,263,706 | Heron dies | ShadowFoxAmbush | (27, 33) |
| 1,268,664 | Wispkit-21 + Emberkit-3 born | mother: Mocha | (41, 22) |
| 1,274,190 | Bramble dies | ShadowFoxAmbush | (32, 19) |
| 1,303,670 | Thymekit-19 born | mother: Mocha | (24, 19) |
| 1,306,429 | Wren dies | ShadowFoxAmbush | (33, 22) |
| 1,306,682 | Simba dies | ShadowFoxAmbush | (33, 22) |
| 1,309,153 | **Mocha dies** | ShadowFoxAmbush | (28, 18) |
| 1,309,257 | Thymekit-19 starves | (104 ticks after Mocha) | (24, 19) |
| 1,309,386 | Wispkit-21 starves | (233 ticks after Mocha) | (41, 22) |
| 1,309,441 | Emberkit-3 starves | (288 ticks after Mocha) | (41, 22) |

All three starved kittens were Mocha's children. By the kitten-starvation tick window the colony was reduced to **Mocha + 3 kittens** (verified via `CatSnapshot` survey: 4 unique living entities in tick range [1,307k, 1,309.7k]).

**Population trajectory** (CatSnapshot count per tick, sampled):

- 1,200,100: 8 (founders)
- 1,210,900: 5 (first cull)
- 1,264,900: 4 (Heron dies)
- 1,272,100: 4 + 2 kittens (twins born)
- 1,304,500: 4 + 3 kittens (Thymekit born)
- 1,308,100: 1 + 3 kittens (Wren and Simba killed at (33,22) within 253 ticks of each other)
- 1,309,153: 0 + 3 kittens (Mocha killed at (28,18))
- 1,309,257–1,309,441: cohort starves

**Adult-cull is geographically clustered.** Of 8 deaths in the run, 7 happened in the (27–33, 18–33) corridor, with 3 separate deaths at (33,22) / (27,25). Looks like a fox-corridor concentration around the colony's hunting paths rather than uniform predation. That dynamic is plausibly **ticket 120**'s territory (`Characterize shadow-fox spawn-rate coupling to cat-presence`).

## Audit-table promotion (post-investigation)

| Layer | Pre-investigation status | Promoted to |
|---|---|---|
| L0 substrate (`KittenCryMap` + `IsParentOfHungryKitten`) | `[verified-correct]` | `[verified-correct]` (164's structural fix intact) |
| L1 markers | `[verified-correct]` | `[verified-correct]` |
| L2 `CaretakeDse` | `[suspect — share comparison]` | `[verified-not-the-cause]` (scenario microexperiment shows Caretake's raw score 0.9026 dominates GroomOther's 0.1892 by ~5×; KittenCryCaretakeLift fires correctly at +0.391) |
| L2 `GroomOtherDse` | `[suspect — share comparison]` | `[verified-not-the-cause]` (same scenario; GroomOther never outscores Caretake at L2) |
| L3 softmax | `[suspect — quantify]` | `[verified-correct]` (softmax is well-behaved; the 6.5× soak ratio claim from `mentoring-extraction.md` Iter 2 is unverified — Mocha created 664 Caretaking plans vs 382 Grooming plans, contradicting the ratio direction) |
| Action → Disposition mapping | `[verified-correct]` | `[verified-correct]` |
| Caretake plan template | `[verified-correct]` | `[verified-correct]` (164's structural fix intact) |
| Cohort-size regression | `[suspect]` | `[downstream-of-bereavement]` (lone mother dies before her cohort can mature; kittens_born 5→3 is downstream of fewer surviving breeding-age adults, not of affiliative redistribution) |

## Falsified hypotheses

- **H1 — softmax-share drift.** The scenario microexperiment showed Caretake's raw score dominates the L2 pool when conditions match. There is no L2 inversion.
- **H2 — Maslow tier-asymmetric suppression.** Caretake (tier 3, SingleMinded) does not get suppressed by tier-2 GroomOther in L2 scoring; the `disposition_failure_cooldown` modifier crushed Caretake in the scenario only because the scenario lacks a `Stores` building, which is a scenario-setup artifact and does not match soak conditions (Mocha had **zero** Caretaking PlanningFailed events in the soak).

## Adjacent findings worth their own tickets

- **`KittenFed = 0` colony-wide in BOTH pre-158 and post-158 soaks** despite Mocha alone creating 664 Caretaking plans. The exemption note at `src/resources/system_activation.rs:639` says "cascade from MatingOccurred (ticket 027)," but ticket 027 landed on 2026-05-01 and `KittenFed` is still silent. **Not opened as a ticket here** because the user confirmed bereavement-orphan starvation is WAI; whether the silent-advance is also WAI or a real defect deserves a discrete answer in its own ticket.
- **Fox-corridor concentration at (27–33, 18–33).** 7 of 8 adult deaths cluster in that corridor, with multiple deaths at the same tile. Plausibly a tail of **ticket 120** (`Characterize shadow-fox spawn-rate coupling to cat-presence`) and/or landed **ticket 161**'s scheduler-perturbation cascade.

## Children unblocked

- **164** (`Seed-42 (38,22) kitten cohort starves despite KittenCryMap`) — its `blocked-by: [165]` clears here. 164's structural fix on `caretake_target` empty-pool fallback + `IsParentOfHungryKitten` author was always intact; the seed-42 hard-gate test it was waiting on is unreachable on this seed because of the bereavement-orphan dynamic, so 164's verification needs a different seed or a hard-gate test that distinguishes systemic-starvation from bereavement-induced starvation.

## Resolution path for the CLAUDE.md hard gate

`deaths_by_cause.Starvation == 0` cannot deterministically pass on seed-42 while bereavement-orphan starvation is WAI. Either:

- The hard gate gets a "bereavement carve-out" (Starvation deaths preceded within N ticks by the kitten's parent's death don't count toward the gate), OR
- The canonical seed picks a run where the lone-mother-death-cascade doesn't fire, OR
- An alloparenting / orphan-adoption mechanism is added (touches landed-but-parked **ticket 015** "Alloparenting Reframe B" and open **ticket 159** "Parent grief consumer") so a surviving non-parent adult adopts orphans before they starve.

These are out of scope for 165's closeout. They are noted here as the open question, not a deferred fix.

## Log

- 2026-05-04: opened. Surfaced during ticket 164's closeout investigation. Re-soak of seed-42 against current HEAD `2b6b49fb` reproduced the post-d1722a33 numbers deterministically.
- 2026-05-04: investigated. Authored `src/scenarios/cohort_starvation_grooming.rs` to triage L2/L3 share. Scenario surfaced an unrelated finding (`disposition_failure_cooldown` from absent `Stores` building) but ruled out the GroomOther-vs-Caretake inversion at L2/L3. `/logq` queries on `logs/tuned-42` revealed Mocha was killed at tick 1,309,153 (ShadowFoxAmbush at (28,18)) — 104 ticks before the first starvation. Verified all three starved kittens were Mocha's children with no surviving alternate caretaker. User confirmed bereavement-orphan starvation under colony-terminal-decline is WAI. Closing as wontfix; scenario removed (it didn't pin a real defect and would mislead future readers); ticket 164's `blocked-by: [165]` cleared in the same commit.
