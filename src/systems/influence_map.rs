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
//! already in the codebase (`FoxScentMap`, `CatScentMap`,
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

use bevy_ecs::prelude::{Resource, World};

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
    /// the ToT belief layer; `cat_scent` (where cats mark) is
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
// InfluenceMapRegistry — Phase 2D registry walk (ticket 207)
// ---------------------------------------------------------------------------

/// Closure that fetches a single L1 sample for the focal cat: looks up
/// the target `Resource` in the world, calls `metadata()` and
/// `base_sample(pos)`, and returns the pair. `None` if the resource
/// isn't present (covers tests and resource-hot-swap edge cases —
/// matches the prior `Option<Res<>>` tolerance in the bundled
/// `L1Maps` SystemParam).
///
/// `Send + Sync + 'static` so the registry can live in a `Resource`.
/// The closures capture nothing — `register::<M>` monomorphizes
/// against the resource type, and `register_with` accepts a free
/// closure that owns no references.
pub type L1Walker =
    Box<dyn Fn(&World, Position) -> Option<(MapMetadata, f32)> + Send + Sync + 'static>;

/// Registry of every `InfluenceMap` whose L1 contributions the trace
/// emitter walks. Replaces the hand-bundled `L1Maps` SystemParam +
/// 12-call `emit_map!` walk in `src/systems/trace_emit.rs`.
///
/// Single source of truth: adding a 13th map is one
/// `registry.register::<M>()` call in
/// `populate_influence_map_registry` (`src/plugins/simulation.rs`),
/// with zero edits to `trace_emit.rs`. The
/// `scripts/check_influence_map_registry.sh` lint enforces that every
/// `impl InfluenceMap for X` in `src/` has a paired registration —
/// catches the 048 → 206 regression shape (impl lands but trace walk
/// not updated) at `just check` time instead of after a focal-cat
/// soak.
///
/// Borrow-adapter maps (e.g., `CorruptionLens` over `&TileMap`) can't
/// be registered via the generic `register::<M>` because they aren't
/// `Resource`s. Use `register_with` with a closure that constructs
/// the adapter inline.
#[derive(Resource, Default)]
pub struct InfluenceMapRegistry {
    walkers: Vec<L1Walker>,
}

impl InfluenceMapRegistry {
    /// Register a `Resource`-backed `InfluenceMap` impl. The walker
    /// monomorphizes to a direct `world.get_resource::<M>()` lookup;
    /// returns `None` if the resource isn't present (the trace
    /// emitter then skips that map for this tick).
    pub fn register<M>(&mut self)
    where
        M: InfluenceMap + Resource,
    {
        self.walkers.push(Box::new(|world, pos| {
            world
                .get_resource::<M>()
                .map(|m| (m.metadata(), m.base_sample(pos)))
        }));
    }

    /// Register a free closure walker. Use for borrow-adapter maps
    /// like `CorruptionLens` whose `InfluenceMap` impl borrows a
    /// `Resource` rather than being one (the lens borrows
    /// `&TileMap.corruption`), or for iterated registrations like
    /// per-species `PreyScentMaps` adapters in ticket 062.
    pub fn register_with<F>(&mut self, walker: F)
    where
        F: Fn(&World, Position) -> Option<(MapMetadata, f32)> + Send + Sync + 'static,
    {
        self.walkers.push(Box::new(walker));
    }

    /// Read-only iterator over registered walkers, for the trace
    /// emitter's L1 walk. Returns the slice rather than an iterator
    /// so callers can index it in tests; production callers iterate.
    pub fn walkers(&self) -> &[L1Walker] {
        &self.walkers
    }

    /// Number of registered walkers — convenience for tests and the
    /// one-shot startup audit (`just check` lint counts impls and
    /// compares).
    pub fn len(&self) -> usize {
        self.walkers.len()
    }

    /// `true` if no walkers have been registered. Default-constructed
    /// registries are empty until `populate_influence_map_registry`
    /// runs at startup.
    pub fn is_empty(&self) -> bool {
        self.walkers.is_empty()
    }
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

/// Borrow-based adapter that exposes one `PreyScentMap` sub-map (from
/// `PreyScentMaps`) as an `InfluenceMap` with per-species metadata.
///
/// Per §5.6.3 row #5 (ticket 062), prey scent is no longer a single
/// aggregate channel — each `PreyKind` carries its own sub-map and L1
/// trace key (`prey_scent_mouse`, `prey_scent_rat`, …).
///
/// **Phase 3 readiness hook.** The
/// `Faction::Species(SensorySpecies::Prey(kind))` tag lets the
/// attenuation pipeline identify which emitter species produced this
/// map's signal. `species_sensitivity` returns a binary gate today
/// (Phase 2A decision); Phase 3+ can apply a per-emitter-species signal
/// modifier via this faction tag without changing this type's
/// interface — observer-side dietary specialization and per-species
/// scent-detect threshold tuning both key on this tag.
pub struct PerSpeciesScentRef<'a>(
    pub &'a crate::resources::PreyScentMap,
    pub crate::components::prey::PreyKind,
);

impl InfluenceMap for PerSpeciesScentRef<'_> {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            name: crate::resources::scent_map_name(self.1),
            channel: ChannelKind::Scent,
            faction: Faction::Species(SensorySpecies::Prey(self.1)),
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.0.get(pos.x, pos.y)
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

impl InfluenceMap for crate::resources::CoverAvailabilityMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // Ticket 423: cover availability is a visual terrain
            // signal — cats see low-cover tiles within sprint range.
            // Sight × Neutral mirrors the framing of terrain-property
            // maps (`ExplorationMap` uses Sight × Observer; cover is
            // colony-wide-identical so Neutral fits).
            name: "cover_availability",
            channel: ChannelKind::Sight,
            faction: Faction::Neutral,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::FoxApproachCorridorMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // 312: per-tile traffic accumulator over observed ShadowFox
            // patrol movement. Tagged Sight × Neutral matching
            // RecentAmbushMap — the substrate is faction-agnostic
            // perception (the *colony* reads where foxes traverse), not
            // a species-aligned scent channel. ShadowFox-only feed
            // today; generalizes to other patrolling predators
            // without metadata churn.
            name: "fox_approach_corridor",
            channel: ChannelKind::Sight,
            faction: Faction::Neutral,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::RecentAmbushMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // 219: colony-shared spatial memory of recent ambush
            // events. Event-typed not species-typed — today only
            // ShadowFoxes feed it, but the substrate generalizes to
            // Hawks / Snakes without metadata churn. Tagged
            // Sight × Neutral matching CarcassScentMap / CorruptionLens
            // — no faction allegiance, no scent channel (the event
            // is a memory of hostile presence, not a fresh trail).
            // Folds into `Memory.LocationModel.last_threat` when
            // ToT cluster C3 (ticket 007) lands.
            name: "recent_ambush",
            channel: ChannelKind::Sight,
            faction: Faction::Neutral,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::CatScentMap {
    fn metadata(&self) -> MapMetadata {
        // 260: re-tagged from ("congregation", Sight) to ("cat_scent",
        // Scent). Cats deposit a steady-state scent every tick plus a
        // patrol/fight/explore bonus (see `cat_scent_tick` in
        // disposition.rs). Foxes route around high-scent tiles
        // (`shadow_fox_cat_scent_avoid` branch in wildlife.rs) —
        // distinct from `CatPatrolDeterrentMap` (Sight × Colony, only
        // active-patrol deposit) and `WardCoverageMap` (Sight × Colony,
        // ward-radiation gradient).
        MapMetadata {
            name: "cat_scent",
            channel: ChannelKind::Scent,
            faction: Faction::Colony,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::CatPatrolDeterrentMap {
    fn metadata(&self) -> MapMetadata {
        // 256 R5: patrol-presence as a deterrent gradient. Channel
        // is Sight (foxes see active patrols, not infer them by
        // scent). Faction Colony — the deterrent originates from
        // colony cats.
        MapMetadata {
            name: "cat_patrol_deterrent",
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

impl InfluenceMap for crate::resources::WardIntentMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // 301: coordinator-stamped ward-placement intent.
            // Sight × Colony matching the WardCoverageMap convention —
            // the field is a colony-faction directive surface that
            // cats read at score-time.
            name: "ward_intent",
            channel: ChannelKind::Sight,
            faction: Faction::Colony,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::GraveAuraMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // 035: small anti-corruption aura around buried graves.
            // Tagged Sight × Colony following the WardCoverageMap
            // convention (no spatial-independent channel).
            name: "grave_aura",
            channel: ChannelKind::Sight,
            faction: Faction::Colony,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::ColonyDistrictMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // 382: "where the colony wants to grow" composite —
            // frontier minus crowding minus threat. Sight × Colony
            // following the WardCoverageMap convention; trace
            // emitters surface the composite scalar, the placement
            // scorer reads per-axis getters directly for per-kind
            // weighting.
            name: "colony_district",
            channel: ChannelKind::Sight,
            faction: Faction::Colony,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.composite(pos.x, pos.y)
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
// 101: env-quality influence maps
// ---------------------------------------------------------------------------

impl InfluenceMap for crate::resources::ComfortMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // 101: terrain ease + building proximity + weather overlay.
            // Sight × Neutral matches `CorruptionLens` — there's no
            // "ambient quality" channel today; future scope may add one
            // if the distinction becomes load-bearing.
            name: "env_comfort",
            channel: ChannelKind::Sight,
            faction: Faction::Neutral,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::CleanlinessMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            name: "env_cleanliness",
            channel: ChannelKind::Sight,
            faction: Faction::Neutral,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::BeautyMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            name: "env_beauty",
            channel: ChannelKind::Sight,
            faction: Faction::Neutral,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::MysteryMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            name: "env_mystery",
            channel: ChannelKind::Sight,
            faction: Faction::Neutral,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
    }
}

impl InfluenceMap for crate::resources::CorruptionInfluenceMap {
    fn metadata(&self) -> MapMetadata {
        MapMetadata {
            // 101: stamped corruption gradient (perception). Distinct
            // from `CorruptionLens` which exposes the raw on-tile field
            // as "corruption" — this is the radially-stamped influence
            // version cats sample to perceive the gradient *before*
            // stepping onto a hot tile.
            name: "env_corruption",
            channel: ChannelKind::Sight,
            faction: Faction::Neutral,
        }
    }

    fn base_sample(&self, pos: Position) -> f32 {
        self.get(pos.x, pos.y)
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
    fn recent_ambush_map_implements_influence_map() {
        use crate::resources::RecentAmbushMap;
        let mut map = RecentAmbushMap::default_map();
        let md = map.metadata();
        assert_eq!(md.name, "recent_ambush");
        assert_eq!(md.channel, ChannelKind::Sight);
        assert!(matches!(md.faction, Faction::Neutral));

        // Sample agrees with direct .get() and surfaces a deposit.
        let pos = Position::new(10, 10);
        assert_eq!(map.base_sample(pos), map.get(pos.x, pos.y));
        map.deposit(10, 10, 1.0);
        assert!((map.base_sample(pos) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cat_scent_map_implements_influence_map() {
        use crate::resources::CatScentMap;
        let map = CatScentMap::default();
        let md = map.metadata();
        assert_eq!(md.name, "cat_scent");
        assert_eq!(md.channel, ChannelKind::Scent);
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
    fn registry_register_walks_resource_maps() {
        use crate::resources::FoxScentMap;
        use bevy_ecs::prelude::World;

        let mut world = World::new();
        let mut map = FoxScentMap::default_map();
        if let Some(i) = map.bucket_index(5, 5) {
            map.marks[i] = 0.7;
        }
        world.insert_resource(map);

        let mut registry = InfluenceMapRegistry::default();
        registry.register::<FoxScentMap>();
        assert_eq!(registry.len(), 1);

        // Walker fetches the resource and surfaces metadata + sample.
        let walker = &registry.walkers()[0];
        let (md, sample) = walker(&world, Position::new(5, 5)).expect("resource present");
        assert_eq!(md.name, "fox_scent");
        assert!((sample - 0.7).abs() < 1e-6);

        // Walker returns None when the resource is absent.
        let empty = World::new();
        assert!(walker(&empty, Position::new(0, 0)).is_none());
    }

    #[test]
    fn registry_register_with_handles_borrow_adapter() {
        use crate::resources::map::{Terrain, TileMap};
        use bevy_ecs::prelude::World;

        let mut world = World::new();
        let mut tiles = TileMap::new(10, 10, Terrain::Grass);
        tiles.get_mut(3, 4).corruption = 0.42;
        world.insert_resource(tiles);

        let mut registry = InfluenceMapRegistry::default();
        registry.register_with(|world, pos| {
            world.get_resource::<TileMap>().map(|t| {
                let lens = CorruptionLens(t);
                (lens.metadata(), lens.base_sample(pos))
            })
        });
        assert_eq!(registry.len(), 1);

        let walker = &registry.walkers()[0];
        let (md, sample) = walker(&world, Position::new(3, 4)).expect("tilemap present");
        assert_eq!(md.name, "corruption");
        assert!((sample - 0.42).abs() < 1e-6);
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

    #[test]
    fn prey_scent_test_per_species_scent_ref_metadata() {
        use crate::components::prey::PreyKind;
        use crate::resources::PreyScentMap;
        let map = PreyScentMap::new(10, 10, 1);
        let cases: [(PreyKind, &str); 5] = [
            (PreyKind::Mouse, "prey_scent_mouse"),
            (PreyKind::Rat, "prey_scent_rat"),
            (PreyKind::Rabbit, "prey_scent_rabbit"),
            (PreyKind::Fish, "prey_scent_fish"),
            (PreyKind::Bird, "prey_scent_bird"),
        ];
        for (kind, expected_name) in cases {
            let adapter = PerSpeciesScentRef(&map, kind);
            let md = adapter.metadata();
            assert_eq!(md.name, expected_name);
            assert_eq!(md.channel, ChannelKind::Scent);
            match md.faction {
                Faction::Species(SensorySpecies::Prey(k)) => assert_eq!(k, kind),
                other => panic!(
                    "expected Faction::Species(SensorySpecies::Prey({:?})), got {:?}",
                    kind, other
                ),
            }
        }
    }
}
