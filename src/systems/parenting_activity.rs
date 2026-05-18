//! L2 ParentingActivity per-tick systems — ticket 400.
//!
//! Three systems run in Chain 2a immediately after `update_parent_markers`:
//!
//! 1. [`update_parenting_activity_biological`] — mirrors
//!    [`update_parent_markers`]'s `KittenDependency`-sync pattern. Each tick,
//!    ensures every cat carrying a living `KittenDependency` (as mother OR
//!    father) has a `Biological`-kind `RelationshipTo` entry on its
//!    `ParentingActivity`. Component is inserted if absent. **Never removes
//!    entries** — persistence is the design contract (kitten maturity / death
//!    / partner death leave the entry in place; only the owner's death drops
//!    the Component, automatically via Bevy entity despawn).
//!
//! 2. [`tick_parental_engagement`] — per-tick EMA update on each
//!    `RelationshipTo.parental_engagement` gradient toward a
//!    personality-derived asymptote. Asymptote is composed of five
//!    orthogonal scales (Presence / Provision / Protection / Cultural /
//!    Autonomy) weighted per [`ParentingActivityConstants`]. Build fires
//!    when owner is in proximity to target (or target is despawned —
//!    grief-substrate semantics let the gradient persist with frustrated
//!    target-taking). Matured-kitten asymptote is multiplied by
//!    `matured_residual_factor` (≈ 0.15) so the residual lift survives
//!    independence ("still your mother").
//!
//! 3. [`populate_parenting_scalars`] — bridge to the modifier pipeline.
//!    For each cat with `ParentingActivity`, computes per-DSE bias sums
//!    plus the JointIntention-aware Caretake suppression factor, writes
//!    them into the [`ParentingScalars`] resource. `ScoringContext`
//!    builders read from this resource; modifier looks up via
//!    `fetch_scalar`.

use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::ai::Action;
use crate::components::{
    held_intention::HeldIntention,
    kitten::KittenDependency,
    parenting_activity::{ParentalKind, ParentingActivity, RelationshipTo},
    personality::Personality,
    physical::{Dead, Position},
    Species,
};
use crate::resources::sim_constants::{ParentingActivityConstants, SimConstants};
use crate::resources::time::TimeState;

// ---------------------------------------------------------------------------
// Five-scale personality composition (asymptote inputs)
// ---------------------------------------------------------------------------

/// **Presence** — compassion + warmth. The values-stance heaviest weight
/// in the asymptote (`w_presence = 0.30`): a low-presence parent lands at
/// moderate engagement even with high diligence, visibly different from a
/// high-presence partner.
#[inline]
pub fn scale_presence(p: &Personality) -> f32 {
    ((p.compassion + p.warmth) * 0.5).clamp(0.0, 1.0)
}

/// **Provision** — diligence + loyalty. Drives provision_bias (whole-DSE
/// Hunt lift in 400; target-axis refinement is follow-on 401).
#[inline]
pub fn scale_provision(p: &Personality) -> f32 {
    ((p.diligence + p.loyalty) * 0.5).clamp(0.0, 1.0)
}

/// **Protection** — boldness + temper. Drives protect_bias (whole-DSE
/// Patrol lift in 400; anxiety-modulated flight-flavor split is follow-on
/// 402).
#[inline]
pub fn scale_protection(p: &Personality) -> f32 {
    ((p.boldness + p.temper) * 0.5).clamp(0.0, 1.0)
}

/// **Cultural commitment** — tradition + ambition. Drives
/// cultural_teach_bias (Mentor DSE lift for custom-class teach events;
/// also boosts RitualWitness rate when ritual substrate lands as 405).
#[inline]
pub fn scale_cultural(p: &Personality) -> f32 {
    ((p.tradition + p.ambition) * 0.5).clamp(0.0, 1.0)
}

/// **Autonomy-fostering** — curiosity + patience + (1 - overprotection).
/// Drives autonomy_teach_bias (Mentor DSE lift for skill-class teach
/// events; gated by mastery substrate follow-on 406). The
/// `overprotection_penalty` is a placeholder 0.0 in 400 — it will
/// eventually fold in protect_bias activity history once the mastery
/// substrate lands.
#[inline]
pub fn scale_autonomy(p: &Personality, overprotection_penalty: f32) -> f32 {
    let raw = (p.curiosity + p.patience + (1.0 - overprotection_penalty)) / 3.0;
    raw.clamp(0.0, 1.0)
}

/// Composed asymptote — the engagement-gradient target value for a cat
/// with `Personality` p toward a particular dependent. Sums each scale
/// weighted by [`ParentingActivityConstants`]'s `w_*` fields; sum of
/// weights = 1.0 by default convention.
pub fn parental_engagement_asymptote(
    p: &Personality,
    overprotection_penalty: f32,
    constants: &ParentingActivityConstants,
) -> f32 {
    (scale_presence(p) * constants.w_presence
        + scale_provision(p) * constants.w_provision
        + scale_protection(p) * constants.w_protection
        + scale_cultural(p) * constants.w_cultural
        + scale_autonomy(p, overprotection_penalty) * constants.w_autonomy)
        .clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// ParentingScalars resource — per-cat scalar bundle consumed by modifier
// ---------------------------------------------------------------------------

/// Per-cat bundle of the six scalars the `ParentingActivityModifier` reads.
/// Built each tick by [`populate_parenting_scalars`]; lookup-by-Entity
/// from the `ScoringContext` builders in `goap.rs` / `disposition.rs`.
///
/// Default (zero) is what the modifier sees for cats with no
/// `ParentingActivity` Component — the modifier's gated-boost contract
/// (`score <= 0 → unchanged`) plus zero-lift case yields no behavior
/// change for non-parents.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParentingScalarBundle {
    /// Σ `scale_presence(p) × rel.parental_engagement × rel.bond_strength`
    /// across `relationships`. Lifts Caretake DSE post-suppression.
    pub caretake_bias_sum: f32,
    /// Σ `scale_provision(p) × rel.parental_engagement × rel.bond_strength`.
    /// Lifts Hunt DSE (whole-DSE in 400; target-axis follow-on 401).
    pub provision_bias_sum: f32,
    /// Σ `scale_protection(p) × rel.parental_engagement × rel.bond_strength`.
    /// Lifts Patrol DSE (whole-DSE in 400; anxiety-flavor follow-on 402).
    pub protect_bias_sum: f32,
    /// Σ `scale_cultural(p) × rel.parental_engagement × rel.bond_strength`.
    /// Lifts Mentor DSE for cultural transmission.
    pub cultural_teach_bias_sum: f32,
    /// Σ `scale_autonomy(p) × rel.parental_engagement × rel.bond_strength`.
    /// Lifts Mentor DSE for autonomy-fostering teach events. Mastery
    /// substrate follow-on (406) will gate by `mastery(action)`; 400
    /// leaves the mastery factor at 1.0.
    pub autonomy_teach_bias_sum: f32,
    /// Multiplier in `[0.0, 1.0]` applied to `caretake_bias_sum` when a
    /// partner already holds Caretake for one of our dependents.
    /// `joint_suppression_factor` (= 0.3 by default) when suppressed,
    /// `1.0` otherwise. Resolves the two-high-compassion-parents corner
    /// case the 398 `HandoffItem` cascade exposed.
    pub caretake_suppression_factor: f32,
    /// Max `parental_engagement` across all `RelationshipTo` entries.
    /// Replaces the binary `is_parent_of_hungry_kitten` axis on the
    /// Caretake DSE WeightedSum (ticket 400 step 5): substrate-side
    /// gradient encodes "how strongly this cat is parenting any
    /// dependent" — for a non-parent, zero; for an engaged parent,
    /// approaches the personality-derived asymptote (~0.3-0.7 typical).
    /// The new `ParentingActivityModifier` adds the personality-
    /// conditional lift on top.
    pub parental_engagement_max: f32,
}

/// Per-cat scalar map populated by [`populate_parenting_scalars`].
/// `ScoringContext` builders look up by entity; absent entries (cats
/// with no `ParentingActivity`) default to zero, so non-parent scoring
/// is byte-identical.
#[derive(Resource, Debug, Default)]
pub struct ParentingScalars {
    map: HashMap<Entity, ParentingScalarBundle>,
}

impl ParentingScalars {
    pub fn get(&self, entity: Entity) -> ParentingScalarBundle {
        self.map.get(&entity).copied().unwrap_or_default()
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    pub fn insert(&mut self, entity: Entity, bundle: ParentingScalarBundle) {
        self.map.insert(entity, bundle);
    }
}

// ---------------------------------------------------------------------------
// 1. update_parenting_activity_biological — KittenDependency sync
// ---------------------------------------------------------------------------

/// Mirrors [`crate::systems::growth::update_parent_markers`]'s pattern:
/// each tick, for every living `KittenDependency`, ensure both parents
/// carry a `Biological`-kind `RelationshipTo` entry targeting that kitten.
/// Inserts `ParentingActivity` Component if absent.
///
/// **Never removes entries** — persistence contract. Kitten maturity
/// (Bevy keeps the entity but removes `KittenDependency`) leaves the
/// entry intact; the engagement asymptote drops to
/// `matured_residual_factor × asymptote` via [`tick_parental_engagement`].
/// Kitten death leaves the entry with frustrated target-taking
/// (grief-substrate foundation, per §7.7.b). Owner death drops the
/// Component automatically when Bevy despawns the entity.
///
/// **Ordering** — Chain 2a, after `update_parent_markers` (which the
/// `Has<Parent>` semantic already depends on the same `KittenDependency`
/// substrate). Before scoring so the population sees freshly-authored
/// entries.
#[allow(clippy::type_complexity)]
pub fn update_parenting_activity_biological(
    mut commands: Commands,
    kittens: Query<(Entity, &KittenDependency), Without<Dead>>,
    mut parents: Query<
        (Entity, Option<&mut ParentingActivity>),
        (With<Species>, Without<Dead>),
    >,
    time: Res<TimeState>,
) {
    let tick = time.tick;
    // (parent_entity, kitten_entity, other_parent_entity_opt) tuples for
    // every living kitten with a parent.
    let mut wanted: Vec<(Entity, Entity, Option<Entity>)> = Vec::new();
    for (kitten_entity, dep) in kittens.iter() {
        if let Some(m) = dep.mother {
            wanted.push((m, kitten_entity, dep.father));
        }
        if let Some(f) = dep.father {
            wanted.push((f, kitten_entity, dep.mother));
        }
    }

    // Group by parent — typically 1-3 entries per parent, so a simple
    // linear scan over `wanted` per parent in the main loop is cheap.
    for (parent_entity, maybe_activity) in parents.iter_mut() {
        // Find this parent's needed entries.
        let needed: Vec<(Entity, Option<Entity>)> = wanted
            .iter()
            .filter_map(|(p, k, partner)| (*p == parent_entity).then_some((*k, *partner)))
            .collect();
        if needed.is_empty() {
            // No living dependents — nothing to insert. Existing
            // entries (matured/dead kittens) stay per persistence
            // contract.
            continue;
        }
        match maybe_activity {
            Some(mut activity) => {
                for (kitten, partner) in needed {
                    if !activity.has_kind(kitten, ParentalKind::Biological) {
                        activity.relationships.push(RelationshipTo::new(
                            kitten,
                            ParentalKind::Biological,
                            partner,
                            tick,
                        ));
                    }
                }
            }
            None => {
                // Insert Component with all needed entries in one shot.
                let mut activity = ParentingActivity::default();
                for (kitten, partner) in needed {
                    activity.relationships.push(RelationshipTo::new(
                        kitten,
                        ParentalKind::Biological,
                        partner,
                        tick,
                    ));
                }
                commands.entity(parent_entity).insert(activity);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 2. tick_parental_engagement — per-tick EMA update on each gradient
// ---------------------------------------------------------------------------

/// For each `ParentingActivity.relationships[i]`:
///   - Compute asymptote from owner's `Personality` via
///     [`parental_engagement_asymptote`]. If the target is despawned OR
///     no longer carries `KittenDependency` (matured / dead), multiply
///     asymptote by `matured_residual_factor` (residual lift survives).
///   - Decide build vs decay: build if owner is within
///     `engagement_range_tiles` of target's `Position`; decay otherwise.
///     ("Performing parental-class action" gating is deferred to a
///     follow-on; proximity covers the typical case.)
///   - Apply EMA: `engagement += (asymptote - engagement) × rate`.
///   - Refresh `last_interaction_tick` on build ticks.
///
/// Partner death: when `partner` is `Some(p)` but `p` is dead/despawned,
/// the partner field is left as-is (we don't have a `Dead` query reach
/// for arbitrary entities from this system without param bloat).
/// Re-asymptote-from-self-only semantics activate at the
/// `populate_parenting_scalars` site where we can check partner liveness
/// for the suppression factor.
pub fn tick_parental_engagement(
    mut owners: Query<(Entity, &Personality, &Position, &mut ParentingActivity), Without<Dead>>,
    target_positions: Query<&Position>,
    target_alive: Query<Has<KittenDependency>>,
    constants: Res<SimConstants>,
    time: Res<TimeState>,
) {
    let pc = &constants.parenting;
    let tick = time.tick;
    let range_sq = pc.engagement_range_tiles * pc.engagement_range_tiles;

    for (_owner, personality, owner_pos, mut activity) in owners.iter_mut() {
        // Asymptote is per-cat (function of Personality), not per-target,
        // so compute once and reuse. The matured-residual multiplier is
        // applied per-relationship.
        let base_asymptote = parental_engagement_asymptote(personality, 0.0, pc);

        for rel in activity.relationships.iter_mut() {
            // Target liveness — has KittenDependency if still a dependent
            // kitten. If despawned or matured, the asymptote drops to
            // residual; engagement decays toward that residual rather
            // than zero.
            let target_is_dependent = target_alive.get(rel.target).unwrap_or(false);
            let asymptote = if target_is_dependent {
                base_asymptote
            } else {
                base_asymptote * pc.matured_residual_factor
            };

            // Proximity check — only build if we can see the target's
            // position AND we're within range. Despawned targets return
            // None from the query, so they decay (frustrated
            // target-taking = grief).
            let in_range = match target_positions.get(rel.target) {
                Ok(target_pos) => {
                    let dx = (target_pos.x - owner_pos.x) as f32;
                    let dy = (target_pos.y - owner_pos.y) as f32;
                    dx * dx + dy * dy <= range_sq
                }
                Err(_) => false,
            };

            if in_range {
                let delta = (asymptote - rel.parental_engagement) * pc.engagement_build_rate;
                rel.parental_engagement = (rel.parental_engagement + delta).clamp(0.0, 1.0);
                rel.last_interaction_tick = tick;
            } else {
                let decay = rel.parental_engagement * pc.engagement_decay_rate;
                rel.parental_engagement = (rel.parental_engagement - decay).max(0.0);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 3. populate_parenting_scalars — bridge to modifier pipeline
// ---------------------------------------------------------------------------

/// For each cat with `ParentingActivity`, compute the six scalars the
/// `ParentingActivityModifier` reads and write them to the
/// [`ParentingScalars`] resource. Runs each tick after
/// [`tick_parental_engagement`] so gradients are fresh.
///
/// **Suppression-factor logic** — for each `RelationshipTo` whose
/// `partner.is_some()`, look up the partner's `HeldIntention`. If the
/// partner currently holds Caretake with `target` matching one of our
/// dependents, our `caretake_suppression_factor` collapses to
/// `joint_suppression_factor` (≈ 0.3); else it stays at `1.0`. We yield
/// to the partner without fully dropping (so a high-compassion second
/// parent can still snap to Caretake if the first lapses).
pub fn populate_parenting_scalars(
    activities: Query<(Entity, &Personality, &ParentingActivity), Without<Dead>>,
    held_intentions: Query<&HeldIntention, Without<Dead>>,
    constants: Res<SimConstants>,
    mut scalars: ResMut<ParentingScalars>,
) {
    scalars.clear();
    let pc = &constants.parenting;

    for (owner, personality, activity) in activities.iter() {
        if activity.relationships.is_empty() {
            continue;
        }

        let s_n = scale_presence(personality);
        let s_d = scale_provision(personality);
        let s_p = scale_protection(personality);
        let s_c = scale_cultural(personality);
        let s_a = scale_autonomy(personality, 0.0);

        let mut bundle = ParentingScalarBundle {
            caretake_suppression_factor: 1.0,
            ..ParentingScalarBundle::default()
        };
        // Collect this owner's dependent set so the suppression check
        // can verify the partner's HeldIntention.target ∈ dependents.
        let dependents: Vec<Entity> =
            activity.relationships.iter().map(|r| r.target).collect();

        for rel in &activity.relationships {
            // Track max engagement for the DSE-axis replacement (step 5).
            if rel.parental_engagement > bundle.parental_engagement_max {
                bundle.parental_engagement_max = rel.parental_engagement;
            }
            // Multiplier `bond_strength × parental_engagement` is the
            // "active parenting drive toward this target" magnitude.
            let weight = rel.bond_strength * rel.parental_engagement;
            if weight <= 0.0 {
                continue;
            }
            bundle.caretake_bias_sum += s_n * weight;
            bundle.provision_bias_sum += s_d * weight;
            bundle.protect_bias_sum += s_p * weight;
            bundle.cultural_teach_bias_sum += s_c * weight;
            bundle.autonomy_teach_bias_sum += s_a * weight;

            // JointIntention-aware suppression: if our partner for this
            // target holds Caretake against one of our dependents,
            // yield. Single-suppression suffices — first matching
            // partner triggers the dampening.
            if bundle.caretake_suppression_factor >= 1.0 {
                if let Some(partner) = rel.partner {
                    if let Ok(held) = held_intentions.get(partner) {
                        if is_held_caretake_against(held, &dependents) {
                            bundle.caretake_suppression_factor = pc.joint_suppression_factor;
                        }
                    }
                }
            }
        }

        scalars.insert(owner, bundle);
    }
}

/// True iff `held` is a Caretake commitment whose target is one of
/// `dependents`. Used by the JointIntention-aware suppression check.
fn is_held_caretake_against(held: &HeldIntention, dependents: &[Entity]) -> bool {
    if held.held_action != Action::Caretake {
        return false;
    }
    match held.target {
        Some(t) => dependents.contains(&t),
        None => false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_personality(v: f32) -> Personality {
        Personality {
            boldness: v,
            sociability: v,
            curiosity: v,
            diligence: v,
            warmth: v,
            spirituality: v,
            ambition: v,
            patience: v,
            anxiety: v,
            optimism: v,
            temper: v,
            stubbornness: v,
            playfulness: v,
            loyalty: v,
            tradition: v,
            compassion: v,
            pride: v,
            independence: v,
        }
    }

    #[test]
    fn asymptote_is_unity_for_flat_unit_personality() {
        let p = flat_personality(1.0);
        let constants = ParentingActivityConstants::default();
        // All five scales at 1.0 × weights summing to 1.0 = 1.0
        assert!((parental_engagement_asymptote(&p, 0.0, &constants) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn asymptote_leans_toward_presence() {
        // Cat with full Presence (compassion + warmth = 1.0) but zero
        // elsewhere: asymptote should be exactly w_presence = 0.30.
        let mut p = flat_personality(0.0);
        p.compassion = 1.0;
        p.warmth = 1.0;
        let constants = ParentingActivityConstants::default();
        // scale_autonomy bakes in (1.0 - overprotection_penalty) / 3.0
        // which means flat-zero autonomy at overprotection_penalty=0.0
        // still contributes (0 + 0 + 1)/3 = 0.333... So the asymptote
        // for a presence-only cat is 0.30 + 0.15 * 0.333... ≈ 0.35.
        let asymptote = parental_engagement_asymptote(&p, 0.0, &constants);
        assert!(asymptote > 0.30 && asymptote < 0.40);
    }

    #[test]
    fn high_compassion_father_engages_more_than_low_compassion_father() {
        let constants = ParentingActivityConstants::default();
        let mut high = flat_personality(0.5);
        high.compassion = 0.9;
        high.warmth = 0.9;
        let mut low = flat_personality(0.5);
        low.compassion = 0.1;
        low.warmth = 0.1;
        assert!(
            parental_engagement_asymptote(&high, 0.0, &constants)
                > parental_engagement_asymptote(&low, 0.0, &constants)
        );
    }
}
