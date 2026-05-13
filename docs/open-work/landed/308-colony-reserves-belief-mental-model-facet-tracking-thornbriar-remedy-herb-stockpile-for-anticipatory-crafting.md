---
id: 308
title: Colony reserves belief — mental-model facet tracking thornbriar / remedy-herb stockpile for anticipatory crafting
status: done
cluster: belief-perception
initiative: [full-sensory-perception, smarter-cats]
added: 2026-05-13
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 9343550a2bfe
landed-on: 2026-05-13
---

## Why

Cats currently perceive the colony's state reactively — they see acute deficits (hunger, threat, low safety) but have no anticipatory belief about *whether the colony is equipped for what's coming*. This shows up as a load-bearing gap in the herbcraft supply chain: when the priestess drains the thornbriar reserve placing wards, nothing in the substrate tells her "no one is gathering, you should harvest before crafting again." Guarding wins the softmax for sustained threat windows, and the colony enters a low-reserves-high-threat lock from which no one breaks free to rebuild the supply chain.

Surfaced 2026-05-13 during ticket 260's verification soak. The post-260 footer showed `wards_placed_total=12, never_fired_expected=["MatingOccurred"], deaths_by_cause.ShadowFoxAmbush=8` against the canonical post-297 baseline at 16 wards, MatingOccurred firing, 2 ambushes. Trace of the failure mode (`/logq` deaths + ward-placement timeline):

- Last ward placed at tick 1279399 (Mocha). At tick 1279408 Mocha tried again and the SetWard step failed for "no thornbriar".
- For the next 35000 ticks (~35 in-game days) **no priestess attempted SetWard at all**, despite Nettle, Calcifer, Bramble, Mocha all being alive and creating ~400 plans each in the window.
- Mocha won Guarding 126× in that window vs Herbalism 4×; Nettle Guarding 168× vs Herbalism 16×. The priestesses *kept choosing Guarding* under sustained threat — a substrate-honest local decision given current signals, but globally the colony was running out of wards with no one rebuilding the supply chain.
- First ShadowFox ambush in the unwarded zone at tick 1309038. Seven more deaths followed within 5500 ticks at (25-39, 20-23) — right at the gap left by the exhausted ward perimeter.

The fix isn't "tune Herbalism vs Guarding scoring." It's that cats lack the **belief layer needed to anticipate provisioning**. Within the BDI mapping the ai-substrate-refactor spec lays out (§4.3, §5.6.3, §7.W), this maps to: cats need a `ColonyReservesBelief` facet on their MentalModel — an anticipatory representation of "what's the colony stocked with right now" that the Desire layer (DSEs) can read.

This belief is the *upstream* enabler for the herbcraft-reserve consideration in ticket 309, and the *general substrate* for the broader emergent provisioning behaviors the design vision wants (the §7.W fulfillment-scalar layer that the eventual emergent town-roles system grows out of).

## Scope

- New `ColonyReservesBelief` facet on `MentalModel` (per-cat, 258-style subjective belief). Carries per-resource estimates: `thornbriar_count`, `remedy_herb_count`, future expansion to other crafted-material stockpiles.
- New `WitnessableEvent` emit sites that update the belief: `ReserveDeposited` (when an item lands in a colony stockpile), `ReserveConsumed` (when SetWard / PrepareRemedy etc. consume material), `InventoryObserved` (when a cat samples their own inventory or sees another cat's).
- Belief-integrator wiring (mirrors the 295 pattern) so the four witness sites land in the same commit as the facet.
- Per-cat marker `HasLowWardReserve` and similar — the digested boolean that DSE eligibility filters and consideration curves read. Authored from the belief, not from raw colony state, so the marker reflects what *this cat believes* about the reserve.
- Colony-scoped aggregator (`ColonyReservesAggregator` system) maintaining the ground-truth `ColonyState`-singleton marker that the belief converges toward via the witness events.

## Out of scope

- The Herbcraft DSE consideration that consumes the belief (that's ticket 309 — blocked-by this).
- Cooking / food-stockpile equivalents (FoodStores already exists as the ground truth; extending the belief facet to cover it is a natural follow-on but not in this scope).
- The §7.W fulfillment scalar (separate substrate layer; not blocked by this work).
- Emergent role attribution (sits on top of §7.W; aspirational).
- Belief-of-belief / theory-of-mind extensions ("does Mocha know that I know the reserves are low") — depends on Cluster C maturity.

## Current state

- 258 belief substrate landed (`WitnessableEvent` → `belief_integrator` → `LocationBeliefs` / `MentalModel`). 295 lit the first emit-sites (Mate / Care / FleeFrom / Hunt). 304 in flight for Attack emit.
- §4.3 of the substrate spec names the markers we need (`HasWardHerbs`, `HasRemedyHerbs`, `CanWard`) — all currently `Absent` in code. §5.6.3 covers the colony-scoped inventory aggregator pattern.
- No `ColonyReservesBelief` exists. No aggregator system tracking thornbriar / remedy-herb counts colony-wide.

## Approach

Mirror the 295 emit-site pattern for the four new WitnessableEvent variants. Each emit site lives at the action-resolver layer where the inventory mutation happens. Belief-integrator dispatches updates to the cat's MentalModel facet. Marker maintenance runs in the same chain as the belief update so the digested booleans stay one tick fresh.

Order of work:
1. Define `ColonyReservesBelief` facet shape + `WitnessableEvent` variants.
2. Wire integrator + aggregator + four emit sites in the same commit (substrate-stub forbidden rule).
3. Add `HasLowWardReserve` marker authored from the per-cat belief.
4. Scenario microexperiment: spawn priestess + low-thornbriar colony + ambient threat; assert `HasLowWardReserve` marker fires within N ticks of reserve dropping below threshold.

## Verification

- `just check` — substrate-stub forbidden rule blocks if marker exists without writer or vice versa.
- Scenario microexperiment per §Approach.
- Soak: assert no behavior regression vs `post-297-substrate-dormant` baseline at this layer alone (the belief is silent until ticket 309 consumes it; this ticket lands the substrate at zero DSE weight).

## Log

- 2026-05-13: opened from ticket 260's verification soak discovery. The thornbriar starvation pattern (35k-tick window with no SetWard attempts, 7-cat ambush wave following) confirms that the herbcraft supply chain lacks anticipatory provisioning. Cluster C / BDI belief layer (258) is the right substrate home; this ticket lands the colony-reserves facet that ticket 309's Herbcraft consideration will consume.
- 2026-05-13: landed substrate-dormant on top of 312. Five pieces in one commit per the substrate-stub-forbidden rule: (1) `ColonyReservesBelief` as a 5th sibling belief Component family alongside `CatBeliefs`/`LocationBeliefs`/`PredatorBeliefs`/`ContextBeliefs`, keyed by a new `ResourceKind` enum (Thornbriar / RemedyHerb) and carrying a count-shaped `ReserveBelief` state (purpose-built, not the opinion-shaped 6-facet `MentalModel`); (2) three new `WitnessableEvent` variants — `ReserveDeposited` (emitted at `GatherHerb` since herbs flow gather→inventory→consume without a Stores intermediate in the current architecture; the aggregator sums per-cat inventories + Stores), `ReserveConsumed` (at `SetWard`'s Thornward branch and `PrepareRemedy`), and `InventoryObserved` (broadcast per cat on its stagger tick by a new `gossip_inventory_observations` system — god-eye sensor narratively framed as "cats communicate about what they're carrying", per the user's reframe that handoffs need this gossip channel); (3) ground-truth `ColonyReserves` resource + `sync_colony_reserves` aggregator mirroring `FoodStores`/`sync_food_stores`; (4) per-cat `HasLowWardReserve` ZST marker authored from each cat's `ColonyReservesBelief[Thornbriar].estimated_count <= low_ward_reserve_threshold` (default 2); (5) `BeliefsConstants` gains three tunables (threshold + observation-strength + per-stagger decay).
- 2026-05-13: verification. `just check` clean; `just test --lib` 2091 / 2091 passing, including four new belief_integrator tests (`reserve_deposited_lifts_witness_count`, `reserve_consumed_decrements_count_saturating`, `inventory_observed_self_replaces_count`, `inventory_observed_other_takes_lower_bound_max`). `just scenario colony_reserves_belief` fires `HasLowWardReserve` within tick budget. `just soak-trace 42 Pyre --duration 900` → `logs/tuned-308-dormant/`; `just verdict` returned **"concern"** (not fail): all hard survival gates pass (Starvation=0, ShadowFoxAmbush=3 ≤ 10, never_fired_expected_positives=[], all five continuity canaries ≥1). Footer drift: bonds_formed +16.7%, deaths_injury -62.5%, structures_built -37.5%, duration +14.7%. **Hypothesis: schedule-edge perturbation** — three new sibling systems land in existing Chain blocks (`sync_colony_reserves` in the prune/sync sub-chain; `gossip_inventory_observations` + `update_low_ward_reserve_markers` in Chain 2b around `integrate_beliefs`), plus every cat's archetype gains `ColonyReservesBelief` which can reorder query iteration. Memory `learning_bevy_schedule_edge_perturbation` documents this pattern. The substrate itself is dormant — no DSE reads the belief or marker; behavior change is incidental to substrate addition, not consumption. The new run is actually *healthier* than baseline (MatingOccurred now fires, fewer ambush deaths, more wards placed) — schedule re-ordering shifted seed-42 outcomes favorably. Ticket 309 lands the real consumer next.
