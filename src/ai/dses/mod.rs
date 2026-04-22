//! Per-DSE modules — each file defines one constructor + its
//! `Dse` trait impl. Registered at plugin load via
//! [`DseRegistryAppExt`](super::eval::DseRegistryAppExt).
//!
//! Phase 3b.2 lands the reference port (Eat). Phase 3c fans out the
//! remaining 20 cat DSEs, 9 fox DSEs, and 9 Herbcraft/PracticeMagic
//! siblings through the same template.

pub mod cook;
pub mod eat;
pub mod fight;
pub mod flee;
pub mod forage;
pub mod fox_avoiding;
pub mod fox_den_defense;
pub mod fox_fleeing;
pub mod fox_hunting;
pub mod fox_raiding;
pub mod hunt;

pub use cook::cook_dse;
pub use eat::eat_dse;
pub use fight::fight_dse;
pub use flee::flee_dse;
pub use forage::forage_dse;
pub use fox_avoiding::fox_avoiding_dse;
pub use fox_den_defense::fox_den_defense_dse;
pub use fox_fleeing::fox_fleeing_dse;
pub use fox_hunting::fox_hunting_dse;
pub use fox_raiding::fox_raiding_dse;
pub use hunt::hunt_dse;
