use rand::Rng;

use crate::ai::Action;
use crate::components::personality::Personality;
use crate::components::physical::Needs;

// ---------------------------------------------------------------------------
// Jitter
// ---------------------------------------------------------------------------

/// Small random noise added to every score to break ties and add variety.
fn jitter(rng: &mut impl Rng) -> f32 {
    rng.random_range(-0.05f32..0.05f32)
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// Score all available actions for a cat given its current state.
///
/// Returns a `Vec` of `(Action, score)` pairs. Higher score = more preferred.
/// The caller should pass the result to [`select_best_action`].
///
/// # Parameters
/// - `needs` — the cat's current Maslow needs
/// - `personality` — 18-axis personality (uses `curiosity` for Wander)
/// - `food_available` — whether food is reachable right now
/// - `rng` — for jitter
pub fn score_actions(
    needs: &Needs,
    personality: &Personality,
    food_available: bool,
    rng: &mut impl Rng,
) -> Vec<(Action, f32)> {
    let mut scores = Vec::with_capacity(4);

    // --- Eat (only when food is present) ---
    if food_available {
        // Urgency rises as hunger falls (lower hunger = more urgent)
        let urgency = (1.0 - needs.hunger) * 2.0 * needs.level_suppression(1);
        scores.push((Action::Eat, urgency + jitter(rng)));
    }

    // --- Sleep ---
    {
        let urgency = (1.0 - needs.energy) * 2.0 * needs.level_suppression(1);
        scores.push((Action::Sleep, urgency + jitter(rng)));
    }

    // --- Wander (curiosity-driven; suppressed by unmet lower needs) ---
    {
        let score = personality.curiosity * 0.5 * needs.level_suppression(5);
        scores.push((Action::Wander, score + jitter(rng)));
    }

    // --- Idle (always-available fallback) ---
    scores.push((Action::Idle, 0.1 + jitter(rng)));

    scores
}

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

/// Pick the action with the highest score. Falls back to [`Action::Idle`] if
/// the slice is empty or all scores are non-finite.
pub fn select_best_action(scores: &[(Action, f32)]) -> Action {
    scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(action, _)| *action)
        .unwrap_or(Action::Idle)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha8Rng;
    use rand_chacha::rand_core::SeedableRng;

    fn seeded_rng(seed: u64) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(seed)
    }

    fn default_personality() -> Personality {
        Personality {
            boldness: 0.5,
            sociability: 0.5,
            curiosity: 0.5,
            diligence: 0.5,
            warmth: 0.5,
            spirituality: 0.5,
            ambition: 0.5,
            patience: 0.5,
            anxiety: 0.5,
            optimism: 0.5,
            temper: 0.5,
            stubbornness: 0.5,
            playfulness: 0.5,
            loyalty: 0.5,
            tradition: 0.5,
            compassion: 0.5,
            pride: 0.5,
            independence: 0.5,
        }
    }

    /// Starving cat (hunger=0.1, energy=0.8) with food available should score Eat highest.
    #[test]
    fn starving_cat_scores_eat_highest() {
        let mut needs = Needs::default();
        needs.hunger = 0.1;
        needs.energy = 0.8;

        let personality = default_personality();
        let mut rng = seeded_rng(1);

        let scores = score_actions(&needs, &personality, true, &mut rng);
        let best = select_best_action(&scores);

        assert_eq!(
            best,
            Action::Eat,
            "starving cat should choose Eat; scores: {scores:?}"
        );

        // Confirm Eat is also strictly above Sleep
        let eat_score = scores.iter().find(|(a, _)| *a == Action::Eat).unwrap().1;
        let sleep_score = scores.iter().find(|(a, _)| *a == Action::Sleep).unwrap().1;
        assert!(
            eat_score > sleep_score,
            "Eat ({eat_score}) should beat Sleep ({sleep_score}) for a starving cat"
        );
    }

    /// Exhausted cat (energy=0.1, hunger=0.8) should score Sleep highest.
    #[test]
    fn exhausted_cat_scores_sleep_highest() {
        let mut needs = Needs::default();
        needs.energy = 0.1;
        needs.hunger = 0.8;

        let personality = default_personality();
        let mut rng = seeded_rng(2);

        let scores = score_actions(&needs, &personality, true, &mut rng);
        let best = select_best_action(&scores);

        assert_eq!(
            best,
            Action::Sleep,
            "exhausted cat should choose Sleep; scores: {scores:?}"
        );

        let sleep_score = scores.iter().find(|(a, _)| *a == Action::Sleep).unwrap().1;
        let eat_score = scores.iter().find(|(a, _)| *a == Action::Eat).unwrap().1;
        assert!(
            sleep_score > eat_score,
            "Sleep ({sleep_score}) should beat Eat ({eat_score}) for an exhausted cat"
        );
    }

    /// Satisfied curious cat (all needs high, high curiosity) with no food available should
    /// not pick Eat or Sleep — Wander or Idle should win.
    #[test]
    fn satisfied_curious_cat_does_not_eat_or_sleep() {
        let mut needs = Needs::default();
        // All needs well-met
        needs.hunger = 0.95;
        needs.energy = 0.95;
        needs.warmth = 0.95;
        needs.safety = 0.95;
        needs.social = 0.95;
        needs.acceptance = 0.95;
        needs.respect = 0.95;
        needs.mastery = 0.95;
        needs.purpose = 0.95;

        let mut personality = default_personality();
        personality.curiosity = 0.9; // highly curious

        let mut rng = seeded_rng(3);

        // No food available, so Eat won't be in the list
        let scores = score_actions(&needs, &personality, false, &mut rng);
        let best = select_best_action(&scores);

        assert!(
            best == Action::Wander || best == Action::Idle,
            "satisfied cat should wander or idle, got {best:?}; scores: {scores:?}"
        );
        assert_ne!(best, Action::Eat, "no food available, Eat should not win");
        assert_ne!(
            best,
            Action::Sleep,
            "well-rested cat should not sleep; scores: {scores:?}"
        );
    }
}
