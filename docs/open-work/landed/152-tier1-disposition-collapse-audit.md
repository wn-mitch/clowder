---
id: 152
title: Tier-1 disposition-collapse audit — sweep for sibling Eat-into-Resting defects
status: done
cluster: ai-substrate
added: 2026-05-03
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 4021f300
landed-on: 2026-05-03
---

## Why

Ticket 150's R5a fix split `Action::Eat` out of
`DispositionKind::Resting` because picking Eat at the L3 softmax was
implicitly committing hungry cats to a multi-need Sleep + SelfGroom
chain — a plan-duration cost asymmetry the softmax couldn't see.

That defect-shape is potentially a class. Each entry in
`disposition.rs::from_action` and `constituent_actions` is a
many-to-one collapse where multiple Actions are bundled under a
single Disposition with a multi-step plan template. When a cat picks
any one of those Actions in the softmax, they commit to the entire
Disposition's plan — including the sibling Actions whose drives the
cat may not actually have.

This ticket sweeps every entry in that mapping and asks, per cluster:
*does picking Action X here drag the cat into siblings the L3
softmax never saw?* For each suspect cluster, decide
**split / extend / rebind / verified-correct** (per the structural-
candidate menu codified in ticket 151).

## Candidates to audit

From `src/components/disposition.rs::constituent_actions` (post-150):

| Disposition | Constituents | Plan template shape | Suspect? |
|---|---|---|---|
| Resting | `[Sleep, Groom]` | Sleep + SelfGroom | low — both address energy/temp; plan-duration symmetric |
| Eating | `[Eat]` | TravelTo(Stores) + EatAtStores | none — split lands in 150 |
| Hunting | `[Hunt]` | TravelTo + Search + Engage + Travel + Deposit | single-action; OK |
| Foraging | `[Forage]` | TravelTo + ForageItem + Travel + Deposit | single-action; OK |
| Guarding | `[Patrol, Fight]` | PatrolArea / EngageThreat / Survey | **suspect** — Patrol and Fight have very different tempo and risk profiles; picking Patrol shouldn't commit to Fight |
| Socializing | `[Socialize, Groom, Mentor]` | SocializeWith / GroomOther / MentorCat | **suspect** — Socialize is brief, Mentor is long-form skill transfer; cost asymmetry |
| Building | `[Build]` | Gather → Deliver → Construct | single-action; OK |
| Farming | `[Farm]` | Tend → Harvest | single-action; OK |
| Crafting | `[Herbcraft, PracticeMagic, Cook]` | Per `CraftingHint` sub-mode | **highly suspect** — Crafting bundles three completely different activities (herbalism, magic, cooking) under one Disposition. The `CraftingHint` mechanism is a workaround for not splitting them; the user has already flagged this informally. |
| Coordinating | `[Coordinate]` | DeliverDirective | single-action; OK |
| Exploring | `[Explore, Wander]` | ExploreSurvey | low — Wander is the slow variant of Explore; minimal asymmetry |
| Mating | `[Mate]` | TravelTo(SocialTarget) + MateWith | single-action; OK |
| Caretaking | `[Caretake]` | Retrieve + Feed | single-action; OK |

## Investigation steps

1. **Plan-duration audit on suspect clusters.** Use `/logq` with the
   `cat-timeline` and `events` subtools on existing healthy soaks
   (e.g., `logs/tuned-42-baseline-0783194/`) to measure typical
   tick-duration of each constituent Action's plan within its parent
   Disposition. Asymmetry > 2× across siblings is the structural
   defect signature.

2. **L3 score vs plan-cost survey.** For each suspect cluster, sample
   the L2 DSE scores via a focal-cat trace and compare against the
   actual plan duration the cat ends up paying. If the score says
   "Patrol > Fight" but picking Patrol commits the cat to a Fight too
   when threats appear, the L3 layer is operating on incomplete cost
   information.

3. **Structural-candidate proposal per suspect cluster.**
   - **Guarding split candidate**: `DispositionKind::Patrolling`
     (Patrol + Survey) vs `DispositionKind::Fighting` (EngageThreat).
     Maslow tier 2 stays for both.
   - **Socializing split candidate**: `Mentor → DispositionKind::Mentoring`
     (its own thread; Mentor is long-form and goal-shaped).
     Socialize and Groom stay together as ambient social.
   - **Crafting split candidate**: split into
     `DispositionKind::Herbalism` (gather/prepare/apply/ward),
     `DispositionKind::Witchcraft` (the magic siblings), and
     `DispositionKind::Cooking` (cook chain). The `CraftingHint`
     mechanism retires.

4. **Verdict per cluster.** Either land the split (open a sub-ticket
   for each that does), or write up why the cluster is structurally
   sound (the asymmetry is small enough that L3 softmax can absorb
   it; the constituents share a real underlying drive; etc.).

## Proposed sequence (after investigation lands)

Each suspect cluster's split is its own follow-on ticket:
- 153 — Guarding split (Patrol vs Fight)
- 154 — Socializing → Mentoring extraction
- 155 — Crafting split (Herbalism / Witchcraft / Cooking)

Each is independently shippable; this ticket is the audit + verdict
matrix only.

## Out of scope

- Actually landing any cluster's split (sub-tickets above).
- The non-tier-1 dispositions (already at higher Maslow tiers; the
  cost asymmetry doesn't translate to starvation directly, only to
  plan-churn).
- Wildlife AI dispositions (`fox_*`, `hawk_*`, `snake_*`) — those have
  their own separate enums and typically simpler plan templates;
  the audit can extend to them as a follow-up if any patterns hold.

## Verdict matrix

**Evidence base.** `logs/032-soak-treatment/` (seed 42, header
`commit_hash_short=883e9f3` with `commit_dirty=true` — post-150 Eating
split + 032 threshold-gated cliff WIP; the latter formally landed at
`930c2fe`). Footer fields and `just q` drills run 2026-05-03.

**Action distribution (`just q actions`, 10,017 `CatSnapshot` rows
across 8 distinct cats):**

| Action | Count | % of cat-time |
|---|---|---|
| Hunt | 3,227 | 32.22 |
| Forage | 3,041 | 30.36 |
| Patrol | 2,218 | 22.14 |
| Coordinate | 557 | 5.56 |
| Sleep | 300 | 2.99 |
| Wander | 178 | 1.78 |
| Eat | 130 | 1.30 |
| **PracticeMagic** | 81 | 0.81 |
| **Herbcraft** | 80 | 0.80 |
| **Socialize** | 73 | 0.73 |
| **Groom** | 71 | 0.71 |
| **Fight** | 47 | 0.47 |
| Flee | 10 | 0.10 |
| Build | 4 | 0.04 |

Conspicuously absent from `current_action` despite being declared in
`constituent_actions`: **Mentor, Cook, Mate, Caretake, Farm, Explore**.

**Footer headlines (`logs/032-soak-treatment/events.jsonl` final line):**
- `planning_failures_by_disposition = { Crafting: 9075, Foraging: 696, Hunting: 840 }` — Crafting still ~10× the next-highest.
- `never_fired_expected_positives = [FoodCooked, MatingOccurred, GroomedOther, MentoredCat, CourtshipInteraction, PairingIntentionEmitted]`.
- `continuity_tallies = { burial: 0, courtship: 0, grooming: 295, mentoring: 0, mythic-texture: 47, play: 284 }` — `courtship` regressed from 999 (pre-150 baseline) to 0; `mentoring` and `burial` remain at 0.
- `deaths_by_cause = { ShadowFoxAmbush: 7, WildlifeCombat: 1 }` — survival hard gates pass (`Starvation: 0`).

**Already-mapped layer-walk facts (from this audit's read pass):**
- All five suspect-cluster completion goals are
  `[StatePredicate::TripsAtLeast(current_trips + 1)]` (single trip;
  `src/ai/planner/goals.rs:65–66`). The 150 R5a defect-shape
  (multi-need completion proxy) does NOT directly transfer to these
  clusters; their generalized defect-shape is L3-picked Action being
  **discarded** post-mapping (only DispositionKind reaches the
  planner; the Action becomes informational — see
  `src/ai/scoring.rs:1910` and `src/ai/goap.rs:1599`).
- Plan templates and step-costs:
  - Resting: `[Sleep (2), SelfGroom (2)]`
  - Guarding: `[PatrolArea (2), EngageThreat (3), Survey (1)]`; Fight-directive override at `src/ai/goap.rs:1744–1748` branches the action set on coordinator directive.
  - Socializing: `[SocializeWith (2), GroomOther (2), MentorCat (3)]`
  - Crafting: 8 sub-modes routed by `CraftingHint` resolved at `src/ai/goap.rs:1622–1671` *after* the L3 softmax — Cook must strictly dominate both Magic and Herbcraft to win the hint, and ties go to Magic.
  - Exploring: `[ExploreSurvey (2)]` (single GoapActionDef)

### Per-cluster verdict

#### Resting `[Sleep, Groom]` — Maslow 1 — **verified-correct**

| Layer | Component | Load-bearing fact | Status |
|---|---|---|---|
| L3 mapping | `disposition.rs:119` | `Action::Sleep → Resting`; `Action::Groom → None` (caller decides self-vs-other) | `[verified-correct]` |
| Plan template | `planner/actions.rs::resting_actions` | `[Sleep (2), SelfGroom (2)]` | `[verified-correct]` |
| Completion proxy | `goals.rs:30–35` | `[EnergyOk(true), TemperatureOk(true)]` — both addressed by the two constituent steps | `[verified-correct]` |
| Runtime | `just q actions` | Sleep 2.99% / Groom 0.71% (ambiguous self-vs-other), no canary failure | `[verified-correct]` |

150 R5a already extracted Eating; the residual `[Sleep, Groom]` pair is
plan-cost symmetric and goal-aligned.

#### Guarding `[Patrol, Fight]` — Maslow 2 — **verified-correct**

| Layer | Component | Load-bearing fact | Status |
|---|---|---|---|
| L3 mapping | `disposition.rs:122` | `Action::Patrol \| Action::Fight → Guarding` | `[verified-correct]` |
| Directive override | `goap.rs:1744–1748` | Fight directive **branches the action set** at substrate level | `[verified-correct]` |
| Plan template | `planner/actions.rs::guarding_actions` | `[PatrolArea (2), EngageThreat (3), Survey (1)]` | `[verified-correct]` |
| Runtime | `just q actions` | Patrol 22.14% / Fight 0.47% — **both fire**; Fight rarity matches threat-driven directive flow | `[verified-correct]` |
| Footer | `032-soak-treatment` | Guarding **not present** in `planning_failures_by_disposition`; no never-fired feature in cluster | `[verified-correct]` |

The L3 Patrol-vs-Fight choice is preserved into observable behavior via
the directive-override path (substrate-level branching, not search-state),
so the bundle is structurally sound. No follow-on opened.

#### Socializing `[Socialize, Groom, Mentor]` — Maslow 3 — **split → 154**

| Layer | Component | Load-bearing fact | Status |
|---|---|---|---|
| L3 mapping | `disposition.rs:123` | `Action::Socialize \| Action::Mentor → Socializing` | `[suspect]` — collapses the cost asymmetry |
| Plan template | `planner/actions.rs::socializing_actions` | `[SocializeWith (2), GroomOther (2), MentorCat (3)]` — MentorCat strictly more expensive | `[suspect]` |
| Completion proxy | `goals.rs:60–67` | `TripsAtLeast(N+1)` — any sibling step satisfies the goal | `[suspect]` — cheaper sibling always wins |
| Runtime | `just q actions` | Socialize 0.73% / Groom 0.71% present; **Mentor absent** from current_action | `[suspect]` — confirmed crowd-out |
| Footer | `032-soak-treatment` | `MentoredCat` and `GroomedOther` both never-fired; `mentoring=0` continuity canary; `courtship=0` regression | `[suspect]` — MentoredCat never fires despite cats spending time in the cluster |

The L3 picks Mentor at some rate (the action *is* in the softmax pool),
but the planner picks the cheaper sibling and the trip-count goal
satisfies on the cheaper step. Mentor never reaches `current_action` and
`MentoredCat` never emits. Open ticket **154** to extract
`DispositionKind::Mentoring` and let Mentor's drive route to its own
plan-template + completion proxy. (Whether `GroomedOther` should also
extract is left as a 154 sub-question — it might just need a Maslow-tier
or directive distinction from `Action::Groom`-as-self.)

#### Crafting `[Herbcraft, PracticeMagic, Cook]` — Maslow 4 — **split → 155**

| Layer | Component | Load-bearing fact | Status |
|---|---|---|---|
| L3 mapping | `disposition.rs:127` | `Action::Herbcraft \| Action::PracticeMagic \| Action::Cook → Crafting` | `[suspect]` — three unrelated drives share one variant |
| Hint mechanism | `goap.rs:1622–1671` | `CraftingHint` resolved *post*-softmax; **Cook must strictly dominate Magic AND Herbcraft to win**, ties go to Magic | `[suspect]` — substrate-over-search-state violation; the hint re-runs scoring after the fact |
| Plan template | `planner/actions.rs::crafting_actions` | 8 sub-modes routed by hint (GatherHerbs, PrepareRemedy, SetWard, Magic, Cleanse, HarvestCarcass, DurableWard, Cook) | `[suspect]` — sub-mode is a Disposition-shaped distinction parading as a hint |
| Runtime | `just q actions` | PracticeMagic 0.81% / Herbcraft 0.80% present; **Cook absent**; Build 0.04% | `[suspect]` — confirmed Cook crowd-out |
| Footer | `032-soak-treatment` | Crafting=**9,075 plan failures** (~10× Foraging at 696, Hunting at 840); FoodCooked never-fired | `[suspect]` — dominant source of planner churn |

The `CraftingHint` band-aid is itself the evidence: three drives that
each need their own substrate signal, plan template, and completion
proxy have been bundled into one DispositionKind and recovered post-hoc
via a sub-mode hint. Open ticket **155** to split into
`DispositionKind::Herbalism`, `Witchcraft`, and `Cooking`, retire
`CraftingHint`, and align each with its perceptual gate (HasHerbs,
HasMagicCapacity, HasRawFood). The 9,075 plan failures should drop
materially when each Disposition's eligibility marker is checked at L1
rather than discovered at planning time.

#### Exploring `[Explore, Wander]` — Maslow 5 — **verified-correct**

| Layer | Component | Load-bearing fact | Status |
|---|---|---|---|
| L3 mapping | `disposition.rs:129` | `Action::Explore \| Action::Wander → Exploring` | `[verified-correct]` |
| Plan template | `planner/actions.rs::exploring_actions` | `[ExploreSurvey (2)]` — single GoapActionDef regardless of L3 pick | `[verified-correct]` |
| Runtime | `just q actions` | Wander 1.78% present; Explore absent — but plan-template is single-step, so the L3 distinction is purely flavour | `[verified-correct]` |

Single-action plan template means there is no sibling to crowd out.
The Explore-vs-Wander choice at L3 is narrative texture; the planner
runs the same step either way.

## Out of audit scope but observed

- **Mating cluster regression.** `MatingOccurred`, `CourtshipInteraction`,
  `PairingIntentionEmitted` all never-fired in the post-balancing soak;
  `continuity_tallies.courtship = 0` (down from 999 in the pre-150
  baseline). Mating is single-constituent (`[Mate]`) and out of this
  audit's scope, but the regression is dramatic and likely tied to either
  150's Eating split or 032's hunger-cliff work suppressing tier-3
  drives. Worth a separate diagnostic ticket; flagging here for the
  paper trail.
- **Wildlife AI dispositions** (`fox_*`, `hawk_*`, `snake_*`) — explicitly
  out of scope per ticket body; the same multi-constituent collapse
  pattern can be audited as a follow-up.

## Log

- 2026-05-03: Opened as a 150-landing sibling. R5a's split-pattern
  generalizes naturally to the other multi-constituent dispositions;
  this ticket is the audit pass that decides which siblings need the
  same treatment.
- 2026-05-03: Audit complete. Verdicts — Resting / Guarding / Exploring
  **verified-correct**; Socializing → **split** (open 154); Crafting →
  **split** (open 155). Evidence anchored on `logs/032-soak-treatment`
  (post-150, post-032-cliff-WIP). Smoking-gun signal: Mentor and Cook
  are entirely absent from `current_action` despite being in the L3
  softmax pool, and their cluster-feature events (`MentoredCat`,
  `FoodCooked`) never fire. Mating-cluster regression observed but
  out of scope. Land alongside 154/155 per ticket 151
  antipattern-migration discipline.
