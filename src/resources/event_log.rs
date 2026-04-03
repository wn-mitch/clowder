use std::collections::VecDeque;

use bevy_ecs::prelude::Resource;

use crate::ai::Action;
use crate::components::personality::Personality;
use crate::components::physical::Needs;
use crate::components::skills::Skills;

// ---------------------------------------------------------------------------
// Snapshot sub-types
// ---------------------------------------------------------------------------

/// A relationship entry for the CatSnapshot event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RelationshipEntry {
    pub cat: String,
    pub fondness: f32,
    pub familiarity: f32,
    pub romantic: f32,
    pub bond: Option<String>,
}

// ---------------------------------------------------------------------------
// EventKind
// ---------------------------------------------------------------------------

/// Structured event types for mechanical debugging.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum EventKind {
    ActionChosen {
        cat: String,
        action: Action,
        score: f32,
        runner_up: Action,
        runner_up_score: f32,
        third: Action,
        third_score: f32,
    },
    CatSnapshot {
        cat: String,
        position: (i32, i32),
        personality: Personality,
        needs: Needs,
        skills: Skills,
        mood_valence: f32,
        mood_modifier_count: usize,
        health: f32,
        corruption: f32,
        magic_affinity: f32,
        current_action: Action,
        relationships: Vec<RelationshipEntry>,
        /// Top-3 scored actions from the last decision (post-bonus, post-suppression).
        last_scores: Vec<(Action, f32)>,
    },
    FoodLevel {
        current: f32,
        capacity: f32,
        fraction: f32,
    },
    CoordinatorElected {
        cat: String,
        social_weight: f32,
    },
    DirectiveIssued {
        coordinator: String,
        kind: String,
        priority: f32,
    },
    Death {
        cat: String,
        cause: String,
    },
}

// ---------------------------------------------------------------------------
// EventEntry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct EventEntry {
    pub tick: u64,
    #[serde(flatten)]
    pub kind: EventKind,
}

// ---------------------------------------------------------------------------
// EventLog resource
// ---------------------------------------------------------------------------

/// Ring-buffer of structured simulation events for debugging.
#[derive(Resource, Debug)]
pub struct EventLog {
    pub entries: VecDeque<EventEntry>,
    pub capacity: usize,
    pub total_pushed: u64,
}

impl Default for EventLog {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            capacity: 500,
            total_pushed: 0,
        }
    }
}

impl EventLog {
    pub fn push(&mut self, tick: u64, kind: EventKind) {
        self.entries.push_back(EventEntry { tick, kind });
        self.total_pushed += 1;
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
    fn push_adds_entry() {
        let mut log = EventLog::default();
        log.push(1, EventKind::FoodLevel {
            current: 10.0,
            capacity: 50.0,
            fraction: 0.2,
        });
        assert_eq!(log.entries.len(), 1);
        assert_eq!(log.total_pushed, 1);
    }

    #[test]
    fn push_trims_to_capacity() {
        let mut log = EventLog::default();
        log.capacity = 3;
        for i in 0..5u64 {
            log.push(i, EventKind::FoodLevel {
                current: i as f32,
                capacity: 50.0,
                fraction: i as f32 / 50.0,
            });
        }
        assert_eq!(log.entries.len(), 3);
        assert_eq!(log.entries[0].tick, 2);
        assert_eq!(log.total_pushed, 5);
    }
}
