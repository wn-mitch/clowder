use crate::ai::Action;
use crate::components::markers;
use crate::components::recipe::StationRequirement;

use super::{
    Carrying, GoapActionDef, GoapActionKind, PlannerZone, StateEffect, StatePredicate,
    ZoneDistances,
};

// ---------------------------------------------------------------------------
// Travel actions — one per reachable (from, to) zone pair
// ---------------------------------------------------------------------------

/// Build TravelTo actions from pre-computed zone distances.
/// Creates one action per (from, to) pair in the distance matrix.
pub fn travel_actions(distances: &ZoneDistances) -> Vec<GoapActionDef> {
    distances
        .distances
        .iter()
        .map(|(&(from, to), &cost)| GoapActionDef {
            kind: GoapActionKind::TravelTo(to),
            cost,
            preconditions: vec![StatePredicate::ZoneIs(from)],
            effects: vec![StateEffect::SetZone(to)],
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Per-disposition action sets
// ---------------------------------------------------------------------------

/// 230: Fleeing plan template — `[PickFleeTarget, Flee, HoldUntilSafe]`.
///
/// `PickFleeTarget` reads the per-replan `RouteCostField` to choose the
/// lowest-cost passable tile within Chebyshev `flee_distance`; the
/// `Flee` umbrella `cat_path_plan!`s to that tile; `HoldUntilSafe`
/// hysteresis-waits `flee_hold_ticks` before incrementing trips.
///
/// The chain is ordered via the search-state predicate
/// `FleeTargetPicked(bool)`. No `TravelTo` leg is needed — `Flee`
/// itself does the travel via the same machinery `TravelTo` would.
/// Costs are kept low (1/2/1) because Fleeing is a tier-1 acute-survival
/// disposition that should outcompete anything but other tier-1 plans
/// when the modifier ramp lifts it.
pub fn fleeing_actions() -> Vec<GoapActionDef> {
    vec![
        GoapActionDef {
            kind: GoapActionKind::PickFleeTarget,
            cost: 1,
            preconditions: vec![StatePredicate::FleeTargetPicked(false)],
            effects: vec![StateEffect::SetFleeTargetPicked(true)],
        },
        GoapActionDef {
            kind: GoapActionKind::Flee,
            cost: 2,
            preconditions: vec![StatePredicate::FleeTargetPicked(true)],
            effects: vec![],
        },
        GoapActionDef {
            kind: GoapActionKind::HoldUntilSafe,
            cost: 1,
            preconditions: vec![StatePredicate::FleeTargetPicked(true)],
            effects: vec![
                StateEffect::IncrementTrips,
                StateEffect::SetFleeTargetPicked(false),
            ],
        },
    ]
}

pub fn hunting_actions() -> Vec<GoapActionDef> {
    vec![
        // Ticket 091: SearchPrey/EngagePrey did NOT require
        // `CarryingIs(Carrying::Nothing)` because the runtime resolver
        // gated on `inventory.is_full()`. The pre-091 planner's
        // `CarryingIs(Carrying::Nothing)` precondition was a permanent
        // veto for any cat with leftover items, which made Hunting
        // plans uniformly unreachable for the post-founding colony
        // (zero PlanCreated{disposition:"Hunting"} across 1.2M ticks
        // for 8 cats).
        //
        // Ticket 235 promotes the slot-availability gate back into the
        // planner, but in the *positive* substrate form (`HasFreeSlot`
        // marker on the substrate-path; `HasFreeSlotThisPlan(true)` on
        // the plan-path after a prefix step) rather than the negative
        // `CarryingIs(Carrying::Nothing)` 091 had to remove. Cats with
        // leftover items now compose `[DropItem, SearchPrey, ...]` or
        // — when a stash is reachable — `[TravelTo(Stores),
        // DepositHerbs(prefix), TravelTo(HuntingGround), SearchPrey,
        // ...]`, mirroring the picking_up_actions / cooking_actions
        // composition introduced in 231.
        GoapActionDef {
            kind: GoapActionKind::DropItem,
            cost: 1,
            preconditions: vec![],
            effects: vec![
                StateEffect::SetHasFreeSlotThisPlan(true),
                StateEffect::SetCarrying(Carrying::Nothing),
            ],
        },
        // 235: DepositHerbs-as-prefix alternative — route through Stores
        // when carrying herbs and stash is reachable.
        GoapActionDef {
            kind: GoapActionKind::DepositHerbs,
            cost: 1,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::CarryingIs(Carrying::Herbs),
                StatePredicate::HasMarker(crate::components::markers::HasHerbStashAccessible::KEY),
                StatePredicate::HasMarker(crate::components::markers::HasHerbsInInventory::KEY),
            ],
            effects: vec![
                StateEffect::SetHasFreeSlotThisPlan(true),
                StateEffect::SetCarrying(Carrying::Nothing),
            ],
        },
        // 235: substrate-path SearchPrey — fires when the cat already
        // has a free slot. Gating on `SearchPrey` (not `EngagePrey`)
        // forces the prefix to fire BEFORE the cat commits to a hunting
        // ground; otherwise an empty-slotted cat could search, fail to
        // engage, and waste the trip.
        GoapActionDef {
            kind: GoapActionKind::SearchPrey,
            cost: 3,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::HuntingGround),
                StatePredicate::HasMarker(crate::components::markers::HasFreeSlot::KEY),
            ],
            effects: vec![StateEffect::SetPreyFound(true)],
        },
        // 235: plan-path SearchPrey — fires after a prefix
        // (DropItem or DepositHerbs) sets HasFreeSlotThisPlan(true).
        GoapActionDef {
            kind: GoapActionKind::SearchPrey,
            cost: 3,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::HuntingGround),
                StatePredicate::HasFreeSlotThisPlan(true),
            ],
            effects: vec![StateEffect::SetPreyFound(true)],
        },
        GoapActionDef {
            kind: GoapActionKind::EngagePrey,
            cost: 2,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::HuntingGround),
                StatePredicate::PreyFound(true),
            ],
            effects: vec![
                StateEffect::SetCarrying(Carrying::Prey),
                StateEffect::SetPreyFound(false),
            ],
        },
        GoapActionDef {
            kind: GoapActionKind::DepositPrey,
            cost: 1,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::CarryingIs(Carrying::Prey),
            ],
            effects: vec![
                StateEffect::SetCarrying(Carrying::Nothing),
                StateEffect::IncrementTrips,
            ],
        },
    ]
}

pub fn foraging_actions() -> Vec<GoapActionDef> {
    vec![
        // Ticket 091: see `hunting_actions` — same `CarryingIs(Nothing)`
        // veto removal applies. The runtime resolver `resolve_forage_item`
        // gates on `inventory.is_full()`; the planner doesn't need to
        // enforce a stricter precondition.
        GoapActionDef {
            kind: GoapActionKind::ForageItem,
            cost: 3,
            preconditions: vec![StatePredicate::ZoneIs(PlannerZone::ForagingGround)],
            effects: vec![StateEffect::SetCarrying(Carrying::ForagedFood)],
        },
        GoapActionDef {
            kind: GoapActionKind::DepositFood,
            cost: 1,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::CarryingIs(Carrying::ForagedFood),
            ],
            effects: vec![
                StateEffect::SetCarrying(Carrying::Nothing),
                StateEffect::IncrementTrips,
            ],
        },
    ]
}

/// 150 R5a: Resting plan is Sleep + SelfGroom only. EatAtStores
/// migrated to the new `eating_actions` template — picking Eat at the
/// L3 softmax no longer commits the cat to a Sleep beat. Resting still
/// runs both Sleep and SelfGroom because they're naturally co-located:
/// a cat that lies down to sleep also self-grooms during the same lull.
pub fn resting_actions() -> Vec<GoapActionDef> {
    vec![
        GoapActionDef {
            kind: GoapActionKind::Sleep,
            cost: 2,
            preconditions: vec![StatePredicate::ZoneIs(PlannerZone::RestingSpot)],
            effects: vec![StateEffect::SetEnergyOk(true)],
        },
        GoapActionDef {
            kind: GoapActionKind::SelfGroom,
            cost: 2,
            // No zone precondition — cats can groom anywhere.
            preconditions: vec![],
            effects: vec![StateEffect::SetTemperatureOk(true)],
        },
    ]
}

/// 150 R5a: single-action template for the new `Eating` disposition.
/// Plan = `[TravelTo(Stores), EatAtStores]` once travel is composed in.
/// Tickets 091/092: `HasStoredFood` marker still gates EatAtStores so
/// the planner can't schedule it against empty stores. Mirrors the
/// substrate-vs-search-state unification that 092 established.
pub fn eating_actions() -> Vec<GoapActionDef> {
    vec![GoapActionDef {
        kind: GoapActionKind::EatAtStores,
        cost: 2,
        preconditions: vec![
            StatePredicate::ZoneIs(PlannerZone::Stores),
            StatePredicate::HasMarker(markers::HasStoredFood::KEY),
        ],
        effects: vec![StateEffect::SetHungerOk(true)],
    }]
}

pub fn guarding_actions() -> Vec<GoapActionDef> {
    vec![
        GoapActionDef {
            kind: GoapActionKind::PatrolArea,
            cost: 2,
            preconditions: vec![StatePredicate::ZoneIs(PlannerZone::PatrolZone)],
            effects: vec![StateEffect::IncrementTrips],
        },
        GoapActionDef {
            kind: GoapActionKind::EngageThreat,
            cost: 3,
            preconditions: vec![StatePredicate::ZoneIs(PlannerZone::PatrolZone)],
            effects: vec![StateEffect::IncrementTrips],
        },
        GoapActionDef {
            kind: GoapActionKind::Survey,
            cost: 1,
            preconditions: vec![StatePredicate::ZoneIs(PlannerZone::PatrolZone)],
            effects: vec![StateEffect::IncrementTrips],
        },
    ]
}

pub fn socializing_actions() -> Vec<GoapActionDef> {
    // 154: MentorCat extracted into `mentoring_actions()` so the L3
    // pick on `Action::Mentor` survives the disposition collapse instead
    // of getting crowded out by the cheaper sibling steps under a
    // count-based completion goal.
    // 158: GroomOther extracted into `grooming_actions()` for the
    // same shape of bug — the post-154 `[SocializeWith (2), GroomOther
    // (2)]` template had two equivalent-effect actions
    // (`SetInteractionDone(true), IncrementTrips`), and A* at
    // `mod.rs:437` pre-pruned the second action because both produced
    // the same `next_state`. The single-action template here makes
    // equivalent-sibling pre-pruning structurally impossible.
    vec![GoapActionDef {
        kind: GoapActionKind::SocializeWith,
        cost: 2,
        preconditions: vec![StatePredicate::ZoneIs(PlannerZone::SocialTarget)],
        effects: vec![
            StateEffect::SetInteractionDone(true),
            StateEffect::IncrementTrips,
        ],
    }]
}

/// 154: single-action template for the new `Mentoring` disposition.
/// Pattern-B (interaction-based, single-trip) — clones the shape of
/// `mating_actions()`. Completion proxy is `InteractionDone(true)`
/// (set in `goal_for_disposition`); no trip counter, so the executor
/// resolves on the first successful mentor session and the L3 Mentor
/// pick can't be overridden by sibling cost-asymmetry.
pub fn mentoring_actions() -> Vec<GoapActionDef> {
    vec![GoapActionDef {
        kind: GoapActionKind::MentorCat,
        cost: 3,
        preconditions: vec![StatePredicate::ZoneIs(PlannerZone::SocialTarget)],
        effects: vec![StateEffect::SetInteractionDone(true)],
    }]
}

/// 158: single-action template for the new `Grooming` disposition.
/// Pattern-B (interaction-based, single-trip) — direct sibling of
/// `mentoring_actions()`. Completion proxy is `InteractionDone(true)`
/// so the L3 GroomOther pick can't be planner-shadowed by an
/// equivalent-effect sibling step. (Pre-158, GroomOther rode under
/// Socializing's `[SocializeWith (2), GroomOther (2)]` template, and
/// A* pre-pruned it because both actions produced the same
/// `(SetInteractionDone(true), IncrementTrips)` next-state.)
pub fn grooming_actions() -> Vec<GoapActionDef> {
    vec![GoapActionDef {
        kind: GoapActionKind::GroomOther,
        cost: 2,
        preconditions: vec![StatePredicate::ZoneIs(PlannerZone::SocialTarget)],
        effects: vec![StateEffect::SetInteractionDone(true)],
    }]
}

/// 035: single-action template for the new `Burying` disposition.
/// Pattern-B (interaction-based, single-trip) — mirrors
/// `mentoring_actions()` / `grooming_actions()`. Routes through
/// `PlannerZone::CorpseTarget` (the dead-cat snapshot zone), distinct
/// from `SocialTarget` because the canonical `cat_positions` snapshot
/// is built `Without<Dead>` for social-family DSEs.
pub fn burying_actions() -> Vec<GoapActionDef> {
    vec![GoapActionDef {
        kind: GoapActionKind::Bury,
        cost: 2,
        preconditions: vec![StatePredicate::ZoneIs(PlannerZone::CorpseTarget)],
        effects: vec![StateEffect::SetInteractionDone(true)],
    }]
}

/// 450: plan template for kitten begging. Single-action chain
/// `[BegForFood]` with no zone precondition (the kitten begs in place)
/// and no state effect (Activity Intention per §L2.10.5 — the
/// kitten's hunger doesn't drop *because of* begging; it drops because
/// a parent witnesses the cry-map signal and feeds them via their own
/// Caretake `Intention::Goal(kitten.hunger < threshold)` chain).
///
/// Pattern-B single-trip on `target_completions = 1`: each tick the
/// kitten begs once, the plan completes, the cat re-elects. If
/// `(NewbornKitten | EyesOpenKitten) ∧ ¬HasFoodInInventory ∧ hungry`
/// still holds, the `BegForFoodDse` wins L2 again on the next tick;
/// the re-election rhythm IS the begging cadence. No commitment
/// substrate Component is required — substrate-honest per the
/// "Commitment is one mechanism, not two" pillar.
pub fn begging_actions() -> Vec<GoapActionDef> {
    vec![GoapActionDef {
        kind: GoapActionKind::BegForFood,
        cost: 1,
        preconditions: vec![],
        // Pattern-B single-trip completion: `IncrementTrips` lets the
        // `TripsAtLeast(current_trips + 1)` goal predicate fire on the
        // first `Advance`, so the disposition completes per beg cycle
        // and the cat re-elects on the next tick. (Without
        // `IncrementTrips` the planner would never reach its goal and
        // would burn through `max_replans` trying to find one.)
        effects: vec![StateEffect::IncrementTrips],
    }]
}

/// 367: plan template for loading a Drying Rack with raw fish / raw
/// organ.
///
/// 367 follow-on (split-shape fix): the original single-step
/// `[DryFood]` template required the cat to already carry a dryable
/// item at score-time. On seed-42 cats deposit raw food at Stores
/// immediately on hunt-return, so the per-cat `HasDryableInInventory`
/// marker was off whenever scoring ran and `DryFood` never fired even
/// when a functional rack existed and stores were full of fish. The
/// chain now mirrors `cooking_actions`'s shape:
///
///   `[DropItem?, RetrieveDryable, DryFood]`
///
/// - `RetrieveDryable` pulls one RawFish / RawOrgan from Stores into
///   the cat's `Inventory` and sets `Carrying::RawFood` (search-
///   state). It's distinct from `RetrieveRawFood` because the drying
///   recipes can't accept mammal/bird raw meat.
/// - `DryFood` requires `CarryingIs(Carrying::RawFood)` so A* can
///   prove the cat ends the retrieve step with the right inventory.
///   The runtime resolver picks the specific recipe (DriedFish vs
///   PreservedOrgan) from what's actually carried.
/// - `DropItem` is the same prefix `cooking_actions` uses for cats
///   carrying something incompatible (Herbs / BuildMaterials / etc.);
///   A* skips it for `Carrying::Nothing`.
/// - Cats already carrying a dryable take the substrate-path
///   `RetrieveDryable` arm (cost 2, gated on `HasFreeSlot`), reaching
///   `Carrying::RawFood` via a single retrieve at Stores. The
///   pre-existing-inventory shortcut (skipping retrieve) is
///   deliberately *not* wired here — Carrying projects RawFish to
///   `Carrying::Prey`, not `RawFood`, so a single-step `[DryFood]`
///   admission would require a second precondition arm and a way for
///   A* to know which Prey kinds are dryable. The current shape
///   accepts a one-tick detour to Stores in exchange for plan
///   simplicity; balance follow-on if the detour proves costly.
pub fn drying_food_actions() -> Vec<GoapActionDef> {
    vec![
        // 367 follow-on: DropItem prefix mirrors `cooking_actions`. Sets
        // `HasFreeSlotThisPlan(true)` and `Carrying::Nothing` so the
        // retrieve step's substrate-path or plan-path precondition is
        // satisfied for cats entering the chain carrying anything.
        GoapActionDef {
            kind: GoapActionKind::DropItem,
            cost: 1,
            preconditions: vec![],
            effects: vec![
                StateEffect::SetHasFreeSlotThisPlan(true),
                StateEffect::SetCarrying(Carrying::Nothing),
            ],
        },
        // 367 follow-on: substrate-path retrieve. Gated on the live
        // per-cat `HasFreeSlot` marker. Cost 2 matches
        // `RetrieveRawFood`.
        GoapActionDef {
            kind: GoapActionKind::RetrieveDryable,
            cost: 2,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::HasMarker(crate::components::markers::HasFreeSlot::KEY),
            ],
            effects: vec![StateEffect::SetCarrying(Carrying::RawFood)],
        },
        // 367 follow-on: plan-path retrieve — for cats whose inventory
        // is full at chain entry. The DropItem prefix above sets
        // `HasFreeSlotThisPlan(true)`; this arm consumes that
        // search-state flag (mirrors `cooking_actions`'s dual-arm
        // pattern from ticket 231).
        GoapActionDef {
            kind: GoapActionKind::RetrieveDryable,
            cost: 2,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::HasFreeSlotThisPlan(true),
            ],
            effects: vec![StateEffect::SetCarrying(Carrying::RawFood)],
        },
        GoapActionDef {
            kind: GoapActionKind::DryFood,
            cost: 2,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::DryingRack),
                StatePredicate::CarryingIs(Carrying::RawFood),
            ],
            effects: vec![
                StateEffect::IncrementTrips,
                StateEffect::SetCarrying(Carrying::Nothing),
            ],
        },
    ]
}

/// 443 follow-on: plan template for loading a Smoking Rack.
///
/// Upgraded from single-step `[SmokeMeat]` to a multi-step chain:
///
///   `[DropItem?, RetrieveSmokeable, SmokeMeat]`
///
/// - `RetrieveSmokeable` pulls raw meat AND fuel from Stores into the
///   cat's `Inventory` in a single stores visit. The resolver handles
///   the two-ingredient case: skips items the cat already carries,
///   retrieves missing items. Sets `Carrying::RawFood` search-state
///   to causally connect to `SmokeMeat`'s `CarryingIs(RawFood)`
///   precondition, guaranteeing A* includes the retrieve step.
/// - `SmokeMeat` requires `CarryingIs(Carrying::RawFood)` + `ZoneIs(SmokingRack)`.
///   The runtime resolver consumes both items from inventory.
/// - `DropItem` is the same inventory-clearance prefix as `drying_food_actions`.
///
/// Mirrors `drying_food_actions`'s shape; dual substrate-path /
/// plan-path arms on `RetrieveSmokeable` cover cats with a live free
/// slot and cats whose inventory was full at chain entry.
pub fn smoking_meat_actions() -> Vec<GoapActionDef> {
    vec![
        // Space-clearing prefix — mirrors drying_food_actions.
        GoapActionDef {
            kind: GoapActionKind::DropItem,
            cost: 1,
            preconditions: vec![],
            effects: vec![
                StateEffect::SetHasFreeSlotThisPlan(true),
                StateEffect::SetCarrying(Carrying::Nothing),
            ],
        },
        // Substrate-path retrieve: cat has a live free slot.
        GoapActionDef {
            kind: GoapActionKind::RetrieveSmokeable,
            cost: 2,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::HasMarker(crate::components::markers::HasFreeSlot::KEY),
            ],
            effects: vec![StateEffect::SetCarrying(Carrying::RawFood)],
        },
        // Plan-path retrieve: after DropItem cleared the slot.
        GoapActionDef {
            kind: GoapActionKind::RetrieveSmokeable,
            cost: 2,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::HasFreeSlotThisPlan(true),
            ],
            effects: vec![StateEffect::SetCarrying(Carrying::RawFood)],
        },
        GoapActionDef {
            kind: GoapActionKind::SmokeMeat,
            cost: 2,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::SmokingRack),
                StatePredicate::CarryingIs(Carrying::RawFood),
            ],
            effects: vec![
                StateEffect::IncrementTrips,
                StateEffect::SetCarrying(Carrying::Nothing),
            ],
        },
    ]
}

/// 367: plan template for one tend cycle on a loaded Smoking Rack.
/// Single-step `[TendSmokingRack]`. Eligibility was already gated by
/// the colony marker `HasLoadedSmokingRackOffCooldown`, so by the
/// time A* runs we know at least one such rack exists.
pub fn tend_smoking_rack_actions() -> Vec<GoapActionDef> {
    vec![GoapActionDef {
        kind: GoapActionKind::TendSmokingRack,
        cost: 2,
        preconditions: vec![StatePredicate::ZoneIs(PlannerZone::SmokingRack)],
        effects: vec![StateEffect::IncrementTrips],
    }]
}

/// 457 / 463 commit 8: legacy fallback Crafting template. Emits
/// `CraftAt<Station>(None)` so the resolver lex-picks an inventory-
/// satisfied recipe at execute time (pre-463 behavior). The 463
/// aspiration path uses `craft_have_item_actions` instead, emitting
/// `CraftAt<Station>(Some(recipe.id))` — same action variants but
/// pinned to the held HaveItem's specific recipe. The fallback is
/// the ticket's explicit "belt-and-braces" allowance: every Crafting
/// election without a held HaveItem still lands a craft via lex-pick,
/// instead of failing `GoalUnreachable`.
pub fn crafting_actions() -> Vec<GoapActionDef> {
    vec![
        GoapActionDef {
            kind: GoapActionKind::CraftAtWorkshop(None),
            cost: 2,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Workshop),
                StatePredicate::HasMarker(markers::HasCraftInputInInventory::KEY),
            ],
            effects: vec![StateEffect::IncrementTrips],
        },
        GoapActionDef {
            kind: GoapActionKind::CraftAtTanningFrame(None),
            cost: 2,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::TanningFrame),
                StatePredicate::HasMarker(markers::HasCraftInputInInventory::KEY),
            ],
            effects: vec![StateEffect::IncrementTrips],
        },
    ]
}

/// 463: HaveItem plan template. Used when the cat holds
/// `Intention::Goal(GoalKind::HaveItem(item))` and elected `Crafting`.
/// Returns the action list that includes a `RetrieveCraftInputs(recipe.id)`
/// prefix so A* sequences `[TravelTo(Stores), RetrieveCraftInputs(recipe.id),
/// TravelTo(station), CraftAtStation]` (with an optional `DropItem` head
/// for full-inventory cats).
///
/// Returns an empty Vec when no recipe produces `item` or when the
/// recipe's station lacks a HaveItem-decomposable craft action; A* will
/// then short-circuit with `NoApplicableActions` and the L2 author site
/// records the planning failure (caller handles re-election).
///
/// Mirrors `drying_food_actions` / `smoking_meat_actions`'s dual-arm
/// pattern: substrate-path retrieve fires for cats with a live
/// `HasFreeSlot` marker; plan-path retrieve fires after the
/// `DropItem` prefix sets `HasFreeSlotThisPlan(true)`. Both retrieve
/// arms set `HasCraftInputsThisPlan(true)`, which the plan-path
/// `CraftAt<Station>` arm consumes. The substrate-marker
/// `CraftAt<Station>` arm stays available for cats already carrying
/// recipe inputs (no Stores trip needed).
pub fn craft_have_item_actions(
    item: crate::components::items::ItemKind,
    recipes: &crate::resources::recipe_registry::RecipeRegistry,
    distances: &ZoneDistances,
) -> Vec<GoapActionDef> {
    let Some(recipe) = recipes.recipe_producing(item) else {
        return Vec::new();
    };
    let (station_zone, craft_action) = match recipe.station {
        StationRequirement::Workshop => (
            PlannerZone::Workshop,
            GoapActionKind::CraftAtWorkshop(Some(recipe.id)),
        ),
        StationRequirement::TanningFrame => (
            PlannerZone::TanningFrame,
            GoapActionKind::CraftAtTanningFrame(Some(recipe.id)),
        ),
        // Kitchen / DryingRack / SmokingRack / None have their own
        // dedicated dispositions (Cooking / DryingFood / SmokingMeat).
        // A HaveItem(_) recipe with one of those stations is a
        // registration mistake — return empty so A* short-circuits.
        StationRequirement::None
        | StationRequirement::Kitchen
        | StationRequirement::DryingRack
        | StationRequirement::SmokingRack => return Vec::new(),
    };
    let mut actions = travel_actions(distances);
    actions.push(GoapActionDef {
        // DropItem prefix — clears one slot so the plan-path retrieve
        // can fire on cats whose inventory is full at chain entry.
        // Mirrors `drying_food_actions` / `smoking_meat_actions`.
        kind: GoapActionKind::DropItem,
        cost: 1,
        preconditions: vec![],
        effects: vec![
            StateEffect::SetHasFreeSlotThisPlan(true),
            StateEffect::SetCarrying(Carrying::Nothing),
        ],
    });
    // Substrate-path retrieve: cat has a live free slot — direct
    // retrieve from Stores into inventory.
    actions.push(GoapActionDef {
        kind: GoapActionKind::RetrieveCraftInputs(recipe.id),
        cost: 2,
        preconditions: vec![
            StatePredicate::ZoneIs(PlannerZone::Stores),
            StatePredicate::HasMarker(markers::HasFreeSlot::KEY),
        ],
        effects: vec![StateEffect::SetHasCraftInputsThisPlan(true)],
    });
    // Plan-path retrieve: after `DropItem` cleared the slot.
    actions.push(GoapActionDef {
        kind: GoapActionKind::RetrieveCraftInputs(recipe.id),
        cost: 2,
        preconditions: vec![
            StatePredicate::ZoneIs(PlannerZone::Stores),
            StatePredicate::HasFreeSlotThisPlan(true),
        ],
        effects: vec![StateEffect::SetHasCraftInputsThisPlan(true)],
    });
    // Plan-state `CraftAt<Station>` arm — cat retrieved the SPECIFIC
    // recipe's inputs via `RetrieveCraftInputs(recipe.id)` earlier in
    // this A* expansion. Consumes `HasCraftInputsThisPlan(true)`.
    //
    // The substrate-marker arm (gated on the generic
    // `HasCraftInputInInventory` marker) is intentionally absent: the
    // marker fires when the cat has *any* craft input, but the
    // resolver checks the *specific* recipe's inputs. Without the
    // per-recipe substrate marker, A* would prefer the cheaper
    // substrate-marker path (cost 2 vs 6 for retrieve+craft) and
    // emit a plan that the resolver then fails. Forcing the retrieve
    // path guarantees the cat carries the exact inputs the held
    // HaveItem Intention names. Future ticket: per-recipe substrate
    // marker (`HasInputsFor(recipe_id)`) lets A* skip the retrieve
    // when the cat already has the right inputs.
    actions.push(GoapActionDef {
        kind: craft_action,
        cost: 2,
        preconditions: vec![
            StatePredicate::ZoneIs(station_zone),
            StatePredicate::HasCraftInputsThisPlan(true),
        ],
        effects: vec![StateEffect::IncrementTrips],
    });
    actions
}

/// 364: plan template for an HTN leaf primitive. The L2 frame-pin
/// (#364 commit b) selects this builder when the cat's `HeldGoalStack`
/// pins a `SubGoal::Primitive { action, .. }` — chosen in place of
/// `actions_for_disposition`. Returns `travel_actions(distances)` ∪
/// the single Pattern-B leaf step keyed to the primitive's
/// `GoapActionKind` (mirrors `actions_for_disposition`'s travel + domain
/// union pattern, so A* can satisfy `ZoneIs(...)` preconditions via the
/// per-zone TravelTo legs).
///
/// Routes through `PlannerZone::SocialTarget` for the kitten-arc leaves
/// (Wean / Teach / Release — kittens are alive cats in `cat_positions`)
/// and `PlannerZone::CorpseTarget` for the mourn-arc leaves (Vigil /
/// GriefSit / ReleaseGrief — grave zone routing reuses CorpseTarget
/// until a dedicated GraveTarget zone lands).
///
/// The leaf's effect is `SetInteractionDone(true)`, so the L2 author
/// must pair this with the matching goal predicate
/// `StatePredicate::InteractionDone(true)` (overridden at the
/// `evaluate_and_plan` call site when frame-pinned — `Caretaking`'s
/// `TripsAtLeast` goal won't satisfy via `SetInteractionDone`).
///
/// # Panics
/// Panics on actions that are not HTN primitive leaves; the L2 frame-pin
/// is the only authoritative caller and pre-filters to supported variants.
pub fn htn_primitive_actions(action: Action, distances: &ZoneDistances) -> Vec<GoapActionDef> {
    // 334: `WearItem` dons in place — no travel, no zone gate. Emit the
    // single leaf with empty preconditions (mirrors `discarding_actions`),
    // not the travel + `ZoneIs(zone)` shape the target-bound leaves use.
    if action == Action::WearItem {
        return vec![GoapActionDef {
            kind: GoapActionKind::WearItem,
            cost: 2,
            preconditions: vec![],
            effects: vec![StateEffect::SetInteractionDone(true)],
        }];
    }
    let (kind, zone) = match action {
        Action::Wean => (GoapActionKind::Wean, PlannerZone::SocialTarget),
        Action::Teach => (GoapActionKind::Teach, PlannerZone::SocialTarget),
        Action::Release => (GoapActionKind::Release, PlannerZone::SocialTarget),
        Action::Vigil => (GoapActionKind::Vigil, PlannerZone::CorpseTarget),
        Action::GriefSit => (GoapActionKind::GriefSit, PlannerZone::CorpseTarget),
        Action::ReleaseGrief => (GoapActionKind::ReleaseGrief, PlannerZone::CorpseTarget),
        other => panic!("htn_primitive_actions: unsupported action {other:?}"),
    };
    let mut actions = travel_actions(distances);
    actions.push(GoapActionDef {
        kind,
        cost: 2,
        preconditions: vec![StatePredicate::ZoneIs(zone)],
        effects: vec![StateEffect::SetInteractionDone(true)],
    });
    actions
}

/// 176: single-action template for `Discarding`. No travel — the cat
/// drops one item where they stand. The plan is `[DropItem]`; the
/// resolver removes one slot and spawns an `Item` entity with
/// `ItemLocation::OnGround` at the cat's position. Completion proxy
/// is `IncrementTrips` (matches Hunting/Foraging shape).
pub fn discarding_actions() -> Vec<GoapActionDef> {
    vec![GoapActionDef {
        kind: GoapActionKind::DropItem,
        cost: 1,
        preconditions: vec![],
        effects: vec![
            StateEffect::SetCarrying(Carrying::Nothing),
            StateEffect::IncrementTrips,
        ],
    }]
}

/// 176: plan template for `Trashing` — `[TravelTo(Wilds),
/// TrashItemAtMidden]`. The Midden building is colony-singleton; the
/// `Wilds` zone is a placeholder. Completion proxy is `IncrementTrips`.
///
// STUB(trashing): PlannerZone::Wilds until PlannerZone::Midden lands.
pub fn trashing_actions() -> Vec<GoapActionDef> {
    vec![GoapActionDef {
        kind: GoapActionKind::TrashItemAtMidden,
        cost: 2,
        // Midden has unlimited capacity — no marker gate needed.
        preconditions: vec![StatePredicate::ZoneIs(PlannerZone::Wilds)],
        effects: vec![
            StateEffect::SetCarrying(Carrying::Nothing),
            StateEffect::IncrementTrips,
        ],
    }]
}

/// 176: plan template for `Handing` — `[TravelTo(SocialTarget),
/// HandoffItem]`. Reuses `SocialTarget` zone; the L2 DSE picks the
/// recipient cat and threads it as the disposition's `target_entity`.
pub fn handing_actions() -> Vec<GoapActionDef> {
    vec![GoapActionDef {
        kind: GoapActionKind::HandoffItem,
        cost: 2,
        preconditions: vec![StatePredicate::ZoneIs(PlannerZone::SocialTarget)],
        effects: vec![
            StateEffect::SetCarrying(Carrying::Nothing),
            StateEffect::IncrementTrips,
        ],
    }]
}

/// Plan template for `PickingUp` — single-step retrieval of an
/// OnGround food `Item` (engage_prey overflow today; forward-
/// compatible with future carcass-as-container child Items).
/// Routes through `PlannerZone::CarcassPile` (193); previously
/// stubbed against `MaterialPile`, which filtered to build
/// materials only and starved the resolver of valid targets.
///
/// Ticket 231: dual-branch composition mirrors the ticket-096
/// Construct precedent. `DropItem` is available as a means-to-end
/// prefix (cost 1, sets `HasFreeSlotThisPlan(true)`); the substrate
/// path of `PickUpItemFromGround` reads `HasMarker(HasFreeSlot::KEY)`
/// and the plan path reads `HasFreeSlotThisPlan(true)`. A* picks the
/// cheaper — substrate-path (cost 1) when the cat already has a free
/// slot, plan-path (cost 1+1=2) when full so the cat drops something
/// to make room.
pub fn picking_up_actions() -> Vec<GoapActionDef> {
    vec![
        // 231: DropItem-as-prefix means-to-end action. No
        // IncrementTrips — this is not the disposition's goal, just
        // a step that frees the inventory slot the substrate-path
        // pickup needs. The runtime resolver picks the lowest-priority
        // slot via `drop_priority` (231 H, goal-aware).
        GoapActionDef {
            kind: GoapActionKind::DropItem,
            cost: 1,
            preconditions: vec![],
            effects: vec![
                StateEffect::SetHasFreeSlotThisPlan(true),
                StateEffect::SetCarrying(Carrying::Nothing),
            ],
        },
        // 235: DepositHerbs-as-prefix means-to-end action. Same
        // free-slot effect as DropItem above, but routed through the
        // herb stash so the cat's carried herbs land usefully instead
        // of in the dirt. Gated on `HasHerbStashAccessible` so cats
        // far from any Stores fall back to DropItem. A* splices
        // `TravelTo(Stores)` from `travel_actions` automatically to
        // satisfy `ZoneIs(Stores)`; effective cost is
        // `travel_to_stores + 1`, which wins over DropItem (cost 1)
        // only when the cat is at/near the stash.
        GoapActionDef {
            kind: GoapActionKind::DepositHerbs,
            cost: 1,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::CarryingIs(Carrying::Herbs),
                StatePredicate::HasMarker(crate::components::markers::HasHerbStashAccessible::KEY),
                StatePredicate::HasMarker(crate::components::markers::HasHerbsInInventory::KEY),
            ],
            effects: vec![
                StateEffect::SetHasFreeSlotThisPlan(true),
                StateEffect::SetCarrying(Carrying::Nothing),
            ],
        },
        // 231: substrate-path pickup. Fires when the cat already has a
        // free slot.
        GoapActionDef {
            kind: GoapActionKind::PickUpItemFromGround,
            cost: 1,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::CarcassPile),
                StatePredicate::HasMarker(crate::components::markers::HasFreeSlot::KEY),
            ],
            effects: vec![StateEffect::IncrementTrips],
        },
        // 231: plan-path pickup. Fires after a DropItem prefix step
        // sets `HasFreeSlotThisPlan(true)` in this plan's search state.
        GoapActionDef {
            kind: GoapActionKind::PickUpItemFromGround,
            cost: 1,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::CarcassPile),
                StatePredicate::HasFreeSlotThisPlan(true),
            ],
            effects: vec![StateEffect::IncrementTrips],
        },
    ]
}

/// Building plans a haul→deliver→construct sequence. The planner emits
/// `[TravelTo(MaterialPile), GatherMaterials, TravelTo(ConstructionSite),
/// DeliverMaterials, Construct]` for an unfunded site with reachable
/// material piles. Multi-trip delivery is handled via iterative replanning.
///
/// Ticket 096: the world-fact half ("a reachable site has
/// `materials_complete()` true") lives in the substrate as the
/// `MaterialsAvailable` marker, authored each tick by
/// `goap.rs::build_planner_markers`. The search-state half ("this plan
/// has executed a Deliver") lives in `PlannerState.materials_delivered_this_plan`,
/// flipped by `SetMaterialsDeliveredThisPlan(true)`. Two `Construct`
/// action defs accept either branch — substrate-path for prefunded sites,
/// plan-path for in-flight haul→deliver cycles.
pub fn building_actions() -> Vec<GoapActionDef> {
    vec![
        // Pickup: cat at a material pile → carrying build materials.
        // Real-world effect (in the executor) is item.location →
        // Carried(cat) and an Inventory slot insert.
        // 175: dropped `CarryingIs(Nothing)` veto for symmetry with
        // hunting/foraging/cooking/herbalism. Construction's
        // `GoalUnreachable` count was 0 in the post-172 soak so this
        // didn't surface, but the veto is the same shape.
        GoapActionDef {
            kind: GoapActionKind::GatherMaterials,
            cost: 3,
            preconditions: vec![StatePredicate::ZoneIs(PlannerZone::MaterialPile)],
            effects: vec![StateEffect::SetCarrying(Carrying::BuildMaterials)],
        },
        // Deliver: cat at the site carrying materials → drops one unit
        // into the site's ledger. Marks the search-state field
        // `materials_delivered_this_plan` so the subsequent `Construct`
        // step is applicable inside the same A* expansion. The next
        // state author rereads from ECS, so a single Deliver that
        // doesn't fully fund the site triggers another haul cycle on
        // replan.
        GoapActionDef {
            kind: GoapActionKind::DeliverMaterials,
            cost: 1,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::ConstructionSite),
                StatePredicate::CarryingIs(Carrying::BuildMaterials),
            ],
            effects: vec![
                StateEffect::SetCarrying(Carrying::Nothing),
                StateEffect::SetMaterialsDeliveredThisPlan(true),
                StateEffect::IncrementTrips,
            ],
        },
        // Construct (substrate path): the world already has materials
        // ready at a reachable site (prefunded coordinator-spawned sites,
        // or a previous tick's haul completed funding). Gates on the
        // `MaterialsAvailable` marker.
        GoapActionDef {
            kind: GoapActionKind::Construct,
            cost: 6,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::ConstructionSite),
                StatePredicate::HasMarker(markers::MaterialsAvailable::KEY),
            ],
            effects: vec![StateEffect::SetConstructionDone(true)],
        },
        // Construct (plan-path): this plan delivered materials earlier
        // in the same A* expansion. Lets `[..., Deliver, Construct]`
        // compose without depending on the substrate marker (which is
        // false for unfunded founding sites until the deliver lands).
        GoapActionDef {
            kind: GoapActionKind::Construct,
            cost: 6,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::ConstructionSite),
                StatePredicate::MaterialsDeliveredThisPlan(true),
            ],
            effects: vec![StateEffect::SetConstructionDone(true)],
        },
    ]
}

pub fn farming_actions() -> Vec<GoapActionDef> {
    vec![
        GoapActionDef {
            kind: GoapActionKind::TendCrops,
            cost: 2,
            preconditions: vec![StatePredicate::ZoneIs(PlannerZone::Farm)],
            effects: vec![StateEffect::SetFarmTended(true)],
        },
        GoapActionDef {
            kind: GoapActionKind::HarvestCrops,
            cost: 2,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Farm),
                StatePredicate::FarmTended(true),
            ],
            effects: vec![StateEffect::IncrementTrips],
        },
    ]
}

/// 155: Herbalism plan-template dispatcher. The chosen sub-action
/// (one of `HerbcraftGather` / `HerbcraftRemedy` / `HerbcraftSetWard`)
/// determines which chain shape the planner sees. Falls back to the
/// single-action gather plan if a non-Herbalism Action is supplied —
/// the caller is responsible for routing correctly via
/// `actions_for_disposition`.
pub fn herbalism_actions(chosen_action: Action) -> Vec<GoapActionDef> {
    match chosen_action {
        Action::HerbcraftGather => vec![
            // 231: DropItem-as-prefix means-to-end. Cat with full
            // inventory drops a slot, then gathers the herb.
            GoapActionDef {
                kind: GoapActionKind::DropItem,
                cost: 1,
                preconditions: vec![],
                effects: vec![
                    StateEffect::SetHasFreeSlotThisPlan(true),
                    StateEffect::SetCarrying(Carrying::Nothing),
                ],
            },
            // 235: DepositHerbs-as-prefix alternative — the cat
            // routes through Stores to stash carried herbs rather
            // than dropping them at HerbPatch. A* distinguishes
            // this from the terminal DepositHerbs below by effects:
            // the prefix omits `IncrementTrips` so the goal isn't
            // satisfied here. State-dedup keeps both branches alive
            // when their post-states diverge (trip count).
            GoapActionDef {
                kind: GoapActionKind::DepositHerbs,
                cost: 1,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::Stores),
                    StatePredicate::CarryingIs(Carrying::Herbs),
                    StatePredicate::HasMarker(
                        crate::components::markers::HasHerbStashAccessible::KEY,
                    ),
                    StatePredicate::HasMarker(crate::components::markers::HasHerbsInInventory::KEY),
                ],
                effects: vec![
                    StateEffect::SetHasFreeSlotThisPlan(true),
                    StateEffect::SetCarrying(Carrying::Nothing),
                ],
            },
            // 084 Commit 2: Gather steps no longer trip-increment.
            // The plan now terminates at Stores with DepositHerbs.
            // The dual-branch substrate/plan-path mirroring stays
            // (231's free-slot composition).
            //
            // 231: substrate-path GatherHerb — cat already has space.
            GoapActionDef {
                kind: GoapActionKind::GatherHerb,
                cost: 3,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::HerbPatch),
                    StatePredicate::HasMarker(crate::components::markers::HasFreeSlot::KEY),
                ],
                effects: vec![StateEffect::SetCarrying(Carrying::Herbs)],
            },
            // 231: plan-path GatherHerb — composes after DropItem.
            GoapActionDef {
                kind: GoapActionKind::GatherHerb,
                cost: 3,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::HerbPatch),
                    StatePredicate::HasFreeSlotThisPlan(true),
                ],
                effects: vec![StateEffect::SetCarrying(Carrying::Herbs)],
            },
            // 084 Commit 2: DepositHerbs terminates the gather chain.
            // The planner sequences `[TravelTo(HerbPatch) → GatherHerb
            // → TravelTo(Stores) → DepositHerbs]` (TravelTo legs come
            // from travel_actions). The trip-increment effect satisfies
            // the `TripsAtLeast(current_trips + 1)` Herbalism goal so
            // the chain only completes after the cat has actually
            // deposited at the stash.
            GoapActionDef {
                kind: GoapActionKind::DepositHerbs,
                cost: 1,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::Stores),
                    StatePredicate::CarryingIs(Carrying::Herbs),
                ],
                effects: vec![
                    StateEffect::SetCarrying(Carrying::Nothing),
                    StateEffect::IncrementTrips,
                ],
            },
        ],
        Action::HerbcraftRemedy => vec![
            // Gather herbs first if not carrying any.
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::HerbPatch),
                cost: 2,
                preconditions: vec![StatePredicate::ZoneIsNot(PlannerZone::HerbPatch)],
                effects: vec![StateEffect::SetZone(PlannerZone::HerbPatch)],
            },
            // 231: DropItem-as-prefix means-to-end.
            GoapActionDef {
                kind: GoapActionKind::DropItem,
                cost: 1,
                preconditions: vec![],
                effects: vec![
                    StateEffect::SetHasFreeSlotThisPlan(true),
                    StateEffect::SetCarrying(Carrying::Nothing),
                ],
            },
            // 235: DepositHerbs-as-prefix alternative — route through
            // Stores instead of dropping at HerbPatch.
            GoapActionDef {
                kind: GoapActionKind::DepositHerbs,
                cost: 1,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::Stores),
                    StatePredicate::CarryingIs(Carrying::Herbs),
                    StatePredicate::HasMarker(
                        crate::components::markers::HasHerbStashAccessible::KEY,
                    ),
                    StatePredicate::HasMarker(crate::components::markers::HasHerbsInInventory::KEY),
                ],
                effects: vec![
                    StateEffect::SetHasFreeSlotThisPlan(true),
                    StateEffect::SetCarrying(Carrying::Nothing),
                ],
            },
            // 231: substrate-path GatherHerb.
            GoapActionDef {
                kind: GoapActionKind::GatherHerb,
                cost: 3,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::HerbPatch),
                    StatePredicate::HasMarker(crate::components::markers::HasFreeSlot::KEY),
                ],
                effects: vec![StateEffect::SetCarrying(Carrying::Herbs)],
            },
            // 231: plan-path GatherHerb (after DropItem).
            GoapActionDef {
                kind: GoapActionKind::GatherHerb,
                cost: 3,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::HerbPatch),
                    StatePredicate::HasFreeSlotThisPlan(true),
                ],
                effects: vec![StateEffect::SetCarrying(Carrying::Herbs)],
            },
            GoapActionDef {
                kind: GoapActionKind::PrepareRemedy,
                cost: 3,
                preconditions: vec![StatePredicate::CarryingIs(Carrying::Herbs)],
                effects: vec![StateEffect::SetCarrying(Carrying::Remedy)],
            },
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::SocialTarget),
                cost: 2,
                preconditions: vec![StatePredicate::ZoneIsNot(PlannerZone::SocialTarget)],
                effects: vec![StateEffect::SetZone(PlannerZone::SocialTarget)],
            },
            GoapActionDef {
                kind: GoapActionKind::ApplyRemedy,
                cost: 2,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::SocialTarget),
                    StatePredicate::CarryingIs(Carrying::Remedy),
                ],
                effects: vec![
                    StateEffect::SetCarrying(Carrying::Nothing),
                    StateEffect::IncrementTrips,
                ],
            },
        ],
        Action::HerbcraftSetWard => vec![
            // Gather herbs first if not carrying any.
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::HerbPatch),
                cost: 2,
                preconditions: vec![StatePredicate::ZoneIsNot(PlannerZone::HerbPatch)],
                effects: vec![StateEffect::SetZone(PlannerZone::HerbPatch)],
            },
            // 231: DropItem-as-prefix means-to-end.
            GoapActionDef {
                kind: GoapActionKind::DropItem,
                cost: 1,
                preconditions: vec![],
                effects: vec![
                    StateEffect::SetHasFreeSlotThisPlan(true),
                    StateEffect::SetCarrying(Carrying::Nothing),
                ],
            },
            // 235: DepositHerbs-as-prefix alternative — route through
            // Stores. For SetWard, the cat may already pass through
            // Stores to RetrieveHerbs(Thornbriar); the deposit prefix
            // composes cleanly with that retrieval branch.
            GoapActionDef {
                kind: GoapActionKind::DepositHerbs,
                cost: 1,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::Stores),
                    StatePredicate::CarryingIs(Carrying::Herbs),
                    StatePredicate::HasMarker(
                        crate::components::markers::HasHerbStashAccessible::KEY,
                    ),
                    StatePredicate::HasMarker(crate::components::markers::HasHerbsInInventory::KEY),
                ],
                effects: vec![
                    StateEffect::SetHasFreeSlotThisPlan(true),
                    StateEffect::SetCarrying(Carrying::Nothing),
                ],
            },
            // 231: substrate-path GatherHerb (with ThornbriarAvailable
            // ecological gate retained from 175).
            GoapActionDef {
                kind: GoapActionKind::GatherHerb,
                cost: 3,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::HerbPatch),
                    StatePredicate::HasMarker(markers::ThornbriarAvailable::KEY),
                    StatePredicate::HasMarker(crate::components::markers::HasFreeSlot::KEY),
                ],
                effects: vec![StateEffect::SetCarrying(Carrying::Herbs)],
            },
            // 231: plan-path GatherHerb (after DropItem).
            GoapActionDef {
                kind: GoapActionKind::GatherHerb,
                cost: 3,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::HerbPatch),
                    StatePredicate::HasMarker(markers::ThornbriarAvailable::KEY),
                    StatePredicate::HasFreeSlotThisPlan(true),
                ],
                effects: vec![StateEffect::SetCarrying(Carrying::Herbs)],
            },
            // 084 Commit 2: retrieve-path RetrieveHerbs(Thornbriar) —
            // the cat picks up a stashed thornbriar from Stores rather
            // than gathering wild. A* picks whichever chain
            // (gather-from-wild vs retrieve-from-stash) is cheaper given
            // ZoneDistances. Substrate-path: free slot available.
            GoapActionDef {
                kind: GoapActionKind::RetrieveHerbs(crate::components::magic::HerbKind::Thornbriar),
                cost: 2,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::Stores),
                    StatePredicate::HasMarker(markers::HasStoredThornbriar::KEY),
                    StatePredicate::HasMarker(crate::components::markers::HasFreeSlot::KEY),
                ],
                effects: vec![StateEffect::SetCarrying(Carrying::Herbs)],
            },
            // 084 Commit 2: plan-path RetrieveHerbs after DropItem.
            GoapActionDef {
                kind: GoapActionKind::RetrieveHerbs(crate::components::magic::HerbKind::Thornbriar),
                cost: 2,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::Stores),
                    StatePredicate::HasMarker(markers::HasStoredThornbriar::KEY),
                    StatePredicate::HasFreeSlotThisPlan(true),
                ],
                effects: vec![StateEffect::SetCarrying(Carrying::Herbs)],
            },
            GoapActionDef {
                kind: GoapActionKind::SetWard,
                cost: 3,
                preconditions: vec![StatePredicate::CarryingIs(Carrying::Herbs)],
                effects: vec![
                    StateEffect::SetCarrying(Carrying::Nothing),
                    StateEffect::IncrementTrips,
                ],
            },
        ],
        // Defensive: if a non-Herbalism Action somehow reaches here, return
        // the cheap single-action gather plan rather than panic.
        // 175: dropped `CarryingIs(Nothing)` veto for symmetry.
        // 231: dual-branch substrate/plan-path mirroring HerbcraftGather.
        _ => vec![
            GoapActionDef {
                kind: GoapActionKind::DropItem,
                cost: 1,
                preconditions: vec![],
                effects: vec![
                    StateEffect::SetHasFreeSlotThisPlan(true),
                    StateEffect::SetCarrying(Carrying::Nothing),
                ],
            },
            GoapActionDef {
                kind: GoapActionKind::GatherHerb,
                cost: 3,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::HerbPatch),
                    StatePredicate::HasMarker(crate::components::markers::HasFreeSlot::KEY),
                ],
                effects: vec![
                    StateEffect::SetCarrying(Carrying::Herbs),
                    StateEffect::IncrementTrips,
                ],
            },
            GoapActionDef {
                kind: GoapActionKind::GatherHerb,
                cost: 3,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::HerbPatch),
                    StatePredicate::HasFreeSlotThisPlan(true),
                ],
                effects: vec![
                    StateEffect::SetCarrying(Carrying::Herbs),
                    StateEffect::IncrementTrips,
                ],
            },
        ],
    }
}

/// 155: Witchcraft plan-template dispatcher. Each chosen sub-action
/// produces a single-action plan whose IncrementTrips effect satisfies
/// the goal proxy. The pre-155 `CraftingHint::Magic` 5-action pool
/// (where A* picked the cheapest) collapses into per-sub-action L3
/// scoring — the softmax now picks Scry vs Commune vs Cleanse etc.
/// directly rather than letting A* re-decide post-hoc.
pub fn witchcraft_actions(chosen_action: Action) -> Vec<GoapActionDef> {
    let kind = match chosen_action {
        Action::MagicScry => GoapActionKind::Scry,
        Action::MagicCommune => GoapActionKind::SpiritCommunion,
        Action::MagicCleanse | Action::MagicColonyCleanse => GoapActionKind::CleanseCorruption,
        Action::MagicHarvest => GoapActionKind::HarvestCarcass,
        // MagicDurableWard maps to SetWard — the resolver picks
        // WardKind::DurableWard based on chosen_action.
        Action::MagicDurableWard => GoapActionKind::SetWard,
        // Defensive fallback for non-Witchcraft Actions.
        _ => GoapActionKind::Scry,
    };
    vec![GoapActionDef {
        kind,
        cost: 1,
        preconditions: vec![],
        effects: vec![StateEffect::IncrementTrips],
    }]
}

/// 155: Cooking plan-template — the round-trip Stores → Kitchen →
/// Stores chain. Travel legs come from `travel_actions` (zone
/// distance matrix); these three actions transition Carrying between
/// Nothing → RawFood → CookedFood → Nothing. Only `DepositCookedFood`
/// terminates with `IncrementTrips` — that forces A* through the
/// full chain.
pub fn cooking_actions() -> Vec<GoapActionDef> {
    vec![
        // 175: chain-entry `CarryingIs(Carrying::Nothing)` veto
        // dropped for the same reason 091 dropped it from
        // hunting/foraging — the runtime resolver gates on
        // `inventory.is_full()` (now via the typed transfer
        // primitive in `components::item_transfer`), and the
        // planner's `Carrying` is a coarse projection of the
        // multi-slot inventory. Cats with leftover items can
        // enter the chain; A* still skips this step when the cat
        // already has `RawFood` and enters at `Cook`.
        //
        // 231: dual-branch substrate-vs-plan-path on `RetrieveRawFood`.
        // The substrate branch reads `HasMarker(HasFreeSlot::KEY)`; the
        // plan branch reads `HasFreeSlotThisPlan(true)`, set by the
        // DropItem-as-prefix step. A* picks substrate (cost 2) when a
        // free slot exists, plan (cost 1+2=3) when full so the cat
        // drops something before retrieving.
        GoapActionDef {
            kind: GoapActionKind::DropItem,
            cost: 1,
            preconditions: vec![],
            effects: vec![
                StateEffect::SetHasFreeSlotThisPlan(true),
                StateEffect::SetCarrying(Carrying::Nothing),
            ],
        },
        // 235: DepositHerbs-as-prefix alternative — herbs land at the
        // stash instead of the kitchen approach tile. Gated on
        // HasHerbStashAccessible; falls back to DropItem when stash
        // is out of range.
        GoapActionDef {
            kind: GoapActionKind::DepositHerbs,
            cost: 1,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::CarryingIs(Carrying::Herbs),
                StatePredicate::HasMarker(crate::components::markers::HasHerbStashAccessible::KEY),
                StatePredicate::HasMarker(crate::components::markers::HasHerbsInInventory::KEY),
            ],
            effects: vec![
                StateEffect::SetHasFreeSlotThisPlan(true),
                StateEffect::SetCarrying(Carrying::Nothing),
            ],
        },
        GoapActionDef {
            kind: GoapActionKind::RetrieveRawFood,
            cost: 2,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::HasMarker(crate::components::markers::HasFreeSlot::KEY),
            ],
            effects: vec![StateEffect::SetCarrying(Carrying::RawFood)],
        },
        GoapActionDef {
            kind: GoapActionKind::RetrieveRawFood,
            cost: 2,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::HasFreeSlotThisPlan(true),
            ],
            effects: vec![StateEffect::SetCarrying(Carrying::RawFood)],
        },
        GoapActionDef {
            kind: GoapActionKind::Cook,
            cost: 3,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Kitchen),
                StatePredicate::CarryingIs(Carrying::RawFood),
            ],
            effects: vec![StateEffect::SetCarrying(Carrying::CookedFood)],
        },
        GoapActionDef {
            kind: GoapActionKind::DepositCookedFood,
            cost: 1,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::CarryingIs(Carrying::CookedFood),
            ],
            effects: vec![
                StateEffect::SetCarrying(Carrying::Nothing),
                StateEffect::IncrementTrips,
            ],
        },
    ]
}

pub fn coordinating_actions() -> Vec<GoapActionDef> {
    vec![GoapActionDef {
        kind: GoapActionKind::DeliverDirective,
        cost: 2,
        preconditions: vec![StatePredicate::ZoneIs(PlannerZone::SocialTarget)],
        effects: vec![
            StateEffect::SetInteractionDone(true),
            StateEffect::IncrementTrips,
        ],
    }]
}

pub fn exploring_actions() -> Vec<GoapActionDef> {
    vec![GoapActionDef {
        kind: GoapActionKind::ExploreSurvey,
        cost: 2,
        preconditions: vec![StatePredicate::ZoneIs(PlannerZone::Wilds)],
        effects: vec![StateEffect::IncrementTrips],
    }]
}

pub fn mating_actions() -> Vec<GoapActionDef> {
    vec![GoapActionDef {
        kind: GoapActionKind::MateWith,
        cost: 2,
        preconditions: vec![StatePredicate::ZoneIs(PlannerZone::SocialTarget)],
        effects: vec![StateEffect::SetInteractionDone(true)],
    }]
}

pub fn caretaking_actions() -> Vec<GoapActionDef> {
    // Phase 4c.4: two-step retrieve→feed chain. Before this fix the
    // planner emitted `[TravelTo(Stores), FeedKitten]` which silently
    // no-op'd because `resolve_feed_kitten` calls `inventory.take_food()`
    // with an empty inventory and advances anyway — kittens never got
    // fed. Carrying::RawFood is used as the abstract "I have food"
    // state even though the retrieve accepts cooked food too (the
    // planner doesn't need to distinguish; only the real ECS inventory
    // matters at execution time).
    //
    // RetrieveFoodForKitten intentionally has no `CarryingIs(Nothing)`
    // precondition — a cat arriving at Stores with herbs, foraged food,
    // or other inventory contents still produces a valid plan (the
    // planner's `Carrying` state is a coarse abstraction over a
    // richer real inventory; `inventory.add_item_with_modifiers` just
    // appends another slot at runtime, and `take_food` picks any
    // food-typed item). A first pass *did* include that precondition,
    // which caused 0 Caretake plans in post-fix soaks: whenever a cat's
    // real inventory was non-empty the planner couldn't satisfy
    // `CarryingIs(Nothing)` and bailed out entirely.
    vec![
        // 231: DropItem-as-prefix + dual-branch RetrieveFoodForKitten.
        // Adult cats with full inventory drop a slot first (resolver
        // picks the lowest-priority slot), then retrieve food for the
        // kitten. Mirrors the cooking_actions / picking_up_actions
        // composition.
        GoapActionDef {
            kind: GoapActionKind::DropItem,
            cost: 1,
            preconditions: vec![],
            effects: vec![
                StateEffect::SetHasFreeSlotThisPlan(true),
                StateEffect::SetCarrying(Carrying::Nothing),
            ],
        },
        // 235: DepositHerbs-as-prefix alternative when the cat is
        // carrying herbs and a stash is reachable.
        GoapActionDef {
            kind: GoapActionKind::DepositHerbs,
            cost: 1,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::CarryingIs(Carrying::Herbs),
                StatePredicate::HasMarker(crate::components::markers::HasHerbStashAccessible::KEY),
                StatePredicate::HasMarker(crate::components::markers::HasHerbsInInventory::KEY),
            ],
            effects: vec![
                StateEffect::SetHasFreeSlotThisPlan(true),
                StateEffect::SetCarrying(Carrying::Nothing),
            ],
        },
        GoapActionDef {
            kind: GoapActionKind::RetrieveFoodForKitten,
            cost: 2,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::HasMarker(crate::components::markers::HasFreeSlot::KEY),
            ],
            effects: vec![StateEffect::SetCarrying(Carrying::RawFood)],
        },
        GoapActionDef {
            kind: GoapActionKind::RetrieveFoodForKitten,
            cost: 2,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::HasFreeSlotThisPlan(true),
            ],
            effects: vec![StateEffect::SetCarrying(Carrying::RawFood)],
        },
        GoapActionDef {
            kind: GoapActionKind::FeedKitten,
            cost: 2,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::Stores),
                StatePredicate::CarryingIs(Carrying::RawFood),
            ],
            effects: vec![
                StateEffect::SetCarrying(Carrying::Nothing),
                StateEffect::IncrementTrips,
            ],
        },
    ]
}

// ---------------------------------------------------------------------------
// Aggregate: collect all actions for a disposition
// ---------------------------------------------------------------------------

use crate::components::disposition::DispositionKind;

/// Build the full action set for a given disposition, including travel actions.
///
/// 155: `chosen_action` replaces the retired `crafting_hint` parameter.
/// It carries the sub-action the L3 softmax picked; for Herbalism /
/// Witchcraft / Cooking the per-Disposition dispatcher branches on it
/// to select the chain shape (which step terminates with
/// `IncrementTrips`). For all other dispositions it's unused — they
/// have a single constituent action.
pub fn actions_for_disposition(
    kind: DispositionKind,
    chosen_action: Action,
    distances: &ZoneDistances,
) -> Vec<GoapActionDef> {
    let mut actions = travel_actions(distances);
    let domain_actions = match kind {
        DispositionKind::Hunting => hunting_actions(),
        DispositionKind::Foraging => foraging_actions(),
        DispositionKind::Resting => resting_actions(),
        DispositionKind::Eating => eating_actions(),
        DispositionKind::Guarding => guarding_actions(),
        DispositionKind::Socializing => socializing_actions(),
        DispositionKind::Building => building_actions(),
        DispositionKind::Farming => farming_actions(),
        DispositionKind::Herbalism => herbalism_actions(chosen_action),
        DispositionKind::Witchcraft => witchcraft_actions(chosen_action),
        DispositionKind::Cooking => cooking_actions(),
        DispositionKind::Coordinating => coordinating_actions(),
        DispositionKind::Exploring => exploring_actions(),
        DispositionKind::Mating => mating_actions(),
        DispositionKind::Caretaking => caretaking_actions(),
        DispositionKind::Mentoring => mentoring_actions(),
        DispositionKind::Grooming => grooming_actions(),
        // 035: burial plan template.
        DispositionKind::Burying => burying_actions(),
        // 176: inventory-disposal plan templates.
        DispositionKind::Discarding => discarding_actions(),
        DispositionKind::Trashing => trashing_actions(),
        DispositionKind::Handing => handing_actions(),
        DispositionKind::PickingUp => picking_up_actions(),
        // 230: Fleeing plan template.
        DispositionKind::Fleeing => fleeing_actions(),
        // 367: preservation plan templates.
        DispositionKind::DryingFood => drying_food_actions(),
        DispositionKind::SmokingMeat => smoking_meat_actions(),
        DispositionKind::TendingSmokingRack => tend_smoking_rack_actions(),
        // 450: Begging plan template — single action, no preconditions,
        // no state effect (Activity Intention, not Goal — §L2.10.5).
        DispositionKind::Begging => begging_actions(),
        // 457: Workshop-craft plan template — single `CraftAtWorkshop`
        // step over `ZoneIs(Workshop) ∧ HasCraftInputInInventory`.
        DispositionKind::Crafting => crafting_actions(),
    };
    actions.extend(domain_actions);
    actions
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::planner::{
        make_plan, Carrying, GoalState, PlanContext, PlannerState, PlannerZone,
    };
    use crate::ai::scoring::MarkerSnapshot;
    use bevy::prelude::Entity;

    fn default_state() -> PlannerState {
        PlannerState {
            zone: PlannerZone::Wilds,
            carrying: Carrying::Nothing,
            trips_done: 0,
            hunger_ok: true,
            energy_ok: true,
            temperature_ok: true,
            interaction_done: false,
            construction_done: false,
            prey_found: false,
            farm_tended: false,
            materials_delivered_this_plan: false,
            flee_target_picked: false,
            has_free_slot_this_plan: false,
            has_craft_inputs_this_plan: false,
        }
    }

    fn empty_markers() -> MarkerSnapshot {
        MarkerSnapshot::new()
    }

    /// Test markers with `MaterialsAvailable` set — exercises the
    /// substrate branch of `Construct` (prefunded site).
    fn materials_available_markers() -> MarkerSnapshot {
        let mut m = food_stocked_markers();
        m.set_entity(markers::MaterialsAvailable::KEY, test_entity(), true);
        m
    }

    /// Default test context: stores have food and the cat has at least
    /// one free inventory slot. Most tests assume the colony is
    /// provisioned so `EatAtStores` is reachable, and that the cat
    /// isn't clogged so the substrate-path of pickup-class actions
    /// (231) fires (vs the plan-path which prepends DropItem). Tests
    /// that explicitly probe empty-stores or full-inventory behavior
    /// pass `empty_markers()` or build their own snapshot.
    fn food_stocked_markers() -> MarkerSnapshot {
        let mut m = MarkerSnapshot::new();
        m.set_colony(markers::HasStoredFood::KEY, true);
        m.set_entity(markers::HasFreeSlot::KEY, test_entity(), true);
        m
    }

    fn thornbriar_markers() -> MarkerSnapshot {
        let mut m = food_stocked_markers();
        m.set_colony(markers::ThornbriarAvailable::KEY, true);
        m
    }

    /// 084: stash-only marker snapshot — colony has stashed thornbriar
    /// but no wild thornbriar available. Used to exercise the
    /// retrieve-path branch of `HerbcraftSetWard`.
    fn stored_thornbriar_markers() -> MarkerSnapshot {
        let mut m = food_stocked_markers();
        m.set_colony(markers::HasStoredThornbriar::KEY, true);
        m
    }

    fn test_entity() -> Entity {
        Entity::from_raw_u32(1).expect("nonzero raw entity id")
    }

    /// Run `make_plan` with a `PlanContext` built from the given marker
    /// snapshot. Default form (no `markers = …`) uses `food_stocked_markers`.
    macro_rules! plan {
        ($start:expr, $actions:expr, $goal:expr, $depth:expr, $nodes:expr, markers = $m:expr) => {{
            let markers = $m;
            let ctx = PlanContext {
                markers: &markers,
                entity: test_entity(),
            };
            let mut scratch = crate::ai::planner::CatPlannerScratch::default();
            make_plan($start, $actions, $goal, $depth, $nodes, &ctx, &mut scratch)
        }};
        ($start:expr, $actions:expr, $goal:expr, $depth:expr, $nodes:expr) => {{
            plan!(
                $start,
                $actions,
                $goal,
                $depth,
                $nodes,
                markers = food_stocked_markers()
            )
        }};
    }

    fn basic_distances() -> ZoneDistances {
        let mut d = ZoneDistances::default();
        let zones = [
            PlannerZone::Stores,
            PlannerZone::HuntingGround,
            PlannerZone::ForagingGround,
            PlannerZone::Farm,
            PlannerZone::ConstructionSite,
            PlannerZone::HerbPatch,
            PlannerZone::Kitchen,
            PlannerZone::RestingSpot,
            PlannerZone::SocialTarget,
            PlannerZone::Wilds,
            PlannerZone::PatrolZone,
            PlannerZone::MaterialPile,
        ];
        // Set uniform distance of 2 between all distinct zone pairs.
        for &from in &zones {
            for &to in &zones {
                if from != to {
                    d.set(from, to, 2);
                }
            }
        }
        d
    }

    #[test]
    fn hunting_full_trip() {
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Hunting, Action::Hunt, &distances);

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("plan found");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![
                GoapActionKind::TravelTo(PlannerZone::HuntingGround),
                GoapActionKind::SearchPrey,
                GoapActionKind::EngagePrey,
                GoapActionKind::TravelTo(PlannerZone::Stores),
                GoapActionKind::DepositPrey,
            ]
        );
    }

    #[test]
    fn foraging_full_trip() {
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions =
            actions_for_disposition(DispositionKind::Foraging, Action::Forage, &distances);

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("plan found");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![
                GoapActionKind::TravelTo(PlannerZone::ForagingGround),
                GoapActionKind::ForageItem,
                GoapActionKind::TravelTo(PlannerZone::Stores),
                GoapActionKind::DepositFood,
            ]
        );
    }

    #[test]
    fn resting_addresses_energy_and_temperature() {
        // 150 R5a: Resting plan is Sleep + SelfGroom. EatAtStores is
        // owned by Eating's plan template — it must NOT appear here.
        let start = PlannerState {
            energy_ok: false,
            temperature_ok: false,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![
                StatePredicate::EnergyOk(true),
                StatePredicate::TemperatureOk(true),
            ],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Resting, Action::Sleep, &distances);

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("plan found");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&GoapActionKind::Sleep));
        assert!(kinds.contains(&GoapActionKind::SelfGroom));
        assert!(
            !kinds.contains(&GoapActionKind::EatAtStores),
            "Resting plan must not include EatAtStores post-150 R5a"
        );
    }

    #[test]
    fn eating_plans_eat_at_stores_when_stocked() {
        // 150 R5a sibling test: Eating's plan template is
        // [TravelTo(Stores), EatAtStores]. The marker-eligibility on
        // `HasStoredFood` is exercised in
        // `eating_unreachable_when_stores_empty`.
        let start = PlannerState {
            hunger_ok: false,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::HungerOk(true)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Eating, Action::Eat, &distances);

        let plan = plan!(start, &actions, &goal, 8, 500)
            .expect("Eating must plan a chain when stores are stocked");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&GoapActionKind::EatAtStores));
        assert!(kinds.contains(&GoapActionKind::TravelTo(PlannerZone::Stores)));
    }

    #[test]
    fn eating_unreachable_when_stores_empty() {
        // 150 R5a: with HasStoredFood absent, EatAtStores has no valid
        // precondition path. The planner returns None — the cat
        // re-elects (Hunt or Forage become the productive paths).
        // Mirrors the 091/092 substrate-marker discipline.
        let start = PlannerState {
            hunger_ok: false,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::HungerOk(true)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Eating, Action::Eat, &distances);
        assert!(
            plan!(start, &actions, &goal, 8, 500, markers = empty_markers()).is_err(),
            "Eating plan must be unreachable when HasStoredFood marker is absent"
        );
    }

    #[test]
    fn resting_independent_of_stores_marker() {
        // 150 R5a: Resting plans Sleep + SelfGroom regardless of stores
        // state. The 091/092 marker-gated partial-goal dance was
        // retired when Eating took over hunger; Resting's plan never
        // mentions stores at all now.
        let start = PlannerState {
            energy_ok: false,
            temperature_ok: false,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![
                StatePredicate::EnergyOk(true),
                StatePredicate::TemperatureOk(true),
            ],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Resting, Action::Sleep, &distances);

        // Empty stores: still plans.
        let plan_empty = plan!(
            start.clone(),
            &actions,
            &goal,
            12,
            1000,
            markers = empty_markers()
        )
        .expect("Resting plans Sleep + SelfGroom even with empty stores");
        let kinds_empty: Vec<_> = plan_empty.iter().map(|s| s.action).collect();
        assert!(kinds_empty.contains(&GoapActionKind::Sleep));
        assert!(kinds_empty.contains(&GoapActionKind::SelfGroom));
        assert!(!kinds_empty.contains(&GoapActionKind::EatAtStores));

        // Stocked stores: same plan; stores marker irrelevant.
        let plan_stocked = plan!(
            start,
            &actions,
            &goal,
            12,
            1000,
            markers = food_stocked_markers()
        )
        .expect("plan found");
        let kinds_stocked: Vec<_> = plan_stocked.iter().map(|s| s.action).collect();
        assert!(kinds_stocked.contains(&GoapActionKind::Sleep));
        assert!(kinds_stocked.contains(&GoapActionKind::SelfGroom));
        assert!(!kinds_stocked.contains(&GoapActionKind::EatAtStores));
    }

    #[test]
    fn foraging_with_carried_herbs_still_plans() {
        // Ticket 091 producer-side fix. Pre-091 the `ForageItem` action
        // def required `CarryingIs(Carrying::Nothing)`. Across the
        // post-H1 1.2M-tick soak this caused 7,440 Foraging planning
        // failures and ZERO PlanCreated{disposition:"Foraging"} for any
        // of 8 cats — every cat holding a leftover herb was permanently
        // locked out. Removing that precondition unblocks Foraging for
        // any cat whose runtime inventory has a free slot (the actual
        // gate, enforced by `resolve_forage_item::!inventory.is_full()`).
        //
        // The deposit chain still works: ForageItem sets `Carrying::ForagedFood`
        // which DepositFood then consumes, regardless of whatever non-
        // food item the cat was already carrying.
        let start = PlannerState {
            carrying: Carrying::Herbs,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions =
            actions_for_disposition(DispositionKind::Foraging, Action::Forage, &distances);
        let plan = plan!(start, &actions, &goal, 12, 1000)
            .expect("Foraging must plan even when carrying non-food (091 fix)");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&GoapActionKind::ForageItem));
        assert!(kinds.contains(&GoapActionKind::DepositFood));
    }

    #[test]
    fn hunting_with_carried_herbs_still_plans() {
        // Companion to `foraging_with_carried_herbs_still_plans` — same
        // 091 fix applied to SearchPrey. Hunting must reach EngagePrey
        // (which sets `Carrying::Prey`) even when the cat is carrying
        // herbs left over from a prior Crafting plan.
        let start = PlannerState {
            carrying: Carrying::Herbs,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Hunting, Action::Hunt, &distances);
        let plan = plan!(start, &actions, &goal, 12, 1000)
            .expect("Hunting must plan even when carrying non-food (091 fix)");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&GoapActionKind::SearchPrey));
        assert!(kinds.contains(&GoapActionKind::EngagePrey));
        assert!(kinds.contains(&GoapActionKind::DepositPrey));
    }

    #[test]
    fn cooking_with_carried_prey_still_plans() {
        // Ticket 175 producer-side fix: same 091 pattern applied to
        // RetrieveRawFood. Pre-175, `RetrieveRawFood` required
        // `CarryingIs(Carrying::Nothing)` — any cat with leftover
        // prey/herbs/etc. electing Cook hit `GoalUnreachable` (2076
        // events on the post-172 seed-42 soak). Post-fix, the
        // chain plans from any starting carry; the runtime's multi-
        // slot inventory absorbs the new RawFood alongside the
        // existing prey, the cooked-food round-trip closes, and
        // the prey persists.
        let start = PlannerState {
            carrying: Carrying::Prey,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Cooking, Action::Cook, &distances);
        let plan = plan!(start, &actions, &goal, 12, 1000)
            .expect("Cooking must plan even when carrying prey (175 fix)");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&GoapActionKind::RetrieveRawFood));
        assert!(kinds.contains(&GoapActionKind::Cook));
        assert!(kinds.contains(&GoapActionKind::DepositCookedFood));
    }

    #[test]
    fn herbcraft_gather_with_carried_prey_still_plans() {
        // Ticket 175 producer-side fix for the Herbalism chain.
        // Pre-175, `GatherHerb` (in all three sub-chains) required
        // `CarryingIs(Carrying::Nothing)`. Same shape regression as
        // Cooking — Herbalism contributed 1663 GoalUnreachable
        // events on the post-172 soak.
        let start = PlannerState {
            carrying: Carrying::Prey,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(
            DispositionKind::Herbalism,
            Action::HerbcraftGather,
            &distances,
        );
        let plan = plan!(start, &actions, &goal, 12, 1000)
            .expect("Herbcraft Gather must plan even when carrying prey (175 fix)");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&GoapActionKind::GatherHerb));
    }

    #[test]
    fn resting_full_goal_no_longer_includes_hunger() {
        // 150 R5a regression-pin: pre-150 the Resting goal was a
        // three-need [HungerOk, EnergyOk, TemperatureOk] vector that
        // had to drop HungerOk via the 091/092 marker-gated branch
        // when stores were empty (otherwise hungry-cold cats deadlocked
        // out of Resting). Post-150 hunger isn't part of Resting at
        // all — Eating owns it. This test pins the new shape: the
        // planner-built Resting goal carries exactly two predicates,
        // never including HungerOk, regardless of marker state.
        let empty = empty_markers();
        let stocked = food_stocked_markers();
        let cx_empty = PlanContext {
            markers: &empty,
            entity: test_entity(),
        };
        let cx_stocked = PlanContext {
            markers: &stocked,
            entity: test_entity(),
        };
        let goal_empty =
            crate::ai::planner::goals::goal_for_disposition(DispositionKind::Resting, 0, &cx_empty);
        let goal_stocked = crate::ai::planner::goals::goal_for_disposition(
            DispositionKind::Resting,
            0,
            &cx_stocked,
        );
        for goal in [&goal_empty, &goal_stocked] {
            assert_eq!(goal.predicates.len(), 2);
            assert!(!goal.predicates.contains(&StatePredicate::HungerOk(true)));
        }
    }

    #[test]
    fn guarding_produces_patrol() {
        let start = PlannerState {
            zone: PlannerZone::PatrolZone,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions =
            actions_for_disposition(DispositionKind::Guarding, Action::Patrol, &distances);

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("plan found");
        assert_eq!(plan.len(), 1);
        // Should pick cheapest: Survey (cost 1).
        assert_eq!(plan[0].action, GoapActionKind::Survey);
    }

    #[test]
    fn building_haul_then_construct() {
        // Ticket 038 — building plans thread through a real haul:
        // [TravelTo(MaterialPile), GatherMaterials, TravelTo(ConstructionSite),
        //  DeliverMaterials, Construct]. Ticket 096 split: with
        // `MaterialsAvailable` marker absent, `Construct` resolves via
        // the plan-path branch (`MaterialsDeliveredThisPlan(true)`)
        // after `DeliverMaterials` flips the search-state field.
        let start = default_state();
        assert!(
            !start.materials_delivered_this_plan,
            "search-state field starts false; the Deliver effect must do the work"
        );
        let goal = GoalState {
            predicates: vec![StatePredicate::ConstructionDone(true)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Building, Action::Build, &distances);

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("plan found");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![
                GoapActionKind::TravelTo(PlannerZone::MaterialPile),
                GoapActionKind::GatherMaterials,
                GoapActionKind::TravelTo(PlannerZone::ConstructionSite),
                GoapActionKind::DeliverMaterials,
                GoapActionKind::Construct,
            ]
        );
    }

    #[test]
    fn building_construct_short_circuit_when_materials_already_available() {
        // Ticket 096 substrate path: when the world already has a
        // funded construction site (the `MaterialsAvailable` marker is
        // set on the entity), the planner skips the haul leg and goes
        // straight to TravelTo + Construct. Pre-096 this used a
        // `materials_available: true` field on PlannerState; post-096
        // the world fact lives in the substrate marker, the
        // search-state field stays false throughout.
        let start = default_state();
        assert!(
            !start.materials_delivered_this_plan,
            "substrate-branch test must not pre-fill the search-state field"
        );
        let goal = GoalState {
            predicates: vec![StatePredicate::ConstructionDone(true)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Building, Action::Build, &distances);

        let plan = plan!(
            start,
            &actions,
            &goal,
            12,
            1000,
            markers = materials_available_markers()
        )
        .expect("plan found");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![
                GoapActionKind::TravelTo(PlannerZone::ConstructionSite),
                GoapActionKind::Construct,
            ]
        );
    }

    #[test]
    fn farming_tend_then_harvest() {
        let start = PlannerState {
            zone: PlannerZone::Farm,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Farming, Action::Farm, &distances);

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("plan found");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![GoapActionKind::TendCrops, GoapActionKind::HarvestCrops,]
        );
    }

    #[test]
    fn mating_plan() {
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::InteractionDone(true)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Mating, Action::Mate, &distances);

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("plan found");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![
                GoapActionKind::TravelTo(PlannerZone::SocialTarget),
                GoapActionKind::MateWith,
            ]
        );
    }

    #[test]
    fn mentoring_plan() {
        // 154: Mentoring's plan template is single-action
        // `[TravelTo(SocialTarget), MentorCat]`, mirroring Mating.
        // Critically, MentorCat's effect is `SetInteractionDone(true)`
        // only — no `IncrementTrips`. The completion proxy at
        // `goal_for_disposition` is `InteractionDone(true)` (Pattern
        // B), so the planner resolves on the first successful mentor
        // session and the L3 Mentor pick can't be overridden by
        // sibling-step cost-asymmetry.
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::InteractionDone(true)],
        };
        let distances = basic_distances();
        let actions =
            actions_for_disposition(DispositionKind::Mentoring, Action::Mentor, &distances);

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("plan found");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![
                GoapActionKind::TravelTo(PlannerZone::SocialTarget),
                GoapActionKind::MentorCat,
            ]
        );

        // Direct shape check: mentoring_actions returns exactly one
        // GoapActionDef whose effects are InteractionDone-only.
        let only = mentoring_actions();
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].kind, GoapActionKind::MentorCat);
        assert!(only[0]
            .effects
            .iter()
            .any(|e| matches!(e, StateEffect::SetInteractionDone(true))));
        assert!(
            !only[0]
                .effects
                .iter()
                .any(|e| matches!(e, StateEffect::IncrementTrips)),
            "mentoring_actions must not IncrementTrips — Pattern B (interaction-based, single-trip)"
        );
    }

    #[test]
    fn socializing_plan_drops_mentor_and_groom_other_steps() {
        // 154 dropped MentorCat into `mentoring_actions`. 158 dropped
        // GroomOther into `grooming_actions` for the same shape of
        // bug — equivalent-effect siblings under Socializing's
        // count-based goal had A* pre-pruning the second action
        // (`tentative_g >= best_g` at planner/mod.rs:437) because
        // both produced the same `(SetInteractionDone, IncrementTrips)`
        // next-state. Socializing's template is now single-action
        // `[SocializeWith]`.
        let distances = basic_distances();
        let actions =
            actions_for_disposition(DispositionKind::Socializing, Action::Socialize, &distances);
        let kinds: Vec<_> = actions.iter().map(|a| a.kind).collect();
        assert!(
            !kinds.contains(&GoapActionKind::MentorCat),
            "Socializing template must not include MentorCat after 154 split"
        );
        assert!(
            !kinds.contains(&GoapActionKind::GroomOther),
            "Socializing template must not include GroomOther after 158 split"
        );
        assert!(kinds.contains(&GoapActionKind::SocializeWith));
    }

    #[test]
    fn grooming_plan_pattern_b_shape() {
        // 158: Grooming mirrors `mentoring_actions`'s Pattern B —
        // single GoapActionDef, `SetInteractionDone(true)` effect, no
        // `IncrementTrips`. The single-action template is the structural
        // guarantee that A* can never pre-prune GroomOther in favor of
        // an equivalent-effect sibling.
        let only = grooming_actions();
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].kind, GoapActionKind::GroomOther);
        assert!(only[0]
            .effects
            .iter()
            .any(|e| matches!(e, StateEffect::SetInteractionDone(true))));
        assert!(
            !only[0]
                .effects
                .iter()
                .any(|e| matches!(e, StateEffect::IncrementTrips)),
            "grooming_actions must not IncrementTrips — Pattern B (interaction-based, single-trip)"
        );
    }

    #[test]
    fn set_ward_plan_requires_thornbriar_available() {
        // 092: GatherHerb (under SetWard hint) gates on the
        // `ThornbriarAvailable` marker. With the marker absent, no plan.
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(
            DispositionKind::Herbalism,
            Action::HerbcraftSetWard,
            &distances,
        );

        let plan = plan!(start, &actions, &goal, 12, 1000, markers = empty_markers());
        assert!(
            plan.is_err(),
            "SetWard plan should be impossible without thornbriar"
        );
    }

    #[test]
    fn set_ward_plan_succeeds_with_thornbriar() {
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(
            DispositionKind::Herbalism,
            Action::HerbcraftSetWard,
            &distances,
        );

        let plan = plan!(
            start,
            &actions,
            &goal,
            12,
            1000,
            markers = thornbriar_markers()
        )
        .expect("plan should succeed");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&GoapActionKind::GatherHerb));
        assert!(kinds.contains(&GoapActionKind::SetWard));
    }

    #[test]
    fn caretaking_plan_works_when_adult_carries_herbs() {
        // Regression test: a first pass gated RetrieveFoodForKitten on
        // `CarryingIs(Nothing)` which meant any cat holding herbs /
        // foraged food / prey couldn't find a plan. Post-fix soaks
        // produced 0 Caretake plans because of this. The planner's
        // Carrying state is a coarse abstraction and shouldn't veto
        // Caretake on non-empty runtime inventory.
        let start = PlannerState {
            carrying: Carrying::Herbs,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions =
            actions_for_disposition(DispositionKind::Caretaking, Action::Caretake, &distances);

        let plan = plan!(start, &actions, &goal, 12, 1000)
            .expect("caretaking plan should succeed even when carrying herbs");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&GoapActionKind::RetrieveFoodForKitten));
        assert!(kinds.contains(&GoapActionKind::FeedKitten));
    }

    #[test]
    fn caretaking_plan_retrieves_food_then_feeds() {
        // §Phase 4c.4 regression test: before this fix the Caretake
        // plan was `[TravelTo(Stores), FeedKitten]` which silently no-
        // op'd because the adult's inventory was empty at FeedKitten
        // time. The fixed catalog requires RetrieveFoodForKitten to
        // precede FeedKitten, so the planner emits a three-step chain
        // (travel in, retrieve, feed) when the adult starts from Wilds.
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions =
            actions_for_disposition(DispositionKind::Caretaking, Action::Caretake, &distances);

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("caretaking plan should succeed");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![
                GoapActionKind::TravelTo(PlannerZone::Stores),
                GoapActionKind::RetrieveFoodForKitten,
                GoapActionKind::FeedKitten,
            ]
        );
    }

    #[test]
    fn cook_plan_travels_through_stores_kitchen_stores() {
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Cooking, Action::Cook, &distances);

        let plan = plan!(start, &actions, &goal, 16, 5000).expect("cook plan should succeed");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![
                GoapActionKind::TravelTo(PlannerZone::Stores),
                GoapActionKind::RetrieveRawFood,
                GoapActionKind::TravelTo(PlannerZone::Kitchen),
                GoapActionKind::Cook,
                GoapActionKind::TravelTo(PlannerZone::Stores),
                GoapActionKind::DepositCookedFood,
            ]
        );
    }

    /// 092 substrate test ported to 150 R5a: with `HasStoredFood`
    /// present, a hungry cat picking the new `Eating` disposition can
    /// plan `EatAtStores` and reach `HungerOk`. The substrate-marker
    /// gating moved from Resting → Eating but the invariant (planner
    /// and DSE eligibility share one source of truth) is preserved.
    #[test]
    fn eat_at_stores_reachable_via_eating_when_food_marker_set() {
        let start = PlannerState {
            hunger_ok: false,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::HungerOk(true)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Eating, Action::Eat, &distances);

        let plan = plan!(
            start,
            &actions,
            &goal,
            8,
            500,
            markers = food_stocked_markers()
        )
        .expect("EatAtStores must be reachable when HasStoredFood marker is set");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&GoapActionKind::EatAtStores));
    }

    /// 092 substrate-invariant ported to 150 R5a: flipping the
    /// `HasStoredFood` marker flips Eating's reachability. The shared-
    /// source-of-truth between planner preconditions and DSE
    /// eligibility holds with the disposition split.
    #[test]
    fn marker_change_flips_eating_plan_reachability() {
        let start = PlannerState {
            hunger_ok: false,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::HungerOk(true)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Eating, Action::Eat, &distances);

        let with_food = plan!(
            start.clone(),
            &actions,
            &goal,
            8,
            500,
            markers = food_stocked_markers()
        );
        assert!(with_food.is_ok(), "marker present → Eating reachable");

        let without_food = plan!(start, &actions, &goal, 8, 500, markers = empty_markers());
        assert!(
            without_food.is_err(),
            "marker absent → Eating unreachable (HungerOk goal)"
        );
    }

    /// Ticket 231 dual-branch composition: substrate path of
    /// PickUpItemFromGround fires when `HasFreeSlot` is authored.
    #[test]
    fn picking_up_substrate_path_when_free_slot_marker_set() {
        let start = PlannerState {
            zone: PlannerZone::CarcassPile,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions =
            actions_for_disposition(DispositionKind::PickingUp, Action::PickUp, &distances);

        // food_stocked_markers() sets HasFreeSlot for test_entity().
        let plan = plan!(start, &actions, &goal, 8, 500)
            .expect("substrate-path PickUp must be reachable when HasFreeSlot is set");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&GoapActionKind::PickUpItemFromGround));
        assert!(
            !kinds.contains(&GoapActionKind::DropItem),
            "substrate path is cheaper (cost 1 vs 1+1=2); A* must NOT prepend DropItem when HasFreeSlot is true"
        );
    }

    /// Ticket 231 dual-branch composition: plan path fires (and prepends
    /// DropItem) when `HasFreeSlot` is absent — the cat is full and
    /// must drop something to make room.
    #[test]
    fn picking_up_plan_path_prepends_drop_when_no_free_slot() {
        let start = PlannerState {
            zone: PlannerZone::CarcassPile,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions =
            actions_for_disposition(DispositionKind::PickingUp, Action::PickUp, &distances);

        // empty_markers() does NOT set HasFreeSlot — substrate-path
        // precondition fails, only plan-path remains expandable.
        let plan = plan!(start, &actions, &goal, 8, 500, markers = empty_markers())
            .expect("plan-path PickUp must compose [DropItem, PickUpItemFromGround]");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(
            kinds.contains(&GoapActionKind::DropItem),
            "plan-path must prepend DropItem when HasFreeSlot is absent; got {kinds:?}"
        );
        assert!(
            kinds.contains(&GoapActionKind::PickUpItemFromGround),
            "plan-path must still finish with PickUpItemFromGround; got {kinds:?}"
        );
        // Ordering: DropItem before PickUpItemFromGround.
        let drop_idx = kinds
            .iter()
            .position(|k| *k == GoapActionKind::DropItem)
            .unwrap();
        let pickup_idx = kinds
            .iter()
            .position(|k| *k == GoapActionKind::PickUpItemFromGround)
            .unwrap();
        assert!(
            drop_idx < pickup_idx,
            "DropItem must come before PickUpItemFromGround; got {kinds:?}"
        );
    }

    // --- 084 Commit 2: HerbcraftGather + HerbcraftSetWard plan templates ---

    /// HerbcraftGather plan terminates at Stores via DepositHerbs —
    /// gather without a deposit terminus no longer trip-increments.
    #[test]
    fn gather_plan_ends_with_deposit() {
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(
            DispositionKind::Herbalism,
            Action::HerbcraftGather,
            &distances,
        );
        let plan = plan!(
            start,
            &actions,
            &goal,
            12,
            1000,
            markers = food_stocked_markers()
        )
        .expect("gather plan should succeed");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(
            kinds.contains(&GoapActionKind::GatherHerb),
            "plan must include GatherHerb; got {kinds:?}"
        );
        assert!(
            kinds.contains(&GoapActionKind::DepositHerbs),
            "plan must terminate with DepositHerbs; got {kinds:?}"
        );
        // Ordering: GatherHerb before DepositHerbs.
        let gather_idx = kinds
            .iter()
            .position(|k| *k == GoapActionKind::GatherHerb)
            .unwrap();
        let deposit_idx = kinds
            .iter()
            .position(|k| *k == GoapActionKind::DepositHerbs)
            .unwrap();
        assert!(
            gather_idx < deposit_idx,
            "GatherHerb must come before DepositHerbs; got {kinds:?}"
        );
    }

    /// HerbcraftSetWard picks the retrieve-from-stash branch when the
    /// colony stash has thornbriar AND wild thornbriar is unavailable.
    /// A* should choose `RetrieveHerbs(Thornbriar) → SetWard` rather
    /// than `GatherHerb → SetWard` (which is gated impossible by the
    /// `ThornbriarAvailable` marker absent).
    #[test]
    fn set_ward_plan_picks_retrieve_path_when_only_stash_available() {
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(
            DispositionKind::Herbalism,
            Action::HerbcraftSetWard,
            &distances,
        );
        let plan = plan!(
            start,
            &actions,
            &goal,
            12,
            1000,
            markers = stored_thornbriar_markers()
        )
        .expect("retrieve-path plan should succeed when stash has thornbriar");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(
            kinds.contains(&GoapActionKind::RetrieveHerbs(
                crate::components::magic::HerbKind::Thornbriar
            )),
            "retrieve-path must use RetrieveHerbs(Thornbriar); got {kinds:?}"
        );
        assert!(
            kinds.contains(&GoapActionKind::SetWard),
            "plan must terminate with SetWard; got {kinds:?}"
        );
        assert!(
            !kinds.contains(&GoapActionKind::GatherHerb),
            "stash-only regime must not use GatherHerb (wild thornbriar unavailable); got {kinds:?}"
        );
    }

    /// 463 — HaveItem-craft template produces the expected
    /// 3-leg plan: TravelTo(Stores) → RetrieveCraftInputs(recipe.id) →
    /// TravelTo(Workshop) → CraftAtWorkshop. The cat starts in Wilds
    /// carrying nothing; HasFreeSlot marker is set (substrate-path
    /// retrieve arm). The plan must NOT call into the legacy
    /// `crafting_actions` lex-pick path — the recipe identity is
    /// pinned by the held HaveItem Intention.
    #[test]
    fn craft_have_item_workshop_plan() {
        use crate::components::items::ItemKind;
        use crate::components::recipe::{
            DisciplineKind, ItemDestination, Recipe, RecipeDuration, RecipeId, RecipeInput,
            RecipeOutput, StationRequirement,
        };
        use crate::resources::recipe_registry::RecipeRegistry;
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        // Extend distances with Workshop (basic_distances omits it).
        let mut distances = basic_distances();
        let extra = [PlannerZone::Workshop, PlannerZone::TanningFrame];
        let basic_zones = [
            PlannerZone::Stores,
            PlannerZone::HuntingGround,
            PlannerZone::ForagingGround,
            PlannerZone::Farm,
            PlannerZone::ConstructionSite,
            PlannerZone::HerbPatch,
            PlannerZone::Kitchen,
            PlannerZone::RestingSpot,
            PlannerZone::SocialTarget,
            PlannerZone::Wilds,
            PlannerZone::PatrolZone,
            PlannerZone::MaterialPile,
        ];
        for &x in &extra {
            for &y in basic_zones.iter().chain(extra.iter()) {
                if x != y {
                    distances.set(x, y, 2);
                    distances.set(y, x, 2);
                }
            }
        }
        let mut recipes = RecipeRegistry::default();
        recipes.insert(Recipe {
            id: RecipeId("bone_tip_spear"),
            discipline: DisciplineKind::BoneShellCraft,
            inputs: vec![
                RecipeInput {
                    kind: ItemKind::Bone,
                    count: 1,
                },
                RecipeInput {
                    kind: ItemKind::Sinew,
                    count: 1,
                },
            ],
            station: StationRequirement::Workshop,
            duration: RecipeDuration::Fixed { ticks: 10 },
            output: RecipeOutput {
                item_kind: ItemKind::BoneTipSpear,
                destination: ItemDestination::Inventory,
            },
            skill_gate: None,
            is_warriors_kit: true,
            discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::BoneShaping),
        });
        let actions = craft_have_item_actions(ItemKind::BoneTipSpear, &recipes, &distances);
        // Markers: free slot present (so the substrate-path retrieve
        // arm fires); HasCraftInputInInventory NOT set (so the legacy
        // craft arm isn't picked — A* must sequence the retrieve).
        let mut markers = empty_markers();
        markers.set_entity(markers::HasFreeSlot::KEY, test_entity(), true);
        let plan = plan!(start, &actions, &goal, 12, 1000, markers = markers)
            .expect("HaveItem craft must plan when free slot + recipe are present");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(
            kinds.contains(&GoapActionKind::RetrieveCraftInputs(RecipeId(
                "bone_tip_spear"
            ))),
            "plan must include the parameterized RetrieveCraftInputs(bone_tip_spear); got {kinds:?}"
        );
        assert!(
            kinds.contains(&GoapActionKind::CraftAtWorkshop(Some(RecipeId(
                "bone_tip_spear"
            )))),
            "plan must terminate with CraftAtWorkshop(Some(bone_tip_spear)); got {kinds:?}"
        );
        // Ordering invariant: retrieve before craft, both pinned to the
        // same RecipeId (HaveItem path emits Some(recipe.id), not None).
        let retrieve_idx = kinds.iter().position(|k| {
            matches!(
                k,
                GoapActionKind::RetrieveCraftInputs(RecipeId("bone_tip_spear"))
            )
        });
        let craft_idx = kinds.iter().position(|k| {
            matches!(
                k,
                GoapActionKind::CraftAtWorkshop(Some(RecipeId("bone_tip_spear")))
            )
        });
        assert!(retrieve_idx < craft_idx, "retrieve must precede craft");
    }

    /// 463 — HaveItem-craft for a recipe with no matching recipe
    /// returns an empty action set; the planner short-circuits with
    /// `NoApplicableActions`.
    #[test]
    fn craft_have_item_missing_recipe_returns_empty() {
        use crate::components::items::ItemKind;
        use crate::resources::recipe_registry::RecipeRegistry;
        let distances = basic_distances();
        let recipes = RecipeRegistry::default();
        let actions = craft_have_item_actions(ItemKind::BoneTipSpear, &recipes, &distances);
        // travel_actions still populates the list, but no
        // RetrieveCraftInputs / CraftAtWorkshop variants present.
        assert!(
            actions.is_empty(),
            "no recipe → empty action set (planner reports NoApplicableActions)"
        );
    }

    /// HerbcraftSetWard with NEITHER wild thornbriar NOR stash available
    /// produces no plan — both gather (ThornbriarAvailable absent) and
    /// retrieve (HasStoredThornbriar absent) branches fail their
    /// preconditions.
    #[test]
    fn set_ward_plan_impossible_when_neither_wild_nor_stash() {
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(
            DispositionKind::Herbalism,
            Action::HerbcraftSetWard,
            &distances,
        );
        let plan = plan!(
            start,
            &actions,
            &goal,
            12,
            1000,
            markers = food_stocked_markers()
        );
        assert!(
            plan.is_err(),
            "SetWard must be unplannable when neither wild thornbriar nor stash is available"
        );
    }
}
