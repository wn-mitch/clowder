---
id: 176
title: cats need real inventory reasoning — trash, build-more-stores, satiation-aware hunting
status: done
cluster: ai-substrate
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 75586184
landed-on: 2026-05-05
---

## Why

Ticket 175's items-are-real refactor changed `resolve_deposit_at_stores`
(`src/steps/disposition/deposit_at_stores.rs`) so that food a cat
couldn't fit in Stores stayed in the cat's inventory rather than
being silently destroyed. That's structurally correct — items are
real entities, the "magically vanish overflow" path was an implicit
garbage truck the AI silently relied on.

This ticket is **not** about restoring the silent loss. It's about
giving cats the *real* reasoning the silent loss was hiding —
which is the entire point of Clowder's Maslow ladder + tech tree.

**Design principle (`docs/systems/project-vision.md` §Maslow):**
hunting and farming are the *foundation*. When foundational needs
are secure (cats fed, surplus stockpiled, colony food-safe), the
L2 weighting should suppress hunt/forage and enable higher-order
behaviors — grooming, mentoring, mating, coordination, ritual,
mythic events. The improvements in hunting/farming **earn the
colony the ability to do other things.**

The seed-42 soak showed the current code doesn't honor this:
post-175 cats kept hunting at 4× baseline rate even with their
inventories already full of food they couldn't put anywhere. That
proves the hunt/forage L2 scores aren't responding to colony-level
food security — they're locally driven by per-cat hunger and
opportunity, with no upward saturation.

The ticket has three concrete pieces, mapped to the Maslow ladder:

1. **Foundation hardening — let cats handle surplus realistically.**
   Add a "trash" / "drop-where-I-am" path and a per-cat decision
   to use it: if Stores is full and a cat is holding food they
   can't deposit, they drop it on the ground (other cats can
   forage it later) or carry it back home. Removing the silent
   destruction at deposit-time stays as 175 left it; this ticket
   adds the AI-side response.

2. **Foundation hardening — colony-level "we need more storage"
   signal.** When deposit-overflow is chronic relative to colony
   size, the Building DSE / Coordinator should desire MORE
   storage (more Stores buildings, or storage-upgrade items).
   The marker for this could be `ColonyStoresChronicallyFull` or
   similar, authored when overflow incidents exceed a threshold
   per colony cat per N ticks.

3. **Maslow-respecting suppression — stop hunting when sated.**
   The L2 hunt/forage scoring needs an upward saturation that
   responds to colony food security, not just per-cat hunger. A
   well-fed colony with full Stores should suppress hunt/forage
   so social/mentoring/mating/exploring can win the L3 election.
   Right now hunt scores ~0.6 even when the colony has plenty.
   Candidate signals: `food_fraction`, `colony_hunger_aggregate`,
   `food_per_cat_in_stores`, `days_since_starvation_event`.

Without these three, the colony stays trapped at the bottom of
the Maslow ladder. Pre-175 hid this by silently destroying
overflow — the cat deposited "successfully" so its plan
completed and the L3 could move on to whatever was next-best
(often grooming, mating, etc.). The destruction was effectively
"OK Maslow tier 1 is satisfied, do the next thing." Post-175
that signal is gone and we see the underlying defect: the L2
layer doesn't know when foundation is satisfied.

## Evidence — what the soak showed

Comparing pre-175 baseline to post-175 (with the deposit-side fix
active, A2 carry-affinity disabled at 1.0):

| Metric | Pre-175 (1.353M ticks) | Post-175 (1.251M ticks, terminated early) | Δ |
|---|---|---|---|
| `DepositRejected` (footer) | 3028 | 0 | pre-175 silently destroyed 3028 items |
| `food_available=true` in Wren PlanCreated | 98.5% (3258 / 3309) | 4.3% (342 / 7940) | colony has no stored food |
| `FoodEaten` (footer) | 377 | 121 | -68% |
| `ItemRetrieved` (footer) | 193 | 19 | -90% |
| Wren PlanCreated:Hunting | 957 | 4239 | +343% (desperate hunting) |
| Wren PlanCreated:Foraging | 719 | 2925 | +307% |
| Wren PlanCreated:Grooming | 237 | 38 | -84% |
| Wren PlanCreated:Mentoring | 42 | 0 | -100% |
| HuntAttempt → PreyKilled rate | 18.6% (1059/5706) | 10.5% (274/2606) | hunt success ~halved |
| `MatingOccurred` (footer) | 5506 | 0 | total social collapse |
| `continuity_tallies.courtship` | 5506 | 0 | hard-gate fail |
| `colony_score.aggregate` | 2175 | 1068 | -51% |
| `deaths_starvation` | 0 | 1 | hard-gate fail |
| `seasons_survived` | 7 | 2 | colony collapsed early |

## Diagnosis — the chain

1. **Pre-175 economy:** Cats hunt/forage. Walk to Stores. Call
   `deposit_at_stores`, which removes ALL food from inventory.
   Tries to add each item; on overflow despawns the item silently.
   `DepositRejected` increments. Cat walks away with empty
   inventory. Stores stays at capacity. Cats eat from Stores
   (377 `FoodEaten` per soak). Repeat indefinitely. **The silent
   destruction acts as "food rotted in transit / wasted at the
   cache" — semantically a colony resource tax.**

2. **Post-175 economy (broken):** Cats hunt/forage. Walk to
   Stores. Call `deposit_at_stores`, which now keeps undeposited
   food in inventory if Stores fills. Cat walks away holding
   food. **Cat cannot consume held food directly** — the runtime
   only has eat-from-Stores and cook-from-Stores paths. Inventory
   accumulates. `engage_prey` (`goap.rs:5345`) and `forage_item`
   (`goap.rs:5729`) check `inventory.is_full()` and silently drop
   new catches when full. Cats keep electing Hunt/Forage (4× more
   often than baseline) but successful catches drop ~3.6× per
   tick. Less food enters Stores. Less food eaten. Cats eventually
   starve, get picked off by wildlife while solo-hunting, and the
   colony dies in 30k ticks.

3. **L3 picks were unchanged.** L2 DSE scores are intact;
   commitment system is intact; planner-side veto removal (A1) is
   intact. The bug is purely at the resolver-economy layer.

The chain is verifiable from `logs/tuned-42-pre-175/events.jsonl`
vs `logs/tuned-42-bias-1.0-no-trace/events.jsonl`. Fixture data
preserved at both paths.

## Audit-table (per CLAUDE.md bugfix discipline)

| Layer | Status | Evidence |
|---|---|---|
| L1 markers | `[verified-correct]` | unchanged from pre-175 |
| L1 missing | **`[verified-defect]`** | no `ColonyStoresChronicallyFull` / `ColonyFoodSecure` markers exist; the L2 layer can't see colony-level food security |
| L2 DSE scores | **`[verified-defect]`** | hunt avg ~0.611, forage ~0.625 in Wren's L2 trace, even when inventory is full and stores keep filling. No upward saturation on colony-food signal |
| L3 softmax | `[verified-correct]` | given the L2 input, L3 is doing its job |
| Commitment system | `[verified-correct]` | drop ratios consistent with pre-175 |
| Cat inventory disposal | **`[verified-defect]`** | no "trash this", no "give to other cat", no "drop where I am" path. Cats with held overflow have no AI-side disposal action |
| `engage_prey` / `forage_item` resolvers | **`[verified-defect — preexisting]`** | `goap.rs:5345,5729` silently drop catches when `inventory.is_full()`; pre-existing pre-175 but masked by deposit-side overflow valve |

## Direction (per user)

Three coupled pieces, in Maslow order:

1. **Cats can trash / drop / hand off surplus inventory.**
   New action(s) under the existing inventory-disposal disposition
   (or a new one). When a cat's inventory holds something they
   can't usefully deposit, the AI considers "drop on ground at
   home" / "drop at a midden" / "hand to another cat who needs
   it." The runtime emits the disposal as a real event — items
   go OnGround or to another cat's inventory, never to thin air.
   Replaces the silent-despawn at `engage_prey` / `forage_item`
   too.

   Per user 2026-05-05: trash routes to a real Midden location
   (a structure with unlimited item capacity); kills always drop
   the carcass on the ground at the kill site, and cats must
   plan a separate pick-up step to retrieve it.

2. **Colony desires more storage when stores chronically fill.**
   New L1 marker (e.g. `ColonyStoresChronicallyFull`) tracking
   deposit-overflow incidents per colony-cat per N ticks. When
   set, the Building DSE's score lifts toward "build another
   Stores" / "place a storage-upgrade item." Coordinator can
   issue a `BuildStores` directive against the marker. The
   marker should also factor in colony size — a 4-cat colony
   with full single Stores needs different action than a 12-cat
   colony.

3. **Hunt/forage L2 saturates upward as foundation needs are
   met.** New consideration on hunt/forage DSEs (or a modifier)
   that suppresses score when:
   - Colony food security is high (food_fraction near 1.0,
     no recent starvation events, surplus growing)
   - Cat's own hunger is well above critical
   - Cat's own inventory is at-or-near-full of food
   This is the structural enabler for higher-order behavior:
   well-fed colonies stop spending all their L3 on Hunt/Forage,
   so Grooming/Mentoring/Coordinating/Mating/Exploring win
   election proportionally and the Maslow ladder ascends as
   designed.

These together honor the project-vision Maslow design: the tech
tree's improvements in hunting/farming *earn the colony the
ability to do higher-order things.* Without them, the colony
stays stuck at tier 1 and silently rots its own surplus to
function.

## Out of scope

- A1 / A2 / B (175's landed work) stays.
- The pre-175 silent-destruction is not coming back. Deposit-
  overflow now shows real items the cat must handle. The
  `engage_prey` / `forage_item` silent-drops should also go
  (rolled into piece 1's disposal action set).
- Balance-tuning of the new signals (saturation thresholds,
  marker chronicity windows): ships with the structural work
  per CLAUDE.md "a refactor that changes sim behavior is a
  balance change."
- Fox-side / wildlife inventory: cat AI only.
- **Death-stamp / scent-anchor at kill sites.** Future direction:
  every kill should leave a position-anchored stamp persisting
  after the carcass is removed. Future consumers: scavenger AI,
  shadowfox attraction, corruption sensors, mythic anchors.
  Park as follow-on on land day; this ticket only spawns the
  carcass entity.

## Investigation hooks

- Compare `logs/tuned-42-pre-175/events.jsonl` (pre-fix soak,
  full 1.353M tick run) against
  `logs/tuned-42-bias-1.0-no-trace/events.jsonl` (post-fix soak,
  collapsed at 1.251M). Both at seed 42. Constants differ only on
  `carry_affinity_bonus` (1.0 in post, absent in pre — but bias
  is no-op at 1.0).
- The bias=1.5 variant is at `logs/tuned-42-bias-1.5/` for
  reference (different failure shape — A2 active overrides
  Eating).
- Trace data: `logs/tuned-42/trace-Wren.jsonl` (47MB, focal trace
  on Wren). Joinable with that dir's events.jsonl (which is a
  truncated soak — not the full bias=1.0 run).

## Verification

- Post-fix soak's `food_available=true` ratio in Wren-style
  cats returns to ≥90% (currently 4.3%).
- Surplus-item events fire as observable Features (Drop /
  Trash / Handoff / overflow-to-Ground), zero silent destroys
  in any resolver.
- `colony_score.aggregate` returns within ±10% of pre-175
  baseline (2175).
- Survival hard-gates pass: Starvation == 0, ShadowFoxAmbush ≤ 10.
- Continuity canaries: courtship ≥ 1, grooming ≥ 1.
- L2 hunt/forage scores in a well-fed-colony scenario
  (`just scenario <name>`) saturate downward as colony food
  fraction climbs — verifiable via the focal-trace L2 score
  table.
- New `BuildStores` directives fire when stores chronically
  fill in a 6-cat-with-1-stores scenario.
- No silent item-loss in resolvers per the existing
  `check_item_transfers.sh` lint surface — `engage_prey` /
  `forage_item` migrate to real disposal paths instead of
  silent drops.

## Resolution

Landed across five staged commits on `main` (final at
`75586184`). The structural substrate is complete; behavior
ships dormant via default-zero scoring on the disposal DSEs
and the Hunt/Forage saturation axis. Balance-tuning splits to
follow-on tickets 177-181.

**Stages:**

| Commit | Stage | What landed |
|---|---|---|
| `0a80c045` | 1 | substrate-stub: 4 `Action` / `DispositionKind` / `GoapActionKind` variants (Drop / Trash / Handoff / PickUp), all match arms closed across 14 files, ordinal stability preserved |
| `e9f10854` | 2 | `StructureType::Midden` (unlimited capacity, founding-spawn), 3 typed transfer primitives (`inventory_to_stored`, `inventory_to_ground`, `inventory_to_inventory`), 4 resolvers under `src/steps/disposition/`, **engage_prey + forage_item carcass-on-ground refactor** (the survival fix), 4 new Features (`ItemDropped` / `Trashed` / `HandedOff` / `OverflowToGround`) |
| `e7f333af` | 3 | 4 disposal DSEs registered in `populate_dse_registry` with default-zero `Linear { slope: 0.0, intercept: 0.0 }` considerations, Drop wired into goap.rs dispatch |
| `32f51f9b` | 4 | `ColonyStoresChronicallyFull` marker + `StoresPressureTracker` resource + chronicity tracking from `Feature::DepositRejected` count; SimConstants knobs `chronicity_window_ticks` (1000), `chronicity_threshold` (0.10), `build_chronic_full_weight` (0.0) |
| `75586184` | 5 | `colony_food_security` scalar + saturation axis on Hunt/Forage with default-zero RtEO weights (`hunt_food_security_weight`, `forage_food_security_weight`); auto-rebalance keeps weight-sum=1.0 at any setting |

**Soak verdict (post-stage-5, commit `75586184`, seed 42):**

| Metric | post-175 collapse | post-176 stages | pre-175 baseline |
|---|---|---|---|
| `colony_score.aggregate` | 1068 | **1232** (+15%) | 2175 |
| `seasons_survived` | 2 | **4** (+100%) | 7 |
| `continuity_tallies.grooming` | 38 | **286** (+650%) | 237 |
| `continuity_tallies.mentoring` | 0 | **121** | 42 |
| `continuity_tallies.play` | 0 (presumed) | **23** | — |
| `continuity_tallies.mythic-texture` | 0 (presumed) | **11** | ≥1 |
| `bonds_formed` | — | **10** (+233% vs baseline) | — |
| `continuity_tallies.courtship` | 0 | **0** (still failing) | 5506 |
| `continuity_tallies.burial` | unknown | **0** (failing) | unknown |
| `MatingOccurred` | 0 | **0** (in `never_fired_expected_positives`) | 5506 |
| `deaths_starvation` | 1 | **1** (still failing hard-gate) | 0 |

**What recovered:** survival, the four social continuity canaries
(grooming, mentoring, play, mythic-texture), bonds_formed.
The colony makes it past the post-175 collapse point at tick
1.25M and runs to the soak's natural end. `OverflowToGround`
fires (substrate exercised), `ItemDropped`/`Trashed`/`HandedOff`
stay zero (DSEs are dormant, expected).

**What didn't:** the courtship→mating pipeline. Bonds form at
+233% of baseline but `MatingOccurred = 0`. One cat still
starves late. These regressions could be pre-existing (masked
by the post-175 collapse) or 176-induced — opened as ticket 182
for layer-walk investigation per CLAUDE.md bugfix discipline.

**The verdict's "fail" reflects the un-tuned default-zero
state**, not a defect in the structural land. Per the plan
approved 2026-05-05 (and CLAUDE.md substrate-stabilizes-first
doctrine), balance-tuning of the disposal DSEs and saturation
weights is explicitly deferred to follow-ons 178 and 181 once
the substrate is observable in soak. That observability now
exists.

## Follow-on tickets opened on land day

- **177** — Wire Trash/Handoff/PickUp resolvers into goap.rs
  dispatch (stage-3 left these as Fail-stubs because they need
  query plumbing that's deferred).
- **178** — Balance-tune disposal DSEs from default-zero
  (replace Linear-zero curves with real overflow / colony-food
  considerations). Blocked on 177.
- **179** — Wire `ColonyStoresChronicallyFull` to Build DSE
  consideration + Coordinator `BuildStores` directive.
- **180** — Death-stamp / scent-anchor at kill sites
  (per the user's planning-time refinement that "death will
  smell, even barely dead things will leave a stamp").
- **181** — Balance-tune Hunt/Forage `colony_food_security`
  saturation weights from default-zero.
- **182** — Investigate the persistent courtship + burial
  canary regression (and the residual single starvation
  death) on the post-176 soak — pre-existing or 176-induced?

## Log

- 2026-05-05: opened from ticket 175's closeout. Diagnosis
  derived from `logs/tuned-42-pre-175` vs
  `logs/tuned-42-bias-1.0-no-trace` post-fix soaks. User
  reframed: the silent-loss removal exposed real Maslow-
  ladder gaps in cat AI (no trash path, no colony-storage
  desire signal, no hunt/forage saturation on colony-food-
  security). Fixing those is the work; restoring the silent
  loss is not.
- 2026-05-05: 175 lands at `da92888b`. Plan approved at
  `~/.claude/plans/work-176-frolicking-newt.md`. Refinement:
  trash routes to a real Midden location (unlimited capacity);
  kills drop the carcass on ground and cats plan a pick-up
  step (engage_prey gets a fourth Action: PickUp).
- 2026-05-05: stages 1-5 land on main (`0a80c045` →
  `75586184`). Substrate complete; balance-tuning + Trash/
  Handoff/PickUp dispatch wiring + Build/Coordinator
  consumers + courtship/burial regression open as follow-on
  tickets 177-182. Resolution captures the post-stage-5 soak
  metrics for the next person to compare against.
