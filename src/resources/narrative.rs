use std::collections::VecDeque;

use bevy_ecs::prelude::Resource;

// ---------------------------------------------------------------------------
// NarrativeTier
// ---------------------------------------------------------------------------

/// Importance level for a narrative entry.
///
/// - `Micro`       — low-salience events (idle observations, ambient colour)
/// - `Action`      — routine actions a cat completes (eating, sleeping, wandering)
/// - `Significant` — story-worthy moments (first fight, death, major discovery)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NarrativeTier {
    Micro,
    Action,
    Significant,
}

// ---------------------------------------------------------------------------
// NarrativeEntry
// ---------------------------------------------------------------------------

/// A single timestamped narrative line.
#[derive(Debug, Clone)]
pub struct NarrativeEntry {
    /// Simulation tick at which this entry was generated.
    pub tick: u64,
    /// Human-readable text.
    pub text: String,
    /// Importance tier for display filtering.
    pub tier: NarrativeTier,
}

// ---------------------------------------------------------------------------
// NarrativeLog resource
// ---------------------------------------------------------------------------

/// Ring-buffer of recent narrative entries. Oldest entries are dropped once
/// `capacity` is exceeded.
#[derive(Resource, Debug)]
pub struct NarrativeLog {
    pub entries: VecDeque<NarrativeEntry>,
    /// Maximum number of entries retained.
    pub capacity: usize,
}

impl Default for NarrativeLog {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            capacity: 200,
        }
    }
}

impl NarrativeLog {
    /// Append a new entry. Drops the oldest entry if capacity is exceeded.
    pub fn push(&mut self, tick: u64, text: String, tier: NarrativeTier) {
        self.entries.push_back(NarrativeEntry { tick, text, tier });
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_log_is_empty_with_capacity_200() {
        let log = NarrativeLog::default();
        assert!(log.entries.is_empty());
        assert_eq!(log.capacity, 200);
    }

    #[test]
    fn push_adds_entry() {
        let mut log = NarrativeLog::default();
        log.push(1, "Mochi eats.".to_string(), NarrativeTier::Action);
        assert_eq!(log.entries.len(), 1);
        let e = &log.entries[0];
        assert_eq!(e.tick, 1);
        assert_eq!(e.text, "Mochi eats.");
        assert_eq!(e.tier, NarrativeTier::Action);
    }

    #[test]
    fn push_trims_to_capacity() {
        let mut log = NarrativeLog::default();
        log.capacity = 3;
        for i in 0..5u64 {
            log.push(i, format!("entry {i}"), NarrativeTier::Micro);
        }
        assert_eq!(log.entries.len(), 3);
        // Oldest two dropped; first remaining is entry 2
        assert_eq!(log.entries[0].tick, 2);
        assert_eq!(log.entries[2].tick, 4);
    }
}
