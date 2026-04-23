//! `PracticeMagic::*` — sibling-DSE split from the retiring cat
//! `PracticeMagic` inline block (§L2.10.10). Six sub-modes lifted
//! into six standalone DSEs; the outer hint-selection plus
//! corruption/ward emergency bonuses stay in the scorer until the
//! §3.5 modifier pipeline lands.
//!
//! All six share the PracticeMagic eligibility contract:
//! `magic_affinity > magic_affinity_threshold && magic_skill >
//! magic_skill_threshold` — handled by the outer gate in
//! `score_actions` until §4 markers port in Phase 3d.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

fn linear() -> Curve {
    Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    }
}

// ---------------------------------------------------------------------------
// Scry — curiosity-driven divination
// ---------------------------------------------------------------------------

pub struct ScryDse {
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl ScryDse {
    pub fn new() -> Self {
        Self {
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("curiosity", linear())),
                Consideration::Scalar(ScalarConsideration::new("spirituality", linear())),
                Consideration::Scalar(ScalarConsideration::new("magic_skill", linear())),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for ScryDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for ScryDse {
    fn id(&self) -> DseId {
        DseId("magic_scry")
    }
    fn considerations(&self) -> &[Consideration] {
        &self.considerations
    }
    fn composition(&self) -> &Composition {
        &self.composition
    }
    fn eligibility(&self) -> &EligibilityFilter {
        &self.eligibility
    }
    fn default_strategy(&self) -> CommitmentStrategy {
        CommitmentStrategy::OpenMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "scried",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        5
    }
}

pub fn scry_dse() -> Box<dyn Dse> {
    Box::new(ScryDse::new())
}

// ---------------------------------------------------------------------------
// DurableWard — spirituality × magic_skill × ward_deficit
// ---------------------------------------------------------------------------

pub struct DurableWardDse {
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl DurableWardDse {
    pub fn new() -> Self {
        Self {
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("spirituality", linear())),
                Consideration::Scalar(ScalarConsideration::new("magic_skill", linear())),
                Consideration::Scalar(ScalarConsideration::new("ward_deficit", linear())),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for DurableWardDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for DurableWardDse {
    fn id(&self) -> DseId {
        DseId("magic_durable_ward")
    }
    fn considerations(&self) -> &[Consideration] {
        &self.considerations
    }
    fn composition(&self) -> &Composition {
        &self.composition
    }
    fn eligibility(&self) -> &EligibilityFilter {
        &self.eligibility
    }
    fn default_strategy(&self) -> CommitmentStrategy {
        CommitmentStrategy::SingleMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "durable_ward_placed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn durable_ward_dse() -> Box<dyn Dse> {
    Box::new(DurableWardDse::new())
}

// ---------------------------------------------------------------------------
// Cleanse — spirituality × magic_skill × tile_corruption
// ---------------------------------------------------------------------------

pub struct CleanseDse {
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl CleanseDse {
    pub fn new() -> Self {
        Self {
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("spirituality", linear())),
                Consideration::Scalar(ScalarConsideration::new("magic_skill", linear())),
                Consideration::Scalar(ScalarConsideration::new("tile_corruption", linear())),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for CleanseDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for CleanseDse {
    fn id(&self) -> DseId {
        DseId("magic_cleanse")
    }
    fn considerations(&self) -> &[Consideration] {
        &self.considerations
    }
    fn composition(&self) -> &Composition {
        &self.composition
    }
    fn eligibility(&self) -> &EligibilityFilter {
        &self.eligibility
    }
    fn default_strategy(&self) -> CommitmentStrategy {
        CommitmentStrategy::SingleMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "tile_cleansed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn cleanse_dse() -> Box<dyn Dse> {
    Box::new(CleanseDse::new())
}

// ---------------------------------------------------------------------------
// ColonyCleanse — large-scale corruption response
// ---------------------------------------------------------------------------

pub struct ColonyCleanseDse {
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl ColonyCleanseDse {
    pub fn new() -> Self {
        Self {
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("spirituality", linear())),
                Consideration::Scalar(ScalarConsideration::new("magic_skill", linear())),
                Consideration::Scalar(ScalarConsideration::new(
                    "territory_max_corruption",
                    linear(),
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for ColonyCleanseDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for ColonyCleanseDse {
    fn id(&self) -> DseId {
        DseId("magic_colony_cleanse")
    }
    fn considerations(&self) -> &[Consideration] {
        &self.considerations
    }
    fn composition(&self) -> &Composition {
        &self.composition
    }
    fn eligibility(&self) -> &EligibilityFilter {
        &self.eligibility
    }
    fn default_strategy(&self) -> CommitmentStrategy {
        CommitmentStrategy::SingleMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "colony_cleansed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn colony_cleanse_dse() -> Box<dyn Dse> {
    Box::new(ColonyCleanseDse::new())
}

// ---------------------------------------------------------------------------
// Harvest — carcass harvesting
// ---------------------------------------------------------------------------

pub struct HarvestDse {
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HarvestDse {
    pub fn new() -> Self {
        Self {
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("curiosity", linear())),
                Consideration::Scalar(ScalarConsideration::new("herbcraft_skill", linear())),
                Consideration::Scalar(ScalarConsideration::new(
                    "carcass_count_saturated",
                    linear(),
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for HarvestDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for HarvestDse {
    fn id(&self) -> DseId {
        DseId("magic_harvest")
    }
    fn considerations(&self) -> &[Consideration] {
        &self.considerations
    }
    fn composition(&self) -> &Composition {
        &self.composition
    }
    fn eligibility(&self) -> &EligibilityFilter {
        &self.eligibility
    }
    fn default_strategy(&self) -> CommitmentStrategy {
        CommitmentStrategy::SingleMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "carcass_harvested",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        3
    }
}

pub fn harvest_dse() -> Box<dyn Dse> {
    Box::new(HarvestDse::new())
}

// ---------------------------------------------------------------------------
// Commune — special-terrain communion
// ---------------------------------------------------------------------------

pub struct CommuneDse {
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl CommuneDse {
    pub fn new() -> Self {
        Self {
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("spirituality", linear())),
                Consideration::Scalar(ScalarConsideration::new("magic_skill", linear())),
                Consideration::Scalar(ScalarConsideration::new("on_special_terrain", linear())),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for CommuneDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for CommuneDse {
    fn id(&self) -> DseId {
        DseId("magic_commune")
    }
    fn considerations(&self) -> &[Consideration] {
        &self.considerations
    }
    fn composition(&self) -> &Composition {
        &self.composition
    }
    fn eligibility(&self) -> &EligibilityFilter {
        &self.eligibility
    }
    fn default_strategy(&self) -> CommitmentStrategy {
        CommitmentStrategy::OpenMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "communed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        5
    }
}

pub fn commune_dse() -> Box<dyn Dse> {
    Box::new(CommuneDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_six_practice_magic_ids_stable() {
        assert_eq!(scry_dse().id().0, "magic_scry");
        assert_eq!(durable_ward_dse().id().0, "magic_durable_ward");
        assert_eq!(cleanse_dse().id().0, "magic_cleanse");
        assert_eq!(colony_cleanse_dse().id().0, "magic_colony_cleanse");
        assert_eq!(harvest_dse().id().0, "magic_harvest");
        assert_eq!(commune_dse().id().0, "magic_commune");
    }
}
