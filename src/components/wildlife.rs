use bevy_ecs::prelude::*;

// ---------------------------------------------------------------------------
// Wildlife species and behavior
// ---------------------------------------------------------------------------

/// The species of a wild animal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum WildSpecies {
    Fox,
    Hawk,
    Snake,
    ShadowFox,
}

impl WildSpecies {
    /// Display name for narrative output.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fox => "fox",
            Self::Hawk => "hawk",
            Self::Snake => "snake",
            Self::ShadowFox => "shadow-fox",
        }
    }

    /// Single-character symbol for the TUI map.
    pub fn symbol(self) -> char {
        match self {
            Self::Fox => 'f',
            Self::Hawk => 'h',
            Self::Snake => 's',
            Self::ShadowFox => 'F',
        }
    }

    /// Default threat power for this species.
    pub fn default_threat_power(self) -> f32 {
        match self {
            Self::Fox => 0.15,
            Self::Hawk => 0.10,
            Self::Snake => 0.08,
            Self::ShadowFox => 0.18,
        }
    }

    /// Default defense value for this species.
    pub fn default_defense(self) -> f32 {
        match self {
            Self::Fox => 0.15,
            Self::Hawk => 0.05,
            Self::Snake => 0.10,
            // Shadow-foxes are spectral and fragile — they rely on
            // ambush-terror for their damage, not armor. A posse of cats
            // can meaningfully harm one without needing elite combat
            // training. See assets/narrative/banishment.ron.
            Self::ShadowFox => 0.08,
        }
    }

    /// Default behavior pattern for this species.
    pub fn default_behavior(self) -> BehaviorType {
        match self {
            Self::Fox => BehaviorType::Patrol,
            Self::Hawk => BehaviorType::Circle,
            Self::Snake => BehaviorType::Ambush,
            Self::ShadowFox => BehaviorType::Patrol,
        }
    }

    /// Maximum population cap for runtime spawning.
    pub fn population_cap(self) -> usize {
        match self {
            Self::Fox => 7,
            Self::Hawk => 5,
            Self::Snake => 5,
            Self::ShadowFox => 2,
        }
    }

    /// Per-tick spawn probability at map edges.
    pub fn spawn_chance(self) -> f32 {
        match self {
            Self::Fox => 0.003,
            Self::Hawk => 0.002,
            Self::Snake => 0.002,
            Self::ShadowFox => 0.0, // corruption-spawned only, not edge-spawned
        }
    }
}

/// How a wild animal moves and hunts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BehaviorType {
    /// Walk along terrain edges (fox).
    Patrol,
    /// Circle around a center point (hawk).
    Circle,
    /// Stay still, strike when prey is adjacent (snake).
    Ambush,
}

// ---------------------------------------------------------------------------
// WildAnimal component
// ---------------------------------------------------------------------------

/// Marks an entity as a wild animal with species-specific behavior.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WildAnimal {
    pub species: WildSpecies,
    pub behavior: BehaviorType,
    pub threat_power: f32,
    pub defense: f32,
    /// Ticks remaining before this animal can initiate a new stalk after ambushing.
    pub ambush_cooldown: u32,
}

impl WildAnimal {
    /// Create a new wild animal with species defaults.
    pub fn new(species: WildSpecies) -> Self {
        Self {
            species,
            behavior: species.default_behavior(),
            threat_power: species.default_threat_power(),
            defense: species.default_defense(),
            ambush_cooldown: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// WildlifeAiState — per-entity behavior state
// ---------------------------------------------------------------------------

/// Mutable AI state for wildlife movement decisions.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum WildlifeAiState {
    /// Patrol: current direction of travel along terrain edge.
    Patrolling { dx: i32, dy: i32 },
    /// Circle: center point and current angle (radians).
    Circling {
        center_x: i32,
        center_y: i32,
        angle: f32,
    },
    /// Ambush: stationary, waiting.
    Waiting,
    /// Fleeing toward map edge after losing a fight.
    Fleeing { dx: i32, dy: i32 },
    /// Stalking: moving toward a cat to ambush it.
    Stalking { target_x: i32, target_y: i32 },
    /// Encircling a ward — shadow fox deposits corruption to siege it.
    EncirclingWard {
        ward_x: i32,
        ward_y: i32,
        angle: f32,
        ticks: u64,
    },
    // ---- Ticket 023 Phase B: shadow-fox motivation states ----
    // These variants are gated by `Has<ShadowFoxDrives>` — only the
    // shadow-fox motivation tick writes them. `wildlife_ai`'s state
    // machine reads them via the same query but only because shadow-
    // foxes also flow through that loop; non-shadow-fox wildlife will
    // never carry these variants.
    /// Reconstituting: hold position on a high-corruption tile while
    /// `shadowfox_coherence_tick` recovers coherence at the
    /// `reconstituting_recovery_multiplier`. Picked when the
    /// Coherence drive dominates the motivation softmax.
    Reconstituting { tile_x: i32, tile_y: i32 },
    /// Tending: orbit a ward's perimeter laying down corruption.
    /// Distinct from `EncirclingWard` (which is the siege pattern that
    /// pre-Phase-B triggered from the patrol-step branch). Tending
    /// is driven by the Resonance drive — corruption is *losing
    /// ground* near the ward and the shadow-fox shores it up. The
    /// siege state can still emerge from `wildlife_ai`'s patrol
    /// branch when the fox stumbles into ward coverage.
    Tending {
        ward_x: i32,
        ward_y: i32,
        angle: f32,
    },
    /// Haunting: pace at the detection-edge distance around a target
    /// cat, applying psychological pressure without combat. The
    /// `ticks` counter is reset to 0 each time the motivation tick
    /// re-elects Haunting and incremented by
    /// `shadowfox_haunting_drain` while the state persists; once it
    /// crosses `shadow_fox_haunting_escalation_ticks` the haunt is
    /// promoted to Stalking (the existing pre-023 combat path).
    Haunting {
        target_x: i32,
        target_y: i32,
        edge_distance: i32,
        ticks: u64,
    },
    /// Seeding: move toward and extend the corruption frontier
    /// (boundary between corrupt and clean tiles), depositing at
    /// `seed_corruption_rate`. Picked when the Entropy drive
    /// dominates.
    Seeding {
        frontier_x: i32,
        frontier_y: i32,
    },
}

// ---------------------------------------------------------------------------
// ShadowFoxDrives — four-drive scored motivation substrate (ticket 023)
// ---------------------------------------------------------------------------

/// Per-shadowfox motivational pressures. Phase A wires only `coherence`
/// (self-preservation via corruption — decays on clean ground, recovers on
/// corrupted ground, dissolves at 0). Phase B uses all four to softmax-select
/// the next `WildlifeAiState` variant; Phase C deepens the targeting reads.
///
/// Doubles as a marker component: presence of `ShadowFoxDrives` is the
/// canonical "this entity is a shadow-fox" test (see ticket 023 plan §3).
/// Hawks, snakes, and normal foxes never carry this component.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShadowFoxDrives {
    /// 0.0 = dissolving, 1.0 = fully manifested. Decays on clean tiles,
    /// recovers on corrupted tiles. Reaching 0.0 despawns the shadow-fox
    /// and emits `EventKind::ShadowFoxDissolved`.
    pub coherence: f32,
    /// Pressure to defend corruption that is losing ground (near wards,
    /// recently cleansed). Unused in Phase A; populated in Phase B.
    pub resonance: f32,
    /// Pressure to terrorize psychologically vulnerable cats. Unused in
    /// Phase A; populated in Phase B (shallow) / Phase C (deep targeting).
    pub dread: f32,
    /// Pressure to extend the corruption frontier (probing ward gaps).
    /// Unused in Phase A; populated in Phase B.
    pub entropy: f32,
    /// Ticks since spawn — narrative attribution + balance histograms.
    pub age_ticks: u64,
    /// Tile-corruption value at spawn, retained for narrative + balance
    /// attribution (does this shadow-fox arise from heavy-corruption
    /// substrate vs marginal-threshold spawn).
    pub origin_corruption: f32,
}

impl ShadowFoxDrives {
    /// Spawn a freshly-manifested shadow-fox with full coherence and no
    /// motivational pressure yet. Phase B will populate drives on the
    /// motivation tick.
    pub fn newly_manifested(origin_corruption: f32) -> Self {
        Self {
            coherence: 1.0,
            resonance: 0.0,
            dread: 0.0,
            entropy: 0.0,
            age_ticks: 0,
            origin_corruption,
        }
    }
}

// ---------------------------------------------------------------------------
// Carcass — left behind by shadow fox kills
// ---------------------------------------------------------------------------

use crate::components::prey::PreyKind;

/// A rotting carcass left by a shadow fox kill. Emits corruption unless cleansed.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Carcass {
    pub prey_kind: PreyKind,
    pub age_ticks: u64,
    pub corruption_rate: f32,
    pub cleansed: bool,
    pub harvested: bool,
}

// ---------------------------------------------------------------------------
// Fox ecology — per-entity state, lifecycle, AI phase, and dens
// ---------------------------------------------------------------------------

/// Sex of a fox, used for pairing and breeding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FoxSex {
    Male,
    Female,
}

/// Life stage of a fox. Determines available behaviors and mortality curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FoxLifeStage {
    /// 0–1 season. Stays at den. Cannot hunt. Vulnerable to predators.
    Cub,
    /// 1–3 seasons. Disperses from natal den seeking unclaimed territory.
    Juvenile,
    /// 3–16 seasons. Full capabilities. Breeds during winter.
    Adult,
    /// 16+ seasons. Declining health, increasing mortality.
    Elder,
}

/// High-level behavioral phase for fox AI decision-making.
///
/// This sits above `WildlifeAiState` (which handles physical movement).
/// `fox_ai_decision` sets both the phase and the corresponding movement state.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FoxAiPhase {
    /// Default: patrol territory perimeter.
    PatrolTerritory { dx: i32, dy: i32 },
    /// Actively seeking small prey (mice, rats, rabbits — NOT cats).
    HuntingPrey { target: Option<u64> },
    /// Heading back to den after a hunt.
    Returning { x: i32, y: i32 },
    /// At den, resting/digesting. Well-fed foxes spend most time here.
    Resting { ticks: u64 },
    /// Juvenile looking for unclaimed territory.
    Dispersing { dx: i32, dy: i32 },
    /// Depositing scent marks at territory boundary.
    ScentMarking,
    /// In a standoff with a cat or rival fox.
    Confronting {
        target_id: u64,
        ticks_remaining: u64,
    },
    /// Retreating from danger toward map edge.
    Fleeing { dx: i32, dy: i32 },
    /// Approaching colony stores to steal food.
    Raiding { target_x: i32, target_y: i32 },
    /// Staying near den with cubs present.
    DenGuarding,
}

/// Per-fox mutable state: needs, lifecycle, and territory association.
///
/// Attached alongside `WildAnimal` to distinguish foxes from other wildlife.
/// Systems query `With<FoxState>` for fox-specific behavior.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FoxState {
    /// 0.0 = full, 1.0 = starving. Decays toward 1.0 each tick unless satiated.
    pub hunger: f32,
    /// Ticks remaining before hunger resumes decaying. Set on successful hunt/raid.
    pub satiation_ticks: u64,
    /// Current life stage.
    pub life_stage: FoxLifeStage,
    /// Ticks since birth.
    pub age_ticks: u64,
    /// Biological sex.
    pub sex: FoxSex,
    /// Associated den entity. None for dispersing juveniles.
    pub home_den: Option<Entity>,
    /// Paired mate entity.
    pub mate: Option<Entity>,
    /// Ticks before next hunt/confrontation attempt.
    pub post_action_cooldown: u64,
    /// 0.0–1.0. Derived from hunger — starving foxes are bold.
    pub boldness: f32,
    /// Consecutive ticks with `hunger >= 1.0`. Resets to 0 when satiated.
    /// Foxes die when this exceeds `fc.starvation_death_ticks`.
    pub starvation_ticks: u64,
    /// Tick when this fox last completed a patrol (DepositScent). Used by
    /// scoring to build pressure for periodic patrolling even when hunger wins.
    pub last_patrol_tick: u64,
}

impl FoxState {
    /// Create a new adult fox.
    pub fn new_adult(sex: FoxSex, den: Option<Entity>) -> Self {
        Self {
            hunger: 0.5,
            satiation_ticks: 0,
            life_stage: FoxLifeStage::Adult,
            age_ticks: 60_000, // ~3 seasons old
            sex,
            home_den: den,
            mate: None,
            post_action_cooldown: 0,
            boldness: 0.25,
            starvation_ticks: 0,
            last_patrol_tick: 0,
        }
    }

    /// Create a new cub at a den.
    pub fn new_cub(sex: FoxSex, den: Entity) -> Self {
        Self {
            hunger: 0.0,
            satiation_ticks: 8000, // cubs are nursed initially; safety buffer
            life_stage: FoxLifeStage::Cub,
            age_ticks: 0,
            sex,
            home_den: Some(den),
            mate: None,
            post_action_cooldown: 0,
            boldness: 0.0,
            starvation_ticks: 0,
            last_patrol_tick: 0,
        }
    }
}

/// A fox den — territory anchor and breeding site.
///
/// Follows `PreyDen` pattern. Each den represents a mated pair's home base.
/// Territory extends `territory_radius` tiles from the den position.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FoxDen {
    /// How far the territory extends from this den.
    pub territory_radius: i32,
    /// Number of living cubs at this den.
    pub cubs_present: u32,
    /// Scent strength at this den (0.0–1.0). Refreshed by patrolling adults.
    pub scent_strength: f32,
    /// Tick when this den was established.
    pub established_tick: u64,
    /// Tick of the last successful `FeedCubs` resolution. Defaults to 0
    /// (never fed). Used by `feed_cubs_at_dens` to refresh cub satiation.
    pub last_fed_tick: u64,
}

impl FoxDen {
    pub fn new(territory_radius: i32, tick: u64) -> Self {
        Self {
            territory_radius,
            cubs_present: 0,
            scent_strength: 0.5,
            established_tick: tick,
            last_fed_tick: tick, // treat spawn as freshly fed
        }
    }
}

// ---------------------------------------------------------------------------
// Hawk ecology — per-entity state, AI phase, and lifecycle (ticket 025 Phase 2)
// ---------------------------------------------------------------------------

/// High-level behavioral phase for hawk AI decision-making.
///
/// Mirrors the [`FoxAiPhase`] role: sits above [`WildlifeAiState`]
/// (physical movement). The hawk GOAP resolver writes both each tick.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum HawkAiPhase {
    /// Default airborne loiter — circle a center point at altitude.
    Soaring {
        center_x: i32,
        center_y: i32,
        angle: f32,
    },
    /// Diving on a target prey animal.
    HuntingPrey { target: Option<u64> },
    /// Perched on terrain — passive rest.
    Perched { ticks: u64 },
    /// Retreating from a threat toward the map edge.
    Fleeing { dx: i32, dy: i32 },
}

/// Per-hawk mutable state: needs, age, action cooldowns.
///
/// Attached alongside [`WildAnimal`] when the species is `Hawk`. Systems
/// query `With<HawkState>` for hawk-specific behavior; the GOAP planner
/// consumes the needs into [`HawkNeeds`](crate::ai::hawk_scoring::HawkNeeds)
/// each tick via `sync_hawk_needs`.
///
/// Hunger semantics here are `0.0 = full, 1.0 = starving` to match
/// [`FoxState::hunger`]. The matching `HawkNeeds::hunger` is the inverse
/// (1.0 = recently fed); `sync_hawk_needs` does the inversion.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HawkState {
    /// 0.0 = full, 1.0 = starving.
    pub hunger: f32,
    /// Ticks remaining before hunger resumes decaying. Set on a successful dive.
    pub satiation_ticks: u64,
    /// Ticks since spawn. Hawks are mono-stage adults — no life stage.
    pub age_ticks: u64,
    /// Ticks before another dive or flee.
    pub post_action_cooldown: u64,
    /// Consecutive ticks at `hunger >= 1.0`. Death at `hawk.starvation_death_duration`.
    pub starvation_ticks: u64,
    /// Tick of last perch. Drives Resting scoring pressure.
    pub last_perch_tick: u64,
    /// Tick of last dive. Drives Hunting scoring + patience curve.
    pub last_dive_tick: u64,
}

impl HawkState {
    /// Edge-spawned adult hawk with mid-range hunger and no cooldowns.
    pub fn new_adult() -> Self {
        Self {
            hunger: 0.5,
            satiation_ticks: 0,
            age_ticks: 0,
            post_action_cooldown: 0,
            starvation_ticks: 0,
            last_perch_tick: 0,
            last_dive_tick: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Snake ecology — per-entity state, AI phase, and thermoregulation
// ---------------------------------------------------------------------------

/// High-level behavioral phase for snake AI decision-making.
///
/// Mirrors the [`FoxAiPhase`] role for snakes. The snake GOAP resolver
/// writes both this and [`WildlifeAiState`] each tick.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SnakeAiPhase {
    /// Default ambush posture — stationary, watching for prey.
    Waiting,
    /// Closing in on a prey position before striking.
    Stalking { target_x: i32, target_y: i32 },
    /// Strike attempt this tick.
    Striking { target: Option<u64> },
    /// Thermoregulating — basking on warm terrain restores `warmth`.
    Basking { ticks: u64 },
    /// Retreating toward cover or the map edge.
    Fleeing { dx: i32, dy: i32 },
}

/// Per-snake mutable state with thermoregulation tier.
///
/// Attached alongside [`WildAnimal`] when the species is `Snake`. Systems
/// query `With<SnakeState>`; the GOAP planner consumes the needs into
/// [`SnakeNeeds`](crate::ai::snake_scoring::SnakeNeeds) each tick.
///
/// `warmth` decays each tick the snake is off warm terrain and resets to
/// `1.0` on a successful `Bask`.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnakeState {
    /// 0.0 = full, 1.0 = starving.
    pub hunger: f32,
    /// Ticks remaining before hunger resumes decaying. Set on a successful strike.
    pub satiation_ticks: u64,
    /// 0.0..=1.0. Decays when `!on_warm_terrain`; restored by `Bask`.
    pub warmth: f32,
    /// Ticks since spawn.
    pub age_ticks: u64,
    /// Ticks before another strike or flee.
    pub post_action_cooldown: u64,
    /// Consecutive ticks at `hunger >= 1.0`. Death at `snake.starvation_death_duration`.
    pub starvation_ticks: u64,
    /// Tick of last successful strike.
    pub last_strike_tick: u64,
    /// Tick of last completed bask. Drives Basking scoring pressure.
    pub last_bask_tick: u64,
}

impl SnakeState {
    /// Edge-spawned adult snake with mid-range hunger and partial warmth.
    pub fn new_adult() -> Self {
        Self {
            hunger: 0.5,
            satiation_ticks: 0,
            warmth: 0.7,
            age_ticks: 0,
            post_action_cooldown: 0,
            starvation_ticks: 0,
            last_strike_tick: 0,
            last_bask_tick: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Wildlife death causes + GOAP-side messages (ticket 025 Phase 2)
// ---------------------------------------------------------------------------

/// Why a non-fox wild animal died this tick. Attached to [`HawkDied`] /
/// [`SnakeDied`] messages so the event log can attribute the death.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WildlifeDeathCause {
    Starvation,
    OldAge,
    Combat,
    Other,
}

/// A hawk completed a dive — emitted by `resolve_dive_attack`. `prey` is
/// `Some` when the dive struck a prey entity this tick and `None` for
/// near-misses; kill-attribution itself stays in `predator_hunt_prey`.
#[derive(Message, Debug, Clone, Copy)]
pub struct HawkDiveLanded {
    pub hawk: Entity,
    pub prey: Option<Entity>,
    pub position: (i32, i32),
}

/// A snake executed a strike — emitted by `resolve_strike`. Same
/// strike/kill separation as [`HawkDiveLanded`].
#[derive(Message, Debug, Clone, Copy)]
pub struct SnakeStrikeLanded {
    pub snake: Entity,
    pub prey: Option<Entity>,
    pub position: (i32, i32),
}

/// A hawk died this tick. Emitted by `hawk_lifecycle_tick`.
#[derive(Message, Debug, Clone, Copy)]
pub struct HawkDied {
    pub hawk: Entity,
    pub cause: WildlifeDeathCause,
}

/// A snake died this tick. Emitted by `snake_lifecycle_tick`.
#[derive(Message, Debug, Clone, Copy)]
pub struct SnakeDied {
    pub snake: Entity,
    pub cause: WildlifeDeathCause,
}

// ---------------------------------------------------------------------------
// ActiveConfrontation — shared state for paired standoffs
// ---------------------------------------------------------------------------

/// Role in an active confrontation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfrontationRole {
    /// Initiated the confrontation (e.g., fox defending den).
    Attacker,
    /// Was confronted (e.g., cat that strayed too close).
    Defender,
}

/// Why the confrontation started. Drives escalation chance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfrontationReason {
    /// Fox is defending cubs at its den — high stakes, high escalation.
    DenDefense,
    /// Fox is starving and attacked a vulnerable cat.
    DesperateAttack,
    /// Territory dispute (future: two foxes).
    TerritoryDispute,
}

/// Shared state for a paired confrontation between two entities (fox vs cat
/// or fox vs fox). Inserted on BOTH participants so each side's AI sees the
/// encounter and can decide fight-or-flight independently.
///
/// The `min_commitment` field prevents oscillation: once locked in, neither
/// side can disengage for at least this many ticks.
#[derive(Component, Debug, Clone)]
pub struct ActiveConfrontation {
    pub partner: Entity,
    pub role: ConfrontationRole,
    pub reason: ConfrontationReason,
    pub ticks_remaining: u64,
    pub min_commitment: u64,
    /// Tick when the confrontation started — used to enforce min_commitment.
    pub started_tick: u64,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn species_defaults_are_consistent() {
        for species in [
            WildSpecies::Fox,
            WildSpecies::Hawk,
            WildSpecies::Snake,
            WildSpecies::ShadowFox,
        ] {
            let animal = WildAnimal::new(species);
            assert_eq!(animal.species, species);
            assert_eq!(animal.behavior, species.default_behavior());
            assert!(animal.threat_power > 0.0);
            assert!(animal.defense >= 0.0);
        }
    }

    #[test]
    fn fox_is_strongest_threat() {
        assert!(
            WildSpecies::ShadowFox.default_threat_power() > WildSpecies::Fox.default_threat_power()
        );
        assert!(WildSpecies::Fox.default_threat_power() > WildSpecies::Hawk.default_threat_power());
        assert!(
            WildSpecies::Hawk.default_threat_power() > WildSpecies::Snake.default_threat_power()
        );
    }

    #[test]
    fn population_caps_are_positive() {
        for species in [
            WildSpecies::Fox,
            WildSpecies::Hawk,
            WildSpecies::Snake,
            WildSpecies::ShadowFox,
        ] {
            assert!(species.population_cap() > 0);
        }
    }

    #[test]
    fn fox_state_new_adult() {
        let state = FoxState::new_adult(FoxSex::Female, None);
        assert_eq!(state.life_stage, FoxLifeStage::Adult);
        assert!((state.hunger - 0.5).abs() < f32::EPSILON);
        assert_eq!(state.satiation_ticks, 0);
        assert!(state.home_den.is_none());
    }

    #[test]
    fn fox_state_new_cub() {
        let den = Entity::from_bits(42);
        let state = FoxState::new_cub(FoxSex::Male, den);
        assert_eq!(state.life_stage, FoxLifeStage::Cub);
        assert_eq!(state.hunger, 0.0);
        assert_eq!(state.home_den, Some(den));
        assert_eq!(state.boldness, 0.0);
    }

    #[test]
    fn fox_den_defaults() {
        let den = FoxDen::new(18, 100);
        assert_eq!(den.territory_radius, 18);
        assert_eq!(den.cubs_present, 0);
        assert!(den.scent_strength > 0.0);
    }

    #[test]
    fn hawk_state_new_adult_defaults() {
        let state = HawkState::new_adult();
        assert!((state.hunger - 0.5).abs() < f32::EPSILON);
        assert_eq!(state.satiation_ticks, 0);
        assert_eq!(state.age_ticks, 0);
        assert_eq!(state.post_action_cooldown, 0);
        assert_eq!(state.starvation_ticks, 0);
    }

    #[test]
    fn snake_state_new_adult_defaults() {
        let state = SnakeState::new_adult();
        assert!((state.hunger - 0.5).abs() < f32::EPSILON);
        assert!((state.warmth - 0.7).abs() < f32::EPSILON);
        assert_eq!(state.satiation_ticks, 0);
        assert_eq!(state.age_ticks, 0);
        assert_eq!(state.starvation_ticks, 0);
    }

    #[test]
    fn hawk_ai_phase_variants_construct() {
        let phases = [
            HawkAiPhase::Soaring {
                center_x: 5,
                center_y: 5,
                angle: 0.0,
            },
            HawkAiPhase::HuntingPrey { target: None },
            HawkAiPhase::Perched { ticks: 0 },
            HawkAiPhase::Fleeing { dx: 1, dy: 0 },
        ];
        assert_eq!(phases.len(), 4);
    }

    #[test]
    fn snake_ai_phase_variants_construct() {
        let phases = [
            SnakeAiPhase::Waiting,
            SnakeAiPhase::Stalking {
                target_x: 0,
                target_y: 0,
            },
            SnakeAiPhase::Striking { target: None },
            SnakeAiPhase::Basking { ticks: 0 },
            SnakeAiPhase::Fleeing { dx: 0, dy: 0 },
        ];
        assert_eq!(phases.len(), 5);
    }

    #[test]
    fn wildlife_death_cause_round_trips() {
        for cause in [
            WildlifeDeathCause::Starvation,
            WildlifeDeathCause::OldAge,
            WildlifeDeathCause::Combat,
            WildlifeDeathCause::Other,
        ] {
            let s = serde_json::to_string(&cause).expect("serialize");
            let back: WildlifeDeathCause = serde_json::from_str(&s).expect("deserialize");
            assert_eq!(back, cause);
        }
    }
}
