use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::resources::sim_constants::RelationshipsConstants;

// ---------------------------------------------------------------------------
// BondType
// ---------------------------------------------------------------------------

/// Named bond between two cats. Ordered by intensity for upgrade detection.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum BondType {
    Friends,
    Partners,
    Mates,
}

// ---------------------------------------------------------------------------
// Relationship
// ---------------------------------------------------------------------------

/// The state of the relationship between two cats.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Relationship {
    /// How much they like each other (-1.0 hostile .. 1.0 devoted).
    pub fondness: f32,
    /// How well they know each other (0.0 stranger .. 1.0 deeply known).
    pub familiarity: f32,
    /// Romantic attachment (0.0 none .. 1.0 deeply in love).
    pub romantic: f32,
    /// Named bond, if any.
    pub bond: Option<BondType>,
    /// Tick of last direct interaction.
    pub last_interaction: u64,
}

impl Default for Relationship {
    fn default() -> Self {
        Self {
            fondness: 0.0,
            familiarity: 0.0,
            romantic: 0.0,
            bond: None,
            last_interaction: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Relationships resource
// ---------------------------------------------------------------------------

/// Colony-wide relationship graph. Symmetric: `get(a, b)` and `get(b, a)`
/// always return the same entry.
///
/// Stored as a `BTreeMap` (not `HashMap`) so `all_for` and `iter` yield a
/// stable, process-independent order. Coordinator election sums f32
/// fondness/familiarity over `all_for(entity)`, and float addition is
/// non-associative, so a `HashMap` produced 1-ULP drift in `social_weight`
/// across same-seed runs of the same binary — enough to flip tiebreaks in
/// downstream sorts.
#[derive(Resource, Debug, Default)]
pub struct Relationships {
    data: BTreeMap<(Entity, Entity), Relationship>,
}

/// Normalize a pair so the entity with the smaller index comes first.
fn normalize_key(a: Entity, b: Entity) -> (Entity, Entity) {
    if a.index() <= b.index() {
        (a, b)
    } else {
        (b, a)
    }
}

impl Relationships {
    /// Look up an existing relationship. Returns `None` if the pair has never
    /// been recorded.
    pub fn get(&self, a: Entity, b: Entity) -> Option<&Relationship> {
        self.data.get(&normalize_key(a, b))
    }

    /// Mutable access to an existing relationship.
    pub fn get_mut(&mut self, a: Entity, b: Entity) -> Option<&mut Relationship> {
        self.data.get_mut(&normalize_key(a, b))
    }

    /// Get or insert a default relationship for the pair.
    pub fn get_or_insert(&mut self, a: Entity, b: Entity) -> &mut Relationship {
        self.data.entry(normalize_key(a, b)).or_default()
    }

    /// Adjust fondness, clamped to \[-1.0, 1.0\].
    pub fn modify_fondness(&mut self, a: Entity, b: Entity, delta: f32) {
        let rel = self.get_or_insert(a, b);
        rel.fondness = (rel.fondness + delta).clamp(-1.0, 1.0);
    }

    /// Adjust familiarity, clamped to \[0.0, 1.0\].
    pub fn modify_familiarity(&mut self, a: Entity, b: Entity, delta: f32) {
        let rel = self.get_or_insert(a, b);
        rel.familiarity = (rel.familiarity + delta).clamp(0.0, 1.0);
    }

    /// Ticket 500 — apply one familiarity `delta` to every pair in
    /// `keys` via a single merge-join walk over the sorted map, instead
    /// of one O(log pairs) `entry` descent per pair.
    ///
    /// `passive_familiarity` calls `modify_familiarity` once per cached
    /// near-pair per tick; at the 2026-06-09 flamegraph those repeated
    /// root-to-leaf `BTreeMap::entry` walks were 16.55% self CPU. Both
    /// `NearPairCache::pairs` and `self.data` are `BTreeMap`s over the
    /// same `normalize_key`-canonicalized `(Entity, Entity)` key, so the
    /// two sorted sequences co-walk with two cursors in
    /// O(pairs_total + keys) with sequential memory access.
    ///
    /// Contract: `keys` must yield keys already normalized (per
    /// [`normalize_key`] / `near_pair_cache::normalize_pair`) in strictly
    /// ascending `(Entity, Entity)` order — exactly what iterating a
    /// `BTreeMap`'s keys produces. Keys absent from the map are inserted
    /// with `Relationship::default()` before the delta lands, matching
    /// [`Self::modify_familiarity`]'s `get_or_insert` semantics.
    pub fn modify_familiarity_batch(
        &mut self,
        keys: impl Iterator<Item = (Entity, Entity)>,
        delta: f32,
    ) {
        let mut keys = keys.peekable();
        // Pairs present in `keys` but not yet in the map — get_or_insert
        // semantics. Usually empty (pairs exist after first contact), so
        // the Vec rarely allocates.
        let mut missing: Vec<(Entity, Entity)> = Vec::new();
        let mut prev: Option<(Entity, Entity)> = None;
        for (&map_key, rel) in self.data.iter_mut() {
            loop {
                match keys.peek() {
                    Some(&k) if k < map_key => {
                        debug_assert!(
                            prev.is_none_or(|p| p < k),
                            "modify_familiarity_batch keys not strictly ascending",
                        );
                        prev = Some(k);
                        missing.push(k);
                        keys.next();
                    }
                    Some(&k) if k == map_key => {
                        debug_assert!(
                            prev.is_none_or(|p| p < k),
                            "modify_familiarity_batch keys not strictly ascending",
                        );
                        prev = Some(k);
                        rel.familiarity = (rel.familiarity + delta).clamp(0.0, 1.0);
                        keys.next();
                        break;
                    }
                    _ => break,
                }
            }
        }
        // Keys beyond the last map entry.
        for k in keys {
            debug_assert!(
                prev.is_none_or(|p| p < k),
                "modify_familiarity_batch keys not strictly ascending",
            );
            prev = Some(k);
            missing.push(k);
        }
        for (a, b) in missing {
            debug_assert_eq!((a, b), normalize_key(a, b));
            let rel = self.data.entry((a, b)).or_default();
            rel.familiarity = (rel.familiarity + delta).clamp(0.0, 1.0);
        }
    }

    /// Adjust romantic attachment, clamped to \[0.0, 1.0\].
    pub fn modify_romantic(&mut self, a: Entity, b: Entity, delta: f32) {
        let rel = self.get_or_insert(a, b);
        rel.romantic = (rel.romantic + delta).clamp(0.0, 1.0);
    }

    /// All relationships involving `entity`, yielding `(other_entity, &Relationship)`.
    pub fn all_for(&self, entity: Entity) -> Vec<(Entity, &Relationship)> {
        self.iter_for(entity).collect()
    }

    /// Ticket 427 Step 4 — no-alloc iterator over relationships
    /// involving `entity`. Preferred over [`Self::all_for`] when the
    /// caller only iterates / filters / sums — `all_for` materializes
    /// the entire Vec which the 427 perf survey flagged as a small but
    /// avoidable per-tick alloc hotspot (~800 KB/soak).
    ///
    /// **Cost: O(total pairs), NOT O(degree)** (ticket 500). This is a
    /// filter over the entire pair-keyed map — there is no per-entity
    /// index. Calling it per-actor (or worse, per-candidate) from a
    /// per-tick path multiplies by total pair count; the 453 courtship
    /// gates did exactly that and cost 22% of the flamegraph before 459
    /// hoisted them to a once-per-tick set. Hoist hot call sites to a
    /// per-tick precomputed set/map before reaching for this in a loop.
    pub fn iter_for(&self, entity: Entity) -> impl Iterator<Item = (Entity, &Relationship)> + '_ {
        self.data.iter().filter_map(move |(&(a, b), rel)| {
            if a == entity {
                Some((b, rel))
            } else if b == entity {
                Some((a, rel))
            } else {
                None
            }
        })
    }

    /// Iterate over all relationship pairs mutably.
    pub fn pairs_iter_mut(
        &mut self,
    ) -> impl Iterator<Item = ((Entity, Entity), &mut Relationship)> {
        self.data.iter_mut().map(|(&key, rel)| (key, rel))
    }

    /// Iterate over all stored relationship pairs and their data.
    pub fn iter(&self) -> impl Iterator<Item = ((Entity, Entity), &Relationship)> {
        self.data.iter().map(|(&key, rel)| (key, rel))
    }

    /// Insert a relationship directly (used by save/load).
    pub fn insert(&mut self, a: Entity, b: Entity, rel: Relationship) {
        self.data.insert(normalize_key(a, b), rel);
    }

    /// Initialize a relationship between two cats with randomized starting
    /// values from `RelationshipsConstants`. Founder graphs straddle the
    /// Friends-graduation gate (`SocialConstants::friends_familiarity_threshold`)
    /// so a fraction of pairs graduate to BondType::Friends on the first
    /// `bond_check_interval` firing — encoding founder heterogeneity through
    /// the existing bond-check flow rather than setup-time bond magic.
    pub fn init_pair(
        &mut self,
        a: Entity,
        b: Entity,
        rng: &mut impl Rng,
        constants: &RelationshipsConstants,
    ) {
        let rel = Relationship {
            fondness: rng
                .random_range(constants.founder_fondness_min..constants.founder_fondness_max),
            familiarity: rng
                .random_range(constants.founder_familiarity_min..constants.founder_familiarity_max),
            romantic: 0.0,
            bond: None,
            last_interaction: 0,
        };
        self.data.insert(normalize_key(a, b), rel);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a pair of dummy entities for testing.
    fn test_entities() -> (Entity, Entity) {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        (a, b)
    }

    #[test]
    fn normalize_key_is_symmetric() {
        let (a, b) = test_entities();
        assert_eq!(normalize_key(a, b), normalize_key(b, a));
    }

    #[test]
    fn get_returns_same_for_either_order() {
        let (a, b) = test_entities();
        let mut rels = Relationships::default();
        rels.init_pair(a, b, &mut rand::rng(), &RelationshipsConstants::default());

        let fondness_ab = rels.get(a, b).unwrap().fondness;
        let fondness_ba = rels.get(b, a).unwrap().fondness;
        assert_eq!(fondness_ab, fondness_ba);
    }

    #[test]
    fn modify_fondness_clamps() {
        let (a, b) = test_entities();
        let mut rels = Relationships::default();
        rels.get_or_insert(a, b);

        rels.modify_fondness(a, b, 5.0);
        assert_eq!(rels.get(a, b).unwrap().fondness, 1.0);

        rels.modify_fondness(a, b, -10.0);
        assert_eq!(rels.get(a, b).unwrap().fondness, -1.0);
    }

    #[test]
    fn modify_familiarity_clamps() {
        let (a, b) = test_entities();
        let mut rels = Relationships::default();
        rels.get_or_insert(a, b);

        rels.modify_familiarity(a, b, 5.0);
        assert_eq!(rels.get(a, b).unwrap().familiarity, 1.0);

        rels.modify_familiarity(a, b, -10.0);
        assert_eq!(rels.get(a, b).unwrap().familiarity, 0.0);
    }

    #[test]
    fn modify_romantic_clamps() {
        let (a, b) = test_entities();
        let mut rels = Relationships::default();
        rels.get_or_insert(a, b);

        rels.modify_romantic(a, b, 5.0);
        assert_eq!(rels.get(a, b).unwrap().romantic, 1.0);

        rels.modify_romantic(a, b, -10.0);
        assert_eq!(rels.get(a, b).unwrap().romantic, 0.0);
    }

    /// Ticket 500 — the batch merge-join must be observationally
    /// identical to N individual `modify_familiarity` calls: same
    /// updates, same clamps, same get_or_insert behavior for missing
    /// pairs.
    #[test]
    fn modify_familiarity_batch_matches_individual_calls() {
        let mut world = World::new();
        let ents: Vec<Entity> = (0..6).map(|_| world.spawn_empty().id()).collect();

        // Seed a map with some pairs at varied familiarity, including
        // one near the 1.0 clamp.
        let seed = |rels: &mut Relationships| {
            rels.get_or_insert(ents[0], ents[1]).familiarity = 0.25;
            rels.get_or_insert(ents[0], ents[2]).familiarity = 0.98;
            rels.get_or_insert(ents[2], ents[3]).familiarity = 0.5;
            rels.get_or_insert(ents[4], ents[5]).familiarity = 0.0;
        };
        let mut batch = Relationships::default();
        let mut individual = Relationships::default();
        seed(&mut batch);
        seed(&mut individual);

        // Keys: two existing pairs, one missing pair (get_or_insert
        // path), one pair beyond the last map entry. Collected via a
        // BTreeMap so ordering matches the batch contract.
        let keys: BTreeMap<(Entity, Entity), ()> = [
            (ents[0], ents[1]),
            (ents[0], ents[2]),
            (ents[1], ents[3]), // missing from the map
            (ents[4], ents[5]),
        ]
        .into_iter()
        .map(|(a, b)| (normalize_key(a, b), ()))
        .collect();

        let delta = 0.05;
        batch.modify_familiarity_batch(keys.keys().copied(), delta);
        for &(a, b) in keys.keys() {
            individual.modify_familiarity(a, b, delta);
        }

        let batch_state: Vec<_> = batch
            .iter()
            .map(|(k, r)| (k, r.familiarity.to_bits()))
            .collect();
        let individual_state: Vec<_> = individual
            .iter()
            .map(|(k, r)| (k, r.familiarity.to_bits()))
            .collect();
        assert_eq!(batch_state, individual_state);
        // Clamp actually engaged on the 0.98 pair.
        assert_eq!(batch.get(ents[0], ents[2]).unwrap().familiarity, 1.0);
        // Missing pair inserted with default + delta.
        assert_eq!(batch.get(ents[1], ents[3]).unwrap().familiarity, delta);
    }

    /// Empty key set must be a no-op (and not touch existing entries).
    #[test]
    fn modify_familiarity_batch_empty_keys_is_noop() {
        let (a, b) = test_entities();
        let mut rels = Relationships::default();
        rels.get_or_insert(a, b).familiarity = 0.4;
        rels.modify_familiarity_batch(std::iter::empty(), 0.1);
        assert_eq!(rels.get(a, b).unwrap().familiarity, 0.4);
    }

    #[test]
    fn all_for_returns_correct_pairs() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();

        let constants = RelationshipsConstants::default();
        let mut rels = Relationships::default();
        rels.init_pair(a, b, &mut rand::rng(), &constants);
        rels.init_pair(a, c, &mut rand::rng(), &constants);
        rels.init_pair(b, c, &mut rand::rng(), &constants);

        let a_rels = rels.all_for(a);
        assert_eq!(a_rels.len(), 2, "entity a should have 2 relationships");

        let b_rels = rels.all_for(b);
        assert_eq!(b_rels.len(), 2, "entity b should have 2 relationships");

        let c_rels = rels.all_for(c);
        assert_eq!(c_rels.len(), 2, "entity c should have 2 relationships");
    }

    #[test]
    fn init_pair_values_in_range() {
        use rand_chacha::rand_core::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        let mut world = World::new();
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let constants = RelationshipsConstants::default();

        for _ in 0..100 {
            let a = world.spawn_empty().id();
            let b = world.spawn_empty().id();

            let mut rels = Relationships::default();
            rels.init_pair(a, b, &mut rng, &constants);

            let rel = rels.get(a, b).unwrap();
            assert!(
                (constants.founder_fondness_min..constants.founder_fondness_max)
                    .contains(&rel.fondness),
                "fondness {} out of range",
                rel.fondness,
            );
            assert!(
                (constants.founder_familiarity_min..constants.founder_familiarity_max)
                    .contains(&rel.familiarity),
                "familiarity {} out of range",
                rel.familiarity,
            );
            assert_eq!(rel.romantic, 0.0);
            assert!(rel.bond.is_none());
        }
    }
}
