---
id: 152
title: Tier-1 disposition-collapse audit — sweep for sibling Eat-into-Resting defects
status: ready
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

## Log

- 2026-05-03: Opened as a 150-landing sibling. R5a's split-pattern
  generalizes naturally to the other multi-constituent dispositions;
  this ticket is the audit pass that decides which siblings need the
  same treatment.
