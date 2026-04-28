use crate::components::physical::Position;
use bevy_ecs::prelude::*;
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Mood
// ---------------------------------------------------------------------------

/// A time-limited mood modifier applied additively to a cat's base mood.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MoodModifier {
    /// Amount to shift valence; positive is happier, negative is sadder.
    pub amount: f32,
    /// How many ticks remain before this modifier expires.
    pub ticks_remaining: u64,
    /// Human-readable source for debugging / narrative ("ate a nice fish").
    pub source: String,
}

/// Current emotional state. Valence is the net mood signal after applying all
/// active modifiers to the base.
///
/// Default valence of 0.2 represents a mildly content cat.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Mood {
    /// Base mood in `[-1.0, 1.0]`. −1 is miserable, +1 is euphoric.
    pub valence: f32,
    /// Active temporary modifiers, oldest-first.
    pub modifiers: VecDeque<MoodModifier>,
}

impl Default for Mood {
    fn default() -> Self {
        Self {
            valence: 0.2,
            modifiers: VecDeque::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------

/// Categories of memorable events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MemoryType {
    ThreatSeen,
    ResourceFound,
    Death,
    MagicEvent,
    Injury,
    SocialEvent,
    /// A moment of colony triumph — e.g. a banished shadow-fox. Carries
    /// very long-lasting emotional weight; defines the cat's sense of
    /// identity and courage for the rest of its life.
    Triumph,
}

/// A single memory entry.
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub event_type: MemoryType,
    /// Where it happened, if relevant.
    pub location: Option<Position>,
    /// Which entities were involved (may be empty for impersonal events).
    pub involved: Vec<Entity>,
    /// Simulation tick when this event occurred.
    pub tick: u64,
    /// Emotional/mnemonic weight in `[0.0, 1.0]`. High-strength memories
    /// persist longer in practice (callers may use this to prioritise).
    pub strength: f32,
    /// `true` if the cat witnessed this directly; `false` for hearsay.
    pub firsthand: bool,
}

/// Rolling memory buffer. When at capacity, the weakest entry is evicted.
#[derive(Component, Debug, Clone)]
pub struct Memory {
    pub events: VecDeque<MemoryEntry>,
    /// Maximum number of entries retained.
    pub capacity: usize,
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            events: VecDeque::new(),
            capacity: 20,
        }
    }
}

impl Memory {
    /// Add a new memory. If at capacity, the weakest (lowest strength) entry is
    /// evicted to make room.
    pub fn remember(&mut self, entry: MemoryEntry) {
        if self.events.len() >= self.capacity {
            if let Some(weakest_idx) = self
                .events
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    a.strength
                        .partial_cmp(&b.strength)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
            {
                self.events.remove(weakest_idx);
            }
        }
        self.events.push_back(entry);
    }
}

// ---------------------------------------------------------------------------
// Location Preferences (tradition system)
// ---------------------------------------------------------------------------

/// Per-cat record of successful action locations. Used by the tradition
/// personality modifier to create "favorite spots" — traditional cats prefer
/// tiles where they've previously succeeded.
///
/// Capped at `MAX_ENTRIES`; evicts the least-visited entry on overflow.
#[derive(Component, Debug, Clone, Default)]
pub struct LocationPreferences {
    /// (x, y, action, success_count) tuples.
    entries: Vec<(i32, i32, crate::ai::Action, u32)>,
}

impl LocationPreferences {
    const MAX_ENTRIES: usize = 20;

    /// Record a successful action at a tile. Increments existing count or adds
    /// a new entry (evicting the least-visited if at capacity).
    pub fn record_success(&mut self, x: i32, y: i32, action: crate::ai::Action) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.0 == x && e.1 == y && e.2 == action)
        {
            entry.3 += 1;
        } else {
            if self.entries.len() >= Self::MAX_ENTRIES {
                if let Some(min_idx) = self
                    .entries
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, e)| e.3)
                    .map(|(i, _)| i)
                {
                    self.entries.remove(min_idx);
                }
            }
            self.entries.push((x, y, action, 1));
        }
    }

    /// How many times this cat succeeded at `action` on tile `(x, y)`.
    pub fn success_count(&self, x: i32, y: i32, action: crate::ai::Action) -> u32 {
        self.entries
            .iter()
            .find(|e| e.0 == x && e.1 == y && e.2 == action)
            .map_or(0, |e| e.3)
    }

    /// The tile position this cat has visited most frequently (across all
    /// actions), or `None` if no data exists.
    ///
    /// Backed by `BTreeMap` (not `HashMap`) because `max_by_key` returns the
    /// last-iterated max on ties, and `HashMap`'s iteration order varies per
    /// process — same input picked different tiles on different runs, breaking
    /// same-seed replay.
    pub fn most_frequented(&self) -> Option<(i32, i32)> {
        let mut totals: std::collections::BTreeMap<(i32, i32), u32> =
            std::collections::BTreeMap::new();
        for &(x, y, _, count) in &self.entries {
            *totals.entry((x, y)).or_default() += count;
        }
        totals
            .into_iter()
            .max_by_key(|&(_, count)| count)
            .map(|(pos, _)| pos)
    }
}

// ---------------------------------------------------------------------------
// Pride Cooldown
// ---------------------------------------------------------------------------

/// Tracks per-cat cooldown for `PrideCrisis` events to prevent spam.
#[derive(Component, Debug, Clone, Default)]
pub struct PrideCooldown {
    pub last_pride_crisis_tick: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(strength: f32) -> MemoryEntry {
        MemoryEntry {
            event_type: MemoryType::ResourceFound,
            location: None,
            involved: vec![],
            tick: 0,
            strength,
            firsthand: true,
        }
    }

    #[test]
    fn remember_evicts_weakest_when_at_capacity() {
        let mut memory = Memory {
            events: VecDeque::new(),
            capacity: 3,
        };

        memory.remember(make_entry(0.5));
        memory.remember(make_entry(0.2)); // weakest
        memory.remember(make_entry(0.8));

        // At capacity — next remember should evict the 0.2 entry.
        memory.remember(make_entry(0.9));

        assert_eq!(memory.events.len(), 3);
        let strengths: Vec<f32> = memory.events.iter().map(|e| e.strength).collect();
        assert!(
            !strengths.contains(&0.2),
            "weakest memory (0.2) should be evicted; got {strengths:?}"
        );
        assert!(
            strengths.contains(&0.9),
            "new memory (0.9) should be present; got {strengths:?}"
        );
    }

    // --- LocationPreferences tests ---

    #[test]
    fn location_prefs_records_and_retrieves() {
        let mut prefs = LocationPreferences::default();
        prefs.record_success(5, 10, crate::ai::Action::Hunt);
        prefs.record_success(5, 10, crate::ai::Action::Hunt);
        assert_eq!(prefs.success_count(5, 10, crate::ai::Action::Hunt), 2);
        assert_eq!(prefs.success_count(5, 10, crate::ai::Action::Forage), 0);
    }

    #[test]
    fn location_prefs_caps_at_20_entries() {
        let mut prefs = LocationPreferences::default();
        for i in 0..25 {
            prefs.record_success(i, 0, crate::ai::Action::Wander);
        }
        assert_eq!(prefs.entries.len(), 20);
    }

    #[test]
    fn location_prefs_evicts_least_visited() {
        let mut prefs = LocationPreferences::default();
        // Fill with 20 entries, each with 1 visit
        for i in 0..20 {
            prefs.record_success(i, 0, crate::ai::Action::Wander);
        }
        // Give one entry a higher count
        prefs.record_success(5, 0, crate::ai::Action::Wander);
        // Add a 21st entry — should evict a 1-count entry, not the 2-count
        prefs.record_success(99, 0, crate::ai::Action::Wander);
        assert_eq!(prefs.entries.len(), 20);
        assert_eq!(
            prefs.success_count(5, 0, crate::ai::Action::Wander),
            2,
            "high-count entry should survive eviction"
        );
        assert_eq!(
            prefs.success_count(99, 0, crate::ai::Action::Wander),
            1,
            "new entry should be present"
        );
    }

    #[test]
    fn location_prefs_most_frequented() {
        let mut prefs = LocationPreferences::default();
        prefs.record_success(1, 1, crate::ai::Action::Hunt);
        prefs.record_success(2, 2, crate::ai::Action::Hunt);
        prefs.record_success(2, 2, crate::ai::Action::Forage);
        prefs.record_success(2, 2, crate::ai::Action::Forage);
        // (2,2) has 3 total visits; (1,1) has 1
        assert_eq!(prefs.most_frequented(), Some((2, 2)));
    }

    #[test]
    fn location_prefs_most_frequented_empty() {
        let prefs = LocationPreferences::default();
        assert_eq!(prefs.most_frequented(), None);
    }
}
