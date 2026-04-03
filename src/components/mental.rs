use std::collections::VecDeque;
use bevy_ecs::prelude::*;
use crate::components::physical::Position;

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
}
