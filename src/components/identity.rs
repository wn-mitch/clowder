use bevy_ecs::prelude::*;

// ---------------------------------------------------------------------------
// Name
// ---------------------------------------------------------------------------

/// The cat's name.
#[derive(Component, Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Name(pub String);

// ---------------------------------------------------------------------------
// Species
// ---------------------------------------------------------------------------

/// Marker: this entity is a cat.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Species;

// ---------------------------------------------------------------------------
// Age
// ---------------------------------------------------------------------------

/// Birth tick. Convert to a life stage using [`Age::stage`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Age {
    pub born_tick: u64,
}

/// Broad life stage derived from age.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LifeStage {
    Kitten,
    Young,
    Adult,
    Elder,
}

impl Age {
    pub fn new(born_tick: u64) -> Self {
        Self { born_tick }
    }

    /// Derive the life stage from the current simulation tick and tick-rate.
    ///
    /// Season count is floored, so a cat born at tick 0 and queried at tick 0
    /// has lived 0 seasons → Kitten.
    ///
    /// The Adult window extends through season 59 (Phase 4.3 retune).
    /// Prior to 2026-04-22 the cap was 47: combined with §7.M.7.1's
    /// `Fertility` removal on Adult→Elder, a 15-min `--duration 900`
    /// soak produced zero `MatingOccurred` because every bonded pair
    /// aged into Elder before bonds matured. Extending Adult to 60
    /// seasons keeps bonded pairs fertile for the duration of a
    /// standard deep-soak window. Paired with
    /// `DeathConstants::elder_entry_seasons: 48 → 60` so old-age
    /// mortality opens at the new Elder boundary rather than inside
    /// the widened Adult range.
    pub fn stage(&self, current_tick: u64, ticks_per_season: u64) -> LifeStage {
        let age_ticks = current_tick.saturating_sub(self.born_tick);
        let seasons = age_ticks / ticks_per_season;
        match seasons {
            0..=3 => LifeStage::Kitten,
            4..=11 => LifeStage::Young,
            12..=59 => LifeStage::Adult,
            _ => LifeStage::Elder,
        }
    }
}

// ---------------------------------------------------------------------------
// Gender
// ---------------------------------------------------------------------------

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Gender {
    Tom,
    Queen,
    Nonbinary,
}

impl Gender {
    /// Whether this gender can carry a pregnancy. Queens and
    /// Nonbinaries gestate; Toms don't. Used by §7.M.7.4's
    /// `resolve_mate_with` partner-selection fix — the mating act
    /// now inserts `Pregnant` on the gestation-capable partner rather
    /// than the initiator, matching feline reproductive anatomy.
    pub fn can_gestate(self) -> bool {
        matches!(self, Self::Queen | Self::Nonbinary)
    }

    pub fn subject_pronoun(self) -> &'static str {
        match self {
            Self::Tom => "he",
            Self::Queen => "she",
            Self::Nonbinary => "they",
        }
    }

    pub fn object_pronoun(self) -> &'static str {
        match self {
            Self::Tom => "him",
            Self::Queen => "her",
            Self::Nonbinary => "them",
        }
    }

    pub fn possessive(self) -> &'static str {
        match self {
            Self::Tom => "his",
            Self::Queen => "her",
            Self::Nonbinary => "their",
        }
    }
}

// ---------------------------------------------------------------------------
// Orientation
// ---------------------------------------------------------------------------

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Orientation {
    Straight,
    Gay,
    Bisexual,
    Asexual,
}

// ---------------------------------------------------------------------------
// Appearance
// ---------------------------------------------------------------------------

#[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Appearance {
    pub fur_color: String,
    pub pattern: String,
    pub eye_color: String,
    pub distinguishing_marks: Vec<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::time::TEST_TICKS_PER_SEASON as TICKS_PER_SEASON;

    fn stage_at_seasons(born_tick: u64, seasons: u64) -> LifeStage {
        let current_tick = born_tick + seasons * TICKS_PER_SEASON;
        Age::new(born_tick).stage(current_tick, TICKS_PER_SEASON)
    }

    #[test]
    fn age_stages_at_boundaries() {
        // Kitten: 0–3 seasons
        assert_eq!(stage_at_seasons(0, 0), LifeStage::Kitten);
        assert_eq!(stage_at_seasons(0, 3), LifeStage::Kitten);

        // Young: 4–11 seasons
        assert_eq!(stage_at_seasons(0, 4), LifeStage::Young);
        assert_eq!(stage_at_seasons(0, 11), LifeStage::Young);

        // Adult: 12–59 seasons (Phase 4.3 window extension)
        assert_eq!(stage_at_seasons(0, 12), LifeStage::Adult);
        assert_eq!(stage_at_seasons(0, 47), LifeStage::Adult);
        assert_eq!(stage_at_seasons(0, 59), LifeStage::Adult);

        // Elder: 60+
        assert_eq!(stage_at_seasons(0, 60), LifeStage::Elder);
        assert_eq!(stage_at_seasons(0, 100), LifeStage::Elder);
    }

    #[test]
    fn age_stage_non_zero_birth_tick() {
        // Born at tick 500, should still compute stages relative to birth
        let born = 500;
        let age = Age::new(born);
        assert_eq!(age.stage(born, TICKS_PER_SEASON), LifeStage::Kitten);
        assert_eq!(
            age.stage(born + 4 * TICKS_PER_SEASON, TICKS_PER_SEASON),
            LifeStage::Young
        );
    }

    #[test]
    fn age_stage_before_birth_is_kitten() {
        // saturating_sub prevents underflow; a tick before birth = 0 seasons
        let age = Age::new(1000);
        assert_eq!(age.stage(0, TICKS_PER_SEASON), LifeStage::Kitten);
    }

    #[test]
    fn gender_pronouns() {
        assert_eq!(Gender::Tom.subject_pronoun(), "he");
        assert_eq!(Gender::Tom.object_pronoun(), "him");
        assert_eq!(Gender::Tom.possessive(), "his");

        assert_eq!(Gender::Queen.subject_pronoun(), "she");
        assert_eq!(Gender::Queen.object_pronoun(), "her");
        assert_eq!(Gender::Queen.possessive(), "her");

        assert_eq!(Gender::Nonbinary.subject_pronoun(), "they");
        assert_eq!(Gender::Nonbinary.object_pronoun(), "them");
        assert_eq!(Gender::Nonbinary.possessive(), "their");
    }
}
