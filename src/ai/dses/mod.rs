//! Per-DSE modules — each file defines one constructor + its
//! `Dse` trait impl. Registered at plugin load via
//! [`DseRegistryAppExt`](super::eval::DseRegistryAppExt).
//!
//! Phase 3b.2 lands the reference port (Eat). Phase 3c fans out the
//! remaining 20 cat DSEs, 9 fox DSEs, and 9 Herbcraft/PracticeMagic
//! siblings through the same template.

pub mod build;
pub mod caretake;
pub mod cook;
pub mod coordinate;
pub mod eat;
pub mod explore;
pub mod farm;
pub mod fight;
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
pub mod groom_self;
pub mod herbcraft_gather;
pub mod herbcraft_prepare;
pub mod herbcraft_ward;
pub mod hunt;
pub mod idle;
pub mod mate;
pub mod mentor;
pub mod patrol;
pub mod practice_magic;
pub mod sleep;
pub mod socialize;
pub mod wander;

pub use build::build_dse;
pub use caretake::caretake_dse;
pub use cook::cook_dse;
pub use coordinate::coordinate_dse;
pub use eat::eat_dse;
pub use explore::explore_dse;
pub use farm::farm_dse;
pub use fight::fight_dse;
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
pub use groom_self::groom_self_dse;
pub use herbcraft_gather::herbcraft_gather_dse;
pub use herbcraft_prepare::herbcraft_prepare_dse;
pub use herbcraft_ward::herbcraft_ward_dse;
pub use hunt::hunt_dse;
pub use idle::idle_dse;
pub use mate::mate_dse;
pub use mentor::mentor_dse;
pub use patrol::patrol_dse;
pub use practice_magic::{
    cleanse_dse, colony_cleanse_dse, commune_dse, durable_ward_dse, harvest_dse, scry_dse,
};
pub use sleep::sleep_dse;
pub use socialize::socialize_dse;
pub use wander::wander_dse;
