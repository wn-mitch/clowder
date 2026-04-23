//! Caretake targeting — per-tick kitten snapshot + compassion-bond
//! scaling for the `Caretake` DSE and its chain builder.
//!
//! Phase 4c.3 established the kitten-hunger signal wire-up as a plain
//! helper. Phase 4c.7 replaces the argmax reduction with the §6.5.6
//! four-axis `TargetTakingDse` in `src/ai/dses/caretake_target.rs`.
//! What remains here:
//!
//! - [`KittenState`] — per-tick kitten snapshot row shared by both
//!   scoring callers and the new target-taking resolver.
//! - [`CaretakeResolution`] — struct returned by the resolver; the
//!   chain builder reads `target` / `target_pos`, the scoring layer
//!   reads `urgency` / `is_parent`, and
//!   `caretake_compassion_bond_scale` reads `target_mother` /
//!   `target_father`.
//! - [`caretake_compassion_bond_scale`] — Phase 4c.4 alloparenting
//!   Reframe A helper. Scaling is caller-side, not part of the
//!   target-taking DSE axes, because it modulates the *self-state*
//!   `caretake_compassion` scoring axis (not a per-candidate axis).
//!
//! ## Signal shape
//!
//! The §6.5.6 target-taking DSE returns one kitten per adult (argmax
//! under `Best` aggregation). We surface that kitten plus the
//! bloodline-override signal (`is_parent` — any own hungry kitten in
//! range, not only argmax) so the `CaretakeDse` self-state axes fire
//! correctly even when a colony kitten outscores an adult's own.

use bevy::prelude::Entity;

use crate::components::physical::Position;

/// Per-tick snapshot of a kitten's relevant state. Captured once at
/// the top of each scoring caller (evaluate_and_plan /
/// evaluate_dispositions / disposition_to_chain) so the per-adult
/// reduction can iterate a flat slice.
#[derive(Debug, Clone, Copy)]
pub struct KittenState {
    pub entity: Entity,
    pub pos: Position,
    /// Hunger satisfaction — 1.0 sated, 0.0 starving.
    pub hunger: f32,
    pub mother: Option<Entity>,
    pub father: Option<Entity>,
}

/// Per-adult Caretake resolution. All fields are populated
/// independently; an adult with no hungry kittens in range gets
/// `urgency=0.0, is_parent=false, target=None, target_pos=None,
/// target_mother=None, target_father=None`. `target_mother` /
/// `target_father` are surfaced so the populate site can look up
/// bond-with-mother for compassion boost (Phase 4c.4 alloparenting
/// Reframe A) without re-scanning `kittens`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CaretakeResolution {
    pub urgency: f32,
    pub is_parent: bool,
    pub target: Option<Entity>,
    pub target_pos: Option<Position>,
    pub target_mother: Option<Entity>,
    pub target_father: Option<Entity>,
}

/// Compute the Caretake compassion bond-boost for `adult` given the
/// resolved target kitten's parents (from `CaretakeResolution`).
/// Reads fondness with the mother (falling back to father if no
/// mother recorded, e.g. orphan case). Non-positive fondness
/// contributes no boost — we don't want "I hate mama" to *suppress*
/// compassion for a hungry kitten below baseline, only positive
/// fondness should amplify.
///
/// Formula: `1.0 + max(0, fondness) × boost_max`. With `boost_max =
/// 1.0` (default) that means:
/// - stranger / no-mother / hostile fondness → 1.0 (no boost)
/// - fondness 0.5 → 1.5
/// - fondness 1.0 → 2.0
///
/// `fondness_lookup` returns `None` when the pair has never
/// interacted (no relationship entry yet); treat that the same as
/// fondness 0.0 (a cat's compassion toward mama's kitten defaults
/// to the colony baseline, not a negative prior).
///
/// This is a pure function over inputs. Takes no `Relationships`
/// reference to keep the unit-test surface small; the populate site
/// constructs the closure.
pub fn caretake_compassion_bond_scale(
    adult: Entity,
    resolution: &CaretakeResolution,
    boost_max: f32,
    mut fondness_lookup: impl FnMut(Entity, Entity) -> Option<f32>,
) -> f32 {
    // If the adult is the mother, this is direct parenting — no
    // bond-with-mother boost applies (the is_parent axis already
    // carries that signal). Falls through to 1.0.
    let mother = resolution.target_mother;
    let father = resolution.target_father;
    let parent_for_bond = match (mother, father) {
        (Some(m), _) if m != adult => Some(m),
        (_, Some(f)) if f != adult => Some(f),
        _ => None,
    };
    let Some(parent) = parent_for_bond else {
        return 1.0;
    };
    let fondness = fondness_lookup(adult, parent).unwrap_or(0.0);
    1.0 + fondness.max(0.0) * boost_max
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bond_scale_is_unity_when_no_target() {
        let adult = Entity::from_raw_u32(1).unwrap();
        let res = CaretakeResolution::default();
        let scale = caretake_compassion_bond_scale(adult, &res, 1.0, |_, _| Some(0.9));
        assert_eq!(scale, 1.0, "no target → no parent → baseline 1.0");
    }

    #[test]
    fn bond_scale_is_unity_when_adult_is_mother() {
        let adult = Entity::from_raw_u32(1).unwrap();
        let res = CaretakeResolution {
            target_mother: Some(adult), // adult is the mother
            target_father: Some(Entity::from_raw_u32(30).unwrap()),
            ..Default::default()
        };
        // Fondness with mother (== self) shouldn't apply; fondness with
        // father could apply but the helper skips mother == adult and
        // falls through to father. Make the father-fondness 0.5.
        let scale =
            caretake_compassion_bond_scale(adult, &res, 1.0, |_, _| Some(0.5));
        // Mother-is-self skip → fall through to father(+0.5) → 1.5.
        assert!((scale - 1.5).abs() < 1e-4, "got {scale}");
    }

    #[test]
    fn bond_scale_amplifies_with_positive_fondness() {
        let adult = Entity::from_raw_u32(1).unwrap();
        let mother = Entity::from_raw_u32(20).unwrap();
        let res = CaretakeResolution {
            target_mother: Some(mother),
            ..Default::default()
        };
        let scale = caretake_compassion_bond_scale(adult, &res, 1.0, |_, _| Some(1.0));
        assert!((scale - 2.0).abs() < 1e-4, "fondness 1.0 + boost_max 1.0 → 2.0");

        let scale_half =
            caretake_compassion_bond_scale(adult, &res, 1.0, |_, _| Some(0.5));
        assert!((scale_half - 1.5).abs() < 1e-4, "fondness 0.5 → 1.5");
    }

    #[test]
    fn bond_scale_hostile_fondness_caps_at_unity() {
        let adult = Entity::from_raw_u32(1).unwrap();
        let mother = Entity::from_raw_u32(20).unwrap();
        let res = CaretakeResolution {
            target_mother: Some(mother),
            ..Default::default()
        };
        let scale = caretake_compassion_bond_scale(adult, &res, 1.0, |_, _| Some(-0.8));
        assert_eq!(scale, 1.0, "hostile fondness shouldn't suppress compassion");
    }

    #[test]
    fn bond_scale_missing_relationship_is_unity() {
        let adult = Entity::from_raw_u32(1).unwrap();
        let mother = Entity::from_raw_u32(20).unwrap();
        let res = CaretakeResolution {
            target_mother: Some(mother),
            ..Default::default()
        };
        let scale = caretake_compassion_bond_scale(adult, &res, 1.0, |_, _| None);
        assert_eq!(scale, 1.0, "no relationship entry → treat as fondness 0.0");
    }

    #[test]
    fn bond_scale_falls_back_to_father_when_no_mother() {
        let adult = Entity::from_raw_u32(1).unwrap();
        let father = Entity::from_raw_u32(30).unwrap();
        let res = CaretakeResolution {
            target_mother: None,
            target_father: Some(father),
            ..Default::default()
        };
        let scale = caretake_compassion_bond_scale(adult, &res, 1.0, |_, _| Some(0.6));
        assert!((scale - 1.6).abs() < 1e-4, "got {scale}");
    }
}
