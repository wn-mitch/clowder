//! L1 influence-map substrate — Phase 2A scaffolding per §5 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! This module defines:
//!
//! - The `InfluenceMap` trait every L1 map implements (metadata +
//!   `base_sample(pos)`).
//! - `Channel` (§5.6.2) and `Faction` enumerations — kept open per the
//!   §5.6.9 extensibility contract.
//! - The `attenuate` helper that applies the §5.6.6 species × role ×
//!   injury × environment pipeline to a base sample.
//! - `species_sensitivity()` lookup from the 40-cell matrix already
//!   committed in `SimConstants::sensory` — wire, don't re-author.
//!
//! **Phase 2A scope:** scaffolding only. Three persistent-grid maps
//! already in the codebase (`FoxScentMap`, `CatPresenceMap`,
//! `ExplorationMap`) get `InfluenceMap` impls so their per-position
//! reads become uniform. Scent-from-on-demand-detection and
//! corruption-from-TileMap are Phase 2B migrations — they require
//! backing-data changes the scaffolding doesn't yet justify.
//!
//! **Phase 2A non-goals:**
//! - Dynamic registry / trait-object dispatch from trace emitter. The
//!   Phase 1 emitter still hardcodes fox-scent; Phase 2D rewrites that
//!   into a registry walk once the registry shape settles.
//! - Template-based stamping (§5.1 templates). The existing bucketed /
//!   per-tile maps already stamp; no new stamping code lands here.
//! - Obstacle-aware propagation (§5.4). Current maps use their existing
//!   propagation; §5.4's Dijkstra-for-pursuit-threat is Phase 2B+.
//!
//! **Non-identity attenuation stays identity at Phase 2A:**
//! - Species sensitivity: read from `SimConstants::sensory`.
//! - Role modifier: `1.0` (active when §4.3 role markers land in
//!   Phase 3a).
//! - Injury deficit: `0.0` (active when body-zones epic lands — out
//!   of refactor scope).
//! - Environment multiplier: `1.0` (activation is Phase 2 balance
//!   work, separate from the structural scaffolding).

use crate::components::physical::Position;
use crate::components::sensing::SensorySpecies;
use crate::resources::sim_constants::SensoryConstants;
use crate::systems::sensing::ChannelKind;

// ---------------------------------------------------------------------------
// Channel labels
// ---------------------------------------------------------------------------

/// Stable lowercase slug for a sensory channel. Mirrors §5.6.2 naming
/// in the trace record format and jq queries. Reuses
/// `crate::systems::sensing::ChannelKind` — the existing enum covers
/// sight / hearing / scent / tremor one-to-one, and §5.6.2 permits
/// adding new channels as registrations rather than refactors via
/// the `#[non_exhaustive]` attribute on the underlying enum.
pub fn channel_label(channel: ChannelKind) -> &'static str {
    match channel {
        ChannelKind::Sight => "sight",
        ChannelKind::Hearing => "hearing",
        ChannelKind::Scent => "scent",
        ChannelKind::Tremor => "tremor",
    }
}

// ---------------------------------------------------------------------------
// Faction (§5.1 "one map per channel × faction")
// ---------------------------------------------------------------------------

/// Faction identity of an influence source. A base map is keyed on
/// `(Channel, Faction)` so a "scent × fox" map and a "scent × prey"
/// map don't collide. Per §5.6.9 the storage registry must be
/// `(channel, faction)`-keyed so adding a 14th map (pheromone,
/// fire-danger, sacred-site draw) is a registration, not a schema
/// change.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub enum Faction {
    /// Species-scoped — the emitters of this map are all of the
    /// named species (fox-scent emitters, prey-scent emitters).
    Species(SensorySpecies),
    /// Neutral, non-faction substrate (corruption, carcasses —
    /// no allegiance). §5.5 defers cat-pairwise social affinity to
    /// the ToT belief layer; congregation (where cats gather) is
    /// `Colony`, not `Neutral`.
    Neutral,
    /// Colony-scoped — wards, colony-cats-as-group, stores,
    /// structures. Shorthand for "the player's faction."
    Colony,
    /// Observer-specific — ExplorationMap is per-observer in the
    /// §5.6.3 catalog (each cat has its own exploration state).
    /// Phase 2A uses the global ExplorationMap; multi-observer
    /// attribution is a follow-on.
    Observer,
}

impl Faction {
    pub fn label(&self) -> String {
        match self {
            Self::Species(s) => format!("species:{:?}", s),
            Self::Neutral => "neutral".to_string(),
            Self::Colony => "colony".to_string(),
            Self::Observer => "observer".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// MapMetadata
// ---------------------------------------------------------------------------

/// Static identity of an L1 map — the fields the trace emitter
/// records per §11.3 L1 record, independent of per-tick sampling.
#[derive(Debug, Clone)]
pub struct MapMetadata {
    /// Stable slug for logs — keep kebab-case identifiers for jq.
    pub name: &'static str,
    pub channel: ChannelKind,
    pub faction: Faction,
}

// ---------------------------------------------------------------------------
// InfluenceMap trait
// ---------------------------------------------------------------------------

/// Common interface every L1 map implements. Phase 2A minimum:
/// metadata + point-sample. Phase 2B adds `top_contributors(pos)`
/// once scent migrates off the on-demand per-pair pattern.
///
/// The trait is intentionally object-safe-adjacent but **not used as
/// a trait object** in Phase 2A. Each map is a distinct Bevy
/// resource; dispatch happens by calling the trait method on the
/// concrete resource type. Phase 2D wires a dynamic registry once
/// all five Partial maps share the shape.
pub trait InfluenceMap {
    fn metadata(&self) -> MapMetadata;
    /// Pre-attenuation base-sample value at a world position. Return
    /// `0.0` for out-of-bounds / unsupported coordinates.
    fn base_sample(&self, pos: Position) -> f32;
}

// ---------------------------------------------------------------------------
// InfluenceMap impls for the three Partial persistent-grid maps
// ---------------------------------------------------------------------------

impl InfluenceMap for crate::resources::FoxScentMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            name: "fox_scent",
            channel: ChannelKind::Scent,
            faction: Faction::Species(SensorySpecies::Wild(
                crate::components::wildlife::WildSpecies::Fox,
            )),
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::PreyScentMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // Single aggregate map across all prey species today
            // (§5.6.3 row #1 lists per-species as the end-state —
            // that's a follow-on once target-selection wants to
            // discriminate). Faction::Neutral since the map covers
            // multiple species.
            name: "prey_scent",
            channel: ChannelKind::Scent,
            faction: Faction::Neutral,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::CarcassScentMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // §5.6.3 row #6: scent × neutral. Carcasses are not
            // faction-aligned — both cats (harvest, cleanse) and
            // wildlife (scavenger draw) read this channel.
            name: "carcass_scent",
            channel: ChannelKind::Scent,
            faction: Faction::Neutral,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::CatPresenceMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            name: "congregation",
            channel: ChannelKind::Sight,
            faction: Faction::Colony,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::ExplorationMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            name: "exploration",
            channel: ChannelKind::Sight,
            faction: Faction::Observer,
        }
    }

    /// Returns the *explored-mass* at a tile (0.0 = never seen, 1.0 =
    /// fully explored). Downstream readers that want the unexplored
    /// inverse should compute `1.0 - base_sample(pos)` explicitly —
    /// exposing the raw grid keeps the `InfluenceMap` semantics
    /// uniform (§5.6.5 lists ExplorationMap as a positive-sense map).
    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::WardCoverageMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // §5.6.3 row #3: ward-coverage map. Tagged Sight for lack
            // of a "spatial-independent" channel today, matching the
            // CorruptionLens convention. Faction::Colony — wards are
            // a colony-faction emitter.
            name: "ward_coverage",
            channel: ChannelKind::Sight,
            faction: Faction::Colony,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::FoodLocationMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // §5.6.3 row #7: food-location (Stores + Kitchen) — sight
            // × colony. Producer landed by ticket 006; consumer
            // cutover (Eat / Forage `SpatialConsideration`) lives in
            // ticket 052.
            name: "food_location",
            channel: ChannelKind::Sight,
            faction: Faction::Colony,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::GardenLocationMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // §5.6.3 row #10: garden-location — sight × colony.
            // Producer landed by ticket 006; consumer cutover (Tend
            // / Harvest target ranking) lives in ticket 052.
            name: "garden_location",
            channel: ChannelKind::Sight,
            faction: Faction::Colony,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::ConstructionSiteMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // §5.6.3 row #9: construction / damaged-building — sight
            // × colony. Producer landed by ticket 006; consumer
            // cutover (Build / Repair target ranking) lives in
            // ticket 052.
            name: "construction_site",
            channel: ChannelKind::Sight,
            faction: Faction::Colony,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::KittenCryMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // §5.6.3 row #13: kitten-cry — hearing × colony.
            // Producer originally landed by ticket 006 as
            // "kitten-urgency / sight"; ticket 156 repurposed it as
            // a Hearing-channel cry broadcast and wired the consumer
            // (`update_kitten_cry_perceived` → `CaretakeDse`).
            name: "kitten_cry",
            channel: ChannelKind::Hearing,
            faction: Faction::Colony,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::HerbLocationMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // §5.6.3 row #8: herb-location — sight × neutral. Producer
            // + initial consumer (HerbcraftTarget DSE) landed by
            // ticket 061. `base_sample` returns the per-kind sum so
            // the trait projection answers "any-herb density"; per-
            // kind queries (e.g., Thornbriar density for ward placement)
            // go through `HerbLocationMap::get` directly.
            name: "herb_location",
            channel: ChannelKind::Sight,
            faction: Faction::Neutral,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.total(pos.x, pos.y)
    }
}

/// Borrow-based adapter that exposes `TileMap`'s per-tile corruption
/// field as an `InfluenceMap`. Corruption lives alongside terrain on
/// `Tile` rather than in a dedicated resource; the lens avoids
/// changing that storage layout while letting the map participate in
/// the uniform substrate API per §5.6.3 row #2.
///
/// The lens is constructed inline at read time (e.g. in the trace
/// emitter's L1 walk) — `InfluenceMap` is not used as a trait object
/// in Phase 2A, so a short-lived borrow adapter is sufficient.
pub struct CorruptionLens<'a>(pub &'a crate::resources::TileMap);

impl InfluenceMap for CorruptionLens<'_> {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            name: "corruption",
            // §5.6.3 row #2: "sight-independent spatial × neutral".
            // Tagged as Sight here for lack of a "spatial-independent"
            // channel variant; Phase 3+ may introduce a dedicated
            // variant when the distinction matters for scoring.
            channel: ChannelKind::Sight,
            faction: Faction::Neutral,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        if !self.0.in_bounds(pos.x, pos.y) {
            return 0.0;
        }
        self.0.get(pos.x, pos.y).corruption
    }
}

// ---------------------------------------------------------------------------
// §5.6.6 attenuation pipeline
// ---------------------------------------------------------------------------

/// Composite per-agent attenuation for a single channel read. Phase
/// 2A wires species and leaves role / injury / env at identity; see
/// module docstring.
///
/// Formula (§5.2):
/// ```text
/// perceived = base_sample
///           × species_sens
///           × role_mod
///           × (1.0 − injury_deficit)
///           × env_mul
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Attenuation {
    pub species_sens: f32,
    pub role_mod: f32,
    pub injury_deficit: f32,
    pub env_mul: f32,
}

impl Attenuation {
    /// All-identity attenuation (no channel modulation).
    pub const IDENTITY: Self = Self {
        species_sens: 1.0,
        role_mod: 1.0,
        injury_deficit: 0.0,
        env_mul: 1.0,
    };

    /// Build an attenuation for the given species × channel, with
    /// role / injury / env at Phase 2A identity. `sensory` is the
    /// global `SensoryConstants` table from `SimConstants::sensory`.
    pub fn for_species_channel(
        sensory: &SensoryConstants,
        species: SensorySpecies,
        channel: ChannelKind,
    ) -> Self {
        Self {
            species_sens: species_sensitivity(sensory, species, channel),
            role_mod: 1.0,
            injury_deficit: 0.0,
            env_mul: 1.0,
        }
    }

    /// Apply this attenuation to a base sample. Returns the
    /// perceived value.
    pub fn apply(&self, base: f32) -> f32 {
        base * self.species_sens * self.role_mod * (1.0 - self.injury_deficit) * self.env_mul
    }
}

/// Look up the species-sensitivity coefficient for a single
/// (species, channel) pair from the 40-cell matrix already committed
/// in `SimConstants::sensory`. Returns `0.0` when the species does
/// not use that channel — `Channel::is_active()` returns false when
/// `base_range == 0.0` (e.g., hawk scent, cat tremor are DISABLED
/// per `src/resources/sim_constants.rs:2605–2696`).
///
/// **Phase 2A semantic:** acts as a binary gate — `1.0` if the
/// species uses the channel, `0.0` if disabled. The existing matrix
/// stores `base_range` + `acuity` + `falloff` per cell rather than a
/// single sensitivity scalar; mapping any of those onto a
/// multiplicative attenuation is a tuning decision that belongs in
/// Phase 3+ balance work (per the refactor plan: "role × channel
/// wired, identity today; active when §4.3 role markers land in
/// Phase 3a"). Phase 2A ships the scaffold with binary gating so
/// downstream code sees disabled channels collapse to zero sample
/// contribution, matching current sensing behaviour.
pub fn species_sensitivity(
    sensory: &SensoryConstants,
    species: SensorySpecies,
    channel: ChannelKind,
) -> f32 {
    let profile = sensory.profile_for(species);
    let ch = match channel {
        ChannelKind::Sight => &profile.sight,
        ChannelKind::Hearing => &profile.hearing,
        ChannelKind::Scent => &profile.scent,
        ChannelKind::Tremor => &profile.tremor,
    };
    if ch.is_active() {
        1.0
    } else {
        0.0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::wildlife::WildSpecies;

    #[test]
    fn attenuation_identity_preserves_base() {
        let a = Attenuation::IDENTITY;
        assert_eq!(a.apply(0.5), 0.5);
        assert_eq!(a.apply(1.0), 1.0);
        assert_eq!(a.apply(0.0), 0.0);
    }

    #[test]
    fn attenuation_composes_species_sens() {
        let a = Attenuation {
            species_sens: 0.5,
            ..Attenuation::IDENTITY
        };
        assert!((a.apply(1.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn attenuation_injury_deficit_subtracts() {
        let a = Attenuation {
            injury_deficit: 0.25,
            ..Attenuation::IDENTITY
        };
        // 1.0 × 1.0 × 1.0 × (1 - 0.25) × 1.0 = 0.75
        assert!((a.apply(1.0) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn attenuation_full_formula() {
        let a = Attenuation {
            species_sens: 0.8,
            role_mod: 1.2,
            injury_deficit: 0.1,
            env_mul: 0.9,
        };
        // 1.0 × 0.8 × 1.2 × 0.9 × 0.9 = 0.7776
        let expected = 0.8 * 1.2 * (1.0 - 0.1) * 0.9;
        assert!((a.apply(1.0) - expected).abs() < 1e-6);
    }

    #[test]
    fn species_sensitivity_one_for_active_channel() {
        let sensory = SensoryConstants::default();
        // Cat scent is active per sim_constants.rs:2631 (base_range=15.0).
        let v = species_sensitivity(&sensory, SensorySpecies::Cat, ChannelKind::Scent);
        assert_eq!(v, 1.0);
    }

    #[test]
    fn species_sensitivity_zero_for_disabled_channel() {
        let sensory = SensoryConstants::default();
        // Hawk does not use scent (Channel::DISABLED).
        let v = species_sensitivity(
            &sensory,
            SensorySpecies::Wild(WildSpecies::Hawk),
            ChannelKind::Scent,
        );
        assert_eq!(v, 0.0);
    }

    #[test]
    fn species_sensitivity_zero_for_cat_tremor() {
        let sensory = SensoryConstants::default();
        // Cat does not tremor-sense (Channel::DISABLED per sim_constants.rs:2632).
        let v = species_sensitivity(&sensory, SensorySpecies::Cat, ChannelKind::Tremor);
        assert_eq!(v, 0.0);
    }

    #[test]
    fn attenuation_for_species_channel_uses_matrix() {
        let sensory = SensoryConstants::default();
        let a = Attenuation::for_species_channel(
            &sensory,
            SensorySpecies::Wild(WildSpecies::Fox),
            ChannelKind::Scent,
        );
        // Fox scent is active → sensitivity gate is 1.0.
        assert_eq!(a.species_sens, 1.0);
        // Role / injury / env stay at Phase 2A identity.
        assert_eq!(a.role_mod, 1.0);
        assert_eq!(a.injury_deficit, 0.0);
        assert_eq!(a.env_mul, 1.0);
    }

    #[test]
    fn faction_label_formats_readably() {
        assert_eq!(Faction::Neutral.label(), "neutral");
        assert_eq!(Faction::Colony.label(), "colony");
        assert_eq!(Faction::Observer.label(), "observer");
        let fox = Faction::Species(SensorySpecies::Wild(WildSpecies::Fox));
        assert!(fox.label().starts_with("species:"));
    }

    #[test]
    fn channel_labels_are_lowercase_slugs() {
        assert_eq!(channel_label(ChannelKind::Sight), "sight");
        assert_eq!(channel_label(ChannelKind::Hearing), "hearing");
        assert_eq!(channel_label(ChannelKind::Scent), "scent");
        assert_eq!(channel_label(ChannelKind::Tremor), "tremor");
    }

    // -----------------------------------------------------------------
    // Real-resource trait impls — "name what already exists"
    // -----------------------------------------------------------------

    #[test]
    fn fox_scent_map_implements_influence_map() {
        use crate::resources::FoxScentMap;
        let mut map = FoxScentMap::default_map();
        // Metadata: scent × fox-faction, named "fox_scent".
        let md = map.metadata();
        assert_eq!(md.name, "fox_scent");
        assert_eq!(md.channel, ChannelKind::Scent);
        match md.faction {
            Faction::Species(SensorySpecies::Wild(WildSpecies::Fox)) => {}
            other => panic!("expected fox faction, got {other:?}"),
        }
        // Base sample agrees with direct .get() call.
        let pos = Position::new(10, 10);
        assert_eq!(map.base_sample(pos), map.get(pos.x, pos.y));

        // Deposit a value and verify it surfaces via the trait.
        if let Some(i) = map.bucket_index(10, 10) {
            map.marks[i] = 0.42;
        }
        assert!((map.base_sample(pos) - 0.42).abs() < 1e-6);
    }

    #[test]
    fn cat_presence_map_implements_influence_map() {
        use crate::resources::CatPresenceMap;
        let map = CatPresenceMap::default();
        let md = map.metadata();
        assert_eq!(md.name, "congregation");
        assert_eq!(md.channel, ChannelKind::Sight);
        assert!(matches!(md.faction, Faction::Colony));
    }

    #[test]
    fn exploration_map_implements_influence_map() {
        use crate::resources::ExplorationMap;
        let map = ExplorationMap::default();
        let md = map.metadata();
        assert_eq!(md.name, "exploration");
        assert_eq!(md.channel, ChannelKind::Sight);
        assert!(matches!(md.faction, Faction::Observer));
    }

    #[test]
    fn ward_coverage_map_implements_influence_map() {
        use crate::resources::WardCoverageMap;
        let mut map = WardCoverageMap::default_map();
        let md = map.metadata();
        assert_eq!(md.name, "ward_coverage");
        assert_eq!(md.channel, ChannelKind::Sight);
        assert!(matches!(md.faction, Faction::Colony));

        // Stamp a ward and verify it surfaces via the trait.
        map.stamp_ward(20, 20, 1.0, 9.0);
        let sampled = map.base_sample(Position::new(22, 22));
        assert_eq!(sampled, map.get(22, 22));
        assert!(sampled > 0.0);
    }

    #[test]
    fn food_location_map_implements_influence_map() {
        use crate::resources::FoodLocationMap;
        let mut map = FoodLocationMap::default_map();
        let md = map.metadata();
        assert_eq!(md.name, "food_location");
        assert_eq!(md.channel, ChannelKind::Sight);
        assert!(matches!(md.faction, Faction::Colony));

        // Stamp a source and verify it surfaces via the trait.
        map.stamp(20, 20, 1.0, 12.0);
        let sampled = map.base_sample(Position::new(22, 22));
        assert_eq!(sampled, map.get(22, 22));
        assert!(sampled > 0.0);
    }

    #[test]
    fn garden_location_map_implements_influence_map() {
        use crate::resources::GardenLocationMap;
        let mut map = GardenLocationMap::default_map();
        let md = map.metadata();
        assert_eq!(md.name, "garden_location");
        assert_eq!(md.channel, ChannelKind::Sight);
        assert!(matches!(md.faction, Faction::Colony));

        map.stamp(20, 20, 1.0, 12.0);
        let sampled = map.base_sample(Position::new(22, 22));
        assert_eq!(sampled, map.get(22, 22));
        assert!(sampled > 0.0);
    }

    #[test]
    fn construction_site_map_implements_influence_map() {
        use crate::resources::ConstructionSiteMap;
        let mut map = ConstructionSiteMap::default_map();
        let md = map.metadata();
        assert_eq!(md.name, "construction_site");
        assert_eq!(md.channel, ChannelKind::Sight);
        assert!(matches!(md.faction, Faction::Colony));

        map.stamp(20, 20, 1.0, 12.0);
        let sampled = map.base_sample(Position::new(22, 22));
        assert_eq!(sampled, map.get(22, 22));
        assert!(sampled > 0.0);
    }

    #[test]
    fn kitten_cry_map_implements_influence_map() {
        use crate::resources::KittenCryMap;
        let mut map = KittenCryMap::default_map();
        let md = map.metadata();
        assert_eq!(md.name, "kitten_cry");
        assert_eq!(md.channel, ChannelKind::Hearing);
        assert!(matches!(md.faction, Faction::Colony));

        map.stamp(20, 20, 1.0, 10.0);
        let sampled = map.base_sample(Position::new(22, 22));
        assert_eq!(sampled, map.get(22, 22));
        assert!(sampled > 0.0);
    }

    #[test]
    fn corruption_lens_implements_influence_map() {
        use crate::resources::map::{Terrain, TileMap};
        let mut tiles = TileMap::new(10, 10, Terrain::Grass);
        // Inject a corrupted tile at (3, 4).
        tiles.get_mut(3, 4).corruption = 0.7;

        let lens = CorruptionLens(&tiles);
        let md = lens.metadata();
        assert_eq!(md.name, "corruption");
        assert_eq!(md.channel, ChannelKind::Sight);
        assert!(matches!(md.faction, Faction::Neutral));

        assert!((lens.base_sample(Position::new(3, 4)) - 0.7).abs() < 1e-6);
        assert_eq!(lens.base_sample(Position::new(0, 0)), 0.0);
        // Out-of-bounds returns 0.0.
        assert_eq!(lens.base_sample(Position::new(-1, 0)), 0.0);
        assert_eq!(lens.base_sample(Position::new(100, 100)), 0.0);
    }
}
