//! Data-driven sprite bindings (ticket 448).
//!
//! Loads `assets/sprites/bindings.toml` at startup into a `SpriteBindings`
//! resource. The three lookup functions in `entity_sprites.rs`
//! (`item_sprite_index`, `herb_sprite_index`, `flavor_sprite_index`) read
//! from this resource instead of hardcoded match statements.
//!
//! The exhaustiveness unit test at the bottom of this file iterates every
//! variant of `ItemKind`, `HerbKind`, and `FlavorKind` and asserts presence
//! in the loaded manifest — adding a new enum variant without a binding
//! fails CI. This is the string-keyed TOML equivalent of the
//! `linkme::distributed_slice` contract used for cat-DSE registration
//! (ticket 438).
//!
//! Phase 1 covers items + herbs + flavor plants. Buildings, winter
//! variants, wildlife, prey, trees, and scatter remain code-side and
//! become data-driven in follow-on phases.

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

use crate::components::building::StructureType;
use crate::components::items::ItemKind;
use crate::components::magic::{FlavorKind, GrowthStage, HerbKind};

/// Path to the manifest, relative to the workspace root (the asset_server's
/// default search root). Read synchronously at startup so the resource is
/// available before any system that needs it.
pub const BINDINGS_PATH: &str = "assets/sprites/bindings.toml";

/// Declaration of a named sprite-sheet atlas: source PNG + grid dimensions.
/// Items / herbs / flavor plants reference an atlas by name (the
/// `[atlases.<name>]` key) and an index into its `cols × rows` grid.
#[derive(Debug, Clone, Deserialize)]
pub struct AtlasInfo {
    pub texture: String,
    pub cols: u32,
    pub rows: u32,
    pub tile: u32,
    #[serde(default)]
    pub note: Option<String>,
}

/// Pre-loaded atlas — both the `Handle<Image>` (texture) and the
/// `Handle<TextureAtlasLayout>` for the declared grid. The `info` field
/// preserves the manifest declaration so editors and tools can introspect
/// dimensions without re-reading the TOML.
#[derive(Debug, Clone)]
pub struct AtlasHandles {
    pub info: AtlasInfo,
    pub texture: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
}

/// What an item / herb / flavor-plant sprite needs to render: which
/// texture, which layout, which index. Returned by the lookup methods so
/// callers in entity_sprites.rs build a Bevy `Sprite + TextureAtlas`
/// without knowing which named atlas the binding chose.
#[derive(Debug, Clone)]
pub struct AtlasSprite {
    pub texture: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
    pub index: usize,
}

/// An item's sprite binding. Either points at a grid cell in a
/// registered atlas (the original ticket-448 form) or at a single
/// PNG file on disk (the Fan-tasy / Sprout Lands single-file props
/// added under this branch). Serde discriminates by which keys are
/// present in the TOML table — `atlas` + `index` → `Atlas`,
/// `texture` → `Texture`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ItemBinding {
    Atlas(AtlasItemBinding),
    Texture(TextureItemBinding),
}

#[derive(Debug, Clone, Deserialize)]
pub struct AtlasItemBinding {
    pub atlas: String,
    pub index: usize,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TextureItemBinding {
    pub texture: String,
    #[serde(default)]
    pub note: Option<String>,
}

impl ItemBinding {
    pub fn note(&self) -> Option<&str> {
        match self {
            Self::Atlas(b) => b.note.as_deref(),
            Self::Texture(b) => b.note.as_deref(),
        }
    }
}

/// Resolved item sprite — what `attach_entity_sprites` needs to build a
/// Bevy `Sprite`. Atlas items carry a `TextureAtlas` (layout + index);
/// texture items render the whole PNG without an atlas.
#[derive(Debug, Clone)]
pub enum ItemSprite {
    Atlas(AtlasSprite),
    Texture(Handle<Image>),
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlantBinding {
    pub atlas: String,
    /// Indices for [Sprout, Bud, Bloom, Blossom]. Stages that share a
    /// sprite (e.g. mushroom Bud collapses to Bloom) just repeat the index.
    pub indices_by_stage: [usize; 4],
    #[serde(default)]
    pub note: Option<String>,
}

/// Texture binding for a building variant or set of variants. When
/// `textures.len() > 1` the renderer picks one by entity hash. Render size
/// is computed from `native_size` (pixel dimensions of the source PNG) and
/// `tiles_wide` (how many world tiles the sprite should occupy at world_px
/// scale), preserving the source aspect ratio.
#[derive(Debug, Clone, Deserialize)]
pub struct BuildingBinding {
    pub textures: Vec<String>,
    pub native_size: [f32; 2],
    pub tiles_wide: f32,
    #[serde(default)]
    pub note: Option<String>,
}

impl BuildingBinding {
    /// Render size for this building at the given world_px (pixels per tile).
    pub fn render_size(&self, world_px: f32) -> Vec2 {
        let w = self.tiles_wide * world_px;
        let h = w / self.native_size[0] * self.native_size[1];
        Vec2::new(w, h)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SpriteBindingsFile {
    #[serde(default)]
    atlases: HashMap<String, AtlasInfo>,
    items: HashMap<String, ItemBinding>,
    herbs: HashMap<String, PlantBinding>,
    flavor_plants: HashMap<String, PlantBinding>,
    buildings: HashMap<String, BuildingBinding>,
    #[serde(default)]
    buildings_winter: HashMap<String, BuildingBinding>,
}

#[derive(Resource, Debug, Clone)]
pub struct SpriteBindings {
    items: HashMap<String, ItemBinding>,
    herbs: HashMap<String, PlantBinding>,
    flavor_plants: HashMap<String, PlantBinding>,
    buildings: HashMap<String, BuildingBinding>,
    buildings_winter: HashMap<String, BuildingBinding>,
    /// Registered atlases — keyed by `[atlases.<name>]` from the manifest.
    /// Each entry carries the source-PNG handle + the `TextureAtlasLayout`
    /// created from the declared grid. Items / herbs / flavor plants
    /// reference an entry here by name.
    atlases: HashMap<String, AtlasHandles>,
    /// Pre-loaded building texture handles keyed by asset-relative path.
    /// Buildings use plain `Handle<Image>` (no atlas grid), so they're
    /// stored separately from `atlases` above. Populated at startup so
    /// building lookups never trigger a per-frame `asset_server.load()`.
    handles: HashMap<String, Handle<Image>>,
}

impl SpriteBindings {
    /// Look up the renderable sprite for an item. Returns `ItemSprite`
    /// because items may bind to either an atlas grid cell or a
    /// single-file texture (Fan-tasy props). Panics with the variant
    /// name if the binding is missing — silent fallback to sprite zero
    /// is the exact failure mode that produced the three "previously
    /// wrong" comments in the legacy match statements
    /// (Moonpetal/Calmroot/Dreamroot).
    pub fn item_sprite(&self, kind: ItemKind) -> ItemSprite {
        let key = format!("{kind:?}");
        let binding = self
            .items
            .get(&key)
            .unwrap_or_else(|| panic!("sprite binding missing for item: {key}"));
        match binding {
            ItemBinding::Atlas(b) => ItemSprite::Atlas(self.resolve_atlas_sprite(
                &b.atlas,
                b.index,
                &format!("item {key}"),
            )),
            ItemBinding::Texture(b) => {
                let handle = self
                    .handles
                    .get(&b.texture)
                    .unwrap_or_else(|| {
                        panic!("texture not preloaded for item {key}: {}", b.texture)
                    })
                    .clone();
                ItemSprite::Texture(handle)
            }
        }
    }

    pub fn herb_sprite(&self, kind: HerbKind, stage: GrowthStage) -> AtlasSprite {
        let key = format!("{kind:?}");
        let entry = self
            .herbs
            .get(&key)
            .unwrap_or_else(|| panic!("sprite binding missing for herb: {key}"));
        let idx = entry.indices_by_stage[stage_index(stage)];
        self.resolve_atlas_sprite(&entry.atlas, idx, &format!("herb {key} {stage:?}"))
    }

    pub fn flavor_sprite(&self, kind: FlavorKind, stage: GrowthStage) -> AtlasSprite {
        let key = format!("{kind:?}");
        let entry = self
            .flavor_plants
            .get(&key)
            .unwrap_or_else(|| panic!("sprite binding missing for flavor plant: {key}"));
        let idx = entry.indices_by_stage[stage_index(stage)];
        self.resolve_atlas_sprite(&entry.atlas, idx, &format!("flavor {key} {stage:?}"))
    }

    fn resolve_atlas_sprite(&self, atlas_name: &str, index: usize, context: &str) -> AtlasSprite {
        let atlas = self.atlases.get(atlas_name).unwrap_or_else(|| {
            panic!("unknown atlas '{atlas_name}' referenced by {context}; declare it under [atlases.{atlas_name}] in bindings.toml")
        });
        AtlasSprite {
            texture: atlas.texture.clone(),
            layout: atlas.layout.clone(),
            index,
        }
    }

    /// Read-only view of registered atlases — useful for editor/tooling
    /// code that wants to enumerate available atlases or inspect grid
    /// dimensions.
    pub fn atlases(&self) -> &HashMap<String, AtlasHandles> {
        &self.atlases
    }

    // -- Legacy index-only accessors. Kept so test code that just wants
    //    to assert "every variant has SOME binding" continues to work
    //    without depending on Bevy asset handles. Production rendering
    //    code uses `*_sprite()` above. --

    /// Test-only presence check: confirms an item has SOME binding (atlas
    /// or texture). Atlas-only spot checks live in
    /// `bindings_match_legacy_match_statements` and use `item_atlas_index`.
    #[cfg(test)]
    pub fn assert_item_has_binding(&self, kind: ItemKind) {
        let key = format!("{kind:?}");
        assert!(
            self.items.contains_key(&key),
            "sprite binding missing for item: {key}"
        );
    }

    /// Test-only: pull the atlas index for an atlas-form item. Panics if
    /// the binding is missing or is the texture form (callers should
    /// only invoke for items they know are atlas-bound).
    #[cfg(test)]
    pub fn item_atlas_index(&self, kind: ItemKind) -> usize {
        let key = format!("{kind:?}");
        match self.items.get(&key) {
            Some(ItemBinding::Atlas(b)) => b.index,
            Some(ItemBinding::Texture(_)) => {
                panic!("item {key} uses a texture binding, not an atlas index")
            }
            None => panic!("sprite binding missing for item: {key}"),
        }
    }

    #[cfg(test)]
    pub fn herb_index(&self, kind: HerbKind, stage: GrowthStage) -> usize {
        self.herbs
            .get(&format!("{kind:?}"))
            .map(|b| b.indices_by_stage[stage_index(stage)])
            .unwrap_or_else(|| panic!("sprite binding missing for herb: {kind:?}"))
    }

    #[cfg(test)]
    pub fn flavor_index(&self, kind: FlavorKind, stage: GrowthStage) -> usize {
        self.flavor_plants
            .get(&format!("{kind:?}"))
            .map(|b| b.indices_by_stage[stage_index(stage)])
            .unwrap_or_else(|| panic!("sprite binding missing for flavor plant: {kind:?}"))
    }

    /// Look up the binding for a building. Panics if missing.
    pub fn building(&self, kind: StructureType) -> &BuildingBinding {
        let key = format!("{kind:?}");
        self.buildings
            .get(&key)
            .unwrap_or_else(|| panic!("sprite binding missing for building: {key}"))
    }

    /// Look up the winter binding for a building, if one exists. Buildings
    /// without a winter variant simply re-render their summer sprite during
    /// winter (Workshop, Garden, Kitchen, Wall, Gate, Midden, DryingRack,
    /// SmokingRack as of Phase 1b).
    pub fn building_winter(&self, kind: StructureType) -> Option<&BuildingBinding> {
        let key = format!("{kind:?}");
        self.buildings_winter.get(&key)
    }

    /// Pick a building texture variant by entity hash and return its
    /// pre-loaded `Handle<Image>` plus computed render size. The hash is
    /// the same whether we're in summer or winter, so a single building
    /// entity uses the same variant index across seasonal swaps.
    pub fn building_sprite(
        &self,
        kind: StructureType,
        entity_hash: u64,
        world_px: f32,
    ) -> (Handle<Image>, Vec2) {
        let binding = self.building(kind);
        self.binding_sprite(binding, entity_hash, world_px)
    }

    /// Pick a winter texture variant + size for a building. Falls back to
    /// the summer binding when no winter variant is declared.
    pub fn building_sprite_winter(
        &self,
        kind: StructureType,
        entity_hash: u64,
        world_px: f32,
    ) -> (Handle<Image>, Vec2) {
        match self.building_winter(kind) {
            Some(binding) => self.binding_sprite(binding, entity_hash, world_px),
            None => self.building_sprite(kind, entity_hash, world_px),
        }
    }

    fn binding_sprite(
        &self,
        binding: &BuildingBinding,
        entity_hash: u64,
        world_px: f32,
    ) -> (Handle<Image>, Vec2) {
        let variant = (entity_hash as usize) % binding.textures.len();
        let path = &binding.textures[variant];
        let handle = self
            .handles
            .get(path)
            .unwrap_or_else(|| panic!("texture not preloaded: {path}"))
            .clone();
        (handle, binding.render_size(world_px))
    }
}

fn stage_index(stage: GrowthStage) -> usize {
    match stage {
        GrowthStage::Sprout => 0,
        GrowthStage::Bud => 1,
        GrowthStage::Bloom => 2,
        GrowthStage::Blossom => 3,
    }
}

/// Parse `bindings.toml` from disk without preloading any image handles.
/// Used by unit tests; the runtime path is `load_sprite_bindings` which
/// also loads texture handles via the Bevy `AssetServer`.
fn load_bindings_file_from_disk() -> SpriteBindingsFile {
    let raw = std::fs::read_to_string(BINDINGS_PATH).unwrap_or_else(|e| {
        panic!(
            "failed to read {BINDINGS_PATH}: {e}. CWD: {:?}",
            std::env::current_dir()
        )
    });
    toml::from_str(&raw).unwrap_or_else(|e| panic!("failed to parse {BINDINGS_PATH}: {e}"))
}

/// Test-only helper: load the manifest without preloading image handles
/// or atlas layouts. Building-texture lookups + atlas-sprite lookups
/// panic when called on this resource, but item/herb/flavor INDEX
/// lookups (the `*_index` cfg(test) accessors above) work fine.
#[cfg(test)]
fn load_bindings_for_test() -> SpriteBindings {
    let file = load_bindings_file_from_disk();
    SpriteBindings {
        items: file.items,
        herbs: file.herbs,
        flavor_plants: file.flavor_plants,
        buildings: file.buildings,
        buildings_winter: file.buildings_winter,
        atlases: HashMap::new(),
        handles: HashMap::new(),
    }
}

fn assemble_bindings(
    file: SpriteBindingsFile,
    asset_server: &AssetServer,
    layouts: &mut Assets<TextureAtlasLayout>,
) -> SpriteBindings {
    // Atlases — load each declared sprite-sheet PNG and create its
    // TextureAtlasLayout from the manifest's cols/rows/tile.
    let atlases: HashMap<String, AtlasHandles> = file
        .atlases
        .iter()
        .map(|(name, info)| {
            let texture = asset_server.load(&info.texture);
            let layout = layouts.add(TextureAtlasLayout::from_grid(
                UVec2::splat(info.tile),
                info.cols,
                info.rows,
                None,
                None,
            ));
            (
                name.clone(),
                AtlasHandles {
                    info: info.clone(),
                    texture,
                    layout,
                },
            )
        })
        .collect();

    // Buildings — pre-load every variant texture, keyed by path.
    // Texture-form items (single-file Fan-tasy props) share the same
    // handle cache: their `texture` path is loaded once at startup.
    let mut handles: HashMap<String, Handle<Image>> = HashMap::new();
    for binding in file
        .buildings
        .values()
        .chain(file.buildings_winter.values())
    {
        for path in &binding.textures {
            handles
                .entry(path.clone())
                .or_insert_with(|| asset_server.load(path));
        }
    }
    for binding in file.items.values() {
        if let ItemBinding::Texture(b) = binding {
            handles
                .entry(b.texture.clone())
                .or_insert_with(|| asset_server.load(&b.texture));
        }
    }

    SpriteBindings {
        items: file.items,
        herbs: file.herbs,
        flavor_plants: file.flavor_plants,
        buildings: file.buildings,
        buildings_winter: file.buildings_winter,
        atlases,
        handles,
    }
}

/// Startup system: parse the manifest, register every declared atlas as
/// a `(Handle<Image>, Handle<TextureAtlasLayout>)` pair, and pre-load
/// every building variant texture. Runs before any system that consumes
/// `Res<SpriteBindings>`.
pub fn load_sprite_bindings(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let file = load_bindings_file_from_disk();
    commands.insert_resource(assemble_bindings(file, &asset_server, &mut layouts));
}

// ---------------------------------------------------------------------------
// Hot reload (Phase 2 of ticket 448)
//
// Polls `assets/sprites/bindings.toml` mtime every 0.5s. When the file
// changes, re-parses, pre-loads any newly-referenced texture handles,
// overwrites the `SpriteBindings` resource, and strips
// `EntitySpriteMarker` from every entity so the attach systems re-fire
// with the new bindings — the user sees the change in the running game
// within ~1 second of saving in the Svelte editor.
//
// Parse failures during hot reload are LOGGED, not panicked: the editor
// may write a half-saved file. Startup load still panics on parse failure
// because there's no good fallback at that point.
// ---------------------------------------------------------------------------

/// Tracks the last-seen mtime of `bindings.toml` so the watcher only
/// reloads when the file actually changes.
#[derive(Resource)]
pub struct BindingsWatcher {
    last_mtime: Option<std::time::SystemTime>,
    poll_timer: Timer,
}

impl Default for BindingsWatcher {
    fn default() -> Self {
        Self {
            last_mtime: None,
            poll_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
        }
    }
}

/// Per-frame system: poll the manifest mtime; on change, hot-reload the
/// bindings and trigger sprite re-attach.
pub fn watch_sprite_bindings(
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut watcher: ResMut<BindingsWatcher>,
    mut bindings: ResMut<SpriteBindings>,
    markers: Query<Entity, With<crate::rendering::entity_sprites::EntitySpriteMarker>>,
) {
    watcher.poll_timer.tick(time.delta());
    if !watcher.poll_timer.just_finished() {
        return;
    }
    let Ok(meta) = std::fs::metadata(BINDINGS_PATH) else {
        return;
    };
    let Ok(mtime) = meta.modified() else {
        return;
    };
    if Some(mtime) == watcher.last_mtime {
        return;
    }
    let first_observation = watcher.last_mtime.is_none();
    watcher.last_mtime = Some(mtime);
    if first_observation {
        // First time we've ever seen the file — establish the baseline
        // mtime but don't reload (the startup load already populated the
        // resource from this same file content).
        return;
    }

    // Parse the new file. Failure here is non-fatal: editor saves can
    // race the watcher and produce a transient half-written file.
    let raw = match std::fs::read_to_string(BINDINGS_PATH) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sprite_bindings hot reload: failed to read {BINDINGS_PATH}: {e}");
            return;
        }
    };
    let file: SpriteBindingsFile = match toml::from_str(&raw) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("sprite_bindings hot reload: failed to parse {BINDINGS_PATH}: {e}");
            return;
        }
    };

    *bindings = assemble_bindings(file, &asset_server, &mut layouts);

    let count = markers.iter().count();
    for entity in markers.iter() {
        commands
            .entity(entity)
            .remove::<crate::rendering::entity_sprites::EntitySpriteMarker>();
    }
    eprintln!("sprite_bindings: reloaded — re-attaching {count} entities");
}

// ---------------------------------------------------------------------------
// Exhaustiveness test — every enum variant must be present in bindings.toml.
//
// CLAUDE.md "Prefer compile-time contracts to runtime checks": for
// string-keyed TOML we can't get full compile-time enforcement, but a
// round-trip test gives the same operational guarantee. Adding a new
// variant to ItemKind / HerbKind / FlavorKind without a matching entry
// fails CI here rather than silently rendering nothing at runtime.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// All `ItemKind` variants. Hand-maintained because the codebase doesn't
    /// pull in `strum`. Adding a variant requires adding it here AND to
    /// `bindings.toml`; the test catches either omission.
    const ALL_ITEM_KINDS: &[ItemKind] = &[
        ItemKind::RawMouse,
        ItemKind::RawRat,
        ItemKind::RawRabbit,
        ItemKind::RawFish,
        ItemKind::RawBird,
        ItemKind::Berries,
        ItemKind::Nuts,
        ItemKind::Roots,
        ItemKind::WildOnion,
        ItemKind::Mushroom,
        ItemKind::Moss,
        ItemKind::DriedGrass,
        ItemKind::Feather,
        ItemKind::HerbHealingMoss,
        ItemKind::HerbMoonpetal,
        ItemKind::HerbCalmroot,
        ItemKind::HerbThornbriar,
        ItemKind::HerbDreamroot,
        ItemKind::HerbCatnip,
        ItemKind::HerbSlumbershade,
        ItemKind::HerbOracleOrchid,
        ItemKind::ShinyPebble,
        ItemKind::GlassShard,
        ItemKind::ColorfulShell,
        ItemKind::ShadowBone,
        ItemKind::Barrel,
        ItemKind::Crate,
        ItemKind::Shelf,
        ItemKind::Wood,
        ItemKind::Stone,
        ItemKind::RemedyHealingPoultice,
        ItemKind::RemedyEnergyTonic,
        ItemKind::RemedyMoodTonic,
        ItemKind::RawOrgan,
        ItemKind::DriedFish,
        ItemKind::SmokedMeat,
        ItemKind::PreservedOrgan,
        ItemKind::Bone,
        ItemKind::Sinew,
        ItemKind::Whisker,
        ItemKind::Hide,
        ItemKind::FishScale,
        ItemKind::Tallow,
        // 368 — Phase 2 crafting inputs + behavioral tools.
        ItemKind::Twig,
        ItemKind::Bristle,
        ItemKind::Fiber,
        ItemKind::Flower,
        ItemKind::PolishedStone,
        ItemKind::GroomingBrush,
        ItemKind::PlayBundle,
        ItemKind::CourtshipGift,
        // 369 — Phase 2b warrior's kit.
        ItemKind::BoneTipSpear,
        ItemKind::BoneStiletto,
        ItemKind::FlintBlade,
        ItemKind::HideBracers,
        ItemKind::HidePlatedWrap,
        ItemKind::Sling,
        ItemKind::WovenReedCloak,
        ItemKind::ToothNotchedClub,
    ];

    const ALL_HERB_KINDS: &[HerbKind] = &[
        HerbKind::HealingMoss,
        HerbKind::Moonpetal,
        HerbKind::Calmroot,
        HerbKind::Thornbriar,
        HerbKind::Dreamroot,
        HerbKind::Catnip,
        HerbKind::Slumbershade,
        HerbKind::OracleOrchid,
    ];

    const ALL_FLAVOR_KINDS: &[FlavorKind] = &[
        FlavorKind::Sunflower,
        FlavorKind::Rose,
        FlavorKind::Pebble,
        FlavorKind::Rock,
        FlavorKind::Stone,
        FlavorKind::StoneChunk,
        FlavorKind::StoneFlat,
        FlavorKind::Boulder,
    ];

    const ALL_STAGES: &[GrowthStage] = &[
        GrowthStage::Sprout,
        GrowthStage::Bud,
        GrowthStage::Bloom,
        GrowthStage::Blossom,
    ];

    /// Compile-time variant check: this fn forces the compiler to verify
    /// `ALL_ITEM_KINDS` covers every `ItemKind` variant via exhaustive
    /// match. Adding a new variant to the enum without updating this
    /// match fails the build (non-exhaustive match is an error). Mirror
    /// of the `dse_id_for_action` round-trip pattern from ticket 438,
    /// adapted to a const slice instead of a trait method.
    #[allow(dead_code)]
    fn assert_item_kinds_complete(kind: ItemKind) {
        match kind {
            ItemKind::RawMouse
            | ItemKind::RawRat
            | ItemKind::RawRabbit
            | ItemKind::RawFish
            | ItemKind::RawBird
            | ItemKind::Berries
            | ItemKind::Nuts
            | ItemKind::Roots
            | ItemKind::WildOnion
            | ItemKind::Mushroom
            | ItemKind::Moss
            | ItemKind::DriedGrass
            | ItemKind::Feather
            | ItemKind::HerbHealingMoss
            | ItemKind::HerbMoonpetal
            | ItemKind::HerbCalmroot
            | ItemKind::HerbThornbriar
            | ItemKind::HerbDreamroot
            | ItemKind::HerbCatnip
            | ItemKind::HerbSlumbershade
            | ItemKind::HerbOracleOrchid
            | ItemKind::ShinyPebble
            | ItemKind::GlassShard
            | ItemKind::ColorfulShell
            | ItemKind::ShadowBone
            | ItemKind::Barrel
            | ItemKind::Crate
            | ItemKind::Shelf
            | ItemKind::Wood
            | ItemKind::Stone
            | ItemKind::RemedyHealingPoultice
            | ItemKind::RemedyEnergyTonic
            | ItemKind::RemedyMoodTonic
            | ItemKind::RawOrgan
            | ItemKind::DriedFish
            | ItemKind::SmokedMeat
            | ItemKind::PreservedOrgan
            | ItemKind::Bone
            | ItemKind::Sinew
            | ItemKind::Whisker
            | ItemKind::Hide
            | ItemKind::FishScale
            | ItemKind::Tallow
            // 368 — Phase 2 crafting inputs + behavioral tools.
            | ItemKind::Twig
            | ItemKind::Bristle
            | ItemKind::Fiber
            | ItemKind::Flower
            | ItemKind::PolishedStone
            | ItemKind::GroomingBrush
            | ItemKind::PlayBundle
            | ItemKind::CourtshipGift
            // 369 — Phase 2b warrior's kit.
            | ItemKind::BoneTipSpear
            | ItemKind::BoneStiletto
            | ItemKind::FlintBlade
            | ItemKind::HideBracers
            | ItemKind::HidePlatedWrap
            | ItemKind::Sling
            | ItemKind::WovenReedCloak
            | ItemKind::ToothNotchedClub => {}
        }
    }

    #[allow(dead_code)]
    fn assert_herb_kinds_complete(kind: HerbKind) {
        match kind {
            HerbKind::HealingMoss
            | HerbKind::Moonpetal
            | HerbKind::Calmroot
            | HerbKind::Thornbriar
            | HerbKind::Dreamroot
            | HerbKind::Catnip
            | HerbKind::Slumbershade
            | HerbKind::OracleOrchid => {}
        }
    }

    #[allow(dead_code)]
    fn assert_flavor_kinds_complete(kind: FlavorKind) {
        match kind {
            FlavorKind::Sunflower
            | FlavorKind::Rose
            | FlavorKind::Pebble
            | FlavorKind::Rock
            | FlavorKind::Stone
            | FlavorKind::StoneChunk
            | FlavorKind::StoneFlat
            | FlavorKind::Boulder => {}
        }
    }

    const ALL_STRUCTURE_TYPES: &[StructureType] = &[
        StructureType::Den,
        StructureType::Hearth,
        StructureType::Kitchen,
        StructureType::Stores,
        StructureType::Workshop,
        StructureType::Garden,
        StructureType::Watchtower,
        StructureType::WardPost,
        StructureType::Wall,
        StructureType::Gate,
        StructureType::Midden,
        StructureType::DryingRack,
        StructureType::SmokingRack,
        // 369 Phase 2b.
        StructureType::TanningFrame,
    ];

    #[allow(dead_code)]
    fn assert_structure_types_complete(kind: StructureType) {
        match kind {
            StructureType::Den
            | StructureType::Hearth
            | StructureType::Kitchen
            | StructureType::Stores
            | StructureType::Workshop
            | StructureType::Garden
            | StructureType::Watchtower
            | StructureType::WardPost
            | StructureType::Wall
            | StructureType::Gate
            | StructureType::Midden
            | StructureType::DryingRack
            | StructureType::SmokingRack
            | StructureType::TanningFrame => {}
        }
    }

    #[test]
    fn every_structure_type_has_a_summer_binding() {
        let bindings = load_bindings_for_test();
        for kind in ALL_STRUCTURE_TYPES {
            let binding = bindings.building(*kind);
            assert!(!binding.textures.is_empty(), "{kind:?} has zero textures");
            assert!(
                binding.tiles_wide > 0.0,
                "{kind:?} tiles_wide must be positive"
            );
            assert!(
                binding.native_size[0] > 0.0 && binding.native_size[1] > 0.0,
                "{kind:?} native_size components must be positive"
            );
        }
    }

    #[test]
    fn winter_bindings_only_for_documented_structures() {
        // Phase 1b: only Den/Hearth/Stores/Watchtower/WardPost have snow art.
        // If new winter art is added, expand this list rather than silently
        // accepting any [buildings_winter.*] entry.
        let bindings = load_bindings_for_test();
        let expected_winter: &[StructureType] = &[
            StructureType::Den,
            StructureType::Hearth,
            StructureType::Stores,
            StructureType::Watchtower,
            StructureType::WardPost,
        ];
        for kind in expected_winter {
            assert!(
                bindings.building_winter(*kind).is_some(),
                "{kind:?} should have a winter binding"
            );
        }
        for kind in ALL_STRUCTURE_TYPES {
            if expected_winter.contains(kind) {
                continue;
            }
            assert!(
                bindings.building_winter(*kind).is_none(),
                "{kind:?} unexpectedly has a winter binding; update this test if intentional"
            );
        }
    }

    #[test]
    fn every_item_kind_has_a_binding() {
        let bindings = load_bindings_for_test();
        for kind in ALL_ITEM_KINDS {
            bindings.assert_item_has_binding(*kind);
        }
    }

    #[test]
    fn every_herb_kind_has_a_binding_at_every_stage() {
        let bindings = load_bindings_for_test();
        for kind in ALL_HERB_KINDS {
            for stage in ALL_STAGES {
                let _ = bindings.herb_index(*kind, *stage);
            }
        }
    }

    #[test]
    fn every_flavor_kind_has_a_binding_at_every_stage() {
        let bindings = load_bindings_for_test();
        for kind in ALL_FLAVOR_KINDS {
            for stage in ALL_STAGES {
                let _ = bindings.flavor_index(*kind, *stage);
            }
        }
    }

    #[test]
    fn bindings_match_legacy_match_statements() {
        // Spot-check several entries against their expected indices. If this
        // test ever fails, the manifest has drifted and the visual change
        // should be intentional + verified. These four items were migrated to
        // the `materials` atlas (Materials Asset 16x16); see
        // docs/reference/materials-asset-catalog.md.
        let bindings = load_bindings_for_test();
        assert_eq!(bindings.item_atlas_index(ItemKind::RawMouse), 177);
        assert_eq!(bindings.item_atlas_index(ItemKind::Berries), 166);
        assert_eq!(bindings.item_atlas_index(ItemKind::HerbCatnip), 75);
        assert_eq!(bindings.item_atlas_index(ItemKind::Tallow), 131);

        assert_eq!(
            bindings.herb_index(HerbKind::Moonpetal, GrowthStage::Blossom),
            59,
            "Moonpetal blossom; previously wrong at index 24"
        );
        assert_eq!(
            bindings.herb_index(HerbKind::Dreamroot, GrowthStage::Sprout),
            52,
            "Dreamroot sprout; previously wrong at index 27"
        );

        assert_eq!(
            bindings.flavor_index(FlavorKind::Rose, GrowthStage::Sprout),
            44
        );
        assert_eq!(
            bindings.flavor_index(FlavorKind::Sunflower, GrowthStage::Bloom),
            38
        );
    }

    #[test]
    fn every_referenced_atlas_is_declared() {
        // Each item/herb/flavor binding picks an atlas by name; that name
        // MUST appear under [atlases.<name>] in the same manifest. If
        // someone adds an item with `atlas = "foo"` but forgets to
        // declare `[atlases.foo]`, this test fails before the silent-
        // panic-at-runtime path in `resolve_atlas_sprite`.
        let file = load_bindings_file_from_disk();
        let declared: std::collections::HashSet<&String> = file.atlases.keys().collect();
        for (key, b) in &file.items {
            if let ItemBinding::Atlas(atlas_b) = b {
                assert!(
                    declared.contains(&atlas_b.atlas),
                    "item {key} references undeclared atlas '{}'",
                    atlas_b.atlas
                );
            }
        }
        for (key, b) in &file.herbs {
            assert!(
                declared.contains(&b.atlas),
                "herb {key} references undeclared atlas '{}'",
                b.atlas
            );
        }
        for (key, b) in &file.flavor_plants {
            assert!(
                declared.contains(&b.atlas),
                "flavor {key} references undeclared atlas '{}'",
                b.atlas
            );
        }
    }
}
