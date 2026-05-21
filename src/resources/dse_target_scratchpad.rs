//! Pre-allocated scratch buffers for target-taking DSE resolvers (ticket
//! 427 Step 1). Each `resolve_*_target` function in `src/ai/dses/` was
//! allocating fresh `Vec<Entity>` + `Vec<Position>` + 1–3 `HashMap`s per
//! cat per tick — measured at ~355 MB / 15-min soak in the 427 survey.
//!
//! This resource lives on `App` once, threaded through
//! `evaluate_and_plan` via `PlanResources::dse_scratchpad`. Each wrapper
//! clears the relevant fields then writes its candidates in; subsequent
//! ticks reuse the underlying allocations (`Vec::clear` and
//! `HashMap::clear` preserve capacity).
//!
//! **Slot semantics.** `entities` / `positions` are universal — every
//! wrapper uses them. The `map_f32_a` / `map_f32_b` slots are generic
//! `HashMap<Entity, f32>` pools that DSEs needing one or two f32-valued
//! lookup tables share by convention (documented at each call site).
//! The typed maps (`skills_by_entity`, `build_kind`, `kitten_by_entity`,
//! `species_map`) host the wrappers whose lookups can't be expressed as
//! plain f32.
//!
//! **Borrow discipline.** The DSE evaluator
//! ([`crate::ai::target_dse::evaluate_target_taking`]) accepts the
//! candidate slices via immutable `&[Entity]` / `&[Position]` and the
//! per-target fetcher closure captures the lookup maps by shared
//! reference. Wrappers MUST populate scratch then drop the `&mut` borrow
//! before constructing the fetch closures — the typical pattern is a
//! destructure-binding split into independent mut/shared slots. See
//! `src/ai/dses/socialize_target.rs::resolve_socialize_target` for the
//! exemplar.

use bevy::prelude::{Entity, Resource};
use std::collections::HashMap;

use crate::ai::caretake_targeting::KittenState;
use crate::ai::dses::build_target::BuildTargetKind;
use crate::ai::faction::FactionSpecies;
use crate::components::physical::Position;
use crate::components::prey::PreyKind;
use crate::components::skills::Skills;

#[derive(Resource, Default, Debug)]
pub struct DseTargetScratchpad {
    /// Universal candidate-entity slot.
    pub entities: Vec<Entity>,
    /// Universal candidate-position slot, parallel to `entities`.
    pub positions: Vec<Position>,
    /// Generic `HashMap<Entity, f32>` slot — shared by fight (`threat`),
    /// herbcraft (`density`), apply_remedy (`injury`), groom_other
    /// (`temperature`), build (`progress`).
    pub map_f32_a: HashMap<Entity, f32>,
    /// Second generic `HashMap<Entity, f32>` slot — for wrappers that
    /// need two f32 maps in one call (herbcraft `maturity`, build
    /// `condition`).
    pub map_f32_b: HashMap<Entity, f32>,
    /// Mentor's per-candidate skill snapshot.
    pub skills_by_entity: HashMap<Entity, Skills>,
    /// Build's per-site kind tag (NewBuild vs Repair).
    pub build_kind: HashMap<Entity, BuildTargetKind>,
    /// Caretake's per-kitten state snapshot.
    pub kitten_by_entity: HashMap<Entity, KittenState>,
    /// Fight's per-candidate resolved species, used by the §9.3 stance
    /// prefilter closure. Also shared by hunt for prey-species stance
    /// resolution.
    pub species_map: HashMap<Entity, FactionSpecies>,
    /// Hunt's per-candidate prey kind (for the yield axis).
    pub prey_kind_map: HashMap<Entity, PreyKind>,
}

impl DseTargetScratchpad {
    /// Clear every buffer. Wrappers typically clear only the slots they
    /// use, but tests / harnesses that share the resource across many
    /// stub calls can reset everything at once.
    pub fn clear_all(&mut self) {
        self.entities.clear();
        self.positions.clear();
        self.map_f32_a.clear();
        self.map_f32_b.clear();
        self.skills_by_entity.clear();
        self.build_kind.clear();
        self.kitten_by_entity.clear();
        self.species_map.clear();
        self.prey_kind_map.clear();
    }
}
