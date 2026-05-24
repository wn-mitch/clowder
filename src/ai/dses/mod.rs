//! Per-DSE modules — each file defines one constructor + its
//! `Dse` trait impl. Registered at plugin load via
//! [`DseRegistryAppExt`](super::eval::DseRegistryAppExt).
//!
//! Phase 3b.2 lands the reference port (Eat). Phase 3c fans out the
//! remaining 20 cat DSEs, 9 fox DSEs, and 9 Herbcraft/PracticeMagic
//! siblings through the same template.
//!
//! Ticket 438 — cat-DSE auto-discovery via [`linkme`]. Each cat-DSE
//! file emits a `#[linkme::distributed_slice(CAT_DSE_REGISTRY)] static
//! X_REGISTRATION: CatDseRegistration = CatDseRegistration { order: N,
//! construct: |_| x_dse() }` so the central [`populate_dse_registry`]
//! reads the slice instead of hand-maintaining a parallel list. The
//! `order` field is the seed-42-load-bearing dispatch order — lower
//! fires first. Gapped by 100 (100, 200, ...) so future DSEs can
//! insert without renumbering. Adding a new cat DSE means writing one
//! constructor + one registration entry in the same file — both
//! required by construction; missing either is a compile error or a
//! never-fired-canary failure.

use crate::resources::sim_constants::ScoringConstants;

/// Sortable cat-DSE registration entry consumed by
/// [`populate_dse_registry`](crate::plugins::simulation::populate_dse_registry).
/// `order` is the per-tick dispatch order in `score_actions` (the
/// registry-iterating loop in `src/ai/scoring.rs`) — lower fires first.
/// Seed-42 determinism depends on this order matching the pre-438
/// hand-written dispatcher exactly, so changing an existing entry's
/// `order` is a balance change and must follow the four-artifact
/// methodology in CLAUDE.md.
pub struct CatDseRegistration {
    /// Per-tick dispatch order (low fires first). Gaps of 100 between
    /// adjacent entries by convention, leaving headroom for insertion.
    pub order: u16,
    /// Constructor invoked by `populate_dse_registry` at plugin load.
    /// `&ScoringConstants` is passed in even for stateless DSEs (most
    /// constructors ignore it) so the function-pointer type stays
    /// uniform across the registry.
    pub construct: fn(&ScoringConstants) -> Box<dyn super::dse::CatDse>,
}

/// Distributed slice of cat-DSE registrations. Populated by each
/// `dses/*.rs` file via `#[linkme::distributed_slice(CAT_DSE_REGISTRY)]
/// static X: CatDseRegistration = ...`. Read by `populate_dse_registry`
/// after sorting by `order`.
#[linkme::distributed_slice]
pub static CAT_DSE_REGISTRY: [CatDseRegistration];

/// Construct every cat DSE in seed-42 dispatch order. Walks
/// [`CAT_DSE_REGISTRY`], sorts by declared `order`, and calls each
/// constructor with the supplied `scoring` constants.
pub fn cat_dse_constructors(scoring: &ScoringConstants) -> Vec<Box<dyn super::dse::CatDse>> {
    let mut entries: Vec<&CatDseRegistration> = CAT_DSE_REGISTRY.iter().collect();
    entries.sort_by_key(|e| e.order);
    entries.iter().map(|e| (e.construct)(scoring)).collect()
}

pub mod apply_remedy_target;
pub mod beg_for_food;
pub mod build;
pub mod build_target;
pub mod bury;
pub mod bury_target;
pub mod caretake;
pub mod caretake_target;
pub mod cook;
pub mod coordinate;
pub mod craft_at_tanning_frame;
pub mod craft_at_workshop;
pub mod dependent_kitten_target;
pub mod discarding;
pub mod dry_food;
pub mod eat;
pub mod explore;
pub mod farm;
pub mod fight;
pub mod fight_target;
pub mod flee;
pub mod forage;
pub mod fox_avoiding;
pub mod fox_den_defense;
pub mod fox_dispersing;
pub mod fox_feeding;
pub mod fox_fleeing;
pub mod fox_hunting;
pub mod fox_patrolling;
pub mod fox_raiding;
pub mod fox_resting;
pub mod groom_other;
pub mod groom_other_target;
pub mod groom_self;
pub mod handing;
pub mod hawk_fleeing;
pub mod hawk_hunting;
pub mod hawk_resting;
pub mod herbcraft_gather;
pub mod herbcraft_prepare;
pub mod herbcraft_target;
pub mod herbcraft_ward;
pub mod hide;
pub mod hunt;
pub mod hunt_target;
pub mod idle;
pub mod mate;
pub mod mate_target;
pub mod mentor;
pub mod mentor_target;
pub mod patrol;
pub mod picking_up;
pub mod practice_magic;
pub mod sleep;
pub mod smoke_meat;
pub mod snake_ambushing;
pub mod snake_basking;
pub mod snake_fleeing;
pub mod snake_foraging;
pub mod socialize;
pub mod socialize_target;
pub mod tend_smoking_rack;
pub mod trashing;
pub mod wander;

pub use apply_remedy_target::apply_remedy_target_dse;
pub use beg_for_food::{beg_for_food_eyes_open_dse, beg_for_food_newborn_dse};
pub use build::build_dse;
pub use build_target::build_target_dse;
pub use bury::bury_dse;
pub use bury_target::{bury_target_dse, resolve_bury_target};
pub use caretake::caretake_dse;
pub use caretake_target::caretake_target_dse;
pub use cook::cook_dse;
pub use coordinate::coordinate_dse;
pub use craft_at_tanning_frame::craft_at_tanning_frame_dse;
pub use craft_at_workshop::craft_at_workshop_dse;
pub use discarding::discarding_dse;
pub use dry_food::dry_food_dse;
pub use eat::eat_dse;
pub use explore::explore_dse;
pub use farm::farm_dse;
pub use fight::fight_dse;
pub use fight_target::fight_target_dse;
pub use flee::flee_dse;
pub use forage::forage_dse;
pub use fox_avoiding::fox_avoiding_dse;
pub use fox_den_defense::fox_den_defense_dse;
pub use fox_dispersing::fox_dispersing_dse;
pub use fox_feeding::fox_feeding_dse;
pub use fox_fleeing::fox_fleeing_dse;
pub use fox_hunting::fox_hunting_dse;
pub use fox_patrolling::fox_patrolling_dse;
pub use fox_raiding::fox_raiding_dse;
pub use fox_resting::fox_resting_dse;
pub use groom_other::groom_other_dse;
pub use groom_other_target::groom_other_target_dse;
pub use groom_self::groom_self_dse;
pub use handing::handing_dse;
pub use hawk_fleeing::hawk_fleeing_dse;
pub use hawk_hunting::hawk_hunting_dse;
pub use hawk_resting::hawk_resting_dse;
pub use herbcraft_gather::herbcraft_gather_dse;
pub use herbcraft_prepare::herbcraft_prepare_dse;
pub use herbcraft_target::herbcraft_target_dse;
pub use herbcraft_ward::herbcraft_ward_dse;
pub use hide::hide_dse;
pub use hunt::hunt_dse;
pub use hunt_target::hunt_target_dse;
pub use idle::idle_dse;
pub use mate::mate_dse;
pub use mate_target::mate_target_dse;
pub use mentor::mentor_dse;
pub use mentor_target::mentor_target_dse;
pub use patrol::patrol_dse;
pub use picking_up::picking_up_dse;
pub use practice_magic::{
    cleanse_dse, colony_cleanse_dse, commune_dse, durable_ward_dse, harvest_dse, scry_dse,
};
pub use sleep::sleep_dse;
pub use smoke_meat::smoke_meat_dse;
pub use snake_ambushing::snake_ambushing_dse;
pub use snake_basking::snake_basking_dse;
pub use snake_fleeing::snake_fleeing_dse;
pub use snake_foraging::snake_foraging_dse;
pub use socialize::socialize_dse;
pub use socialize_target::socialize_target_dse;
pub use tend_smoking_rack::tend_smoking_rack_dse;
pub use trashing::trashing_dse;
pub use wander::wander_dse;
