//! Per-DSE modules — each file defines one constructor + its
//! `Dse` trait impl. Registered at plugin load via
//! [`DseRegistryAppExt`](super::eval::DseRegistryAppExt).
//!
//! Phase 3b.2 lands the reference port (Eat). Phase 3c fans out the
//! remaining 20 cat DSEs, 9 fox DSEs, and 9 Herbcraft/PracticeMagic
//! siblings through the same template.

pub mod eat;

pub use eat::eat_dse;
