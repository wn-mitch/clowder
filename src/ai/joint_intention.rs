//! Ticket 127 — JointIntention author system. Commit B.
//!
//! # Commit B — full author with stage progression + cascade
//!
//! Per-tick maintenance of `JointIntention { practice: Courtship, .. }`:
//!
//! 1. **Pre-tick partner snapshot.** Build
//!    `HashMap<Entity, (PracticeKind, Entity)>` from every existing JI.
//!    The snapshot is the pre-flush state — partner-removal happens via
//!    buffered `Commands` so cascade detection lags by one tick (cat A
//!    drops on tick T → cat B's snapshot still has A's JI on tick T →
//!    cat B drops on tick T+1 with `PartnerLeftPractice`). This meets
//!    the §Exit criterion 3 "within 1 tick" budget.
//!
//! 2. **Pre-tick interaction message drain.** Read
//!    [`JointInteractionObserved`] messages emitted by bias-reader call
//!    sites on the prior tick. For each message, bump the cat's
//!    `JointIntention.last_interaction_tick` to the message tick. This
//!    is the structural feed for the `Approach → Courting` stage-
//!    advance predicate.
//!
//! 3. **Per-cat drop + stage advance.** Build proxies and evaluate
//!    [`should_drop_joint`] / [`next_stage`]; remove the JI on drop
//!    and advance stage on transition. Fires `JointIntentionDropped`
//!    or `JointStageAdvanced` respectively.
//!
//! 4. **Mismatch-tick accrual.** Per pair, the lower-Entity-index side
//!    compares its own stage to partner's; on mismatch fires
//!    `JointStageMismatchTickAccrued { practice }`. Lower-index reports
//!    to avoid double-counting symmetric pairs.
//!
//! 5. **Pass 3 — mirror PA emission.** For each cat with
//!    `PairingActivity` but no `JointIntention` (PA author just emitted
//!    in the same tick), insert a fresh JI starting at
//!    `PracticeStage::CourtshipApproach`. Fire
//!    `JointIntentionEmitted { practice }`. **Once Commit C deletes PA,
//!    this pass is replaced by a full matchmaker** — for now we
//!    re-use PA's matchmaker via the lockstep mirror, which preserves
//!    migration parity mechanically.
//!
//! # Schedule
//!
//! Registered after `author_pairing_intentions` on the same chain so
//! the apply_deferred boundary makes Pass 3 see the freshly-inserted
//! PAs from this tick. Reads `MessageReader<JointInteractionObserved>`
//! to consume bias-reader interaction events from the prior tick (Bevy
//! Messages buffer between system runs; read order is intra-tick LIFO
//! against write order, which doesn't matter for our tick-bump
//! semantics).

use bevy_ecs::prelude::*;

use crate::ai::mating::{MatingFitness, MatingFitnessParams};
use crate::components::identity::{LifeStage, Orientation};
use crate::components::joint_intention::{
    next_stage, should_drop_joint, JointIntention, JointIntentionDropConfig, JointIntentionProxies,
    PracticeKind, PracticeStage, StageAdvanceProxies,
};
use crate::components::physical::Dead;
use crate::components::physical::Position;
use crate::components::pregnancy::Pregnant;
use crate::resources::relationships::{BondType, Relationships};
use crate::resources::sim_constants::{CourtshipPracticeConstants, SimConstants};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{SimConfig, TimeState};
use crate::systems::social::are_orientation_compatible;
use std::collections::HashMap;

/// Ticket 127 — Bias-reader call sites emit this when their resolver
/// target matches the actor's `JointIntention { Courtship }.partner`.
/// The author system reads the batch on the following tick and bumps
/// `JointIntention.last_interaction_tick` to the message tick, which
/// gates the `Approach → Courting` stage advance.
///
/// Decouples reads from writes — bias readers iterate `&JointIntention`
/// queries; the per-tick `last_interaction_tick` bump happens via
/// `&mut JointIntention` here. Bevy Messages are buffered between
/// system runs, so the bump lags by one tick.
#[derive(Message, Debug, Clone, Copy)]
pub struct JointInteractionObserved {
    pub entity: Entity,
    pub partner: Entity,
    pub practice: PracticeKind,
    pub tick: u64,
}

/// Ticket 127 Commit B — full author / drop / stage-advance / cascade
/// system. Replaces the Commit A `mirror_joint_intentions` mirror.
///
/// See module docs for the per-tick contract.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn author_joint_intentions(
    mut commands: Commands,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    constants: Res<SimConstants>,
    relationships: Res<Relationships>,
    mut activation: ResMut<SystemActivation>,
    mut interactions: MessageReader<JointInteractionObserved>,
    mating: MatingFitnessParams,
    // Mutable JI query — needed for `last_interaction_tick` bump and
    // stage advance. Disjoint from `Without<JointIntention>` query below
    // via marker disjointness.
    mut joints: Query<(Entity, &mut JointIntention), Without<Dead>>,
    // Eligible cats lacking a JointIntention — Pass 3 matchmaker
    // candidates. Carries position for range filter.
    needs_emit: Query<(Entity, &Position), (Without<JointIntention>, Without<Dead>)>,
    // All-cat positions for matchmaker peer scan (includes JI-holding
    // cats so the matchmaker can re-pair after a drop+rematch on the
    // same tick).
    all_positions: Query<(Entity, &Position), Without<Dead>>,
    // Partner-validity probe used by `PartnerInvalid` branch.
    invalidity: Query<(
        Has<Dead>,
        Has<crate::components::markers::Banished>,
        Has<crate::components::markers::Incapacitated>,
    )>,
    pregnant_q: Query<(), With<Pregnant>>,
) {
    let now_tick = time.tick;
    let season = time.season(&config);
    let drop_config = JointIntentionDropConfig {
        romantic_floor: constants.practices.courtship.romantic_floor,
        fondness_floor: constants.practices.courtship.fondness_floor,
        stage_stall_ticks: constants.practices.courtship.stage_stall_ticks,
    };

    // -----------------------------------------------------------------
    // Pass 0: drain interaction messages → bump last_interaction_tick.
    // -----------------------------------------------------------------
    let mut interaction_tick_for: HashMap<Entity, u64> = HashMap::new();
    for msg in interactions.read() {
        // Practice + partner filter at consume time — defensive against
        // future practices wiring stale messages.
        if msg.practice == PracticeKind::Courtship {
            // Keep the latest tick per entity.
            let entry = interaction_tick_for.entry(msg.entity).or_insert(msg.tick);
            if msg.tick > *entry {
                *entry = msg.tick;
            }
        }
    }
    for (entity, mut joint) in joints.iter_mut() {
        if let Some(&tick) = interaction_tick_for.get(&entity) {
            if tick > joint.last_interaction_tick {
                joint.last_interaction_tick = tick;
            }
        }
    }

    // -----------------------------------------------------------------
    // Pass 1: partner-state snapshot (post-Pass-0, pre-drop).
    // -----------------------------------------------------------------
    let partner_snapshot: HashMap<Entity, (PracticeKind, Entity)> = joints
        .iter()
        .map(|(e, j)| (e, (j.practice, j.partner)))
        .collect();
    let pregnant_snapshot: HashMap<Entity, bool> = joints
        .iter()
        .map(|(e, j)| (e, pregnant_q.get(j.partner).is_ok()))
        .collect();
    // Self-pregnant snapshot — used by Approach→Mating→Bonded gate.
    let self_pregnant_snapshot: HashMap<Entity, bool> = joints
        .iter()
        .map(|(e, _)| (e, pregnant_q.get(e).is_ok()))
        .collect();

    // -----------------------------------------------------------------
    // Pass 2: per-cat drop + stage advance + mismatch tracking.
    //
    // Snapshot of (entity, joint-copy) so we can use `commands` for
    // removal without aliasing the mutable JI query. Stage-advance
    // mutations write back through `joints` after the drop decisions
    // are made.
    // -----------------------------------------------------------------
    let fitness = mating.snapshot();
    let mut drop_decisions: Vec<(Entity, bool)> = Vec::new();
    let mut stage_advances: Vec<(Entity, PracticeStage)> = Vec::new();
    let mut mismatch_emissions: Vec<()> = Vec::new();

    for (entity, joint) in joints.iter() {
        let Some(self_fit) = fitness.get(&entity).copied() else {
            // Cat is dead / not in fitness snapshot — drop defensively.
            drop_decisions.push((entity, true));
            continue;
        };

        // Partner-validity probe.
        let partner_invalid = match invalidity.get(joint.partner) {
            Ok((dead, banished, incapacitated)) => dead || banished || incapacitated,
            Err(_) => true, // Despawned.
        };
        let bond = relationships
            .get(entity, joint.partner)
            .and_then(|r| r.bond);
        let (romantic, fondness) = relationships
            .get(entity, joint.partner)
            .map(|r| (r.romantic, r.fondness))
            .unwrap_or((0.0, 0.0));
        let self_is_pregnant = self_pregnant_snapshot
            .get(&entity)
            .copied()
            .unwrap_or(false)
            || self_fit.is_pregnant;
        let partner_is_pregnant = pregnant_snapshot.get(&entity).copied().unwrap_or(false);
        // Partner-in-practice cascade detection. Snapshot is pre-Pass-2,
        // so a partner who hasn't yet been removed appears in the
        // snapshot — the cascade fires on the FOLLOWING tick.
        let partner_in_practice = partner_snapshot
            .get(&joint.partner)
            .is_some_and(|(kind, p_partner)| *kind == joint.practice && *p_partner == entity);

        let proxies = JointIntentionProxies {
            self_stage: self_fit.stage,
            self_orientation: self_fit.orientation,
            self_gender: self_fit.gender,
            self_is_pregnant,
            self_fertility_phase: self_fit.fertility_phase,
            partner_invalid,
            partner_in_practice,
            partner_is_pregnant,
            bond,
            romantic,
            fondness,
            season,
            practice: joint.practice,
            current_stage: joint.stage,
            stage_entered_tick: joint.stage_entered_tick,
            now_tick,
            still_compatible: is_practice_compatible_now(
                joint.practice,
                self_fit.stage,
                self_fit.orientation,
                self_is_pregnant,
            ),
        };

        if let Some(_branch) = should_drop_joint(&proxies, &drop_config) {
            drop_decisions.push((entity, true));
            continue;
        }

        // Stage advance — only when drop didn't fire.
        let stage_proxies = StageAdvanceProxies {
            current_stage: joint.stage,
            last_interaction_tick: joint.last_interaction_tick,
            adopted_tick: joint.adopted_tick,
            bond,
            self_gender: self_fit.gender,
            season,
            self_fertility_phase: self_fit.fertility_phase,
            self_is_pregnant,
            partner_is_pregnant,
        };
        if let Some(new_stage) = next_stage(&stage_proxies) {
            stage_advances.push((entity, new_stage));
        }

        // Mismatch tracking — lower-Entity-index side reports to avoid
        // double-counting. Only when partner still has a JI and stages
        // differ.
        if let Some((p_kind, p_partner_entity)) = partner_snapshot.get(&joint.partner) {
            if *p_kind == joint.partner_practice() && *p_partner_entity == entity {
                // Look up partner's stage from the joints query.
                if let Some((_, partner_joint)) = joints.iter().find(|(pe, _)| *pe == joint.partner)
                {
                    if entity.index() < joint.partner.index() && partner_joint.stage != joint.stage
                    {
                        mismatch_emissions.push(());
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------
    // Apply mutations.
    // -----------------------------------------------------------------
    for (entity, drop) in drop_decisions {
        if drop {
            commands.entity(entity).remove::<JointIntention>();
            activation.record(Feature::JointIntentionDropped {
                practice: PracticeKind::Courtship,
            });
        }
    }
    for (entity, new_stage) in stage_advances {
        if let Ok((_, mut joint)) = joints.get_mut(entity) {
            joint.stage = new_stage;
            joint.stage_entered_tick = now_tick;
            activation.record(Feature::JointStageAdvanced {
                practice: PracticeKind::Courtship,
            });
        }
    }
    for _ in mismatch_emissions {
        activation.record(Feature::JointStageMismatchTickAccrued {
            practice: PracticeKind::Courtship,
        });
    }

    // -----------------------------------------------------------------
    // Pass 3: matchmaker — emit JointIntention for eligible cats
    // lacking one. Replaces the Commit B PA-mirror with a real
    // matchmaker now that Commit C deletes `author_pairing_intentions`.
    //
    // Matchmaker: scan within `candidate_range`; orientation-compatible
    // + reproductive + Friends-or-better bonded peers; quality score
    // > `emission_threshold`; tie-break by stable Entity::index() asc.
    // Mirrors the prior PA matchmaker 1:1 so migration parity is
    // mechanical — the only behavioral lift comes from the substrate
    // adding stage progression and partner-cascade, not from the
    // emission predicate changing.
    // -----------------------------------------------------------------
    let practice_constants = &constants.practices.courtship;
    let positions: Vec<(Entity, Position)> = all_positions.iter().map(|(e, p)| (e, *p)).collect();

    for (entity, position) in needs_emit.iter() {
        let Some(self_fit) = fitness.get(&entity).copied() else {
            continue;
        };
        if !is_reproductive_for_courtship(&self_fit) {
            continue;
        }
        let Some(partner) = pick_courtship_partner(
            entity,
            position,
            self_fit,
            &positions,
            &fitness,
            &relationships,
            practice_constants,
        ) else {
            continue;
        };

        commands.entity(entity).insert(JointIntention::new(
            PracticeKind::Courtship,
            partner,
            now_tick,
        ));
        activation.record(Feature::JointIntentionEmitted {
            practice: PracticeKind::Courtship,
        });
    }
}

// ---------------------------------------------------------------------------
// Courtship matchmaker helpers (moved from `crate::ai::pairing` in 127 Commit C)
// ---------------------------------------------------------------------------

/// L1-equivalent reproductive-eligibility predicate for Courtship.
/// Mirrors the prior `crate::ai::pairing::is_reproductive` 1:1.
fn is_reproductive_for_courtship(f: &MatingFitness) -> bool {
    matches!(
        f.stage,
        crate::components::identity::LifeStage::Adult
            | crate::components::identity::LifeStage::Elder
    ) && f.orientation != crate::components::identity::Orientation::Asexual
        && !f.is_pregnant
}

/// Map a bond tier to a graduated scalar for the courtship-quality
/// score. Mirrors `socialize_target::bond_score`'s vocabulary so the
/// L2 emission decision uses the same scale the bias readers do.
fn bond_tier_score(bond: Option<BondType>) -> f32 {
    match bond {
        Some(BondType::Mates | BondType::Partners) => 1.0,
        Some(BondType::Friends) => 0.5,
        None => 0.0,
    }
}

/// Pick the best Courtship partner for `self_entity` from within
/// `candidate_range`. Returns `None` when no candidate clears
/// `emission_threshold`. Stable tie-break via `Entity::index()` asc.
///
/// Moved from `crate::ai::pairing::pick_partner` in 127 Commit C.
/// Future practices declare their own matchmaker via per-practice
/// dispatch.
fn pick_courtship_partner(
    self_entity: Entity,
    self_position: &Position,
    self_fit: MatingFitness,
    positions: &[(Entity, Position)],
    fitness: &HashMap<Entity, MatingFitness>,
    relationships: &Relationships,
    practice_constants: &CourtshipPracticeConstants,
) -> Option<Entity> {
    // Ticket 453: Mates-exclusivity perception gate. A cat already in a
    // `BondType::Mates` bond does not emit new Courtship JointIntentions
    // toward third parties under the current substrate shape.
    if relationships
        .iter_for(self_entity)
        .any(|(_, rel)| rel.bond == Some(BondType::Mates))
    {
        return None;
    }

    let range = practice_constants.candidate_range;
    let weights = (
        practice_constants.quality_fondness_weight,
        practice_constants.quality_romantic_weight,
        practice_constants.quality_bond_weight,
    );

    let mut best: Option<(Entity, f32)> = None;
    for (other, other_pos) in positions.iter() {
        if *other == self_entity {
            continue;
        }
        let manhattan =
            (self_position.x - other_pos.x).abs() + (self_position.y - other_pos.y).abs();
        if manhattan > range {
            continue;
        }
        let Some(other_fit) = fitness.get(other) else {
            continue;
        };
        if !is_reproductive_for_courtship(other_fit) {
            continue;
        }
        if !are_orientation_compatible(
            self_fit.gender,
            self_fit.orientation,
            other_fit.gender,
            other_fit.orientation,
        ) {
            continue;
        }
        let Some(rel) = relationships.get(self_entity, *other) else {
            continue;
        };
        let bond_score = bond_tier_score(rel.bond);
        if bond_score == 0.0 {
            continue;
        }
        // Ticket 453: skip candidates already Mates-bonded to a third
        // party. (Self-Mates already excluded by the early return above.)
        if relationships.iter_for(*other).any(|(third, third_rel)| {
            third != self_entity && third_rel.bond == Some(BondType::Mates)
        }) {
            continue;
        }
        let fondness = rel.fondness.max(0.0);
        let romantic = rel.romantic.max(0.0);
        let score = weights.0 * fondness + weights.1 * romantic + weights.2 * bond_score;
        if score < practice_constants.emission_threshold {
            continue;
        }
        let candidate = (*other, score);
        match best {
            Some((_, best_score)) if best_score >= score => {}
            _ => best = Some(candidate),
        }
    }
    best.map(|(e, _)| e)
}

/// Inline practice-compatibility recheck. Mirrors the eligibility
/// predicate `crate::ai::pairing::is_reproductive` for Courtship:
/// life-stage Adult/Elder + non-Asexual + not pregnant.
///
/// Distinct from the `AspirationCascade` drop branch which fires on
/// `self_stage / self_orientation / self_is_pregnant`: this captures
/// the practice's own compat predicate, which today happens to be the
/// same for Courtship. Future practices declare their own.
fn is_practice_compatible_now(
    practice: PracticeKind,
    stage: LifeStage,
    orientation: Orientation,
    self_is_pregnant: bool,
) -> bool {
    match practice {
        PracticeKind::Courtship => {
            matches!(stage, LifeStage::Adult | LifeStage::Elder)
                && orientation != Orientation::Asexual
                && !self_is_pregnant
        }
    }
}

// Helper trait extension — keeps the mismatch comparison readable.
trait JointIntentionExt {
    fn partner_practice(&self) -> PracticeKind;
}
impl JointIntentionExt for JointIntention {
    fn partner_practice(&self) -> PracticeKind {
        // For symmetric practices, the partner's expected practice is
        // the same as self's. Asymmetric practices (future) may differ;
        // resolved by a per-practice dispatch.
        self.practice
    }
}

// ---------------------------------------------------------------------------
// InLaw adoption rule — ticket 400
// ---------------------------------------------------------------------------

/// Ticket 400 — InLaw adoption on `PracticeStage::CourtshipBonded`
/// transition. When a `JointIntention { practice: Courtship }` advances
/// to `CourtshipBonded`, each partner's biological parents gain an
/// `InLaw`-kind `RelationshipTo` entry on their `ParentingActivity`
/// pointing at the other partner. Mirrored both directions: my parents
/// get InLaw entries toward my new mate; my mate's parents get InLaw
/// entries toward me.
///
/// **Detection.** Runs after [`author_joint_intentions`] (which sets
/// `joint.stage_entered_tick = now_tick` on advance). A cat with
/// `joint.stage == CourtshipBonded && joint.stage_entered_tick == now_tick`
/// just transitioned this tick.
///
/// **Reverse-lookup.** Biological-parent identification scans all
/// `ParentingActivity` Components for entries with `kind = Biological`
/// and `target = X`: each such owner is one of X's biological parents.
/// `KittenDependency` is unusable for this — adults shed the Component
/// when they mature.
///
/// **Idempotency.** `ParentingActivity::has_kind` guards against
/// duplicate InLaw entries. If a JointIntention briefly drops and
/// re-enters Bonded, the rule fires again but does not duplicate.
///
/// **No alloparenting yet.** Only `Biological`-kind parents propagate
/// InLaw status. `BondFormed` and `Adopted` parental adoption rules
/// land in follow-on tickets 403 / 404 and will participate naturally
/// in this same rule (any cat carrying a parental stance toward X
/// gains InLaw toward X's new mate).
#[allow(clippy::type_complexity)]
pub fn apply_inlaw_adoption_on_bonded(
    time: Res<TimeState>,
    joints: Query<(Entity, &JointIntention), Without<Dead>>,
    mut parenting: bevy_ecs::system::ParamSet<(
        Query<(
            Entity,
            &crate::components::parenting_activity::ParentingActivity,
        )>,
        Query<&mut crate::components::parenting_activity::ParentingActivity>,
    )>,
) {
    use crate::components::parenting_activity::{ParentalKind, RelationshipTo};

    let now_tick = time.tick;

    // 1. Find cats that just entered CourtshipBonded this tick.
    let fresh_bonds: Vec<(Entity, Entity)> = joints
        .iter()
        .filter_map(|(entity, joint)| {
            (joint.stage == PracticeStage::CourtshipBonded && joint.stage_entered_tick == now_tick)
                .then_some((entity, joint.partner))
        })
        .collect();
    if fresh_bonds.is_empty() {
        return;
    }

    // 2. Build reverse-lookup: child -> Vec<parent_entity> via
    // ParentingActivity scan. Only Biological-kind entries propagate
    // InLaw status in 400.
    let mut parents_of: HashMap<Entity, Vec<Entity>> = HashMap::new();
    {
        let read = parenting.p0();
        for (cat, pa) in read.iter() {
            for rel in &pa.relationships {
                if rel.kind == ParentalKind::Biological {
                    parents_of.entry(rel.target).or_default().push(cat);
                }
            }
        }
    }

    // 3. Determine InLaw insertions per (parent_to_modify, target_inlaw).
    // Mirror in both directions per the design.
    let mut insertions: Vec<(Entity, Entity)> = Vec::new();
    for (entity, partner) in &fresh_bonds {
        if let Some(my_parents) = parents_of.get(entity) {
            for &p in my_parents {
                insertions.push((p, *partner));
            }
        }
        if let Some(partner_parents) = parents_of.get(partner) {
            for &p in partner_parents {
                insertions.push((p, *entity));
            }
        }
    }
    if insertions.is_empty() {
        return;
    }

    // 4. Apply insertions via the mutable arm of the ParamSet.
    let mut write = parenting.p1();
    for (parent_entity, in_law_target) in insertions {
        if let Ok(mut pa) = write.get_mut(parent_entity) {
            if pa.has_kind(in_law_target, ParentalKind::InLaw) {
                continue;
            }
            pa.relationships.push(RelationshipTo::new(
                in_law_target,
                ParentalKind::InLaw,
                None,
                now_tick,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::fertility::{Fertility, FertilityPhase};
    use crate::components::identity::{Age, Gender, Name, Orientation};
    use crate::components::mental::Mood;
    use crate::components::physical::{Health, Needs, Position};
    use crate::resources::relationships::BondType;
    use crate::resources::time::{SimConfig, TimeState};
    use crate::resources::SimConstants;
    use bevy_ecs::schedule::Schedule;

    /// Spawn an Adult cat with all per-cat fertility / sated-and-happy
    /// gates open. Adapted from `pairing::tests::spawn_eligible_adult`.
    fn spawn_eligible_adult(
        world: &mut World,
        name: &str,
        gender: Gender,
        orientation: Orientation,
        position: Position,
    ) -> Entity {
        let mut needs = Needs::default();
        needs.hunger = 0.9;
        needs.energy = 0.9;
        needs.mating = 0.3;
        let mut mood = Mood::default();
        mood.valence = 0.5;
        let fertility = if matches!(gender, Gender::Tom) {
            None
        } else {
            Some(Fertility {
                phase: FertilityPhase::Estrus,
                cycle_offset: 0,
                post_partum_remaining_ticks: 0,
            })
        };
        let mut entity = world.spawn((
            Name(name.to_string()),
            Age { born_tick: 0 },
            gender,
            orientation,
            mood,
            needs,
            position,
            Health::default(),
        ));
        if let Some(f) = fertility {
            entity.insert(f);
        }
        entity.id()
    }

    fn author_world() -> World {
        let mut world = World::new();
        let mut time = TimeState::default();
        time.tick = 20_000 * 13;
        world.insert_resource(time);
        world.insert_resource(SimConfig::default());
        world.insert_resource(SimConstants::default());
        world.insert_resource(Relationships::default());
        world.insert_resource(SystemActivation::default());
        world.insert_resource(bevy_ecs::message::Messages::<JointInteractionObserved>::default());
        world
    }

    fn run_author(world: &mut World) {
        let mut schedule = Schedule::default();
        schedule.add_systems(author_joint_intentions);
        schedule.run(world);
    }

    fn make_paired_cats(world: &mut World) -> (Entity, Entity) {
        let a = spawn_eligible_adult(
            world,
            "Fern",
            Gender::Queen,
            Orientation::Straight,
            Position::new(0, 0),
        );
        let b = spawn_eligible_adult(
            world,
            "Reed",
            Gender::Tom,
            Orientation::Straight,
            Position::new(1, 0),
        );
        let mut rels = world.resource_mut::<Relationships>();
        let rel = rels.get_or_insert(a, b);
        rel.bond = Some(BondType::Friends);
        rel.fondness = 0.5;
        (a, b)
    }

    #[test]
    fn author_emits_joint_intention_for_compatible_paired_adults() {
        let mut world = author_world();
        let (a, b) = make_paired_cats(&mut world);

        run_author(&mut world);

        for (entity, expected_partner) in [(a, b), (b, a)] {
            let ji = world
                .get::<JointIntention>(entity)
                .expect("JI authored by Commit C in-tree matchmaker");
            assert_eq!(ji.partner, expected_partner);
            assert_eq!(ji.practice, PracticeKind::Courtship);
            assert_eq!(ji.stage, PracticeStage::CourtshipApproach);
        }
        let activation = world.resource::<SystemActivation>();
        assert_eq!(
            activation
                .counts
                .get(&Feature::JointIntentionEmitted {
                    practice: PracticeKind::Courtship,
                })
                .copied(),
            Some(2),
            "symmetric pair → both cats emit one JI each"
        );
    }

    #[test]
    fn stage_advances_approach_to_courting_after_interaction() {
        let mut world = author_world();
        let (a, b) = make_paired_cats(&mut world);
        run_author(&mut world);
        // Sanity — both at Approach.
        assert_eq!(
            world.get::<JointIntention>(a).unwrap().stage,
            PracticeStage::CourtshipApproach
        );

        // Emit a JointInteractionObserved for `a` at a later tick.
        let now = world.resource::<TimeState>().tick;
        let later = now + 100;
        world
            .resource_mut::<bevy_ecs::message::Messages<JointInteractionObserved>>()
            .write(JointInteractionObserved {
                entity: a,
                partner: b,
                practice: PracticeKind::Courtship,
                tick: later,
            });

        // Advance time so the author sees the message + bumps the tick.
        world.resource_mut::<TimeState>().tick = later;
        run_author(&mut world);

        let ji_a = world.get::<JointIntention>(a).unwrap();
        assert_eq!(ji_a.last_interaction_tick, later);
        assert_eq!(
            ji_a.stage,
            PracticeStage::CourtshipCourting,
            "Approach→Courting on first paired-resolver interaction"
        );
        let activation = world.resource::<SystemActivation>();
        assert!(
            activation
                .counts
                .get(&Feature::JointStageAdvanced {
                    practice: PracticeKind::Courtship,
                })
                .copied()
                .unwrap_or(0)
                >= 1,
            "JointStageAdvanced fires on the transition"
        );
    }

    #[test]
    fn partner_left_practice_cascade_fires_within_one_tick() {
        // §Exit criterion 3 — drop cascade works within 1 tick.
        let mut world = author_world();
        let (a, b) = make_paired_cats(&mut world);
        run_author(&mut world);
        assert!(world.get::<JointIntention>(a).is_some());
        assert!(world.get::<JointIntention>(b).is_some());

        // Force-remove a's JI (simulating a desire-drift drop on a).
        // Also drop the relationship bond so the matchmaker can't
        // re-emit JI for a or b in the same tick's Pass 3.
        world.entity_mut(a).remove::<JointIntention>();
        let mut rels = world.resource_mut::<Relationships>();
        rels.get_or_insert(a, b).bond = None;

        // Tick T+1 — partner snapshot reflects a's removal; b's drop
        // gate fires PartnerLeftPractice.
        run_author(&mut world);

        assert!(
            world.get::<JointIntention>(b).is_none(),
            "b's JointIntention dropped via PartnerLeftPractice cascade"
        );
        let activation = world.resource::<SystemActivation>();
        assert!(
            activation
                .counts
                .get(&Feature::JointIntentionDropped {
                    practice: PracticeKind::Courtship,
                })
                .copied()
                .unwrap_or(0)
                >= 1,
            "JointIntentionDropped fires on the cascade"
        );
    }

    // -----------------------------------------------------------------
    // Ticket 453 — Mates-exclusivity perception gates on
    // `pick_courtship_partner`. The matchmaker refuses to emit a
    // Courtship intention when the actor is already Mates-bonded, and
    // skips candidates already Mates-bonded with a third party.
    // -----------------------------------------------------------------

    fn default_eligible_fitness(gender: Gender) -> MatingFitness {
        MatingFitness {
            stage: LifeStage::Adult,
            gender,
            orientation: Orientation::Straight,
            mood_valence: 0.5,
            hunger: 0.9,
            energy: 0.9,
            is_pregnant: false,
            fertility_phase: if matches!(gender, Gender::Tom) {
                None
            } else {
                Some(FertilityPhase::Estrus)
            },
            body_condition: 1.0,
        }
    }

    #[test]
    fn pick_courtship_partner_returns_none_when_self_already_mated() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();

        let mut fitness: HashMap<Entity, MatingFitness> = HashMap::new();
        fitness.insert(a, default_eligible_fitness(Gender::Queen));
        fitness.insert(b, default_eligible_fitness(Gender::Tom));
        fitness.insert(c, default_eligible_fitness(Gender::Tom));

        let mut rels = Relationships::default();
        // A is already Mates with B — exclusivity precondition.
        let ab = rels.get_or_insert(a, b);
        ab.bond = Some(BondType::Mates);
        ab.fondness = 0.8;
        ab.romantic = 0.8;
        // A and C also share a strong courtship-bonded relationship that
        // would otherwise qualify C as a target.
        let ac = rels.get_or_insert(a, c);
        ac.bond = Some(BondType::Partners);
        ac.fondness = 0.8;
        ac.romantic = 0.8;

        let positions = vec![
            (a, Position::new(0, 0)),
            (b, Position::new(1, 0)),
            (c, Position::new(2, 0)),
        ];
        let constants = SimConstants::default();
        let picked = pick_courtship_partner(
            a,
            &Position::new(0, 0),
            default_eligible_fitness(Gender::Queen),
            &positions,
            &fitness,
            &rels,
            &constants.practices.courtship,
        );
        assert!(
            picked.is_none(),
            "actor already Mates-bonded must not emit new courtship; got {picked:?}"
        );
    }

    #[test]
    fn pick_courtship_partner_skips_candidates_mated_elsewhere() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();
        let d = world.spawn_empty().id();

        let mut fitness: HashMap<Entity, MatingFitness> = HashMap::new();
        fitness.insert(a, default_eligible_fitness(Gender::Queen));
        fitness.insert(b, default_eligible_fitness(Gender::Tom));
        fitness.insert(c, default_eligible_fitness(Gender::Tom));
        fitness.insert(d, default_eligible_fitness(Gender::Queen));

        let mut rels = Relationships::default();
        // A's nearby targets: B and C, both with Partners bonds —
        // ordinarily both would be eligible. But C is Mates-bonded
        // with D elsewhere, so C should be skipped.
        let ab = rels.get_or_insert(a, b);
        ab.bond = Some(BondType::Partners);
        ab.fondness = 0.8;
        ab.romantic = 0.8;
        let ac = rels.get_or_insert(a, c);
        ac.bond = Some(BondType::Partners);
        ac.fondness = 0.9; // higher quality — would win without the gate
        ac.romantic = 0.9;
        let cd = rels.get_or_insert(c, d);
        cd.bond = Some(BondType::Mates);
        cd.fondness = 0.9;
        cd.romantic = 0.9;

        let positions = vec![
            (a, Position::new(0, 0)),
            (b, Position::new(1, 0)),
            (c, Position::new(1, 1)),
            (d, Position::new(5, 5)),
        ];
        let constants = SimConstants::default();
        let picked = pick_courtship_partner(
            a,
            &Position::new(0, 0),
            default_eligible_fitness(Gender::Queen),
            &positions,
            &fitness,
            &rels,
            &constants.practices.courtship,
        );
        assert_eq!(
            picked,
            Some(b),
            "Mates-bonded candidate C must be filtered out, leaving B; got {picked:?}"
        );
    }
}
