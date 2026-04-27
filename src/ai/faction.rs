//! Faction model — §9 of `docs/systems/ai-substrate-refactor.md`.
//!
//! Three layers, per §9:
//!
//! 1. **[`FactionStance`]** — the six stance values every directed
//!    species-pair resolves to.
//! 2. **[`FactionRelations`]** — the 10×10 biological base matrix
//!    (100 directed cells) committed in §9.1. Keyed by observer ×
//!    target species pair.
//! 3. **[`resolve_stance`]** — applies the §9.2 ECS-marker overlay on
//!    top of the base matrix using most-negative-wins precedence.
//!
//! The resolver is pure — callers query overlay markers from the ECS
//! (`Visitor`, `HostileVisitor`, `Banished`, `BefriendedAlly` in
//! [`crate::components::markers`]) and pass them in. This keeps the
//! matrix + overlay logic testable without a live `World`.
//!
//! DSE filter bindings (§9.3) consume the resolver output via
//! [`StanceRequirement::accepts`] to decide whether a candidate
//! target is eligible for scoring.

use bevy_ecs::prelude::*;

use crate::components::prey::PreyKind;
use crate::components::sensing::SensorySpecies;
use crate::components::wildlife::WildSpecies;

// ---------------------------------------------------------------------------
// FactionStance
// ---------------------------------------------------------------------------

/// Base stance between an observer species and a target species. The
/// full 100-cell directed matrix lives in [`FactionRelations`]; this
/// enum is the value-shape every cell holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FactionStance {
    /// Same species, same colony. Default for cat-on-cat pre-overlay;
    /// intra-species peers for wildlife.
    Same,
    /// Different species, aligned (e.g. a befriended fox).
    Ally,
    Neutral,
    /// Hunting target.
    Prey,
    /// Flee target.
    Predator,
    /// Combat target (banished cats, hostile visitors, shadowfoxes).
    Enemy,
}

impl FactionStance {
    /// Ordinal used by the §9.2 most-negative-wins resolver. Lower is
    /// friendlier; higher is more hostile.
    pub fn negativity(self) -> u8 {
        match self {
            Self::Same => 0,
            Self::Ally => 1,
            Self::Neutral => 2,
            Self::Prey => 3,
            Self::Predator => 4,
            Self::Enemy => 5,
        }
    }
}

// ---------------------------------------------------------------------------
// FactionSpecies — the flattened 10-variant key
// ---------------------------------------------------------------------------

/// Flattened 10-variant species key matching §9.0's vocabulary
/// reconciliation: `Cat, Fox, Hawk, Snake, ShadowFox, Mouse, Rat,
/// Rabbit, Fish, Bird`. §9.0 commits the *value-shape* (100 cells,
/// directed) and leaves the key type as an implementation choice.
/// Flattened is simpler for matrix indexing than the nested
/// [`SensorySpecies`] — conversion is a one-shot mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FactionSpecies {
    Cat,
    Fox,
    Hawk,
    Snake,
    ShadowFox,
    Mouse,
    Rat,
    Rabbit,
    Fish,
    Bird,
}

impl FactionSpecies {
    pub const ALL: [Self; 10] = [
        Self::Cat,
        Self::Fox,
        Self::Hawk,
        Self::Snake,
        Self::ShadowFox,
        Self::Mouse,
        Self::Rat,
        Self::Rabbit,
        Self::Fish,
        Self::Bird,
    ];

    /// Index into the 10×10 matrix row/column order.
    pub fn index(self) -> usize {
        self as usize
    }

    /// Map the nested [`SensorySpecies`] onto the flattened key.
    pub fn from_sensory(s: SensorySpecies) -> Self {
        match s {
            SensorySpecies::Cat => Self::Cat,
            SensorySpecies::Wild(WildSpecies::Fox) => Self::Fox,
            SensorySpecies::Wild(WildSpecies::Hawk) => Self::Hawk,
            SensorySpecies::Wild(WildSpecies::Snake) => Self::Snake,
            SensorySpecies::Wild(WildSpecies::ShadowFox) => Self::ShadowFox,
            SensorySpecies::Prey(PreyKind::Mouse) => Self::Mouse,
            SensorySpecies::Prey(PreyKind::Rat) => Self::Rat,
            SensorySpecies::Prey(PreyKind::Rabbit) => Self::Rabbit,
            SensorySpecies::Prey(PreyKind::Fish) => Self::Fish,
            SensorySpecies::Prey(PreyKind::Bird) => Self::Bird,
        }
    }

    pub fn is_cat(self) -> bool {
        matches!(self, Self::Cat)
    }
}

// ---------------------------------------------------------------------------
// FactionRelations — §9.1 biological base matrix
// ---------------------------------------------------------------------------

/// The 10×10 directed base-stance matrix per §9.1. 100 cells
/// committed. Rows = observer; columns = target.
///
/// Asymmetry is first-class: `Cat → Fox = Predator` and
/// `Fox → Cat = Prey` coexist without contradiction. Diagonal is
/// `Same` by convention.
///
/// Footnote-labelled cells from §9.1:
///
/// - Hawk → Cat = Prey (kittens / injured adults); size-scaling lives
///   in `AttackDse` target considerations, not the matrix.
/// - Cat → Snake = Predator / Snake → Cat = Neutral (cornered snakes
///   become Enemy via a `Threatened` marker overlay, not base upgrade).
/// - Fox × ShadowFox = Neutral (conservative stance; upgrade to Enemy
///   on a fox-rejects-corruption system).
/// - ShadowFox × prey = Prey (runs same predator-hunt code).
/// - Aquatic carve-out: terrestrial predators × Fish = Neutral
///   (boundary today).
/// - Rat → Mouse = Prey / Mouse → Rat = Predator (latent ecology hook).
#[derive(Resource, Debug, Clone)]
pub struct FactionRelations {
    cells: [[FactionStance; 10]; 10],
}

impl FactionRelations {
    /// Construct the canonical §9.1 matrix. Called once at plugin load.
    pub fn canonical() -> Self {
        use FactionStance::{Enemy as E, Neutral as N, Predator as Pd, Prey as Py, Same as S};

        // Rows indexed by observer = FactionSpecies::ALL ordering:
        // 0 Cat, 1 Fox, 2 Hawk, 3 Snake, 4 ShadowFox,
        // 5 Mouse, 6 Rat, 7 Rabbit, 8 Fish, 9 Bird.
        //
        // Cell notation below matches the §9.1 prose matrix byte-for-
        // byte — verify against spec when the matrix changes.
        let cells = [
            // Cat:     Ct   Fx   Hw   Sn   ShFx Mo   Rt   Rb   Fi   Bd
            [S, Pd, Pd, Pd, E, Py, Py, Py, Py, Py],
            // Fox:
            [Py, S, N, N, N, Py, Py, Py, N, Py],
            // Hawk:
            [Py, N, S, Py, N, Py, Py, Py, N, Py],
            // Snake:
            [N, N, Pd, S, N, Py, Py, Py, N, Py],
            // ShadowFox:
            [E, N, N, N, S, Py, Py, Py, N, Py],
            // Mouse:
            [Pd, Pd, Pd, Pd, Pd, S, Pd, N, N, N],
            // Rat:
            [Pd, Pd, Pd, Pd, Pd, Py, S, N, N, N],
            // Rabbit:
            [Pd, Pd, Pd, Pd, Pd, N, N, S, N, N],
            // Fish:
            [Pd, N, N, N, N, N, N, N, S, N],
            // Bird:
            [Pd, Pd, Pd, Pd, Pd, N, N, N, N, S],
        ];
        Self { cells }
    }

    /// Base stance from observer's POV toward target.
    pub fn stance(&self, observer: FactionSpecies, target: FactionSpecies) -> FactionStance {
        self.cells[observer.index()][target.index()]
    }
}

impl Default for FactionRelations {
    fn default() -> Self {
        Self::canonical()
    }
}

// ---------------------------------------------------------------------------
// §9.2 Overlay resolver
// ---------------------------------------------------------------------------

/// Overlay markers attached to the target entity — the §9.2 ECS
/// markers that refine a cat-on-cat base stance (or upgrade
/// cat-wildlife Predator/Prey to Ally).
///
/// The evaluator (Phase 3a task #8) queries
/// [`Visitor`](crate::components::markers::Visitor),
/// [`HostileVisitor`](crate::components::markers::HostileVisitor),
/// [`Banished`](crate::components::markers::Banished),
/// [`BefriendedAlly`](crate::components::markers::BefriendedAlly) on
/// the target entity and packs the bools into this struct before
/// calling [`resolve_stance`].
#[derive(Debug, Clone, Copy, Default)]
pub struct StanceOverlays {
    pub visitor: bool,
    pub hostile_visitor: bool,
    pub banished: bool,
    pub befriended_ally: bool,
}

/// Apply the §9.2 overlay resolver to a base stance.
///
/// Most-negative-wins order:
/// `Banished ≻ HostileVisitor ≻ Visitor ≻ base ≻ BefriendedAlly`.
///
/// - `Banished`: demote cat-on-cat `Same` → `Enemy`.
/// - `HostileVisitor`: demote cat-on-cat `Same` → `Enemy`.
/// - `Visitor`: demote cat-on-cat `Same` → `Neutral`.
/// - `BefriendedAlly`: upgrade `Predator` → `Ally` (cat observer) or
///   `Prey` → `Ally` (wildlife observer toward cat target).
///
/// The `observer_is_cat` flag is required because Visitor / HostileVisitor
/// / Banished only fire for cat-on-cat observation (per §9.2's "Observer-
/// Cat × target-Cat" framing).
pub fn resolve_stance(
    base: FactionStance,
    observer_is_cat: bool,
    overlays: StanceOverlays,
) -> FactionStance {
    if observer_is_cat {
        if overlays.banished && base == FactionStance::Same {
            return FactionStance::Enemy;
        }
        if overlays.hostile_visitor && base == FactionStance::Same {
            return FactionStance::Enemy;
        }
        if overlays.visitor && base == FactionStance::Same {
            return FactionStance::Neutral;
        }
    }
    if overlays.befriended_ally {
        // Cat observer toward a wildlife target: Predator → Ally.
        // Wildlife observer toward a cat target: Prey → Ally.
        match base {
            FactionStance::Predator | FactionStance::Prey => return FactionStance::Ally,
            _ => {}
        }
    }
    base
}

/// §9.3 candidate prefilter — drop candidates whose resolved stance
/// fails `requirement`. Returns the kept entities and their parallel
/// positions, preserving input order.
///
/// The `target_species_of` closure resolves each candidate to a
/// [`FactionSpecies`]; returning `None` drops the candidate (the
/// candidate is unfactionable, e.g. a building entity reaching this
/// helper by mistake). The `overlays_of` closure reads the four §9.2
/// overlay markers from the ECS for each candidate.
///
/// Caller pre-filtering by distance / role is unchanged — this helper
/// only enforces the stance band, not target eligibility writ large.
pub fn filter_candidates_by_stance(
    relations: &FactionRelations,
    observer_species: FactionSpecies,
    candidates: &[Entity],
    positions: &[crate::components::physical::Position],
    target_species_of: &dyn Fn(Entity) -> Option<FactionSpecies>,
    overlays_of: &dyn Fn(Entity) -> StanceOverlays,
    requirement: &StanceRequirement,
) -> (Vec<Entity>, Vec<crate::components::physical::Position>) {
    debug_assert_eq!(
        candidates.len(),
        positions.len(),
        "candidate/position slices must match length"
    );
    let observer_is_cat = observer_species.is_cat();
    let mut kept = Vec::with_capacity(candidates.len());
    let mut kept_pos = Vec::with_capacity(candidates.len());
    for (entity, pos) in candidates.iter().zip(positions.iter()) {
        let Some(target_species) = target_species_of(*entity) else {
            continue;
        };
        let base = relations.stance(observer_species, target_species);
        let resolved = resolve_stance(base, observer_is_cat, overlays_of(*entity));
        if requirement.accepts(resolved) {
            kept.push(*entity);
            kept_pos.push(*pos);
        }
    }
    (kept, kept_pos)
}

// ---------------------------------------------------------------------------
// §9.3 StanceRequirement
// ---------------------------------------------------------------------------

/// "Target must be one of" stance set — the §9.3 DSE filter binding
/// shape. Matches the spec's pipe-separated notation: `Same | Ally`
/// becomes `StanceRequirement::any_of(&[Same, Ally])`.
#[derive(Debug, Clone)]
pub struct StanceRequirement {
    pub any_of: Vec<FactionStance>,
}

impl StanceRequirement {
    pub fn any_of(stances: &[FactionStance]) -> Self {
        Self {
            any_of: stances.to_vec(),
        }
    }

    pub fn accepts(&self, stance: FactionStance) -> bool {
        self.any_of.contains(&stance)
    }

    /// Socialize: cat talks to `Same | Ally` (§9.3).
    pub fn socialize() -> Self {
        Self::any_of(&[FactionStance::Same, FactionStance::Ally])
    }

    /// Attack: Enemy | Prey (§9.3).
    pub fn attack() -> Self {
        Self::any_of(&[FactionStance::Enemy, FactionStance::Prey])
    }

    /// Flee: Predator only (§9.3).
    pub fn flee() -> Self {
        Self::any_of(&[FactionStance::Predator])
    }

    /// Hunt: Prey only (§9.3).
    pub fn hunt() -> Self {
        Self::any_of(&[FactionStance::Prey])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- FactionStance negativity ---

    #[test]
    fn negativity_rank_most_negative_wins() {
        assert!(FactionStance::Enemy.negativity() > FactionStance::Predator.negativity());
        assert!(FactionStance::Predator.negativity() > FactionStance::Neutral.negativity());
        assert!(FactionStance::Neutral.negativity() > FactionStance::Ally.negativity());
        assert!(FactionStance::Ally.negativity() > FactionStance::Same.negativity());
    }

    // --- FactionSpecies mapping ---

    #[test]
    fn flattened_indices_stable() {
        assert_eq!(FactionSpecies::Cat.index(), 0);
        assert_eq!(FactionSpecies::Bird.index(), 9);
        assert_eq!(FactionSpecies::ALL.len(), 10);
    }

    #[test]
    fn sensory_to_flattened_roundtrip() {
        for species in FactionSpecies::ALL {
            // Round-trip only makes sense for species with a sensory
            // representation. All ten have one.
            let sensory = match species {
                FactionSpecies::Cat => SensorySpecies::Cat,
                FactionSpecies::Fox => SensorySpecies::Wild(WildSpecies::Fox),
                FactionSpecies::Hawk => SensorySpecies::Wild(WildSpecies::Hawk),
                FactionSpecies::Snake => SensorySpecies::Wild(WildSpecies::Snake),
                FactionSpecies::ShadowFox => SensorySpecies::Wild(WildSpecies::ShadowFox),
                FactionSpecies::Mouse => SensorySpecies::Prey(PreyKind::Mouse),
                FactionSpecies::Rat => SensorySpecies::Prey(PreyKind::Rat),
                FactionSpecies::Rabbit => SensorySpecies::Prey(PreyKind::Rabbit),
                FactionSpecies::Fish => SensorySpecies::Prey(PreyKind::Fish),
                FactionSpecies::Bird => SensorySpecies::Prey(PreyKind::Bird),
            };
            assert_eq!(FactionSpecies::from_sensory(sensory), species);
        }
    }

    // --- §9.1 matrix spot-checks ---

    #[test]
    fn diagonal_is_same() {
        let rel = FactionRelations::canonical();
        for species in FactionSpecies::ALL {
            assert_eq!(rel.stance(species, species), FactionStance::Same);
        }
    }

    #[test]
    fn cat_fox_asymmetric_predator_prey() {
        let rel = FactionRelations::canonical();
        assert_eq!(
            rel.stance(FactionSpecies::Cat, FactionSpecies::Fox),
            FactionStance::Predator
        );
        assert_eq!(
            rel.stance(FactionSpecies::Fox, FactionSpecies::Cat),
            FactionStance::Prey
        );
    }

    #[test]
    fn cat_snake_asymmetric_predator_neutral() {
        // §9.1 footnote 3: Cat → Snake = Predator, Snake → Cat = Neutral
        let rel = FactionRelations::canonical();
        assert_eq!(
            rel.stance(FactionSpecies::Cat, FactionSpecies::Snake),
            FactionStance::Predator
        );
        assert_eq!(
            rel.stance(FactionSpecies::Snake, FactionSpecies::Cat),
            FactionStance::Neutral
        );
    }

    #[test]
    fn cat_shadowfox_is_enemy() {
        let rel = FactionRelations::canonical();
        assert_eq!(
            rel.stance(FactionSpecies::Cat, FactionSpecies::ShadowFox),
            FactionStance::Enemy
        );
    }

    #[test]
    fn shadowfox_prey_is_prey() {
        // §9.1 footnote 8: ShadowFox × prey = Prey (shares predator-hunt code).
        let rel = FactionRelations::canonical();
        for prey in [
            FactionSpecies::Mouse,
            FactionSpecies::Rat,
            FactionSpecies::Rabbit,
            FactionSpecies::Bird,
        ] {
            assert_eq!(
                rel.stance(FactionSpecies::ShadowFox, prey),
                FactionStance::Prey,
                "ShadowFox → {prey:?}",
            );
        }
        // Fish is a neutral carve-out per footnote 6.
        assert_eq!(
            rel.stance(FactionSpecies::ShadowFox, FactionSpecies::Fish),
            FactionStance::Neutral
        );
    }

    #[test]
    fn aquatic_carve_out_fox_snake_hawk_shfx_to_fish_is_neutral() {
        // §9.1 footnote 6: terrestrial predators × Fish = Neutral.
        let rel = FactionRelations::canonical();
        for terrestrial in [
            FactionSpecies::Fox,
            FactionSpecies::Hawk,
            FactionSpecies::Snake,
            FactionSpecies::ShadowFox,
        ] {
            assert_eq!(
                rel.stance(terrestrial, FactionSpecies::Fish),
                FactionStance::Neutral,
                "{terrestrial:?} → Fish",
            );
        }
        // Cat → Fish is still Prey (cats fish over water edges).
        assert_eq!(
            rel.stance(FactionSpecies::Cat, FactionSpecies::Fish),
            FactionStance::Prey
        );
    }

    #[test]
    fn fox_shadowfox_conservative_neutral() {
        // §9.1 footnote 4: Fox × ShadowFox = Neutral on both rows.
        let rel = FactionRelations::canonical();
        assert_eq!(
            rel.stance(FactionSpecies::Fox, FactionSpecies::ShadowFox),
            FactionStance::Neutral
        );
        assert_eq!(
            rel.stance(FactionSpecies::ShadowFox, FactionSpecies::Fox),
            FactionStance::Neutral
        );
    }

    #[test]
    fn rat_mouse_asymmetric_prey_predator() {
        // §9.1 footnote 9: Rat × Mouse = Prey, Mouse × Rat = Predator.
        let rel = FactionRelations::canonical();
        assert_eq!(
            rel.stance(FactionSpecies::Rat, FactionSpecies::Mouse),
            FactionStance::Prey
        );
        assert_eq!(
            rel.stance(FactionSpecies::Mouse, FactionSpecies::Rat),
            FactionStance::Predator
        );
    }

    #[test]
    fn hundred_cells_populated() {
        // Every cell is reachable and has a defined stance.
        let rel = FactionRelations::canonical();
        for observer in FactionSpecies::ALL {
            for target in FactionSpecies::ALL {
                let _ = rel.stance(observer, target);
            }
        }
    }

    // --- §9.2 overlay resolver ---

    #[test]
    fn overlay_banished_demotes_same_to_enemy() {
        let out = resolve_stance(
            FactionStance::Same,
            true,
            StanceOverlays {
                banished: true,
                ..Default::default()
            },
        );
        assert_eq!(out, FactionStance::Enemy);
    }

    #[test]
    fn overlay_hostile_visitor_demotes_same_to_enemy() {
        let out = resolve_stance(
            FactionStance::Same,
            true,
            StanceOverlays {
                hostile_visitor: true,
                ..Default::default()
            },
        );
        assert_eq!(out, FactionStance::Enemy);
    }

    #[test]
    fn overlay_visitor_demotes_same_to_neutral() {
        let out = resolve_stance(
            FactionStance::Same,
            true,
            StanceOverlays {
                visitor: true,
                ..Default::default()
            },
        );
        assert_eq!(out, FactionStance::Neutral);
    }

    #[test]
    fn overlay_befriended_ally_upgrades_predator_to_ally() {
        // Cat observer toward a fox target: Predator → Ally.
        let out = resolve_stance(
            FactionStance::Predator,
            true,
            StanceOverlays {
                befriended_ally: true,
                ..Default::default()
            },
        );
        assert_eq!(out, FactionStance::Ally);
    }

    #[test]
    fn overlay_befriended_ally_upgrades_prey_to_ally_reciprocal() {
        // Fox observer toward a cat target: Prey → Ally.
        let out = resolve_stance(
            FactionStance::Prey,
            false,
            StanceOverlays {
                befriended_ally: true,
                ..Default::default()
            },
        );
        assert_eq!(out, FactionStance::Ally);
    }

    #[test]
    fn overlay_most_negative_wins_banished_over_visitor() {
        // Target has both Banished and Visitor: Banished wins (Enemy).
        let out = resolve_stance(
            FactionStance::Same,
            true,
            StanceOverlays {
                visitor: true,
                banished: true,
                ..Default::default()
            },
        );
        assert_eq!(out, FactionStance::Enemy);
    }

    #[test]
    fn overlay_most_negative_wins_hostile_over_visitor() {
        let out = resolve_stance(
            FactionStance::Same,
            true,
            StanceOverlays {
                visitor: true,
                hostile_visitor: true,
                ..Default::default()
            },
        );
        assert_eq!(out, FactionStance::Enemy);
    }

    #[test]
    fn overlay_befriended_ally_loses_to_banished() {
        // Pathological coexistence: Banished (Enemy) should win over
        // BefriendedAlly per §9.2 precedence chain. Base is Same so
        // BefriendedAlly's Predator/Prey upgrade path would not fire
        // regardless, but we commit the precedence explicitly.
        let out = resolve_stance(
            FactionStance::Same,
            true,
            StanceOverlays {
                banished: true,
                befriended_ally: true,
                ..Default::default()
            },
        );
        assert_eq!(out, FactionStance::Enemy);
    }

    #[test]
    fn overlay_wildlife_observer_ignores_visitor_banished_chain() {
        // Visitor/HostileVisitor/Banished are cat-on-cat concepts;
        // wildlife observer should ignore them.
        let out = resolve_stance(
            FactionStance::Prey,
            false,
            StanceOverlays {
                visitor: true,
                hostile_visitor: true,
                banished: true,
                ..Default::default()
            },
        );
        assert_eq!(out, FactionStance::Prey);
    }

    #[test]
    fn overlay_no_markers_returns_base() {
        let out = resolve_stance(FactionStance::Neutral, true, StanceOverlays::default());
        assert_eq!(out, FactionStance::Neutral);
    }

    // --- §9.3 StanceRequirement ---

    #[test]
    fn stance_requirement_socialize_accepts_same_ally() {
        let req = StanceRequirement::socialize();
        assert!(req.accepts(FactionStance::Same));
        assert!(req.accepts(FactionStance::Ally));
        assert!(!req.accepts(FactionStance::Enemy));
        assert!(!req.accepts(FactionStance::Predator));
    }

    #[test]
    fn stance_requirement_attack_accepts_enemy_prey() {
        let req = StanceRequirement::attack();
        assert!(req.accepts(FactionStance::Enemy));
        assert!(req.accepts(FactionStance::Prey));
        assert!(!req.accepts(FactionStance::Same));
    }

    #[test]
    fn stance_requirement_flee_accepts_predator_only() {
        let req = StanceRequirement::flee();
        assert!(req.accepts(FactionStance::Predator));
        assert!(!req.accepts(FactionStance::Enemy));
        assert!(!req.accepts(FactionStance::Prey));
    }

    #[test]
    fn stance_requirement_hunt_accepts_prey_only() {
        let req = StanceRequirement::hunt();
        assert!(req.accepts(FactionStance::Prey));
        assert!(!req.accepts(FactionStance::Enemy));
    }

    // --- §9.3 filter_candidates_by_stance ---

    fn entity_n(n: u32) -> Entity {
        Entity::from_raw_u32(n).unwrap()
    }

    #[test]
    fn filter_empty_candidates_returns_empty() {
        let rel = FactionRelations::canonical();
        let species_of = |_: Entity| Some(FactionSpecies::Cat);
        let overlays_of = |_: Entity| StanceOverlays::default();
        let (kept, kept_pos) = filter_candidates_by_stance(
            &rel,
            FactionSpecies::Cat,
            &[],
            &[],
            &species_of,
            &overlays_of,
            &StanceRequirement::socialize(),
        );
        assert!(kept.is_empty());
        assert!(kept_pos.is_empty());
    }

    #[test]
    fn filter_keeps_all_when_base_stance_satisfies_requirement() {
        let rel = FactionRelations::canonical();
        let a = entity_n(10);
        let b = entity_n(11);
        let candidates = [a, b];
        let positions = [
            crate::components::physical::Position::new(0, 0),
            crate::components::physical::Position::new(1, 0),
        ];
        let species_of = |_: Entity| Some(FactionSpecies::Cat);
        let overlays_of = |_: Entity| StanceOverlays::default();
        let (kept, kept_pos) = filter_candidates_by_stance(
            &rel,
            FactionSpecies::Cat,
            &candidates,
            &positions,
            &species_of,
            &overlays_of,
            &StanceRequirement::socialize(),
        );
        assert_eq!(kept, vec![a, b]);
        assert_eq!(kept_pos.len(), 2);
    }

    #[test]
    fn filter_drops_banished_cat_under_socialize() {
        let rel = FactionRelations::canonical();
        let normal = entity_n(10);
        let banished = entity_n(11);
        let candidates = [normal, banished];
        let positions = [
            crate::components::physical::Position::new(0, 0),
            crate::components::physical::Position::new(1, 0),
        ];
        let species_of = |_: Entity| Some(FactionSpecies::Cat);
        let overlays_of = move |e: Entity| {
            if e == banished {
                StanceOverlays {
                    banished: true,
                    ..Default::default()
                }
            } else {
                StanceOverlays::default()
            }
        };
        let (kept, _) = filter_candidates_by_stance(
            &rel,
            FactionSpecies::Cat,
            &candidates,
            &positions,
            &species_of,
            &overlays_of,
            &StanceRequirement::socialize(),
        );
        assert_eq!(kept, vec![normal]);
    }

    #[test]
    fn filter_befriended_fox_kept_by_socialize_dropped_by_hunt() {
        let rel = FactionRelations::canonical();
        let fox = entity_n(20);
        let candidates = [fox];
        let positions = [crate::components::physical::Position::new(0, 0)];
        let species_of = |_: Entity| Some(FactionSpecies::Fox);
        let overlays_of = |_: Entity| StanceOverlays {
            befriended_ally: true,
            ..Default::default()
        };

        // Cat → Fox base = Predator. BefriendedAlly upgrades to Ally.
        // Socialize accepts Ally → kept.
        let (kept_soc, _) = filter_candidates_by_stance(
            &rel,
            FactionSpecies::Cat,
            &candidates,
            &positions,
            &species_of,
            &overlays_of,
            &StanceRequirement::socialize(),
        );
        assert_eq!(kept_soc, vec![fox]);

        // Hunt requires Prey; Ally is not Prey → dropped.
        let (kept_hunt, _) = filter_candidates_by_stance(
            &rel,
            FactionSpecies::Cat,
            &candidates,
            &positions,
            &species_of,
            &overlays_of,
            &StanceRequirement::hunt(),
        );
        assert!(kept_hunt.is_empty());
    }

    #[test]
    fn filter_resolves_per_candidate_species() {
        // Cat observer with mixed candidates: a fellow cat (Same →
        // Socialize accepts) and a snake (Predator → Socialize
        // rejects). The species_of closure must resolve each candidate
        // independently.
        let rel = FactionRelations::canonical();
        let cat = entity_n(30);
        let snake = entity_n(31);
        let candidates = [cat, snake];
        let positions = [
            crate::components::physical::Position::new(0, 0),
            crate::components::physical::Position::new(1, 0),
        ];
        let species_of = move |e: Entity| {
            if e == cat {
                Some(FactionSpecies::Cat)
            } else {
                Some(FactionSpecies::Snake)
            }
        };
        let overlays_of = |_: Entity| StanceOverlays::default();
        let (kept, _) = filter_candidates_by_stance(
            &rel,
            FactionSpecies::Cat,
            &candidates,
            &positions,
            &species_of,
            &overlays_of,
            &StanceRequirement::socialize(),
        );
        assert_eq!(kept, vec![cat]);
    }

    #[test]
    fn filter_drops_candidate_when_species_resolver_returns_none() {
        let rel = FactionRelations::canonical();
        let candidate = entity_n(40);
        let candidates = [candidate];
        let positions = [crate::components::physical::Position::new(0, 0)];
        let species_of = |_: Entity| None;
        let overlays_of = |_: Entity| StanceOverlays::default();
        let (kept, kept_pos) = filter_candidates_by_stance(
            &rel,
            FactionSpecies::Cat,
            &candidates,
            &positions,
            &species_of,
            &overlays_of,
            &StanceRequirement::socialize(),
        );
        assert!(kept.is_empty());
        assert!(kept_pos.is_empty());
    }
}
