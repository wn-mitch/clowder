use std::path::Path;

use bevy_ecs::prelude::Resource;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::ai::Action;
use crate::components::identity::{Gender, LifeStage};
use crate::components::personality::Personality;
use crate::components::physical::Needs;
use crate::resources::map::Terrain;
use crate::resources::narrative::NarrativeTier;
use crate::resources::time::{DayPhase, Season};
use crate::resources::weather::Weather;

// ---------------------------------------------------------------------------
// PersonalityAxis + bucket
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PersonalityAxis {
    // Core Drives
    Boldness,
    Sociability,
    Curiosity,
    Diligence,
    Warmth,
    Spirituality,
    Ambition,
    Patience,
    // Temperament
    Anxiety,
    Optimism,
    Temper,
    Stubbornness,
    Playfulness,
    // Values
    Loyalty,
    Tradition,
    Compassion,
    Pride,
    Independence,
}

impl PersonalityAxis {
    pub const ALL: [PersonalityAxis; 18] = [
        Self::Boldness,
        Self::Sociability,
        Self::Curiosity,
        Self::Diligence,
        Self::Warmth,
        Self::Spirituality,
        Self::Ambition,
        Self::Patience,
        Self::Anxiety,
        Self::Optimism,
        Self::Temper,
        Self::Stubbornness,
        Self::Playfulness,
        Self::Loyalty,
        Self::Tradition,
        Self::Compassion,
        Self::Pride,
        Self::Independence,
    ];

    /// Human-readable label for display (e.g. in the prompt generator).
    pub fn label(self) -> &'static str {
        match self {
            Self::Boldness => "Boldness",
            Self::Sociability => "Sociability",
            Self::Curiosity => "Curiosity",
            Self::Diligence => "Diligence",
            Self::Warmth => "Warmth",
            Self::Spirituality => "Spirituality",
            Self::Ambition => "Ambition",
            Self::Patience => "Patience",
            Self::Anxiety => "Anxiety",
            Self::Optimism => "Optimism",
            Self::Temper => "Temper",
            Self::Stubbornness => "Stubbornness",
            Self::Playfulness => "Playfulness",
            Self::Loyalty => "Loyalty",
            Self::Tradition => "Tradition",
            Self::Compassion => "Compassion",
            Self::Pride => "Pride",
            Self::Independence => "Independence",
        }
    }

    /// Low-end descriptor for display (e.g. "cautious" for Boldness Low).
    pub fn low_label(self) -> &'static str {
        match self {
            Self::Boldness => "cautious",
            Self::Sociability => "solitary",
            Self::Curiosity => "routine",
            Self::Diligence => "lazy",
            Self::Warmth => "aloof",
            Self::Spirituality => "pragmatic",
            Self::Ambition => "content",
            Self::Patience => "impulsive",
            Self::Anxiety => "serene",
            Self::Optimism => "melancholic",
            Self::Temper => "even-keeled",
            Self::Stubbornness => "flexible",
            Self::Playfulness => "serious",
            Self::Loyalty => "self-interested",
            Self::Tradition => "iconoclast",
            Self::Compassion => "detached",
            Self::Pride => "humble",
            Self::Independence => "communal",
        }
    }

    /// High-end descriptor for display (e.g. "bold" for Boldness High).
    pub fn high_label(self) -> &'static str {
        match self {
            Self::Boldness => "bold",
            Self::Sociability => "gregarious",
            Self::Curiosity => "adventurous",
            Self::Diligence => "industrious",
            Self::Warmth => "affectionate",
            Self::Spirituality => "mystical",
            Self::Ambition => "ambitious",
            Self::Patience => "deliberate",
            Self::Anxiety => "nervous",
            Self::Optimism => "cheerful",
            Self::Temper => "volatile",
            Self::Stubbornness => "headstrong",
            Self::Playfulness => "mischievous",
            Self::Loyalty => "devoted",
            Self::Tradition => "traditionalist",
            Self::Compassion => "empathetic",
            Self::Pride => "proud",
            Self::Independence => "self-reliant",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PersonalityBucket {
    Low,
    Mid,
    High,
}

impl PersonalityBucket {
    pub fn from_value(v: f32) -> Self {
        if v < 0.33 {
            Self::Low
        } else if v < 0.67 {
            Self::Mid
        } else {
            Self::High
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Mid => "Mid",
            Self::High => "High",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityReq {
    pub axis: PersonalityAxis,
    pub bucket: PersonalityBucket,
}

// ---------------------------------------------------------------------------
// NeedAxis + level
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NeedAxis {
    Hunger,
    Energy,
    Warmth,
    Safety,
    Social,
    Acceptance,
    Respect,
    Mastery,
    Purpose,
}

impl NeedAxis {
    pub const ALL: [NeedAxis; 9] = [
        Self::Hunger,
        Self::Energy,
        Self::Warmth,
        Self::Safety,
        Self::Social,
        Self::Acceptance,
        Self::Respect,
        Self::Mastery,
        Self::Purpose,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hunger => "Hunger",
            Self::Energy => "Energy",
            Self::Warmth => "Warmth",
            Self::Safety => "Safety",
            Self::Social => "Social",
            Self::Acceptance => "Acceptance",
            Self::Respect => "Respect",
            Self::Mastery => "Mastery",
            Self::Purpose => "Purpose",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NeedLevel {
    Critical,
    Low,
    Moderate,
    Satisfied,
}

impl NeedLevel {
    pub fn from_value(v: f32) -> Self {
        if v < 0.2 {
            Self::Critical
        } else if v < 0.4 {
            Self::Low
        } else if v < 0.7 {
            Self::Moderate
        } else {
            Self::Satisfied
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Critical => "Critical",
            Self::Low => "Low",
            Self::Moderate => "Moderate",
            Self::Satisfied => "Satisfied",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeedReq {
    pub axis: NeedAxis,
    pub level: NeedLevel,
}

// ---------------------------------------------------------------------------
// MoodBucket
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoodBucket {
    Miserable,
    Low,
    Neutral,
    Happy,
    Euphoric,
}

impl MoodBucket {
    pub fn from_valence(v: f32) -> Self {
        if v < -0.3 {
            Self::Miserable
        } else if v < 0.0 {
            Self::Low
        } else if v < 0.3 {
            Self::Neutral
        } else if v < 0.7 {
            Self::Happy
        } else {
            Self::Euphoric
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Miserable => "Miserable",
            Self::Low => "Low",
            Self::Neutral => "Neutral",
            Self::Happy => "Happy",
            Self::Euphoric => "Euphoric",
        }
    }
}

// ---------------------------------------------------------------------------
// NarrativeTemplate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeTemplate {
    pub text: String,
    pub tier: NarrativeTier,
    #[serde(default = "default_weight")]
    pub weight: f32,

    // Condition fields — None / empty means "matches any"
    #[serde(default)]
    pub action: Option<Action>,
    #[serde(default)]
    pub day_phase: Option<DayPhase>,
    #[serde(default)]
    pub season: Option<Season>,
    #[serde(default)]
    pub weather: Option<Weather>,
    #[serde(default)]
    pub mood: Option<MoodBucket>,
    #[serde(default)]
    pub personality: Vec<PersonalityReq>,
    #[serde(default)]
    pub needs: Vec<NeedReq>,
    #[serde(default)]
    pub life_stage: Option<LifeStage>,
    /// Whether the action targets another entity (e.g. groom-other vs self-groom).
    #[serde(default)]
    pub has_target: Option<bool>,
    /// Terrain the cat is standing on (e.g. Forest, Grass, Water).
    #[serde(default)]
    pub terrain: Option<Terrain>,
}

fn default_weight() -> f32 {
    1.0
}

// ---------------------------------------------------------------------------
// TemplateContext — snapshot of current state for matching
// ---------------------------------------------------------------------------

pub struct TemplateContext {
    pub action: Action,
    pub day_phase: DayPhase,
    pub season: Season,
    pub weather: Weather,
    pub mood_bucket: MoodBucket,
    pub life_stage: LifeStage,
    pub has_target: bool,
    pub terrain: Terrain,
}

// ---------------------------------------------------------------------------
// Matching + selection
// ---------------------------------------------------------------------------

impl NarrativeTemplate {
    /// Returns true if all non-None conditions match the current context.
    pub fn matches(
        &self,
        ctx: &TemplateContext,
        personality: &Personality,
        needs: &Needs,
    ) -> bool {
        if let Some(a) = self.action {
            if a != ctx.action {
                return false;
            }
        }
        if let Some(dp) = self.day_phase {
            if dp != ctx.day_phase {
                return false;
            }
        }
        if let Some(s) = self.season {
            if s != ctx.season {
                return false;
            }
        }
        if let Some(w) = self.weather {
            if w != ctx.weather {
                return false;
            }
        }
        if let Some(m) = self.mood {
            if m != ctx.mood_bucket {
                return false;
            }
        }
        if let Some(ls) = self.life_stage {
            if ls != ctx.life_stage {
                return false;
            }
        }
        if let Some(ht) = self.has_target {
            if ht != ctx.has_target {
                return false;
            }
        }
        if let Some(t) = self.terrain {
            if t != ctx.terrain {
                return false;
            }
        }

        for req in &self.personality {
            let value = personality.get_axis(req.axis);
            if PersonalityBucket::from_value(value) != req.bucket {
                return false;
            }
        }

        for req in &self.needs {
            let value = needs.get_axis(req.axis);
            if NeedLevel::from_value(value) != req.level {
                return false;
            }
        }

        true
    }

    /// Count of non-None / non-empty condition fields. More specific templates
    /// are preferred during selection.
    pub fn specificity(&self) -> u32 {
        let mut count = 0u32;
        if self.action.is_some() {
            count += 1;
        }
        if self.day_phase.is_some() {
            count += 1;
        }
        if self.season.is_some() {
            count += 1;
        }
        if self.weather.is_some() {
            count += 1;
        }
        if self.mood.is_some() {
            count += 1;
        }
        if self.life_stage.is_some() {
            count += 1;
        }
        if self.has_target.is_some() {
            count += 1;
        }
        if self.terrain.is_some() {
            count += 1;
        }
        count += self.personality.len() as u32;
        count += self.needs.len() as u32;
        count
    }
}

// ---------------------------------------------------------------------------
// Variable resolution
// ---------------------------------------------------------------------------

/// Context for resolving template variables.
pub struct VariableContext<'a> {
    pub name: &'a str,
    pub gender: Gender,
    pub weather: Weather,
    pub day_phase: DayPhase,
    pub season: Season,
    pub life_stage: LifeStage,
    pub other: Option<&'a str>,
    pub fur_color: &'a str,
    /// Prey species name (e.g. "rat", "fish") — set by action outcomes.
    pub prey: Option<&'a str>,
    /// Item name (e.g. "berries", "mushrooms") — set by action outcomes.
    pub item: Option<&'a str>,
    /// Quality tier label (e.g. "fine", "exceptional") — only set for notable quality.
    pub quality: Option<&'a str>,
}

pub fn resolve_variables(template_text: &str, ctx: &VariableContext<'_>) -> String {
    let subject = ctx.gender.subject_pronoun();
    let cap_subject = capitalize(subject);

    template_text
        .replace("{name}", ctx.name)
        .replace("{Subject}", &cap_subject)
        .replace("{subject}", subject)
        .replace("{object}", ctx.gender.object_pronoun())
        .replace("{possessive}", ctx.gender.possessive())
        .replace("{weather_desc}", ctx.weather.label())
        .replace("{time_desc}", ctx.day_phase.label())
        .replace("{season}", ctx.season.label())
        .replace("{life_stage}", life_stage_label(ctx.life_stage))
        .replace("{fur_color}", ctx.fur_color)
        .replace("{other}", ctx.other.unwrap_or("a companion"))
        .replace("{prey}", ctx.prey.unwrap_or("prey"))
        .replace("{item}", ctx.item.unwrap_or("something"))
        .replace("{quality}", ctx.quality.unwrap_or(""))
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().to_string() + c.as_str(),
    }
}

fn life_stage_label(ls: LifeStage) -> &'static str {
    match ls {
        LifeStage::Kitten => "kitten",
        LifeStage::Young => "young cat",
        LifeStage::Adult => "cat",
        LifeStage::Elder => "elder",
    }
}

// ---------------------------------------------------------------------------
// TemplateRegistry resource
// ---------------------------------------------------------------------------

#[derive(Resource, Debug)]
pub struct TemplateRegistry {
    templates: Vec<NarrativeTemplate>,
}

impl TemplateRegistry {
    /// Load all `.ron` files from a directory and parse them as template lists.
    pub fn load_from_dir(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut templates = Vec::new();
        let mut entries: Vec<_> = std::fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "ron")
            })
            .collect();
        // Sort by filename for deterministic load order.
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let contents = std::fs::read_to_string(entry.path())?;
            let file_templates: Vec<NarrativeTemplate> = ron::from_str(&contents)?;
            templates.extend(file_templates);
        }
        Ok(Self { templates })
    }

    /// Create a registry from an explicit list. Used in tests.
    pub fn from_templates(templates: Vec<NarrativeTemplate>) -> Self {
        Self { templates }
    }

    /// How many templates are loaded.
    pub fn len(&self) -> usize {
        self.templates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    /// Select a template matching the given context using weighted randomness.
    ///
    /// More specific templates (more conditions set) are preferred. Returns
    /// `None` if no templates match.
    pub fn select<R: Rng>(
        &self,
        ctx: &TemplateContext,
        personality: &Personality,
        needs: &Needs,
        rng: &mut R,
    ) -> Option<&NarrativeTemplate> {
        let candidates: Vec<(usize, f32)> = self
            .templates
            .iter()
            .enumerate()
            .filter(|(_, t)| t.matches(ctx, personality, needs))
            .map(|(i, t)| {
                let score = (t.specificity().max(1) as f32) * t.weight;
                (i, score)
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        let total: f32 = candidates.iter().map(|(_, w)| w).sum();
        let mut roll: f32 = rng.random_range(0.0..total);
        for (idx, weight) in &candidates {
            if roll < *weight {
                return Some(&self.templates[*idx]);
            }
            roll -= weight;
        }
        // Fallback for float rounding — return last candidate.
        candidates.last().map(|(idx, _)| &self.templates[*idx])
    }
}

// ---------------------------------------------------------------------------
// Helper methods on existing types
// ---------------------------------------------------------------------------

impl Personality {
    pub fn get_axis(&self, axis: PersonalityAxis) -> f32 {
        match axis {
            PersonalityAxis::Boldness => self.boldness,
            PersonalityAxis::Sociability => self.sociability,
            PersonalityAxis::Curiosity => self.curiosity,
            PersonalityAxis::Diligence => self.diligence,
            PersonalityAxis::Warmth => self.warmth,
            PersonalityAxis::Spirituality => self.spirituality,
            PersonalityAxis::Ambition => self.ambition,
            PersonalityAxis::Patience => self.patience,
            PersonalityAxis::Anxiety => self.anxiety,
            PersonalityAxis::Optimism => self.optimism,
            PersonalityAxis::Temper => self.temper,
            PersonalityAxis::Stubbornness => self.stubbornness,
            PersonalityAxis::Playfulness => self.playfulness,
            PersonalityAxis::Loyalty => self.loyalty,
            PersonalityAxis::Tradition => self.tradition,
            PersonalityAxis::Compassion => self.compassion,
            PersonalityAxis::Pride => self.pride,
            PersonalityAxis::Independence => self.independence,
        }
    }
}

impl Needs {
    pub fn get_axis(&self, axis: NeedAxis) -> f32 {
        match axis {
            NeedAxis::Hunger => self.hunger,
            NeedAxis::Energy => self.energy,
            NeedAxis::Warmth => self.warmth,
            NeedAxis::Safety => self.safety,
            NeedAxis::Social => self.social,
            NeedAxis::Acceptance => self.acceptance,
            NeedAxis::Respect => self.respect,
            NeedAxis::Mastery => self.mastery,
            NeedAxis::Purpose => self.purpose,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Bucket boundaries ---

    #[test]
    fn personality_bucket_boundaries() {
        assert_eq!(PersonalityBucket::from_value(0.0), PersonalityBucket::Low);
        assert_eq!(PersonalityBucket::from_value(0.32), PersonalityBucket::Low);
        assert_eq!(PersonalityBucket::from_value(0.33), PersonalityBucket::Mid);
        assert_eq!(PersonalityBucket::from_value(0.5), PersonalityBucket::Mid);
        assert_eq!(PersonalityBucket::from_value(0.66), PersonalityBucket::Mid);
        assert_eq!(PersonalityBucket::from_value(0.67), PersonalityBucket::High);
        assert_eq!(PersonalityBucket::from_value(1.0), PersonalityBucket::High);
    }

    #[test]
    fn need_level_boundaries() {
        assert_eq!(NeedLevel::from_value(0.0), NeedLevel::Critical);
        assert_eq!(NeedLevel::from_value(0.19), NeedLevel::Critical);
        assert_eq!(NeedLevel::from_value(0.2), NeedLevel::Low);
        assert_eq!(NeedLevel::from_value(0.39), NeedLevel::Low);
        assert_eq!(NeedLevel::from_value(0.4), NeedLevel::Moderate);
        assert_eq!(NeedLevel::from_value(0.69), NeedLevel::Moderate);
        assert_eq!(NeedLevel::from_value(0.7), NeedLevel::Satisfied);
        assert_eq!(NeedLevel::from_value(1.0), NeedLevel::Satisfied);
    }

    #[test]
    fn mood_bucket_boundaries() {
        assert_eq!(MoodBucket::from_valence(-1.0), MoodBucket::Miserable);
        assert_eq!(MoodBucket::from_valence(-0.31), MoodBucket::Miserable);
        assert_eq!(MoodBucket::from_valence(-0.3), MoodBucket::Low);
        assert_eq!(MoodBucket::from_valence(-0.01), MoodBucket::Low);
        assert_eq!(MoodBucket::from_valence(0.0), MoodBucket::Neutral);
        assert_eq!(MoodBucket::from_valence(0.29), MoodBucket::Neutral);
        assert_eq!(MoodBucket::from_valence(0.3), MoodBucket::Happy);
        assert_eq!(MoodBucket::from_valence(0.69), MoodBucket::Happy);
        assert_eq!(MoodBucket::from_valence(0.7), MoodBucket::Euphoric);
        assert_eq!(MoodBucket::from_valence(1.0), MoodBucket::Euphoric);
    }

    // --- get_axis helpers ---

    #[test]
    fn personality_get_axis_round_trips() {
        use rand_chacha::ChaCha8Rng;
        use rand_chacha::rand_core::SeedableRng;

        let p = Personality::random(&mut ChaCha8Rng::seed_from_u64(42));
        for axis in PersonalityAxis::ALL {
            let value = p.get_axis(axis);
            assert!((0.0..=1.0).contains(&value), "{axis:?} out of range: {value}");
        }
        // Spot check one axis
        assert_eq!(p.get_axis(PersonalityAxis::Boldness), p.boldness);
    }

    #[test]
    fn needs_get_axis_round_trips() {
        let n = Needs::default();
        assert_eq!(n.get_axis(NeedAxis::Hunger), n.hunger);
        assert_eq!(n.get_axis(NeedAxis::Energy), n.energy);
        assert_eq!(n.get_axis(NeedAxis::Purpose), n.purpose);
    }

    // --- Template matching ---

    fn generic_eat_template() -> NarrativeTemplate {
        NarrativeTemplate {
            text: "{name} eats from the stores.".to_string(),
            tier: NarrativeTier::Action,
            weight: 1.0,
            action: Some(Action::Eat),
            day_phase: None,
            season: None,
            weather: None,
            mood: None,
            personality: vec![],
            needs: vec![],
            life_stage: None,
            has_target: None,
            terrain: None,
        }
    }

    fn specific_eat_template() -> NarrativeTemplate {
        NarrativeTemplate {
            text: "{name} wolfs down {possessive} food.".to_string(),
            tier: NarrativeTier::Action,
            weight: 1.0,
            action: Some(Action::Eat),
            day_phase: None,
            season: None,
            weather: None,
            mood: None,
            personality: vec![PersonalityReq {
                axis: PersonalityAxis::Boldness,
                bucket: PersonalityBucket::High,
            }],
            needs: vec![NeedReq {
                axis: NeedAxis::Hunger,
                level: NeedLevel::Critical,
            }],
            life_stage: None,
            has_target: None,
            terrain: None,
        }
    }

    fn default_personality() -> Personality {
        use rand_chacha::ChaCha8Rng;
        use rand_chacha::rand_core::SeedableRng;
        Personality::random(&mut ChaCha8Rng::seed_from_u64(0))
    }

    fn make_context(action: Action) -> TemplateContext {
        TemplateContext {
            action,
            day_phase: DayPhase::Day,
            season: Season::Summer,
            weather: Weather::Clear,
            mood_bucket: MoodBucket::Neutral,
            life_stage: LifeStage::Adult,
            has_target: false,
            terrain: Terrain::Grass,
        }
    }

    #[test]
    fn generic_template_matches_any_eat() {
        let t = generic_eat_template();
        let ctx = make_context(Action::Eat);
        let p = default_personality();
        let n = Needs::default();
        assert!(t.matches(&ctx, &p, &n));
    }

    #[test]
    fn generic_template_rejects_wrong_action() {
        let t = generic_eat_template();
        let ctx = make_context(Action::Sleep);
        let p = default_personality();
        let n = Needs::default();
        assert!(!t.matches(&ctx, &p, &n));
    }

    #[test]
    fn specific_template_rejects_wrong_personality() {
        let t = specific_eat_template();
        let ctx = make_context(Action::Eat);
        // Personality with low boldness
        let mut p = default_personality();
        p.boldness = 0.1;
        let mut n = Needs::default();
        n.hunger = 0.1; // Critical
        assert!(!t.matches(&ctx, &p, &n));
    }

    #[test]
    fn specific_template_matches_correct_state() {
        let t = specific_eat_template();
        let ctx = make_context(Action::Eat);
        let mut p = default_personality();
        p.boldness = 0.9; // High
        let mut n = Needs::default();
        n.hunger = 0.1; // Critical
        assert!(t.matches(&ctx, &p, &n));
    }

    #[test]
    fn specificity_counts_conditions() {
        assert_eq!(generic_eat_template().specificity(), 1); // just action
        assert_eq!(specific_eat_template().specificity(), 3); // action + personality + need
    }

    // --- Selection ---

    #[test]
    fn select_from_empty_registry_returns_none() {
        use rand_chacha::ChaCha8Rng;
        use rand_chacha::rand_core::SeedableRng;

        let reg = TemplateRegistry::from_templates(vec![]);
        let ctx = make_context(Action::Eat);
        let p = default_personality();
        let n = Needs::default();
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        assert!(reg.select(&ctx, &p, &n, &mut rng).is_none());
    }

    #[test]
    fn select_prefers_specific_templates() {
        use rand_chacha::ChaCha8Rng;
        use rand_chacha::rand_core::SeedableRng;

        let generic = generic_eat_template();
        let specific = specific_eat_template();
        let reg = TemplateRegistry::from_templates(vec![generic, specific]);

        // Set up state that matches the specific template
        let ctx = make_context(Action::Eat);
        let mut p = default_personality();
        p.boldness = 0.9;
        let mut n = Needs::default();
        n.hunger = 0.1;
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        // Over many samples, the specific template should appear more often
        let mut specific_count = 0;
        for _ in 0..100 {
            if let Some(t) = reg.select(&ctx, &p, &n, &mut rng) {
                if t.text.contains("wolfs") {
                    specific_count += 1;
                }
            }
        }
        // Specific has weight 3 (specificity 3 * weight 1.0) vs generic weight 1.
        // Expected ~75% specific.
        assert!(
            specific_count > 50,
            "specific template should be selected more often, got {specific_count}/100"
        );
    }

    // --- Variable resolution ---

    #[test]
    fn resolve_all_variables() {
        let text = "{name} ({subject}/{object}/{possessive}) {Subject} {weather_desc} {time_desc} {season} {fur_color} {life_stage}";
        let ctx = VariableContext {
            name: "Bramble",
            gender: Gender::Queen,
            weather: Weather::Snow,
            day_phase: DayPhase::Dusk,
            season: Season::Winter,
            life_stage: LifeStage::Elder,
            fur_color: "tortoiseshell",
            other: None,
            prey: None,
            item: None,
            quality: None,
        };
        let result = resolve_variables(text, &ctx);
        assert_eq!(
            result,
            "Bramble (she/her/her) She Snow Dusk Winter tortoiseshell elder"
        );
    }

    #[test]
    fn resolve_nonbinary_pronouns() {
        let text = "{Subject} curls up. {name} wraps {possessive} tail around {object}self.";
        let ctx = VariableContext {
            name: "Fern",
            gender: Gender::Nonbinary,
            weather: Weather::Clear,
            day_phase: DayPhase::Night,
            season: Season::Spring,
            life_stage: LifeStage::Adult,
            fur_color: "grey",
            other: None,
            prey: None,
            item: None,
            quality: None,
        };
        let result = resolve_variables(text, &ctx);
        assert_eq!(
            result,
            "They curls up. Fern wraps their tail around themself."
        );
    }

    #[test]
    fn unknown_variables_left_as_is() {
        let text = "{name} gives {unknown_var} a fish.";
        let ctx = VariableContext {
            name: "Moss",
            gender: Gender::Tom,
            weather: Weather::Clear,
            day_phase: DayPhase::Day,
            season: Season::Summer,
            life_stage: LifeStage::Adult,
            fur_color: "black",
            other: None,
            prey: None,
            item: None,
            quality: None,
        };
        let result = resolve_variables(text, &ctx);
        assert_eq!(result, "Moss gives {unknown_var} a fish.");
    }

    #[test]
    fn other_variable_resolves_when_present() {
        let text = "{name} sits beside {other}.";
        let ctx = VariableContext {
            name: "Fern",
            gender: Gender::Queen,
            weather: Weather::Clear,
            day_phase: DayPhase::Day,
            season: Season::Summer,
            life_stage: LifeStage::Adult,
            fur_color: "grey",
            other: Some("Reed"),
            prey: None,
            item: None,
            quality: None,
        };
        let result = resolve_variables(text, &ctx);
        assert_eq!(result, "Fern sits beside Reed.");
    }

    #[test]
    fn other_variable_empty_when_none() {
        let text = "{name} grooms carefully.";
        let ctx = VariableContext {
            name: "Bramble",
            gender: Gender::Tom,
            weather: Weather::Clear,
            day_phase: DayPhase::Day,
            season: Season::Summer,
            life_stage: LifeStage::Adult,
            fur_color: "ginger",
            other: None,
            prey: None,
            item: None,
            quality: None,
        };
        let result = resolve_variables(text, &ctx);
        assert_eq!(result, "Bramble grooms carefully.");
    }

    // --- RON round-trip ---

    #[test]
    fn template_ron_round_trip() {
        let t = specific_eat_template();
        let ron_str = ron::to_string(&t).expect("serialize");
        let parsed: NarrativeTemplate = ron::from_str(&ron_str).expect("deserialize");
        assert_eq!(parsed.text, t.text);
        assert_eq!(parsed.action, t.action);
        assert_eq!(parsed.personality.len(), 1);
        assert_eq!(parsed.needs.len(), 1);
    }

    #[test]
    fn template_list_ron_round_trip() {
        let templates = vec![generic_eat_template(), specific_eat_template()];
        let ron_str = ron::to_string(&templates).expect("serialize");
        let parsed: Vec<NarrativeTemplate> = ron::from_str(&ron_str).expect("deserialize");
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn load_asset_templates() {
        let path = std::path::Path::new("assets/narrative");
        let reg = TemplateRegistry::load_from_dir(path).expect("should load RON files");
        // 4 files × ~8 templates each = ~32 total
        assert!(
            reg.len() >= 30,
            "expected at least 30 templates from assets, got {}",
            reg.len()
        );
    }
}
