use crate::components::disposition::CraftingHint;
use crate::components::markers;

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

pub fn hunting_actions() -> Vec<GoapActionDef> {
    vec![
        // Ticket 091: SearchPrey/EngagePrey intentionally do NOT require
        // `CarryingIs(Carrying::Nothing)`. The runtime resolver gates on
        // `inventory.is_full()`, not on a specific Carrying state — a
        // cat carrying herbs from an aborted Crafting plan can still
        // hunt as long as the inventory has a free slot. Pre-091 the
        // planner's `CarryingIs(Carrying::Nothing)` precondition was a
        // permanent veto for any cat with leftover items, which made
        // Hunting plans uniformly unreachable for the post-founding
        // colony (zero PlanCreated{disposition:"Hunting"} across 1.2M
        // ticks for 8 cats). Mirrors the same fix applied to
        // `caretaking_actions::RetrieveFoodForKitten` in Phase 4c.4.
        GoapActionDef {
            kind: GoapActionKind::SearchPrey,
            cost: 3,
            preconditions: vec![StatePredicate::ZoneIs(PlannerZone::HuntingGround)],
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
    vec![
        GoapActionDef {
            kind: GoapActionKind::SocializeWith,
            cost: 2,
            preconditions: vec![StatePredicate::ZoneIs(PlannerZone::SocialTarget)],
            effects: vec![
                StateEffect::SetInteractionDone(true),
                StateEffect::IncrementTrips,
            ],
        },
        GoapActionDef {
            kind: GoapActionKind::GroomOther,
            cost: 2,
            preconditions: vec![StatePredicate::ZoneIs(PlannerZone::SocialTarget)],
            effects: vec![
                StateEffect::SetInteractionDone(true),
                StateEffect::IncrementTrips,
            ],
        },
        GoapActionDef {
            kind: GoapActionKind::MentorCat,
            cost: 3,
            preconditions: vec![StatePredicate::ZoneIs(PlannerZone::SocialTarget)],
            effects: vec![
                StateEffect::SetInteractionDone(true),
                StateEffect::IncrementTrips,
            ],
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
        // Pickup: cat at a material pile, hands empty → carrying build
        // materials. Real-world effect (in the executor) is item.location
        // → Carried(cat) and an Inventory slot insert.
        GoapActionDef {
            kind: GoapActionKind::GatherMaterials,
            cost: 3,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::MaterialPile),
                StatePredicate::CarryingIs(Carrying::Nothing),
            ],
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

/// Crafting actions depend on which sub-mode the scorer selected.
pub fn crafting_actions(hint: CraftingHint) -> Vec<GoapActionDef> {
    match hint {
        CraftingHint::GatherHerbs => vec![GoapActionDef {
            kind: GoapActionKind::GatherHerb,
            cost: 3,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::HerbPatch),
                StatePredicate::CarryingIs(Carrying::Nothing),
            ],
            effects: vec![
                StateEffect::SetCarrying(Carrying::Herbs),
                StateEffect::IncrementTrips,
            ],
        }],
        CraftingHint::PrepareRemedy => vec![
            // Gather herbs first if not carrying any.
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::HerbPatch),
                cost: 2,
                preconditions: vec![StatePredicate::ZoneIsNot(PlannerZone::HerbPatch)],
                effects: vec![StateEffect::SetZone(PlannerZone::HerbPatch)],
            },
            GoapActionDef {
                kind: GoapActionKind::GatherHerb,
                cost: 3,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::HerbPatch),
                    StatePredicate::CarryingIs(Carrying::Nothing),
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
        CraftingHint::SetWard => vec![
            // Gather herbs first if not carrying any.
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::HerbPatch),
                cost: 2,
                preconditions: vec![StatePredicate::ZoneIsNot(PlannerZone::HerbPatch)],
                effects: vec![StateEffect::SetZone(PlannerZone::HerbPatch)],
            },
            GoapActionDef {
                kind: GoapActionKind::GatherHerb,
                cost: 3,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::HerbPatch),
                    StatePredicate::CarryingIs(Carrying::Nothing),
                    StatePredicate::HasMarker(markers::ThornbriarAvailable::KEY),
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
        CraftingHint::Magic => vec![
            GoapActionDef {
                kind: GoapActionKind::Scry,
                cost: 2,
                preconditions: vec![],
                effects: vec![StateEffect::IncrementTrips],
            },
            GoapActionDef {
                kind: GoapActionKind::SetWard,
                cost: 3,
                preconditions: vec![],
                effects: vec![StateEffect::IncrementTrips],
            },
            GoapActionDef {
                kind: GoapActionKind::SpiritCommunion,
                cost: 3,
                preconditions: vec![],
                effects: vec![StateEffect::IncrementTrips],
            },
            GoapActionDef {
                kind: GoapActionKind::CleanseCorruption,
                cost: 4,
                preconditions: vec![],
                effects: vec![StateEffect::IncrementTrips],
            },
            GoapActionDef {
                kind: GoapActionKind::HarvestCarcass,
                cost: 3,
                preconditions: vec![],
                effects: vec![StateEffect::IncrementTrips],
            },
        ],
        // Directed cleanse — only one action available so the planner must use it.
        CraftingHint::Cleanse => vec![GoapActionDef {
            kind: GoapActionKind::CleanseCorruption,
            cost: 1,
            preconditions: vec![],
            effects: vec![StateEffect::IncrementTrips],
        }],
        // Directed carcass harvest — only HarvestCarcass available.
        CraftingHint::HarvestCarcass => vec![GoapActionDef {
            kind: GoapActionKind::HarvestCarcass,
            cost: 1,
            preconditions: vec![],
            effects: vec![StateEffect::IncrementTrips],
        }],
        // Directed durable-ward — magic-specialist cats whose durable_ward
        // sub-score won the PracticeMagic contest. Single action so A* can't
        // fall back to cheaper alternatives like Scry.
        CraftingHint::DurableWard => vec![GoapActionDef {
            kind: GoapActionKind::SetWard,
            cost: 1,
            preconditions: vec![],
            effects: vec![StateEffect::IncrementTrips],
        }],
        // Cook: fetch a raw food from Stores, take it to a Kitchen, cook it,
        // and return it to Stores. Travel legs come from `travel_actions`
        // (the zone distance matrix); these three actions are the cook-only
        // steps that transition Carrying between Nothing → RawFood → CookedFood
        // → Nothing.
        CraftingHint::Cook => vec![
            GoapActionDef {
                kind: GoapActionKind::RetrieveRawFood,
                cost: 2,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::Stores),
                    StatePredicate::CarryingIs(Carrying::Nothing),
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
        ],
    }
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
        GoapActionDef {
            kind: GoapActionKind::RetrieveFoodForKitten,
            cost: 2,
            preconditions: vec![StatePredicate::ZoneIs(PlannerZone::Stores)],
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
pub fn actions_for_disposition(
    kind: DispositionKind,
    crafting_hint: Option<CraftingHint>,
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
        DispositionKind::Crafting => crafting_actions(crafting_hint.unwrap_or(CraftingHint::Magic)),
        DispositionKind::Coordinating => coordinating_actions(),
        DispositionKind::Exploring => exploring_actions(),
        DispositionKind::Mating => mating_actions(),
        DispositionKind::Caretaking => caretaking_actions(),
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
    use crate::ai::planner::{make_plan, Carrying, GoalState, PlanContext, PlannerState, PlannerZone};
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

    /// Default test context: stores have food. Most tests assume the
    /// colony is provisioned so `EatAtStores` is reachable; tests that
    /// explicitly probe empty-stores behavior pass `empty_markers()`
    /// instead.
    fn food_stocked_markers() -> MarkerSnapshot {
        let mut m = MarkerSnapshot::new();
        m.set_colony(markers::HasStoredFood::KEY, true);
        m
    }

    fn thornbriar_markers() -> MarkerSnapshot {
        let mut m = food_stocked_markers();
        m.set_colony(markers::ThornbriarAvailable::KEY, true);
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
            make_plan($start, $actions, $goal, $depth, $nodes, &ctx)
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
        let actions = actions_for_disposition(DispositionKind::Hunting, None, &distances);

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
        let actions = actions_for_disposition(DispositionKind::Foraging, None, &distances);

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
        let actions = actions_for_disposition(DispositionKind::Resting, None, &distances);

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
        let actions = actions_for_disposition(DispositionKind::Eating, None, &distances);

        let plan = plan!(start, &actions, &goal, 8, 500).expect("Eating must plan a chain when stores are stocked");
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
        let actions = actions_for_disposition(DispositionKind::Eating, None, &distances);
        assert!(
            plan!(start, &actions, &goal, 8, 500, markers = empty_markers()).is_none(),
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
        let actions = actions_for_disposition(DispositionKind::Resting, None, &distances);

        // Empty stores: still plans.
        let plan_empty = plan!(start.clone(), &actions, &goal, 12, 1000, markers = empty_markers())
            .expect("Resting plans Sleep + SelfGroom even with empty stores");
        let kinds_empty: Vec<_> = plan_empty.iter().map(|s| s.action).collect();
        assert!(kinds_empty.contains(&GoapActionKind::Sleep));
        assert!(kinds_empty.contains(&GoapActionKind::SelfGroom));
        assert!(!kinds_empty.contains(&GoapActionKind::EatAtStores));

        // Stocked stores: same plan; stores marker irrelevant.
        let plan_stocked =
            plan!(start, &actions, &goal, 12, 1000, markers = food_stocked_markers())
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
        let actions = actions_for_disposition(DispositionKind::Foraging, None, &distances);
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
        let actions = actions_for_disposition(DispositionKind::Hunting, None, &distances);
        let plan = plan!(start, &actions, &goal, 12, 1000)
            .expect("Hunting must plan even when carrying non-food (091 fix)");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&GoapActionKind::SearchPrey));
        assert!(kinds.contains(&GoapActionKind::EngagePrey));
        assert!(kinds.contains(&GoapActionKind::DepositPrey));
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
        let goal_empty = crate::ai::planner::goals::goal_for_disposition(
            DispositionKind::Resting,
            0,
            &cx_empty,
        );
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
        let actions = actions_for_disposition(DispositionKind::Guarding, None, &distances);

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
        let actions = actions_for_disposition(DispositionKind::Building, None, &distances);

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
        let actions = actions_for_disposition(DispositionKind::Building, None, &distances);

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
        let actions = actions_for_disposition(DispositionKind::Farming, None, &distances);

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
        let actions = actions_for_disposition(DispositionKind::Mating, None, &distances);

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
    fn set_ward_plan_requires_thornbriar_available() {
        // 092: GatherHerb (under SetWard hint) gates on the
        // `ThornbriarAvailable` marker. With the marker absent, no plan.
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(
            DispositionKind::Crafting,
            Some(CraftingHint::SetWard),
            &distances,
        );

        let plan = plan!(start, &actions, &goal, 12, 1000, markers = empty_markers());
        assert!(
            plan.is_none(),
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
            DispositionKind::Crafting,
            Some(CraftingHint::SetWard),
            &distances,
        );

        let plan = plan!(start, &actions, &goal, 12, 1000, markers = thornbriar_markers())
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
        let actions = actions_for_disposition(DispositionKind::Caretaking, None, &distances);

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
        let actions = actions_for_disposition(DispositionKind::Caretaking, None, &distances);

        let plan =
            plan!(start, &actions, &goal, 12, 1000).expect("caretaking plan should succeed");
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
        let actions = actions_for_disposition(
            DispositionKind::Crafting,
            Some(CraftingHint::Cook),
            &distances,
        );

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
        let actions = actions_for_disposition(DispositionKind::Eating, None, &distances);

        let plan = plan!(start, &actions, &goal, 8, 500, markers = food_stocked_markers())
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
        let actions = actions_for_disposition(DispositionKind::Eating, None, &distances);

        let with_food =
            plan!(start.clone(), &actions, &goal, 8, 500, markers = food_stocked_markers());
        assert!(with_food.is_some(), "marker present → Eating reachable");

        let without_food = plan!(start, &actions, &goal, 8, 500, markers = empty_markers());
        assert!(
            without_food.is_none(),
            "marker absent → Eating unreachable (HungerOk goal)"
        );
    }
}
