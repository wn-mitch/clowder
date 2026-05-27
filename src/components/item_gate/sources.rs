//! [`ItemSource`](super::ItemSource) impls for every item-creating event in
//! the colony economy. One file per origin; each emits a distinct
//! `Feature::ItemSourced*` variant so the never-fired canary can detect
//! a stuck Source per-origin (not in aggregate).
//!
//! Adding a new Source means: (a) one file here implementing
//! [`super::ItemSource`], (b) one `Feature::*` variant in
//! `src/resources/system_activation.rs` enrolled in
//! `expected_to_fire_per_soak`, (c) replacing the old inline
//! `inventory.pouch.push(...)` site with the trait dispatch.

pub mod den_raid_carcass;
pub mod forage_catch;
pub mod hunt_byproduct;
pub mod hunt_catch;

pub use den_raid_carcass::DenRaidCarcassSource;
pub use forage_catch::ForageCatchSource;
pub use hunt_byproduct::HuntByproductSource;
pub use hunt_catch::HuntCatchSource;
