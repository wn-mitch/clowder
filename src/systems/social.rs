use std::collections::{BTreeMap, BTreeSet, HashMap};

use bevy_ecs::prelude::*;

use crate::components::building::Structure;
use crate::components::identity::{Age, Gender, LifeStage, Name, Orientation};
use crate::components::physical::{Dead, Position};
use crate::messages::cat_moved::CatMoved;
use crate::resources::event_log::{EventKind, EventLog};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::near_pair_cache::{normalize_pair, NearPairCache};
use crate::resources::relationships::{BondType, Relationships};
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{SimConfig, TimeScale, TimeState};

// ---------------------------------------------------------------------------
// passive_familiarity — event-driven (ticket 431 Stage B)
// ---------------------------------------------------------------------------

/// Maintains `NearPairCache` in response to cat movement. Authored by
/// ticket 431 Stage B as the event-driven replacement for the per-tick
/// O(N²) sweep that pre-Stage-B `passive_familiarity` ran every tick
/// (64.43% inclusive CPU per the 2026-05-20 flamegraph).
///
/// The cache is rebuilt incrementally on each `CatMoved` event: the
/// moving cat's existing pairings are dropped, then re-inserted by
/// re-scanning live cat positions for distances within
/// `passive_familiarity_range`. Departed entities are pruned by
/// diffing the live set against `cache.last_seen` (ticket 486 —
/// retires the pre-486 unconditional `BTreeMap::retain` that walked
/// the full pair map every tick for 13.27% inclusive CPU at HEAD).
/// The diff catches all departure paths uniformly (cat death via
/// `check_death`, wildlife / item / prey despawn via direct
/// `commands.entity(_).despawn()`). Newborn cats (in the current live
/// set but not in `last_seen`) are added to the re-scan set so they
/// enter the cache on their first tick — without this, a cat spawned
/// post-bootstrap wouldn't get passive familiarity until it first moved.
///
/// First-tick bootstrap: when the cache is empty AND no `CatMoved`
/// events arrived (e.g. tick 1 after world setup), every live cat is
/// treated as "newly appeared" and the full O(N²) pair set materializes
/// — identical work to the legacy `passive_familiarity` loop, but
/// amortized over the entire run. Subsequent ticks see only the
/// incremental work for movers + newborns.
#[allow(clippy::type_complexity)]
pub fn update_near_pair_cache(
    mut cache: ResMut<NearPairCache>,
    mut moved_reader: MessageReader<CatMoved>,
    // Ticket 506 — admission is restricted to the entities that
    // participate in the familiarity/co-presence substrate: cats
    // (`CatBeliefs` rides every cat spawn incl. kittens — the
    // blueprint bundle in setup.rs, pinned by pregnancy.rs's parity
    // test) and wildlife (`WildAnimal` — the §9.2 BefriendedAlly
    // surface reads cat×wildlife familiarity). The pre-506 filter —
    // only `(Without<Dead>, Without<Structure>)` — admitted every
    // prey animal and ground item: ~26k of ~26k cached pairs had a
    // non-cat endpoint, `passive_familiarity` minted Relationships
    // entries for all of them every tick, and threshold-crossing
    // prey pairs emitted SustainedCoPresence whose actors became
    // unread CatBeliefs ballast. See ticket 506.
    cats: Query<
        (Entity, &Position),
        (
            Without<Dead>,
            Without<Structure>,
            Or<(
                With<crate::components::CatBeliefs>,
                With<crate::components::wildlife::WildAnimal>,
            )>,
        ),
    >,
    constants: Res<SimConstants>,
) {
    let range = constants.social.passive_familiarity_range;

    // Snapshot live cats + positions. The `BTreeSet` ensures `live` (and
    // downstream `to_rescan`) iterate in entity-index order, matching
    // `NearPairCache`'s pair canonicalization for stable insert order
    // in the cache. Float-determinism for `passive_familiarity` does
    // not actually depend on this (one `+= delta` per independent
    // pair entry per tick), but determinism-safe by construction is
    // cheaper to reason about than determinism-by-accident.
    let cats_vec: Vec<(Entity, Position)> = cats.iter().map(|(e, p)| (e, *p)).collect();
    let live: BTreeSet<Entity> = cats_vec.iter().map(|(e, _)| *e).collect();

    // 486 — departure-set eviction. Pre-486 this system ran an
    // unconditional `BTreeMap::retain` over the full pair map every
    // tick (13.27% inclusive CPU). The retain only does work when an
    // entity was removed from the live set — which we can detect by
    // diffing `live` against `cache.last_seen`.
    //
    // We diff (not the `CatDied` stream the ticket proposed) because
    // the admitted set still spans two departure paths: cats leave
    // via `Dead` insertion, wildlife (506 keeps them for the §9.2
    // befriend surface) via direct `commands.entity(_).despawn()`.
    // The diff catches both uniformly; using only `CatDied` would
    // leak wildlife entries and trip `passive_familiarity`'s debug
    // divergence assert. (Pre-506 the query also admitted prey +
    // items, which made the diff even more load-bearing.)
    //
    // Steady-state (no departures, no moves): both `gone` and `moved`
    // are empty, both conditional blocks are skipped, and the system's
    // work is just the cats-query snapshot + the two set builds.
    let gone: BTreeSet<Entity> = cache.last_seen.difference(&live).copied().collect();

    // Drain `CatMoved` and collect the set of moved entities. Filter out
    // events for cats that already departed (their entries are evicted
    // by the `gone` path below regardless).
    let moved: BTreeSet<Entity> = moved_reader
        .read()
        .map(|m| m.entity)
        .filter(|e| live.contains(e))
        .collect();

    // Newborn detection: cats present in `live` but absent from
    // `last_seen`. On the very first tick after world setup `last_seen`
    // is empty and `live` is all founders — this is the bootstrap path.
    let newborns: BTreeSet<Entity> = live.difference(&cache.last_seen).copied().collect();

    // Combined re-scan set. Moved cats must drop+re-derive their entries;
    // newborns must materialize fresh entries (no prior state to drop).
    let to_rescan: BTreeSet<Entity> = moved.union(&newborns).copied().collect();

    // Targeted pair removal for every departed or moved entity. One linear
    // pass of `cache.pairs` collects matching keys (O(P)), then targeted
    // `remove` per key. Skipped entirely when both sets are empty — the
    // steady-state case.
    let to_evict: BTreeSet<Entity> = gone.union(&moved).copied().collect();
    if !to_evict.is_empty() {
        let to_remove: Vec<(Entity, Entity)> = cache
            .pairs
            .keys()
            .filter(|(a, b)| to_evict.contains(a) || to_evict.contains(b))
            .copied()
            .collect();
        for key in to_remove {
            cache.pairs.remove(&key);
        }
    }

    // Re-insert pairs for cats needing a re-scan. The position lookup is
    // O(N) in the worst case (per cat), but the outer loop is O(rescan)
    // — typically 1–3 movers per tick plus 0 newborns in steady state,
    // vs the pre-Stage-B O(N²) sweep every tick.
    for rescan_entity in &to_rescan {
        let Some(&(_, rescan_pos)) = cats_vec.iter().find(|(e, _)| e == rescan_entity) else {
            continue;
        };
        for (other, other_pos) in cats_vec.iter() {
            if *other == *rescan_entity {
                continue;
            }
            let dist = other_pos.distance_to(&rescan_pos);
            if dist <= range {
                let key = normalize_pair(*rescan_entity, *other);
                cache.pairs.insert(key, dist);
            }
        }
    }

    // Record the live set for next-tick newborn detection.
    cache.last_seen = live;
}

/// Applies the per-tick familiarity delta to every cached near-pair.
/// Reads `NearPairCache` (built by `update_near_pair_cache` from
/// `CatMoved` events) and calls `Relationships::modify_familiarity`
/// once per pair, in `BTreeMap` key order.
///
/// Pre-Stage-B this system ran the O(N²) pair-distance sweep itself
/// (64.43% inclusive CPU at the 2026-05-20 baseline). Stage B retires
/// the sweep — the work moves to `update_near_pair_cache`, which only
/// runs on movement, so most ticks see only the small per-pair iteration
/// below.
#[allow(clippy::type_complexity)]
pub fn passive_familiarity(
    cache: Res<NearPairCache>,
    mut relationships: ResMut<Relationships>,
    constants: Res<SimConstants>,
    time_scale: Res<TimeScale>,
    // 431 Stage B drift investigation — debug-only brute-force pair-set
    // comparison. Builds the set of `(a, b)` pairs the pre-Stage-B O(N²)
    // sweep would have visited and panics if it diverges from `cache.pairs`.
    // First panic localizes the exact tick + pair where `update_near_pair_cache`
    // diverges from the original loop's invariant. Release builds skip both
    // params (zero cost).
    // 506 — filter mirrors `update_near_pair_cache`'s admission set;
    // a mismatch here trips the brute-force parity panic below.
    #[cfg(debug_assertions)] cats: Query<
        (Entity, &Position),
        (
            Without<Dead>,
            Without<Structure>,
            Or<(
                With<crate::components::CatBeliefs>,
                With<crate::components::wildlife::WildAnimal>,
            )>,
        ),
    >,
    #[cfg(debug_assertions)] time: Res<TimeState>,
) {
    #[cfg(debug_assertions)]
    {
        let range = constants.social.passive_familiarity_range;
        let cats_vec: Vec<(Entity, Position)> = cats.iter().map(|(e, p)| (e, *p)).collect();
        let mut brute: BTreeSet<(Entity, Entity)> = BTreeSet::new();
        for i in 0..cats_vec.len() {
            for j in (i + 1)..cats_vec.len() {
                if cats_vec[i].1.distance_to(&cats_vec[j].1) <= range {
                    brute.insert(normalize_pair(cats_vec[i].0, cats_vec[j].0));
                }
            }
        }
        let cached: BTreeSet<(Entity, Entity)> = cache.pairs.keys().copied().collect();
        if cached != brute {
            let only_cache: Vec<(Entity, Entity)> = cached.difference(&brute).copied().collect();
            let only_brute: Vec<(Entity, Entity)> = brute.difference(&cached).copied().collect();
            panic!(
                "NearPairCache divergence at tick {}: only_in_cache={:?}, only_in_brute={:?}, cache_size={}, brute_size={}",
                time.tick,
                only_cache,
                only_brute,
                cached.len(),
                brute.len(),
            );
        }
    }

    let passive_familiarity_rate = constants
        .social
        .passive_familiarity_rate
        .per_tick(&time_scale);
    // Ticket 500 — single merge-join walk instead of one BTreeMap
    // `entry` descent per pair (16.55% self CPU at the 06-09
    // flamegraph). `cache.pairs` keys are normalize_pair-canonical and
    // BTreeMap-sorted, exactly the batch contract.
    relationships.modify_familiarity_batch(cache.pairs.keys().copied(), passive_familiarity_rate);
}

// ---------------------------------------------------------------------------
// befriend_wildlife author (§9.2 BefriendedAlly)
// ---------------------------------------------------------------------------

/// §9.2 / ticket 049 — author the `BefriendedAlly` marker on cats and
/// wildlife once their cross-species relationship familiarity crosses
/// `constants.social.befriend_familiarity_threshold`. Tags both sides
/// of the pair (a befriended fox carries the marker on the fox; the
/// cat that befriended it also carries it).
///
/// Hysteresis: removed when familiarity drops below
/// `(threshold - hysteresis)`. Without the band, repeated socialize
/// vs. avoid would flicker the marker each tick at the boundary.
///
/// **Note**: `Relationships` accepts cat ↔ wildlife pairs at the
/// storage layer, and `passive_familiarity` DOES write familiarity
/// for them — wildlife are admitted into `NearPairCache` (kept
/// deliberately by ticket 506's composition fix precisely so this
/// surface stays live; a pre-506 version of this comment claimed no
/// production writer existed, which had been wrong since the 431
/// cache landed). A cat that persistently shares space with a fox
/// accrues the familiarity this author reads.
///
/// Algorithm: for each (cat, wildlife) pair where familiarity ≥
/// upgrade-threshold, tag both. If either side carries the marker
/// but the highest pairwise familiarity drops below the
/// downgrade-threshold, remove the marker. Per-entity decision —
/// the marker is *per-entity*, not per-pair (consumers like
/// fox_raiding read off the fox itself; the per-pair "befriended-by-
/// whom" model is a follow-on per ticket 049 D5).
#[allow(clippy::type_complexity)]
pub fn befriend_wildlife(
    mut commands: Commands,
    cats: Query<
        (
            Entity,
            bevy::prelude::Has<crate::components::markers::BefriendedAlly>,
        ),
        (
            With<crate::components::identity::Species>,
            Without<Dead>,
            Without<Structure>,
            Without<crate::components::wildlife::WildAnimal>,
        ),
    >,
    wildlife: Query<
        (
            Entity,
            bevy::prelude::Has<crate::components::markers::BefriendedAlly>,
        ),
        (With<crate::components::wildlife::WildAnimal>, Without<Dead>),
    >,
    relationships: Res<Relationships>,
    constants: Res<SimConstants>,
) {
    let s = &constants.social;
    let upgrade = s.befriend_familiarity_threshold;
    let downgrade = (upgrade - s.befriend_familiarity_hysteresis).max(0.0);

    let cat_list: Vec<(Entity, bool)> = cats.iter().collect();
    let wildlife_list: Vec<(Entity, bool)> = wildlife.iter().collect();

    // Per-entity max familiarity over all cross-species pairs the
    // entity participates in. The marker is per-entity, so a cat with
    // *any* befriended wildlife counterpart carries it; same for a
    // wildlife creature with any befriending cat.
    let mut cat_max_fam: HashMap<Entity, f32> = HashMap::new();
    let mut wild_max_fam: HashMap<Entity, f32> = HashMap::new();
    for (cat_entity, _) in &cat_list {
        for (wild_entity, _) in &wildlife_list {
            let fam = relationships
                .get(*cat_entity, *wild_entity)
                .map(|r| r.familiarity)
                .unwrap_or(0.0);
            let cmax = cat_max_fam.entry(*cat_entity).or_insert(0.0);
            if fam > *cmax {
                *cmax = fam;
            }
            let wmax = wild_max_fam.entry(*wild_entity).or_insert(0.0);
            if fam > *wmax {
                *wmax = fam;
            }
        }
    }

    for (cat_entity, has_marker) in &cat_list {
        let fam = cat_max_fam.get(cat_entity).copied().unwrap_or(0.0);
        toggle_marker(
            &mut commands,
            *cat_entity,
            *has_marker,
            fam,
            upgrade,
            downgrade,
        );
    }
    for (wild_entity, has_marker) in &wildlife_list {
        let fam = wild_max_fam.get(wild_entity).copied().unwrap_or(0.0);
        toggle_marker(
            &mut commands,
            *wild_entity,
            *has_marker,
            fam,
            upgrade,
            downgrade,
        );
    }
}

fn toggle_marker(
    commands: &mut Commands,
    entity: Entity,
    has: bool,
    familiarity: f32,
    upgrade: f32,
    downgrade: f32,
) {
    let want = if has {
        familiarity >= downgrade
    } else {
        familiarity >= upgrade
    };
    match (want, has) {
        (true, false) => {
            commands
                .entity(entity)
                .insert(crate::components::markers::BefriendedAlly);
        }
        (false, true) => {
            commands
                .entity(entity)
                .remove::<crate::components::markers::BefriendedAlly>();
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// check_bonds system
// ---------------------------------------------------------------------------

/// Per-cat fields relevant to courtship drift and bond-upgrade gating.
///
/// Snapshotted before the main loop so we can look up both sides of each
/// relationship pair without re-querying components per iteration.
#[derive(Clone, Copy)]
struct CourtshipFitness {
    stage: LifeStage,
    gender: Gender,
    orientation: Orientation,
}

/// Collect existing `BondType::Mates` partners per cat from the relationship
/// graph. Both directions of each Mates pair contribute: the pair (a, b)
/// adds `b` to `by_cat[a]` and `a` to `by_cat[b]`.
///
/// Ticket 453: feeds the exclusivity invariant in [`check_bonds`] — a cat
/// with more than one Mates partner needs all but its canonical partner
/// demoted.
fn mates_partners_by_cat(relationships: &Relationships) -> BTreeMap<Entity, Vec<Entity>> {
    let mut by_cat: BTreeMap<Entity, Vec<Entity>> = BTreeMap::new();
    for ((a, b), rel) in relationships.iter() {
        if rel.bond == Some(BondType::Mates) {
            by_cat.entry(a).or_default().push(b);
            by_cat.entry(b).or_default().push(a);
        }
    }
    by_cat
}

/// From a per-cat Mates-partner map, identify the pair keys that must be
/// demoted to satisfy the "at most one Mates per cat" invariant.
/// Deterministic by `Entity::index()` ascending — matches the canonical
/// ordering [`Relationships`] already uses in `normalize_key`.
///
/// A cat's *canonical* Mates partner is the partner with the lowest
/// `Entity::index()`. A pair (a, b) survives iff a's canonical is b AND
/// b's canonical is a; any other Mates pair is demoted. This breaks
/// triangular polyamory deterministically: in {A-B, A-C, B-C}, the
/// indices order one pair as canonical for both ends, and the other two
/// drop to Partners.
fn collect_excess_mates_to_demote(
    by_cat: &BTreeMap<Entity, Vec<Entity>>,
) -> BTreeSet<(Entity, Entity)> {
    let canonical: BTreeMap<Entity, Entity> = by_cat
        .iter()
        .filter_map(|(cat, partners)| {
            partners
                .iter()
                .min_by_key(|e| e.index())
                .map(|p| (*cat, *p))
        })
        .collect();
    let mut to_demote = BTreeSet::new();
    for (cat, partners) in by_cat {
        let mine = canonical.get(cat).copied();
        for other in partners {
            let theirs = canonical.get(other).copied();
            if mine != Some(*other) || theirs != Some(*cat) {
                let pair = if cat.index() <= other.index() {
                    (*cat, *other)
                } else {
                    (*other, *cat)
                };
                to_demote.insert(pair);
            }
        }
    }
    to_demote
}

/// Periodically check all relationships and upgrade bonds when thresholds are
/// met. Emits Tier::Significant narrative on bond formation.
///
/// Also accumulates romantic attachment for orientation-compatible pairs of
/// adult cats whose fondness and familiarity have crossed the courtship
/// gates. Without this, romantic stays at 0.0 forever — the MateWith step is
/// the only other writer, and it requires a Partners bond to reach.
///
/// **Mates exclusivity** (ticket 453): at most one `BondType::Mates` bond
/// per cat is enforced as a current-substrate-shape invariant. A pre-loop
/// migration pass demotes excess pre-existing Mates bonds to Partners
/// deterministically (lowest `Entity::index()` wins). In the main pair
/// loop, a Mates promotion is capped at Partners when either side already
/// holds a Mates bond elsewhere. This is a promotion-time invariant,
/// *not* a semantic property of `BondType::Mates` — future romantic-depth
/// work (infidelity, polyamory, jealousy) flips the gate without
/// redefining the bond type.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn check_bonds(
    time: Res<TimeState>,
    config: Res<SimConfig>,
    time_scale: Res<TimeScale>,
    mut relationships: ResMut<Relationships>,
    mut log: ResMut<NarrativeLog>,
    names: Query<&Name>,
    positions: Query<&Position>,
    fitness_query: Query<
        (Entity, &Age, &Gender, &Orientation),
        (Without<Dead>, Without<Structure>),
    >,
    mut colony_score: Option<ResMut<crate::resources::colony_score::ColonyScore>>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
    mut pushback: MessageWriter<crate::systems::magic::CorruptionPushback>,
    mut event_log: Option<ResMut<EventLog>>,
) {
    let c = &constants.social;
    // Only check every bond_check_interval ticks.
    if !time.tick.is_multiple_of(c.bond_check_interval) {
        return;
    }
    // Per-check semantics: courtship_romantic_rate is the value added each
    // time the cadence fires. RatePerDay value × ticks_per_day_phase →
    // that legacy per-tick numeric.
    let courtship_romantic_rate = c.courtship_romantic_rate.per_tick(&time_scale);

    let fitness: HashMap<Entity, CourtshipFitness> = fitness_query
        .iter()
        .map(|(e, age, gender, orient)| {
            (
                e,
                CourtshipFitness {
                    stage: age.stage(time.tick, config.ticks_per_season),
                    gender: *gender,
                    orientation: *orient,
                },
            )
        })
        .collect();

    // Ticket 453 — migration: demote any cat's excess Mates bonds to
    // Partners. Idempotent once the invariant holds (`to_demote` is empty
    // when every cat has ≤1 Mates partner).
    {
        let by_cat = mates_partners_by_cat(&relationships);
        let to_demote = collect_excess_mates_to_demote(&by_cat);
        for (a, b) in to_demote {
            if let Some(rel) = relationships.get_mut(a, b) {
                rel.bond = Some(BondType::Partners);
            }
        }
    }

    // Ticket 453 — set of cats currently holding a Mates bond. Kept in
    // sync below: any new Mates promotion inserts both sides; the
    // pre-loop migration has already collapsed pre-existing polyamory
    // so this set is invariant-correct on entry.
    let mut mates_holders: BTreeSet<Entity> = relationships
        .iter()
        .filter(|(_, rel)| rel.bond == Some(BondType::Mates))
        .flat_map(|((a, b), _)| [a, b])
        .collect();

    for ((a, b), rel) in relationships.pairs_iter_mut() {
        let old_bond = rel.bond;

        // Orientation + life-stage gate for romantic involvement. Friends bonds
        // remain open to anyone, including kittens and asexual cats; only
        // romantic outcomes require compatibility.
        let romantic_eligible = match (fitness.get(&a), fitness.get(&b)) {
            (Some(fa), Some(fb)) => {
                matches!(fa.stage, LifeStage::Adult | LifeStage::Elder)
                    && matches!(fb.stage, LifeStage::Adult | LifeStage::Elder)
                    && are_orientation_compatible(
                        fa.gender,
                        fa.orientation,
                        fb.gender,
                        fb.orientation,
                    )
            }
            _ => false,
        };

        // Courtship drift: compatible close-friend pairs develop romantic
        // attraction over time, breaking the Partners/Mate chicken-and-egg.
        //
        // Ticket 027 Bug 1: emit `Feature::CourtshipInteraction` and push
        // an `EventKind::CourtshipDrifted` event each time the gate fires.
        // Without this the `continuity_tallies.courtship` canary tracks
        // only `MatingOccurred` (which is currently zero per Bugs 2/3),
        // hiding the fact that passive drift IS accumulating.
        if romantic_eligible
            && rel.fondness > c.courtship_fondness_gate
            && rel.familiarity > c.courtship_familiarity_gate
        {
            rel.romantic = (rel.romantic + courtship_romantic_rate).min(1.0);
            activation.record(Feature::CourtshipInteraction);
            if let Some(elog) = event_log.as_mut() {
                if let (Ok(name_a), Ok(name_b)) = (names.get(a), names.get(b)) {
                    elog.push(
                        time.tick,
                        EventKind::CourtshipDrifted {
                            cat_a: name_a.0.clone(),
                            cat_b: name_b.0.clone(),
                        },
                    );
                }
            }
        }

        let proposed_new_bond = if romantic_eligible
            && rel.romantic > c.mates_romantic_threshold
            && rel.fondness > c.mates_fondness_threshold
            && rel.familiarity > c.mates_familiarity_threshold
        {
            Some(BondType::Mates)
        } else if romantic_eligible
            && rel.romantic > c.partners_romantic_threshold
            && rel.fondness > c.partners_fondness_threshold
            && rel.familiarity > c.partners_familiarity_threshold
        {
            Some(BondType::Partners)
        } else if rel.fondness > c.friends_fondness_threshold
            && rel.familiarity > c.friends_familiarity_threshold
        {
            Some(BondType::Friends)
        } else {
            None
        };

        // Ticket 453: exclusivity cap. A pair already at Mates is allowed
        // to stay there (old_bond == Some(Mates)). A new Mates promotion
        // is capped at Partners when either side already holds a Mates
        // bond elsewhere.
        let new_bond = if proposed_new_bond == Some(BondType::Mates)
            && old_bond != Some(BondType::Mates)
            && (mates_holders.contains(&a) || mates_holders.contains(&b))
        {
            Some(BondType::Partners)
        } else {
            proposed_new_bond
        };

        // Only upgrade bonds, never downgrade.
        if new_bond > old_bond {
            rel.bond = new_bond;
            if new_bond == Some(BondType::Mates) {
                mates_holders.insert(a);
                mates_holders.insert(b);
            }
            activation.record(Feature::BondFormed);
            if let Some(ref mut score) = colony_score {
                score.bonds_formed += 1;
            }
            if let (Ok(name_a), Ok(name_b)) = (names.get(a), names.get(b)) {
                let text = match new_bond.unwrap() {
                    BondType::Friends => {
                        format!("{} and {} have become close friends.", name_a.0, name_b.0)
                    }
                    BondType::Partners => {
                        format!("{} and {} have become partners.", name_a.0, name_b.0)
                    }
                    BondType::Mates => {
                        format!("{} and {} have become mates.", name_a.0, name_b.0)
                    }
                };
                log.push(time.tick, text, NarrativeTier::Significant);
            }
            // Bond warmth pushes back corruption.
            if let Ok(pos) = positions.get(a) {
                pushback.write(crate::systems::magic::CorruptionPushback {
                    position: *pos,
                    radius: 3.0,
                    amount: 0.05,
                });
            }
        }
    }

    debug_assert!(
        {
            let post = mates_partners_by_cat(&relationships);
            post.values().all(|partners| partners.len() <= 1)
        },
        "check_bonds invariant (ticket 453): every cat must hold at most 1 Mates bond"
    );
}

// ---------------------------------------------------------------------------
// Orientation compatibility
// ---------------------------------------------------------------------------

/// Check whether two cats can develop romantic feelings for each other based
/// on gender and orientation.
///
/// Nonbinary cats are compatible with all orientations (Straight, Gay, Bisexual).
/// Only Asexual blocks romantic development entirely.
pub fn are_orientation_compatible(
    a_gender: Gender,
    a_orient: Orientation,
    b_gender: Gender,
    b_orient: Orientation,
) -> bool {
    if a_orient == Orientation::Asexual || b_orient == Orientation::Asexual {
        return false;
    }

    let a_attracted = match a_orient {
        Orientation::Straight => {
            a_gender != b_gender || b_gender == Gender::Nonbinary || a_gender == Gender::Nonbinary
        }
        Orientation::Gay => {
            a_gender == b_gender || b_gender == Gender::Nonbinary || a_gender == Gender::Nonbinary
        }
        Orientation::Bisexual => true,
        Orientation::Asexual => false,
    };
    let b_attracted = match b_orient {
        Orientation::Straight => {
            b_gender != a_gender || a_gender == Gender::Nonbinary || b_gender == Gender::Nonbinary
        }
        Orientation::Gay => {
            b_gender == a_gender || a_gender == Gender::Nonbinary || b_gender == Gender::Nonbinary
        }
        Orientation::Bisexual => true,
        Orientation::Asexual => false,
    };

    a_attracted && b_attracted
}

// ---------------------------------------------------------------------------
// Value compatibility
// ---------------------------------------------------------------------------

/// Compute fondness delta from comparing two cats' value axes during interaction.
/// Same-side values: +same_delta per axis. Divergent values: +divergent_delta per axis.
#[allow(clippy::too_many_arguments)]
pub fn value_compatibility_delta(
    a_loyalty: f32,
    a_tradition: f32,
    a_compassion: f32,
    a_pride: f32,
    a_independence: f32,
    b_loyalty: f32,
    b_tradition: f32,
    b_compassion: f32,
    b_pride: f32,
    b_independence: f32,
    constants: &crate::resources::sim_constants::SocialConstants,
) -> f32 {
    let axes = [
        (a_loyalty, b_loyalty),
        (a_tradition, b_tradition),
        (a_compassion, b_compassion),
        (a_pride, b_pride),
        (a_independence, b_independence),
    ];
    let mut delta = 0.0;
    for (va, vb) in axes {
        let same_side = (va > constants.value_compat_same_threshold
            && vb > constants.value_compat_same_threshold)
            || (va < constants.value_compat_same_threshold
                && vb < constants.value_compat_same_threshold);
        let divergent = (va > constants.value_compat_divergent_high
            && vb < constants.value_compat_divergent_low)
            || (va < constants.value_compat_divergent_low
                && vb > constants.value_compat_divergent_high);
        if same_side {
            delta += constants.value_compat_same_delta;
        }
        if divergent {
            delta += constants.value_compat_divergent_delta;
        }
    }
    delta
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedule;

    use crate::components::physical::Position;
    use crate::resources::narrative::NarrativeLog;
    use crate::resources::relationships::Relationships;
    use crate::resources::time::TimeState;

    fn test_time_scale() -> TimeScale {
        TimeScale::from_config(&SimConfig::default(), 16.6667)
    }

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(Relationships::default());
        world.insert_resource(TimeState::default());
        world.insert_resource(crate::resources::time::SimConfig::default());
        world.insert_resource(test_time_scale());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());
        // 431 Stage B — NearPairCache substrate. The cache update system
        // bootstraps on first run when the cache is empty AND no CatMoved
        // events arrived (which is the test case here, since there's no
        // movement). The `Messages<CatMoved>` resource lets `MessageReader`
        // construct cleanly with zero events.
        world.insert_resource(crate::resources::near_pair_cache::NearPairCache::default());
        world.insert_resource(bevy_ecs::message::Messages::<CatMoved>::default());
        let mut schedule = Schedule::default();
        schedule.add_systems((update_near_pair_cache, passive_familiarity).chain());
        (world, schedule)
    }

    /// 506 — the cache admission filter requires `CatBeliefs` (cats)
    /// or `WildAnimal` (wildlife); test entities spawn with the cat
    /// discriminator, mirroring the production blueprint bundle.
    fn spawn_near_pair_cat(world: &mut World, pos: Position) -> Entity {
        world
            .spawn((pos, crate::components::CatBeliefs::default()))
            .id()
    }

    #[test]
    fn passive_familiarity_increases_for_adjacent_cats() {
        let (mut world, mut schedule) = setup_world();

        let a = spawn_near_pair_cat(&mut world, Position::new(5, 5));
        let b = spawn_near_pair_cat(&mut world, Position::new(5, 6));

        // Init relationship.
        world
            .resource_mut::<Relationships>()
            .get_or_insert(a, b)
            .familiarity = 0.0;

        schedule.run(&mut world);

        let fam = world
            .resource::<Relationships>()
            .get(a, b)
            .unwrap()
            .familiarity;
        assert!(
            (fam - 0.0003).abs() < 1e-6,
            "familiarity should be ~0.0003; got {fam}"
        );
    }

    #[test]
    fn passive_familiarity_unchanged_for_distant_cats() {
        let (mut world, mut schedule) = setup_world();

        let a = spawn_near_pair_cat(&mut world, Position::new(0, 0));
        let b = spawn_near_pair_cat(&mut world, Position::new(10, 10));

        world
            .resource_mut::<Relationships>()
            .get_or_insert(a, b)
            .familiarity = 0.0;

        schedule.run(&mut world);

        let fam = world
            .resource::<Relationships>()
            .get(a, b)
            .unwrap()
            .familiarity;
        assert_eq!(fam, 0.0, "distant cats should not gain familiarity");
    }

    /// Ticket 506 — the cache admits cats (`CatBeliefs`) and wildlife
    /// (`WildAnimal`) ONLY. Prey and bare-Position entities (items)
    /// sitting inside `passive_familiarity_range` must produce zero
    /// cache pairs and zero Relationships entries. Pre-506 the filter
    /// admitted everything with a Position: 26k of 26k cached pairs in
    /// a seed-42 soak had a non-cat endpoint.
    #[test]
    fn near_pair_cache_excludes_prey_and_items() {
        let (mut world, mut schedule) = setup_world();

        let cat_a = spawn_near_pair_cat(&mut world, Position::new(5, 5));
        let cat_b = spawn_near_pair_cat(&mut world, Position::new(5, 6));
        // Wildlife next to the cats — admitted (befriend surface).
        let fox = world
            .spawn((
                Position::new(5, 7),
                crate::components::wildlife::WildAnimal::new(
                    crate::components::wildlife::WildSpecies::Fox,
                ),
            ))
            .id();
        // Prey + bare-Position (item-shaped) entities in range — excluded.
        let prey = world
            .spawn((Position::new(5, 4), crate::components::prey::PreyAnimal))
            .id();
        let item_shaped = world.spawn(Position::new(6, 5)).id();

        schedule.run(&mut world);

        let cache = world.resource::<crate::resources::near_pair_cache::NearPairCache>();
        for &(a, b) in cache.pairs.keys() {
            for e in [a, b] {
                assert!(
                    e != prey && e != item_shaped,
                    "cache must not contain prey/item endpoints; found pair ({a:?}, {b:?})"
                );
            }
        }
        // The admitted trio all pair up (within range).
        let key_ab = crate::resources::near_pair_cache::normalize_pair(cat_a, cat_b);
        let key_bfox = crate::resources::near_pair_cache::normalize_pair(cat_b, fox);
        assert!(
            cache.pairs.contains_key(&key_ab),
            "cat×cat pair must be cached"
        );
        assert!(
            cache.pairs.contains_key(&key_bfox),
            "cat×wildlife pair must be cached (§9.2 befriend surface)"
        );
    }

    #[test]
    fn value_compatibility_positive_for_aligned_values() {
        let sc = &crate::resources::SimConstants::default().social;
        // Both cats have all values > 0.5 (same side).
        let delta = value_compatibility_delta(0.8, 0.7, 0.9, 0.6, 0.8, 0.7, 0.8, 0.6, 0.9, 0.7, sc);
        assert!(
            delta > 0.0,
            "aligned values should produce positive delta; got {delta}"
        );
        assert!(
            (delta - 0.001).abs() < 1e-6,
            "5 same-side axes should give +0.001; got {delta}"
        );
    }

    #[test]
    fn value_compatibility_negative_for_divergent_values() {
        let sc = &crate::resources::SimConstants::default().social;
        // Cat A has high values, Cat B has low values (all divergent).
        let delta = value_compatibility_delta(0.8, 0.8, 0.8, 0.8, 0.8, 0.2, 0.2, 0.2, 0.2, 0.2, sc);
        // Each axis: same_side is true (both effectively "above or below") — wait, 0.8 > 0.5 and 0.2 < 0.5, so NOT same side.
        // Each axis: divergent is true (0.8 > 0.7, 0.2 < 0.3).
        // So delta = 5 * (-0.0001) = -0.0005
        assert!(
            delta < 0.0,
            "divergent values should produce negative delta; got {delta}"
        );
        assert!(
            (delta - (-0.0005)).abs() < 1e-6,
            "5 divergent axes should give -0.0005; got {delta}"
        );
    }

    #[test]
    fn romantic_stays_zero_for_asexual_cats() {
        assert!(
            !are_orientation_compatible(
                Gender::Queen,
                Orientation::Asexual,
                Gender::Tom,
                Orientation::Straight
            ),
            "asexual cat should not be romantically compatible"
        );
        assert!(
            !are_orientation_compatible(
                Gender::Tom,
                Orientation::Straight,
                Gender::Queen,
                Orientation::Asexual
            ),
            "cat paired with asexual should not be compatible"
        );
    }

    #[test]
    fn orientation_compatibility_matrix() {
        // Straight Tom + Queen → compatible
        assert!(are_orientation_compatible(
            Gender::Tom,
            Orientation::Straight,
            Gender::Queen,
            Orientation::Straight
        ));
        // Straight Tom + Tom → NOT compatible
        assert!(!are_orientation_compatible(
            Gender::Tom,
            Orientation::Straight,
            Gender::Tom,
            Orientation::Straight
        ));
        // Gay Tom + Tom → compatible
        assert!(are_orientation_compatible(
            Gender::Tom,
            Orientation::Gay,
            Gender::Tom,
            Orientation::Gay
        ));
        // Gay Tom + Queen → NOT compatible
        assert!(!are_orientation_compatible(
            Gender::Tom,
            Orientation::Gay,
            Gender::Queen,
            Orientation::Gay
        ));
        // Bisexual + any non-asexual → compatible
        assert!(are_orientation_compatible(
            Gender::Tom,
            Orientation::Bisexual,
            Gender::Tom,
            Orientation::Bisexual
        ));
        assert!(are_orientation_compatible(
            Gender::Tom,
            Orientation::Bisexual,
            Gender::Queen,
            Orientation::Straight
        ));
        // Nonbinary + Straight → compatible
        assert!(are_orientation_compatible(
            Gender::Nonbinary,
            Orientation::Straight,
            Gender::Tom,
            Orientation::Straight
        ));
        // Nonbinary + Gay → compatible
        assert!(are_orientation_compatible(
            Gender::Nonbinary,
            Orientation::Gay,
            Gender::Tom,
            Orientation::Gay
        ));
    }

    /// Helper: build a test world with `check_bonds` ready to run.
    /// Pre-registers every resource and the single message type the system writes.
    fn bond_test_world(tick: u64) -> (World, Schedule) {
        let mut world = World::new();
        let mut time = TimeState::default();
        time.tick = tick;
        world.insert_resource(time);
        world.insert_resource(crate::resources::time::SimConfig::default());
        world.insert_resource(test_time_scale());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());
        bevy_ecs::message::MessageRegistry::register_message::<
            crate::systems::magic::CorruptionPushback,
        >(&mut world);
        let mut schedule = Schedule::default();
        schedule.add_systems(bevy_ecs::message::message_update_system);
        schedule.add_systems(check_bonds);
        (world, schedule)
    }

    /// Helper: spawn a cat at life stage Adult by using a born_tick old enough
    /// for a 12+ season age under the default ticks_per_season (20_000).
    fn spawn_adult(
        world: &mut World,
        name: &str,
        gender: Gender,
        orientation: Orientation,
    ) -> Entity {
        world
            .spawn((
                Name(name.to_string()),
                Age { born_tick: 0 },
                gender,
                orientation,
            ))
            .id()
    }

    #[test]
    fn bond_forms_at_threshold() {
        // Age cats to Adult: tick 50 + ticks_per_season * 12 is enough.
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        let a = spawn_adult(&mut world, "Fern", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.4;
        rel.familiarity = 0.5;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let rels = world.resource::<Relationships>();
        assert_eq!(
            rels.get(a, b).unwrap().bond,
            Some(BondType::Friends),
            "bond should be Friends at f=0.4, fam=0.5"
        );

        let log = world.resource::<NarrativeLog>();
        assert!(
            log.entries.iter().any(|e| e.text.contains("close friends")),
            "should narrate bond formation"
        );
    }

    #[test]
    fn courtship_drift_grows_romantic_for_compatible_pair() {
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        let a = spawn_adult(&mut world, "Fern", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.5;
        rel.familiarity = 0.5;
        rel.romantic = 0.0;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let rels = world.resource::<Relationships>();
        let ts = test_time_scale();
        let rate = crate::resources::SimConstants::default()
            .social
            .courtship_romantic_rate
            .per_tick(&ts);
        assert!(
            (rels.get(a, b).unwrap().romantic - rate).abs() < 1e-6,
            "one tick of courtship should add exactly courtship_romantic_rate to romantic"
        );
    }

    #[test]
    fn courtship_drift_skips_incompatible_orientation() {
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        // Two straight Toms — not orientation-compatible.
        let a = spawn_adult(&mut world, "Flint", Gender::Tom, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.5;
        rel.familiarity = 0.5;
        rel.romantic = 0.0;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let rels = world.resource::<Relationships>();
        assert_eq!(
            rels.get(a, b).unwrap().romantic,
            0.0,
            "incompatible orientations should not accumulate romantic"
        );
    }

    #[test]
    fn courtship_drift_skips_kittens() {
        // Cats born at tick 0, checked at tick 50 → Kitten stage.
        let (mut world, mut schedule) = bond_test_world(50);
        let a = spawn_adult(&mut world, "Sprout", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Brook", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.5;
        rel.familiarity = 0.5;
        rel.romantic = 0.0;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let rels = world.resource::<Relationships>();
        assert_eq!(
            rels.get(a, b).unwrap().romantic,
            0.0,
            "kittens cannot accumulate romantic"
        );
    }

    #[test]
    fn courtship_drift_engages_at_friends_tier_fondness() {
        // The courtship_fondness_gate is aligned with friends_fondness_threshold
        // (0.3) so that drift engages the moment a Friends bond can form.
        // Previously this was 0.4, leaving a dead zone where Friends-tier pairs
        // never developed romantic attraction.
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        let a = spawn_adult(&mut world, "Fern", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.35; // above Friends (0.3) and the new gate (0.3)
        rel.familiarity = 0.45; // above Friends (0.4) and the gate (0.4)
        rel.romantic = 0.0;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let rels = world.resource::<Relationships>();
        let ts = test_time_scale();
        let rate = crate::resources::SimConstants::default()
            .social
            .courtship_romantic_rate
            .per_tick(&ts);
        assert!(
            rels.get(a, b).unwrap().romantic > 0.0,
            "drift should engage for Friends-tier pair under new fondness gate"
        );
        assert!(
            (rels.get(a, b).unwrap().romantic - rate).abs() < 1e-6,
            "one tick of drift should add exactly courtship_romantic_rate"
        );
    }

    #[test]
    fn compatible_adults_reach_partners_bond_in_expected_time() {
        // Confirms the math: courtship_romantic_rate = 0.0015 per check means
        // partners_romantic_threshold (0.5) is reached in ~334 checks. We
        // simulate the needed number of checks directly rather than advancing
        // time ticks through a full schedule.
        let c = crate::resources::SimConstants::default().social;
        let ts = test_time_scale();
        let courtship_rate_per_tick = c.courtship_romantic_rate.per_tick(&ts);
        let checks_needed = (c.partners_romantic_threshold / courtship_rate_per_tick).ceil() as u64;

        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        let a = spawn_adult(&mut world, "Fern", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.7;
        rel.familiarity = 0.6;
        rel.romantic = 0.0;
        world.insert_resource(rels);

        for i in 0..checks_needed + 1 {
            // Advance tick by bond_check_interval each iteration so check_bonds fires.
            world.resource_mut::<TimeState>().tick = adult_tick + (i + 1) * c.bond_check_interval;
            schedule.run(&mut world);
        }

        let rels = world.resource::<Relationships>();
        let bond = rels.get(a, b).unwrap().bond;
        assert_eq!(
            bond,
            Some(BondType::Partners),
            "compatible adults with strong fondness/familiarity should reach Partners in ~{checks_needed} checks; got bond {bond:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Ticket 027 Bug 1: courtship-drift gate emits Feature + EventKind so the
    // continuity_tallies.courtship canary tracks passive drift independently
    // of the deadlocked MateWith path.
    // -----------------------------------------------------------------------

    fn count_courtship_drifted(world: &World) -> usize {
        world
            .resource::<EventLog>()
            .entries
            .iter()
            .filter(|e| matches!(e.kind, EventKind::CourtshipDrifted { .. }))
            .count()
    }

    #[test]
    fn courtship_drift_emits_feature_and_event_when_gate_fires() {
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        world.insert_resource(EventLog::default());
        let a = spawn_adult(&mut world, "Fern", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.5;
        rel.familiarity = 0.5;
        rel.romantic = 0.0;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let activation = world.resource::<SystemActivation>();
        assert_eq!(
            activation
                .counts
                .get(&Feature::CourtshipInteraction)
                .copied()
                .unwrap_or(0),
            1,
            "drift gate should record exactly one CourtshipInteraction this tick"
        );
        assert_eq!(
            count_courtship_drifted(&world),
            1,
            "drift gate should push exactly one CourtshipDrifted event this tick"
        );
        let log = world.resource::<EventLog>();
        assert_eq!(
            log.continuity_tallies
                .get("courtship")
                .copied()
                .unwrap_or(0),
            1,
            "CourtshipDrifted should bump continuity_tallies.courtship"
        );
    }

    #[test]
    fn courtship_drift_emits_nothing_for_incompatible_orientation() {
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        world.insert_resource(EventLog::default());
        // Two straight Toms — orientation-incompatible.
        let a = spawn_adult(&mut world, "Flint", Gender::Tom, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.5;
        rel.familiarity = 0.5;
        rel.romantic = 0.0;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let activation = world.resource::<SystemActivation>();
        assert_eq!(
            activation
                .counts
                .get(&Feature::CourtshipInteraction)
                .copied()
                .unwrap_or(0),
            0,
            "incompatible orientation should not record CourtshipInteraction"
        );
        assert_eq!(
            count_courtship_drifted(&world),
            0,
            "incompatible orientation should not push CourtshipDrifted"
        );
    }

    #[test]
    fn courtship_drift_emits_nothing_below_gates() {
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        world.insert_resource(EventLog::default());
        let a = spawn_adult(&mut world, "Fern", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        // Below courtship_fondness_gate (0.3) and courtship_familiarity_gate (0.4).
        rel.fondness = 0.1;
        rel.familiarity = 0.1;
        rel.romantic = 0.0;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let activation = world.resource::<SystemActivation>();
        assert_eq!(
            activation
                .counts
                .get(&Feature::CourtshipInteraction)
                .copied()
                .unwrap_or(0),
            0,
            "below-gate fondness/familiarity should not record CourtshipInteraction"
        );
        assert_eq!(
            count_courtship_drifted(&world),
            0,
            "below-gate fondness/familiarity should not push CourtshipDrifted"
        );
    }

    // -----------------------------------------------------------------------
    // §9.2 / ticket 049 befriend_wildlife author tests
    // -----------------------------------------------------------------------

    fn befriend_test_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(Relationships::default());
        world.insert_resource(SimConstants::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(befriend_wildlife);
        (world, schedule)
    }

    fn spawn_test_cat(world: &mut World) -> Entity {
        world
            .spawn((crate::components::identity::Species, Position::new(0, 0)))
            .id()
    }

    fn spawn_test_fox(world: &mut World) -> Entity {
        world
            .spawn((
                crate::components::wildlife::WildAnimal::new(
                    crate::components::wildlife::WildSpecies::Fox,
                ),
                Position::new(0, 0),
            ))
            .id()
    }

    #[test]
    fn befriend_inserts_marker_on_cat_and_fox_when_familiarity_crosses_threshold() {
        let (mut world, mut schedule) = befriend_test_world();
        let cat = spawn_test_cat(&mut world);
        let fox = spawn_test_fox(&mut world);
        // Threshold is 0.6; push familiarity to 0.7.
        world
            .resource_mut::<Relationships>()
            .modify_familiarity(cat, fox, 0.7);
        schedule.run(&mut world);
        assert!(
            world
                .get::<crate::components::markers::BefriendedAlly>(cat)
                .is_some(),
            "cat should carry BefriendedAlly after familiarity crosses 0.6"
        );
        assert!(
            world
                .get::<crate::components::markers::BefriendedAlly>(fox)
                .is_some(),
            "fox should carry BefriendedAlly reciprocally"
        );
    }

    #[test]
    fn befriend_omits_marker_below_threshold() {
        let (mut world, mut schedule) = befriend_test_world();
        let cat = spawn_test_cat(&mut world);
        let fox = spawn_test_fox(&mut world);
        // Familiarity 0.4 — below the 0.6 upgrade threshold.
        world
            .resource_mut::<Relationships>()
            .modify_familiarity(cat, fox, 0.4);
        schedule.run(&mut world);
        assert!(world
            .get::<crate::components::markers::BefriendedAlly>(cat)
            .is_none());
        assert!(world
            .get::<crate::components::markers::BefriendedAlly>(fox)
            .is_none());
    }

    #[test]
    fn befriend_hysteresis_keeps_marker_until_below_band() {
        // Threshold 0.6, hysteresis 0.1 → downgrade at 0.5.
        let (mut world, mut schedule) = befriend_test_world();
        let cat = spawn_test_cat(&mut world);
        let fox = spawn_test_fox(&mut world);

        // Cross threshold → marker on.
        world
            .resource_mut::<Relationships>()
            .modify_familiarity(cat, fox, 0.7);
        schedule.run(&mut world);
        assert!(world
            .get::<crate::components::markers::BefriendedAlly>(cat)
            .is_some());

        // Decay to 0.55 — still above downgrade band, marker persists.
        let mut rels = world.resource_mut::<Relationships>();
        rels.get_or_insert(cat, fox).familiarity = 0.55;
        schedule.run(&mut world);
        assert!(
            world
                .get::<crate::components::markers::BefriendedAlly>(cat)
                .is_some(),
            "marker should persist within hysteresis band"
        );

        // Drop below downgrade — marker comes off.
        let mut rels = world.resource_mut::<Relationships>();
        rels.get_or_insert(cat, fox).familiarity = 0.4;
        schedule.run(&mut world);
        assert!(
            world
                .get::<crate::components::markers::BefriendedAlly>(cat)
                .is_none(),
            "marker should clear once familiarity drops below threshold-hysteresis"
        );
    }

    #[test]
    fn befriend_marker_stable_when_no_wildlife_present() {
        let (mut world, mut schedule) = befriend_test_world();
        let cat = spawn_test_cat(&mut world);
        // No wildlife in the world — author runs as a no-op.
        schedule.run(&mut world);
        assert!(world
            .get::<crate::components::markers::BefriendedAlly>(cat)
            .is_none());
    }

    #[test]
    fn befriend_marker_stable_when_no_familiarity_written() {
        let (mut world, mut schedule) = befriend_test_world();
        let _cat = spawn_test_cat(&mut world);
        let _fox = spawn_test_fox(&mut world);
        // Relationships unwritten — fam defaults to 0.0, no markers.
        schedule.run(&mut world);
        let mut q =
            world.query_filtered::<Entity, With<crate::components::markers::BefriendedAlly>>();
        assert_eq!(q.iter(&world).count(), 0);
    }

    #[test]
    fn befriend_per_entity_max_familiarity_promotes_a_cat_with_any_partner() {
        // A cat with one wildlife partner above threshold and another
        // below — the cat carries the marker (per-entity max is taken).
        let (mut world, mut schedule) = befriend_test_world();
        let cat = spawn_test_cat(&mut world);
        let fox_a = spawn_test_fox(&mut world);
        let fox_b = spawn_test_fox(&mut world);
        {
            let mut rels = world.resource_mut::<Relationships>();
            rels.modify_familiarity(cat, fox_a, 0.4);
            rels.modify_familiarity(cat, fox_b, 0.7);
        }
        schedule.run(&mut world);
        assert!(world
            .get::<crate::components::markers::BefriendedAlly>(cat)
            .is_some());
        assert!(world
            .get::<crate::components::markers::BefriendedAlly>(fox_b)
            .is_some());
        assert!(
            world
                .get::<crate::components::markers::BefriendedAlly>(fox_a)
                .is_none(),
            "fox_a's max familiarity (0.4) is below threshold; should not carry marker"
        );
    }

    // ---------------------------------------------------------------------
    // Ticket 453 — Mates-exclusivity invariant helpers (pure-function
    // unit tests against `mates_partners_by_cat` /
    // `collect_excess_mates_to_demote`).
    // ---------------------------------------------------------------------

    fn make_pair_at_mates(rels: &mut Relationships, a: Entity, b: Entity) {
        let rel = rels.get_or_insert(a, b);
        rel.bond = Some(BondType::Mates);
        rel.romantic = 0.9;
        rel.fondness = 0.9;
        rel.familiarity = 0.9;
    }

    #[test]
    fn mates_partners_by_cat_counts_both_directions() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let mut rels = Relationships::default();
        make_pair_at_mates(&mut rels, a, b);

        let by_cat = mates_partners_by_cat(&rels);
        assert_eq!(by_cat.get(&a).map(|v| v.as_slice()), Some(&[b][..]));
        assert_eq!(by_cat.get(&b).map(|v| v.as_slice()), Some(&[a][..]));
    }

    #[test]
    fn collect_excess_mates_demotes_one_side_of_v_shape() {
        // V-shape: A has Mates(B) and Mates(C). B and C only mate A.
        // A's canonical partner is the lower-index of {B, C}; say B.
        // Then (A, B) survives, (A, C) demoted.
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();
        assert!(b.index() < c.index(), "test relies on spawn order");

        let mut rels = Relationships::default();
        make_pair_at_mates(&mut rels, a, b);
        make_pair_at_mates(&mut rels, a, c);

        let by_cat = mates_partners_by_cat(&rels);
        let to_demote = collect_excess_mates_to_demote(&by_cat);

        let pair_ab = if a.index() <= b.index() {
            (a, b)
        } else {
            (b, a)
        };
        let pair_ac = if a.index() <= c.index() {
            (a, c)
        } else {
            (c, a)
        };
        assert!(!to_demote.contains(&pair_ab), "canonical pair must survive");
        assert!(
            to_demote.contains(&pair_ac),
            "non-canonical pair must demote"
        );
    }

    #[test]
    fn collect_excess_mates_breaks_triangle_to_one_pair() {
        // Triangle: A-B, A-C, B-C all at Mates. Each cat has 2 Mates
        // partners. Canonical partner is the lowest-index neighbor;
        // only one pair has matching canonicals on both ends.
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();

        let mut rels = Relationships::default();
        make_pair_at_mates(&mut rels, a, b);
        make_pair_at_mates(&mut rels, a, c);
        make_pair_at_mates(&mut rels, b, c);

        let by_cat = mates_partners_by_cat(&rels);
        let to_demote = collect_excess_mates_to_demote(&by_cat);
        assert_eq!(
            to_demote.len(),
            2,
            "triangle collapses to one surviving Mates pair; got {to_demote:?}"
        );

        // Apply the demotions and verify the invariant.
        for (x, y) in &to_demote {
            rels.get_mut(*x, *y).unwrap().bond = Some(BondType::Partners);
        }
        let after = mates_partners_by_cat(&rels);
        assert!(
            after.values().all(|partners| partners.len() <= 1),
            "post-demote invariant: every cat ≤ 1 Mates partner; got {after:?}"
        );
    }

    #[test]
    fn check_bonds_refuses_second_mates_promotion() {
        // A already Mates with B. A↔C primed above Mates thresholds —
        // pre-453 would promote to Mates on the next check_bonds tick.
        // The exclusivity gate caps the new bond at Partners.
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        let a = spawn_adult(&mut world, "Fern", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);
        let c = spawn_adult(&mut world, "Tamsin", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let ab = rels.get_or_insert(a, b);
        ab.bond = Some(BondType::Mates);
        ab.fondness = 0.9;
        ab.familiarity = 0.9;
        ab.romantic = 0.9;
        let ac = rels.get_or_insert(a, c);
        ac.fondness = 0.9;
        ac.familiarity = 0.9;
        ac.romantic = 0.9;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let rels = world.resource::<Relationships>();
        assert_eq!(
            rels.get(a, b).unwrap().bond,
            Some(BondType::Mates),
            "the existing Mates pair stays at Mates"
        );
        assert_eq!(
            rels.get(a, c).unwrap().bond,
            Some(BondType::Partners),
            "second Mates promotion capped at Partners by the exclusivity gate"
        );
    }

    #[test]
    fn check_bonds_demotes_pre_existing_polyamory_to_one_partner() {
        // Pre-seed an invalid state: A-Mates(B) AND A-Mates(C). One tick
        // of check_bonds migrates the colony to the invariant by
        // demoting the non-canonical pair to Partners.
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        let a = spawn_adult(&mut world, "Fern", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);
        let c = spawn_adult(&mut world, "Tamsin", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        make_pair_at_mates(&mut rels, a, b);
        make_pair_at_mates(&mut rels, a, c);
        world.insert_resource(rels);

        schedule.run(&mut world);

        let rels = world.resource::<Relationships>();
        let a_mates: Vec<_> = rels
            .iter_for(a)
            .filter(|(_, rel)| rel.bond == Some(BondType::Mates))
            .collect();
        assert_eq!(
            a_mates.len(),
            1,
            "A must end with exactly 1 Mates bond after migration; got {a_mates:?}"
        );
    }

    #[test]
    fn collect_excess_mates_is_noop_when_invariant_holds() {
        // Two disjoint Mates pairs — no demotions needed.
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();
        let d = world.spawn_empty().id();

        let mut rels = Relationships::default();
        make_pair_at_mates(&mut rels, a, b);
        make_pair_at_mates(&mut rels, c, d);

        let by_cat = mates_partners_by_cat(&rels);
        let to_demote = collect_excess_mates_to_demote(&by_cat);
        assert!(
            to_demote.is_empty(),
            "no demotions when each cat already has ≤1 Mates partner"
        );
    }
}
