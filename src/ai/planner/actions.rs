use crate::components::disposition::CraftingHint;

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
        GoapActionDef {
            kind: GoapActionKind::SearchPrey,
            cost: 3,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::HuntingGround),
                StatePredicate::CarryingIs(Carrying::Nothing),
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
        GoapActionDef {
            kind: GoapActionKind::ForageItem,
            cost: 3,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::ForagingGround),
                StatePredicate::CarryingIs(Carrying::Nothing),
            ],
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

pub fn resting_actions() -> Vec<GoapActionDef> {
    vec![
        GoapActionDef {
            kind: GoapActionKind::EatAtStores,
            cost: 2,
            preconditions: vec![StatePredicate::ZoneIs(PlannerZone::Stores)],
            effects: vec![StateEffect::SetHungerOk(true)],
        },
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
/// material piles. Multi-trip delivery is handled via iterative replanning
/// — `materials_available` is authored from the site's true
/// `materials_complete()` status each tick, so a single Deliver that
/// doesn't fully fund the site results in another haul cycle next replan.
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
        // into the site's ledger. Optimistically flips
        // `materials_available` true; the next state author rereads from
        // ECS and corrects to false if the site needs more.
        GoapActionDef {
            kind: GoapActionKind::DeliverMaterials,
            cost: 1,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::ConstructionSite),
                StatePredicate::CarryingIs(Carrying::BuildMaterials),
            ],
            effects: vec![
                StateEffect::SetCarrying(Carrying::Nothing),
                StateEffect::SetMaterialsAvailable(true),
                StateEffect::IncrementTrips,
            ],
        },
        // Construct: gated on materials_available. Pre-038, this had no
        // gate — the executor would Fail when materials weren't ready and
        // the plan dropped. Now the planner reasons about the dependency
        // explicitly.
        GoapActionDef {
            kind: GoapActionKind::Construct,
            cost: 6,
            preconditions: vec![
                StatePredicate::ZoneIs(PlannerZone::ConstructionSite),
                StatePredicate::MaterialsAvailable(true),
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
                    StatePredicate::ThornbriarAvailable(true),
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
    use crate::ai::planner::{make_plan, Carrying, GoalState, PlannerState, PlannerZone};

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
            thornbriar_available: false,
            materials_available: false,
        }
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

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("plan found");
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

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("plan found");
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
    fn resting_addresses_all_unmet_needs() {
        let start = PlannerState {
            hunger_ok: false,
            energy_ok: false,
            temperature_ok: false,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![
                StatePredicate::HungerOk(true),
                StatePredicate::EnergyOk(true),
                StatePredicate::TemperatureOk(true),
            ],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Resting, None, &distances);

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("plan found");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&GoapActionKind::EatAtStores));
        assert!(kinds.contains(&GoapActionKind::Sleep));
        assert!(kinds.contains(&GoapActionKind::SelfGroom));
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

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("plan found");
        assert_eq!(plan.len(), 1);
        // Should pick cheapest: Survey (cost 1).
        assert_eq!(plan[0].action, GoapActionKind::Survey);
    }

    #[test]
    fn building_haul_then_construct() {
        // Ticket 038 — building plans now thread through a real haul:
        // [TravelTo(MaterialPile), GatherMaterials, TravelTo(ConstructionSite),
        //  DeliverMaterials, Construct].
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::ConstructionDone(true)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Building, None, &distances);

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("plan found");
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
        // If the state author has already flipped `materials_available`
        // (the executor saw the site complete from a previous haul cycle),
        // the planner should skip the haul leg and go straight to
        // TravelTo + Construct.
        let start = PlannerState {
            materials_available: true,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::ConstructionDone(true)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(DispositionKind::Building, None, &distances);

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("plan found");
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

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("plan found");
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

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("plan found");
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
        let start = default_state(); // thornbriar_available: false
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(
            DispositionKind::Crafting,
            Some(CraftingHint::SetWard),
            &distances,
        );

        let plan = make_plan(start, &actions, &goal, 12, 1000);
        assert!(
            plan.is_none(),
            "SetWard plan should be impossible without thornbriar"
        );
    }

    #[test]
    fn set_ward_plan_succeeds_with_thornbriar() {
        let start = PlannerState {
            thornbriar_available: true,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let distances = basic_distances();
        let actions = actions_for_disposition(
            DispositionKind::Crafting,
            Some(CraftingHint::SetWard),
            &distances,
        );

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("plan should succeed");
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

        let plan = make_plan(start, &actions, &goal, 12, 1000)
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
            make_plan(start, &actions, &goal, 12, 1000).expect("caretaking plan should succeed");
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

        let plan = make_plan(start, &actions, &goal, 16, 5000).expect("cook plan should succeed");
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
}
