use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rand::Rng;

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
#[derive(Resource, Debug, Default)]
pub struct Relationships {
    data: HashMap<(Entity, Entity), Relationship>,
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

    /// Adjust romantic attachment, clamped to \[0.0, 1.0\].
    pub fn modify_romantic(&mut self, a: Entity, b: Entity, delta: f32) {
        let rel = self.get_or_insert(a, b);
        rel.romantic = (rel.romantic + delta).clamp(0.0, 1.0);
    }

    /// All relationships involving `entity`, yielding `(other_entity, &Relationship)`.
    pub fn all_for(&self, entity: Entity) -> Vec<(Entity, &Relationship)> {
        self.data
            .iter()
            .filter_map(move |(&(a, b), rel)| {
                if a == entity {
                    Some((b, rel))
                } else if b == entity {
                    Some((a, rel))
                } else {
                    None
                }
            })
            .collect()
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

    /// Initialize a relationship between two cats with randomized starting values
    /// appropriate for a newly-formed colony.
    pub fn init_pair(&mut self, a: Entity, b: Entity, rng: &mut impl Rng) {
        let rel = Relationship {
            fondness: rng.random_range(-0.2f32..0.3f32),
            familiarity: rng.random_range(0.1f32..0.3f32),
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
        rels.init_pair(a, b, &mut rand::rng());

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

    #[test]
    fn all_for_returns_correct_pairs() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();

        let mut rels = Relationships::default();
        rels.init_pair(a, b, &mut rand::rng());
        rels.init_pair(a, c, &mut rand::rng());
        rels.init_pair(b, c, &mut rand::rng());

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

        for _ in 0..100 {
            let a = world.spawn_empty().id();
            let b = world.spawn_empty().id();

            let mut rels = Relationships::default();
            rels.init_pair(a, b, &mut rng);

            let rel = rels.get(a, b).unwrap();
            assert!(
                (-0.2..0.3).contains(&rel.fondness),
                "fondness {} out of range",
                rel.fondness,
            );
            assert!(
                (0.1..0.3).contains(&rel.familiarity),
                "familiarity {} out of range",
                rel.familiarity,
            );
            assert_eq!(rel.romantic, 0.0);
            assert!(rel.bond.is_none());
        }
    }
}
