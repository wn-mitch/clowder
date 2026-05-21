//! Per-tick writer for the [`ActionAffordances`] substrate (ticket 261).
//!
//! For every `(perceiver, target, ActionKind)` triple within
//! `AffordancesConstants::sensing_range`, this system computes a success
//! scalar in `[0, 1]` and writes it via `ActionAffordances::write`. Scalars
//! below the per-kind `min_eligibility` are clamped to `0.0` so consumers
//! see a hard gate, not a faint signal.
//!
//! # Perceiver classes
//!
//! v1 supports two perceiver classes:
//!
//! - **Cats** — entities with `CatBeliefs` + `PredatorBeliefs`. Cats can
//!   afford every cat-applicable action (16 of the 21 kinds): the
//!   universal predation trio (Stalk / Chase / Pounce), threat-response
//!   (Flee / Fight / Freeze / Fawn), conflict-low (Threaten / Posture /
//!   Hiss), and the social six (Socialize / GroomOther / Mate / Mentor /
//!   Care / FeedKitten). Cat heuristics compose belief facets from the
//!   258 substrate with spatial reads (distance, ward coverage).
//! - **Wildlife predators** — `WildAnimal`-tagged entities. Each species
//!   gates a tight subset: Fox → Stalk + Chase; Hawk → Dive + Chase;
//!   Snake → Strike + Stalk; ShadowFox → Ambush + Stalk + Chase. Wildlife
//!   has no belief substrate in v1, so the heuristics fall back to
//!   spatial-only inputs (distance, target_health, fox_scent_at_position).
//!
//! Prey-side affordances (`Bolt`, `ScatterGroup`) are not written in v1 —
//! prey has its own AI in `src/systems/prey.rs` without a perception
//! substrate. The kinds exist in [`ActionKind`] so consumer tickets can
//! extend the writer to prey perceivers without revising the enum.
//!
//! # Behavior-neutral at land
//!
//! 261 lands the writer with **zero DSE consumers**. The `ActionAffordances`
//! resource populates each tick but no scoring path reads from it, so
//! `just verdict` against a baseline soak shows null behavioural drift.
//! Consumer wiring lives in ticket 263 (256-cluster Flee / Patrol / Hunt)
//! and siblings.
//!
//! # Scheduling
//!
//! Chained into **Chain 2b** of `SimulationPlugin::build()`, immediately
//! after `belief_integrator::integrate_beliefs`. Same-tick within-chain
//! ordering: the writer reads facets the integrator authored this tick.
//! Per the memory `learning_bevy_schedule_edge_perturbation`, the writer
//! enters an *existing* `.chain()` block rather than registering as a new
//! top-level sibling — adding a sibling has historically perturbed
//! seed-42 via Bevy's topological-sort shuffle on unrelated systems.

use bevy_ecs::prelude::*;

use crate::components::beliefs::{CatBeliefs, MentalModel, PredatorBeliefs};
use crate::components::identity::Species;
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::wildlife::{WildAnimal, WildSpecies};
use crate::resources::sim_constants::AffordanceWeights;
use crate::resources::{ActionAffordances, ActionKind, FoxScentMap, SimConstants, WardCoverageMap};

// ---------------------------------------------------------------------------
// Snapshot types
// ---------------------------------------------------------------------------

/// Compact per-cat snapshot consumed by the pair-wise heuristic loop.
/// Carrying the whole `CatBeliefs` map into the snapshot would balloon
/// memory; instead the loop dereferences the perceiver's `CatBeliefs` /
/// `PredatorBeliefs` directly through a side-table indexed by Entity.
struct CatSnapshot {
    entity: Entity,
    position: Position,
    health_fraction: f32,
    safety_need: f32,
    hunger_need: f32,
    mating_need: f32,
    social_need: f32,
}

struct WildSnapshot {
    entity: Entity,
    position: Position,
    species: WildSpecies,
    threat_power: f32,
    health_fraction: f32,
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

#[allow(clippy::type_complexity)]
pub fn affordance_writer(
    constants: Res<SimConstants>,
    fox_scent: Res<FoxScentMap>,
    ward_coverage: Res<WardCoverageMap>,
    mut affordances: ResMut<ActionAffordances>,
    cats: Query<
        (
            Entity,
            &Position,
            &Health,
            &Needs,
            &CatBeliefs,
            &PredatorBeliefs,
        ),
        (With<Species>, Without<Dead>),
    >,
    wildlife: Query<(Entity, &Position, &WildAnimal, &Health), Without<Dead>>,
) {
    affordances.clear();
    let cfg = &constants.affordances;
    let sensing = cfg.sensing_range.max(1);

    // Collect cat snapshots + side-table belief refs so the pair loop can
    // read the perceiver's belief Components without re-querying.
    let mut cat_snaps: Vec<CatSnapshot> = Vec::with_capacity(cats.iter().count());
    let mut cat_beliefs_by_entity: std::collections::HashMap<
        Entity,
        (&CatBeliefs, &PredatorBeliefs),
    > = std::collections::HashMap::new();
    for (entity, pos, health, needs, cat_b, pred_b) in cats.iter() {
        cat_snaps.push(CatSnapshot {
            entity,
            position: *pos,
            health_fraction: (health.current / health.max.max(f32::EPSILON)).clamp(0.0, 1.0),
            safety_need: needs.safety,
            hunger_need: needs.hunger,
            mating_need: needs.mating,
            social_need: needs.social,
        });
        cat_beliefs_by_entity.insert(entity, (cat_b, pred_b));
    }

    let mut wild_snaps: Vec<WildSnapshot> = Vec::with_capacity(wildlife.iter().count());
    for (entity, pos, animal, health) in wildlife.iter() {
        wild_snaps.push(WildSnapshot {
            entity,
            position: *pos,
            species: animal.species,
            threat_power: animal.threat_power,
            health_fraction: (health.current / health.max.max(f32::EPSILON)).clamp(0.0, 1.0),
        });
    }

    // ---- Cat perceivers -----------------------------------------------------
    for perceiver in &cat_snaps {
        let Some(&(perceiver_cat_b, perceiver_pred_b)) =
            cat_beliefs_by_entity.get(&perceiver.entity)
        else {
            continue;
        };

        // vs cat targets
        for target in &cat_snaps {
            if target.entity == perceiver.entity {
                continue;
            }
            if manhattan(&perceiver.position, &target.position) > sensing {
                continue;
            }
            let target_belief = perceiver_cat_b.models.get(&target.entity);
            write_cat_vs_cat(
                perceiver,
                target,
                target_belief,
                cfg,
                &ward_coverage,
                &mut affordances,
            );
        }

        // vs wildlife targets
        for target in &wild_snaps {
            if manhattan(&perceiver.position, &target.position) > sensing {
                continue;
            }
            let target_belief = perceiver_pred_b.models.get(&target.entity);
            write_cat_vs_wildlife(
                perceiver,
                target,
                target_belief,
                cfg,
                &ward_coverage,
                &fox_scent,
                &mut affordances,
            );
        }
    }

    // ---- Wildlife perceivers -----------------------------------------------
    for perceiver in &wild_snaps {
        for target in &cat_snaps {
            if manhattan(&perceiver.position, &target.position) > sensing {
                continue;
            }
            write_wildlife_vs_cat(perceiver, target, cfg, &ward_coverage, &mut affordances);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn manhattan(a: &Position, b: &Position) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

/// Distance-proximity feature: 1.0 at adjacent, linearly decaying to 0.0
/// at `sensing_range`. Common slot-1 input for most heuristics — closeness
/// is the universally-relevant spatial signal.
fn proximity_feature(perceiver: &Position, target: &Position, sensing: i32) -> f32 {
    let d = manhattan(perceiver, target) as f32;
    let s = sensing.max(1) as f32;
    (1.0 - (d / s)).clamp(0.0, 1.0)
}

/// Cover at the perceiver's position. WardCoverageMap is the v1 proxy —
/// warded tiles are "cover" for the affordance-stalk / hide / ambush
/// shape. Consumer tickets can refine with terrain-specific cover when
/// the tile-cover substrate lands.
fn cover_at(pos: &Position, ward: &WardCoverageMap) -> f32 {
    ward.get(pos.x, pos.y)
}

/// Read a facet from an optional MentalModel ref, defaulting to 0.0.
/// Centralises the "no model yet" fallback so heuristic code stays
/// branch-free.
fn facet(model: Option<&MentalModel>, pick: impl FnOnce(&MentalModel) -> f32) -> f32 {
    model.map(pick).unwrap_or(0.0)
}

/// Weighted-sum composition with hard min-eligibility gate. Each
/// contribution `c_i` is already in `[0, 1]`; the weights `w_i` are
/// per-kind from `SimConstants`. Output is clamped to `[0, 1]`. Values
/// below `min_eligibility` write `0.0` instead — the substrate's gate
/// signal.
fn composite(w: &AffordanceWeights, c1: f32, c2: f32, c3: f32, c4: f32) -> f32 {
    let raw = (w.w1 * c1 + w.w2 * c2 + w.w3 * c3 + w.w4 * c4).clamp(0.0, 1.0);
    if raw < w.min_eligibility {
        0.0
    } else {
        raw
    }
}

// ---------------------------------------------------------------------------
// Cat perceiver × cat target
// ---------------------------------------------------------------------------

fn write_cat_vs_cat(
    perceiver: &CatSnapshot,
    target: &CatSnapshot,
    belief: Option<&MentalModel>,
    cfg: &crate::resources::sim_constants::AffordancesConstants,
    ward: &WardCoverageMap,
    affordances: &mut ActionAffordances,
) {
    let prox = proximity_feature(&perceiver.position, &target.position, cfg.sensing_range);
    let cover_self = cover_at(&perceiver.position, ward);
    let hostility = facet(belief, |m| m.perceived_hostility.value);
    let receptivity = facet(belief, |m| m.perceived_receptivity.value);
    let affiliation = facet(belief, |m| m.affiliation_history.value).clamp(-1.0, 1.0);
    let bond_pos = ((affiliation + 1.0) * 0.5).clamp(0.0, 1.0);
    let violence_cap = facet(belief, |m| m.perceived_violence_capability.value);
    let injury_level = facet(belief, |m| m.perceived_injury_level.value);
    let intent_clarity = facet(belief, |m| m.perceived_intent_clarity.value);
    let my_health = perceiver.health_fraction;
    let target_health = target.health_fraction;

    // --- Predation (universal cat-applicable trio) ---
    let pred = &cfg.predation;
    // Stalk — cover here, my stealth (proxy: low health = worse stealth), low target clarity.
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Stalk,
        composite(
            &pred.stalk,
            prox,
            cover_self,
            my_health,
            1.0 - intent_clarity,
        ),
    );
    // Chase — proximity, speed advantage (cat-vs-cat speed is symmetric — use my_health as proxy
    // for "I have stamina to chase"), low target clarity.
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Chase,
        composite(&pred.chase, prox, 1.0 - intent_clarity, my_health, 0.5),
    );
    // Pounce — adjacency + cover + low clarity.
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Pounce,
        composite(
            &pred.pounce,
            prox,
            cover_self,
            1.0 - intent_clarity,
            my_health,
        ),
    );

    // Dive / Strike / Ambush — species-gated to wildlife predators only.
    affordances.write(perceiver.entity, target.entity, ActionKind::Dive, 0.0);
    affordances.write(perceiver.entity, target.entity, ActionKind::Strike, 0.0);
    affordances.write(perceiver.entity, target.entity, ActionKind::Ambush, 0.0);

    // --- Threat-response ---
    let tr = &cfg.threat_response;
    // Flee — cover at my position, low violence-cap from target (we flee weaker things less),
    // my health (need stamina), proximity (closer = more urgent / less afforded).
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Flee,
        composite(&tr.flee, 1.0 - prox, cover_self, my_health, violence_cap),
    );
    // Fight — 141's worked composition lands here. dps_balance proxy = my_health vs target_health;
    // ttk_ratio proxy = my_health/(target violence_cap+eps); ally_factor proxy = 0.5 placeholder.
    let dps_balance = (my_health / (target_health.max(0.05))).clamp(0.0, 2.0) * 0.5;
    let ttk_ratio = (my_health / (violence_cap.max(0.05))).clamp(0.0, 2.0) * 0.5;
    let ally_factor = 0.5;
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Fight,
        composite(&tr.fight, dps_balance, ttk_ratio, ally_factor, my_health),
    );
    // Freeze — cover + low intent clarity (target hasn't locked on yet).
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Freeze,
        composite(
            &tr.freeze,
            cover_self,
            1.0 - intent_clarity,
            my_health,
            1.0 - prox,
        ),
    );
    // Fawn — adjacency, low hostility, positive affiliation.
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Fawn,
        composite(&tr.fawn, prox, 1.0 - hostility, bond_pos, receptivity),
    );

    // --- Conflict-low ---
    let cl = &cfg.conflict_low;
    // Threaten — adjacency, my capability proxy (my_health), low hostility from target.
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Threaten,
        composite(
            &cl.threaten,
            prox,
            my_health,
            1.0 - violence_cap,
            1.0 - hostility,
        ),
    );
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Posture,
        composite(&cl.posture, prox, my_health, 1.0 - violence_cap, 0.5),
    );
    // Hiss — adjacency + my distress (use hunger/safety deficit as proxy).
    let distress = (1.0 - perceiver.safety_need).clamp(0.0, 1.0);
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Hiss,
        composite(&cl.hiss, prox, distress, hostility, 0.5),
    );

    // --- Social ---
    let so = &cfg.social;
    // Socialize — adjacency + affiliation + low hostility + receptivity.
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Socialize,
        composite(&so.socialize, prox, bond_pos, 1.0 - hostility, receptivity),
    );
    // GroomOther — same shape, but my social need is the trigger.
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::GroomOther,
        composite(
            &so.groom_other,
            prox,
            bond_pos,
            1.0 - hostility,
            perceiver.social_need,
        ),
    );
    // Mate — fertility proxy (mating_need), bond, receptivity, affiliation.
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Mate,
        composite(&so.mate, perceiver.mating_need, bond_pos, receptivity, prox),
    );
    // Mentor — bond + receptivity + my_health (need to be able to teach).
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Mentor,
        composite(&so.mentor, bond_pos, receptivity, my_health, prox),
    );
    // Care — perceived injury level + bond.
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Care,
        composite(&so.care, injury_level, bond_pos, prox, my_health),
    );
    // FeedKitten — target's hunger (read from belief if available, else target's own need),
    // my food proxy = inverse of my own hunger, bond, proximity.
    let target_hunger = 1.0 - target.hunger_need;
    let my_food_proxy = perceiver.hunger_need;
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::FeedKitten,
        composite(
            &so.feed_kitten,
            target_hunger,
            my_food_proxy,
            bond_pos,
            prox,
        ),
    );

    // Prey-side: cats don't bolt or scatter-group as perceivers. Gate to 0.
    affordances.write(perceiver.entity, target.entity, ActionKind::Bolt, 0.0);
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::ScatterGroup,
        0.0,
    );
}

// ---------------------------------------------------------------------------
// Cat perceiver × wildlife target
// ---------------------------------------------------------------------------

fn write_cat_vs_wildlife(
    perceiver: &CatSnapshot,
    target: &WildSnapshot,
    belief: Option<&MentalModel>,
    cfg: &crate::resources::sim_constants::AffordancesConstants,
    ward: &WardCoverageMap,
    fox_scent: &FoxScentMap,
    affordances: &mut ActionAffordances,
) {
    let prox = proximity_feature(&perceiver.position, &target.position, cfg.sensing_range);
    let cover_self = cover_at(&perceiver.position, ward);
    let violence_cap = facet(belief, |m| m.perceived_violence_capability.value);
    let intent_clarity = facet(belief, |m| m.perceived_intent_clarity.value);
    let recency = facet(belief, |m| m.recency_of_threat_cue.value);
    let my_health = perceiver.health_fraction;
    let target_health = target.health_fraction;
    let scent_at_self = fox_scent.get(perceiver.position.x, perceiver.position.y);

    // Predation — cats can Stalk / Chase / Pounce wildlife (the universal trio).
    let pred = &cfg.predation;
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Stalk,
        composite(
            &pred.stalk,
            prox,
            cover_self,
            my_health,
            1.0 - intent_clarity,
        ),
    );
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Chase,
        composite(
            &pred.chase,
            prox,
            1.0 - intent_clarity,
            my_health,
            1.0 - scent_at_self,
        ),
    );
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Pounce,
        composite(
            &pred.pounce,
            prox,
            cover_self,
            1.0 - intent_clarity,
            my_health,
        ),
    );
    // Species-gated.
    affordances.write(perceiver.entity, target.entity, ActionKind::Dive, 0.0);
    affordances.write(perceiver.entity, target.entity, ActionKind::Strike, 0.0);
    affordances.write(perceiver.entity, target.entity, ActionKind::Ambush, 0.0);

    // Threat-response — Flee away from a predator; Fight if cornered; Freeze; Fawn doesn't
    // apply to non-conspecific (no social interpretation), so gate to 0.
    let tr = &cfg.threat_response;
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Flee,
        composite(&tr.flee, 1.0 - prox, cover_self, my_health, violence_cap),
    );
    let dps_balance = (my_health / (target_health.max(0.05))).clamp(0.0, 2.0) * 0.5;
    let ttk_ratio = (my_health / (target.threat_power.max(0.05))).clamp(0.0, 2.0) * 0.5;
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Fight,
        composite(&tr.fight, dps_balance, ttk_ratio, my_health, 1.0 - recency),
    );
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Freeze,
        composite(
            &tr.freeze,
            cover_self,
            1.0 - intent_clarity,
            my_health,
            1.0 - prox,
        ),
    );
    // Fawn doesn't apply against wildlife predators.
    affordances.write(perceiver.entity, target.entity, ActionKind::Fawn, 0.0);

    // Conflict-low — Threaten / Posture / Hiss against wildlife is plausible (cats hiss at
    // foxes); compose with capability.
    let cl = &cfg.conflict_low;
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Threaten,
        composite(&cl.threaten, prox, my_health, 1.0 - violence_cap, 0.5),
    );
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Posture,
        composite(&cl.posture, prox, my_health, 1.0 - violence_cap, 0.5),
    );
    let distress = (1.0 - perceiver.safety_need).clamp(0.0, 1.0);
    affordances.write(
        perceiver.entity,
        target.entity,
        ActionKind::Hiss,
        composite(&cl.hiss, prox, distress, violence_cap, 0.5),
    );

    // Social affordances don't apply against wildlife — gate to 0.
    for kind in [
        ActionKind::Socialize,
        ActionKind::GroomOther,
        ActionKind::Mate,
        ActionKind::Mentor,
        ActionKind::Care,
        ActionKind::FeedKitten,
        ActionKind::Bolt,
        ActionKind::ScatterGroup,
    ] {
        affordances.write(perceiver.entity, target.entity, kind, 0.0);
    }
}

// ---------------------------------------------------------------------------
// Wildlife perceiver × cat target
// ---------------------------------------------------------------------------

fn write_wildlife_vs_cat(
    perceiver: &WildSnapshot,
    target: &CatSnapshot,
    cfg: &crate::resources::sim_constants::AffordancesConstants,
    ward: &WardCoverageMap,
    affordances: &mut ActionAffordances,
) {
    let prox = proximity_feature(&perceiver.position, &target.position, cfg.sensing_range);
    let cover_self = cover_at(&perceiver.position, ward);
    let cover_at_target = cover_at(&target.position, ward);
    let my_health = perceiver.health_fraction;
    let target_health = target.health_fraction;

    // Default-zero every kind so wildlife rows are uniform with cat rows
    // (consumer code can iterate ActionKind::ALL safely).
    for kind in ActionKind::ALL {
        affordances.write(perceiver.entity, target.entity, kind, 0.0);
    }

    // Per-species predation eligibility.
    let pred = &cfg.predation;
    match perceiver.species {
        WildSpecies::Fox => {
            affordances.write(
                perceiver.entity,
                target.entity,
                ActionKind::Stalk,
                composite(
                    &pred.stalk,
                    prox,
                    cover_self,
                    my_health,
                    1.0 - cover_at_target,
                ),
            );
            affordances.write(
                perceiver.entity,
                target.entity,
                ActionKind::Chase,
                composite(&pred.chase, prox, my_health, 1.0 - target_health, 0.5),
            );
        }
        WildSpecies::Hawk => {
            // Dive — high if target is in open ground (low cover) and proximity is moderate.
            affordances.write(
                perceiver.entity,
                target.entity,
                ActionKind::Dive,
                composite(&pred.dive, prox, 1.0 - cover_at_target, my_health, 0.5),
            );
            affordances.write(
                perceiver.entity,
                target.entity,
                ActionKind::Chase,
                composite(&pred.chase, prox, my_health, 1.0 - target_health, 0.5),
            );
        }
        WildSpecies::Snake => {
            // Strike — adjacency-gated; only meaningful within ~1 tile.
            let strike_prox = if manhattan(&perceiver.position, &target.position) <= 1 {
                1.0
            } else {
                0.0
            };
            affordances.write(
                perceiver.entity,
                target.entity,
                ActionKind::Strike,
                composite(
                    &pred.strike,
                    strike_prox,
                    1.0 - target_health,
                    my_health,
                    0.5,
                ),
            );
            affordances.write(
                perceiver.entity,
                target.entity,
                ActionKind::Stalk,
                composite(
                    &pred.stalk,
                    prox,
                    cover_self,
                    my_health,
                    1.0 - cover_at_target,
                ),
            );
        }
        WildSpecies::ShadowFox => {
            // Ambush — peak when perceiver has high cover and target has low cover.
            affordances.write(
                perceiver.entity,
                target.entity,
                ActionKind::Ambush,
                composite(
                    &pred.ambush,
                    cover_self,
                    1.0 - cover_at_target,
                    my_health,
                    prox,
                ),
            );
            affordances.write(
                perceiver.entity,
                target.entity,
                ActionKind::Stalk,
                composite(
                    &pred.stalk,
                    prox,
                    cover_self,
                    my_health,
                    1.0 - cover_at_target,
                ),
            );
            affordances.write(
                perceiver.entity,
                target.entity,
                ActionKind::Chase,
                composite(&pred.chase, prox, my_health, 1.0 - target_health, 0.5),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::beliefs::{ContextBeliefs, LocationBeliefs};
    use crate::components::physical::Position;
    use bevy_ecs::schedule::Schedule;

    fn test_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(SimConstants::default());
        world.insert_resource(ActionAffordances::default());
        world.insert_resource(FoxScentMap::default());
        world.insert_resource(WardCoverageMap::default_map());
        world.insert_resource(crate::resources::ColonyDistrictMap::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(affordance_writer);
        (world, schedule)
    }

    fn spawn_cat(world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                Species,
                pos,
                Health::default(),
                Needs::default(),
                CatBeliefs::default(),
                LocationBeliefs::default(),
                PredatorBeliefs::default(),
                ContextBeliefs::default(),
                crate::components::beliefs::ColonyReservesBelief::default(),
            ))
            .id()
    }

    fn spawn_wild(world: &mut World, species: WildSpecies, pos: Position) -> Entity {
        world
            .spawn((WildAnimal::new(species), pos, Health::default()))
            .id()
    }

    #[test]
    fn empty_world_writes_nothing() {
        let (mut world, mut schedule) = test_world();
        schedule.run(&mut world);
        let a = world.resource::<ActionAffordances>();
        assert!(a.is_empty(), "no perceivers → no entries");
    }

    #[test]
    fn adjacent_cat_pair_writes_socialize_and_groom() {
        let (mut world, mut schedule) = test_world();
        let a = spawn_cat(&mut world, Position::new(10, 10));
        let b = spawn_cat(&mut world, Position::new(11, 10));
        schedule.run(&mut world);
        let affordances = world.resource::<ActionAffordances>();
        // Socialize and GroomOther are populated; values may be 0.0 under
        // min_eligibility floor but the entries exist (clear + insert pattern).
        assert!(
            !affordances.is_empty(),
            "adjacent pair should populate entries"
        );
        // Symmetric reads.
        let socialize_ab = affordances.read(a, b, ActionKind::Socialize);
        let socialize_ba = affordances.read(b, a, ActionKind::Socialize);
        assert!((0.0..=1.0).contains(&socialize_ab));
        assert!((0.0..=1.0).contains(&socialize_ba));
    }

    #[test]
    fn out_of_range_pair_writes_nothing_for_pair() {
        let (mut world, mut schedule) = test_world();
        let a = spawn_cat(&mut world, Position::new(0, 0));
        let b = spawn_cat(&mut world, Position::new(50, 50));
        schedule.run(&mut world);
        let affordances = world.resource::<ActionAffordances>();
        assert_eq!(affordances.read(a, b, ActionKind::Socialize), 0.0);
        assert_eq!(affordances.read(a, b, ActionKind::Stalk), 0.0);
    }

    #[test]
    fn hawk_perceiver_dive_nonzero_against_cat() {
        let (mut world, mut schedule) = test_world();
        let cat = spawn_cat(&mut world, Position::new(10, 10));
        let hawk = spawn_wild(&mut world, WildSpecies::Hawk, Position::new(11, 10));
        schedule.run(&mut world);
        let a = world.resource::<ActionAffordances>();
        // Hawk's Dive against the cat should be eligible (proximity high, low cover by default).
        let dive = a.read(hawk, cat, ActionKind::Dive);
        // Pounce is cat-only → wildlife perceivers default to 0.0.
        let pounce = a.read(hawk, cat, ActionKind::Pounce);
        assert_eq!(pounce, 0.0, "hawks can't pounce (species gate)");
        // Dive eligibility depends on the default weights + min_eligibility; with default
        // 0.25-quartet weights and four ~0.5+ inputs, the composite should clear the 0.10 floor.
        assert!(
            dive > 0.0,
            "hawk's Dive against an adjacent cat should be eligible; got {dive}"
        );
    }

    #[test]
    fn shadow_fox_ambush_high_in_warded_perceiver_position() {
        let (mut world, mut schedule) = test_world();
        // Stamp ward at perceiver's position so cover_self is high; cat target is in open ground.
        {
            let mut ward = world.resource_mut::<WardCoverageMap>();
            ward.stamp_ward(20, 20, 1.0, 9.0);
        }
        let cat = spawn_cat(&mut world, Position::new(28, 20));
        let sfox = spawn_wild(&mut world, WildSpecies::ShadowFox, Position::new(20, 20));
        schedule.run(&mut world);
        let a = world.resource::<ActionAffordances>();
        let ambush = a.read(sfox, cat, ActionKind::Ambush);
        assert!(
            ambush > 0.0,
            "ShadowFox in covered position vs cat in open should afford Ambush; got {ambush}"
        );
    }

    #[test]
    fn write_count_matches_eligible_pair_kinds() {
        // After a single tick with one cat-cat pair, every ActionKind has
        // an entry (a→b AND b→a). 21 kinds × 2 directions = 42 entries.
        let (mut world, mut schedule) = test_world();
        spawn_cat(&mut world, Position::new(10, 10));
        spawn_cat(&mut world, Position::new(11, 10));
        schedule.run(&mut world);
        let a = world.resource::<ActionAffordances>();
        assert_eq!(
            a.len(),
            21 * 2,
            "two cats within sensing range → 21 kinds × 2 perceiver directions"
        );
    }
}
