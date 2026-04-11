use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::components::mental::MemoryType;
use crate::components::physical::Position;

// ---------------------------------------------------------------------------
// KnowledgeEntry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct KnowledgeEntry {
    pub event_type: MemoryType,
    /// Bucketed location (rounded to nearest 5 tiles).
    pub location: Option<Position>,
    /// Knowledge strength in `[0.0, 1.0]`. Decays very slowly.
    pub strength: f32,
    /// Number of living cats who hold a matching individual memory.
    pub carrier_count: u32,
}

// ---------------------------------------------------------------------------
// ColonyKnowledge resource
// ---------------------------------------------------------------------------

/// Collective knowledge shared across the colony. Entries are promoted from
/// individual cat memories when 3+ cats hold the same memory. Colony knowledge
/// decays much more slowly than individual memory and provides colony-wide
/// utility modifiers to AI scoring.
#[derive(Resource, Debug, Default)]
pub struct ColonyKnowledge {
    pub entries: Vec<KnowledgeEntry>,
    /// Tracks the last tick each knowledge description was narrated as forgotten,
    /// preventing the same "colony has forgotten X" message from spamming the log
    /// when knowledge is repeatedly promoted and decayed.
    pub recently_forgotten: HashMap<String, u64>,
}

impl ColonyKnowledge {
    /// Bucket a position to a ~5-tile grid for approximate matching.
    pub fn bucket_position(pos: &Position) -> Position {
        Position::new((pos.x / 5) * 5 + 2, (pos.y / 5) * 5 + 2)
    }

    /// Check whether an entry matching this (event_type, bucketed_location) exists.
    pub fn has_entry(&self, event_type: MemoryType, location: &Option<Position>) -> bool {
        self.entries
            .iter()
            .any(|e| e.event_type == event_type && approx_location_match(&e.location, location))
    }

    /// Find the index of an entry matching (event_type, bucketed_location).
    pub fn find_entry(&self, event_type: MemoryType, location: &Option<Position>) -> Option<usize> {
        self.entries.iter().position(|e| {
            e.event_type == event_type && approx_location_match(&e.location, location)
        })
    }
}

/// Check whether two bucketed locations are approximately the same.
fn approx_location_match(a: &Option<Position>, b: &Option<Position>) -> bool {
    match (a, b) {
        (Some(pa), Some(pb)) => pa.x == pb.x && pa.y == pb.y,
        (None, None) => true,
        _ => false,
    }
}

/// Human-readable description of a knowledge entry for narrative purposes.
pub fn knowledge_description(entry: &KnowledgeEntry) -> String {
    let location_desc = match &entry.location {
        Some(pos) => {
            let compass = if pos.x > 40 { "eastern" } else { "western" };
            let terrain = if pos.y < 20 { "ridge" } else { "lowlands" };
            format!("the {compass} {terrain}")
        }
        None => "the colony's surroundings".to_string(),
    };

    match entry.event_type {
        MemoryType::ThreatSeen => format!("the danger near {location_desc}"),
        MemoryType::Death => format!("the loss near {location_desc}"),
        MemoryType::ResourceFound => format!("the good foraging near {location_desc}"),
        MemoryType::MagicEvent => format!("the strange happenings near {location_desc}"),
        MemoryType::Injury => format!("the peril near {location_desc}"),
        MemoryType::SocialEvent => format!("the gathering near {location_desc}"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_position_rounds_to_grid() {
        assert_eq!(
            ColonyKnowledge::bucket_position(&Position::new(0, 0)),
            Position::new(2, 2)
        );
        assert_eq!(
            ColonyKnowledge::bucket_position(&Position::new(3, 7)),
            Position::new(2, 7)
        );
        assert_eq!(
            ColonyKnowledge::bucket_position(&Position::new(5, 10)),
            Position::new(7, 12)
        );
        assert_eq!(
            ColonyKnowledge::bucket_position(&Position::new(12, 23)),
            Position::new(12, 22)
        );
    }

    #[test]
    fn has_entry_matches_by_type_and_location() {
        let mut ck = ColonyKnowledge::default();
        let bucketed = ColonyKnowledge::bucket_position(&Position::new(10, 10));
        ck.entries.push(KnowledgeEntry {
            event_type: MemoryType::ThreatSeen,
            location: Some(bucketed),
            strength: 0.5,
            carrier_count: 3,
        });

        assert!(ck.has_entry(MemoryType::ThreatSeen, &Some(bucketed)));
        assert!(!ck.has_entry(MemoryType::ResourceFound, &Some(bucketed)));
        assert!(!ck.has_entry(MemoryType::ThreatSeen, &None));
    }

    #[test]
    fn find_entry_returns_correct_index() {
        let mut ck = ColonyKnowledge::default();
        let pos_a = ColonyKnowledge::bucket_position(&Position::new(5, 5));
        let pos_b = ColonyKnowledge::bucket_position(&Position::new(20, 20));

        ck.entries.push(KnowledgeEntry {
            event_type: MemoryType::ResourceFound,
            location: Some(pos_a),
            strength: 0.5,
            carrier_count: 3,
        });
        ck.entries.push(KnowledgeEntry {
            event_type: MemoryType::ThreatSeen,
            location: Some(pos_b),
            strength: 0.8,
            carrier_count: 5,
        });

        assert_eq!(ck.find_entry(MemoryType::ThreatSeen, &Some(pos_b)), Some(1));
        assert_eq!(ck.find_entry(MemoryType::Death, &Some(pos_a)), None);
    }

    #[test]
    fn knowledge_description_varies_by_type() {
        let entry = KnowledgeEntry {
            event_type: MemoryType::ThreatSeen,
            location: Some(Position::new(50, 10)),
            strength: 0.5,
            carrier_count: 3,
        };
        let desc = knowledge_description(&entry);
        assert!(desc.contains("danger"), "expected 'danger', got: {desc}");
        assert!(
            desc.contains("eastern"),
            "expected 'eastern' for x=50, got: {desc}"
        );
    }
}
