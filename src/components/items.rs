use bevy_ecs::prelude::*;

// ---------------------------------------------------------------------------
// ItemKind
// ---------------------------------------------------------------------------

/// Every distinct type of physical item that can exist in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ItemKind {
    // --- Raw prey ---
    RawMouse,
    RawRat,
    RawRabbit,
    RawFish,
    RawBird,

    // --- Foraged ---
    Berries,
    Nuts,
    Roots,
    WildOnion,
    Mushroom,
    Moss,
    DriedGrass,
    Feather,

    // --- Herbs (mirror HerbKind) ---
    HerbHealingMoss,
    HerbMoonpetal,
    HerbCalmroot,
    HerbThornbriar,
    HerbDreamroot,
    HerbCatnip,
    HerbSlumbershade,
    HerbOracleOrchid,

    // --- Curiosities ---
    ShinyPebble,
    GlassShard,
    ColorfulShell,

    // --- Shadow materials ---
    ShadowBone,

    // --- Storage upgrades ---
    Barrel,
    Crate,
    Shelf,

    // --- Build materials (bridge into the construction `Material` enum;
    // physical-causality form of materials cats haul to ConstructionSites). ---
    Wood,
    Stone,

    // --- Crafted remedies (ticket 365 — 016 Phase 1a). ---
    // Real inventory items spawned by `resolve_prepare_remedy`,
    // consumed by `resolve_apply_remedy`. Replace the prior
    // search-state-only `Carrying::Remedy` virtual carry.
    RemedyHealingPoultice,
    RemedyEnergyTonic,
    RemedyMoodTonic,

    // --- Raw organ (ticket 367 — 016 Phase 1b). ---
    // Prey-byproduct dropped by the hunt resolver on a probabilistic
    // roll (mammals + birds only; fish are not organ donors). Feeds
    // a cat directly as a small meal, or is the input to the
    // `preserve.preserved_organ` recipe at the Drying Rack.
    RawOrgan,

    // --- Prey byproducts (ticket 375 — 016 input substrate). ---
    // Non-food materials dropped alongside meat by `resolve_engage_prey`.
    // Each prey species emits a fixed set (see SimConstants.prey_byproducts):
    // Mouse + Rat → Bone, Sinew (Rat also Whisker); Rabbit → Hide, Bone, Sinew;
    // Fish → FishScale, Tallow, RawOrgan; Bird → Feather (existing), Bone.
    // Slow-decay organic (0.0005, alongside Feather/Moss/DriedGrass); never
    // food. Downstream sinks: 016 Phase 2/3 crafting children (368–372).
    Bone,
    Sinew,
    Whisker,
    Hide,
    FishScale,
    Tallow,

    // --- Preserved food (ticket 367 — 016 Phase 1b). ---
    // Crafted, non-spoiling food items produced at the Drying Rack
    // and Smoking Rack. `decay_rate == 0.0` (the corruption stamped
    // at the source meat's catch time still rides on the item via
    // `ItemModifiers.corruption`, but the item's `condition` no
    // longer decays). Reduced hunger restore per
    // `docs/systems/crafting.md` Phase 1 table.
    DriedFish,
    SmokedMeat,
    PreservedOrgan,

    // --- Crafting input substrate (ticket 368 — 016 Phase 2). ---
    // Inputs that feed the Phase 2 behavioral-tool recipes
    // (Grooming Brush / Play Bundle / Courtship Gift). Twig / Fiber /
    // Flower come from forage producers; Bristle is shed by prey on
    // death (extends `prey_byproducts`); PolishedStone is the output
    // of a Workshop sub-recipe (Stone → PolishedStone).
    // Slow-decay organic (0.0005, matches Feather / Bone / etc.) for
    // the four foraged/shed inputs; PolishedStone is inorganic (0.0).
    Twig,
    Bristle,
    Fiber,
    Flower,
    PolishedStone,

    // --- Crafted behavioral tools (ticket 368 — 016 Phase 2). ---
    // Tools whose effect lives on the corresponding action resolver
    // (groom_other / socialize / mate_with) keyed to item identity,
    // per `docs/systems/crafting.md` §Design constraints. Durable
    // (0.0 decay) — crafted at the Workshop, carried in inventory,
    // read by the resolver as a presence check; the effect is on the
    // action's outcome magnitude (fondness delta multiplier), never
    // a stat-stick modifier on the item itself.
    GroomingBrush,
    PlayBundle,
    CourtshipGift,

    // --- Phase 2b warrior's kit (ticket 369 — 016 Phase 2b). ---
    // Material-property substrate read by hunt-strike / combat /
    // ranged-attack / movement-detection / noise resolvers. Each
    // variant maps via `equip_material()` / `weapon_class()` /
    // `armor_class()` / `noise_class()` / `durability_tier()` (all
    // exhaustive `match` on ItemKind). Durable (0.0 decay) — the
    // mechanical degradation channel for these is the Fragile
    // snap-on-failed-strike branch, not the per-tick decay clock.
    // Subsumes ticket 334 (stealth cloak == Woven Reed Cloak).
    BoneTipSpear,
    BoneStiletto,
    FlintBlade,
    HideBracers,
    HidePlatedWrap,
    Sling,
    WovenReedCloak,
    ToothNotchedClub,
}

impl ItemKind {
    /// Per-tick decay rate applied to `Item::condition`.
    ///
    /// - Raw prey: 0.0005 (spoils in ~2000 ticks)
    /// - Foraged organic: 0.0005 (same rate as raw prey)
    /// - Herbs: 0.0003 (preserved longer)
    /// - Inorganic / curiosities: 0.0 (no decay)
    pub fn decay_rate(self) -> f32 {
        match self {
            Self::RawMouse | Self::RawRat | Self::RawRabbit | Self::RawFish | Self::RawBird => {
                0.0001
            }

            Self::Berries
            | Self::Nuts
            | Self::Roots
            | Self::WildOnion
            | Self::Mushroom
            | Self::Moss
            | Self::DriedGrass
            | Self::Feather
            | Self::Bone
            | Self::Sinew
            | Self::Whisker
            | Self::Hide
            | Self::FishScale
            | Self::Tallow
            // 368: Phase 2 crafting inputs decay at the same slow rate
            // as the other foraged/shed organic materials.
            | Self::Twig
            | Self::Fiber
            | Self::Flower
            | Self::Bristle => 0.0005,

            Self::HerbHealingMoss
            | Self::HerbMoonpetal
            | Self::HerbCalmroot
            | Self::HerbThornbriar
            | Self::HerbDreamroot
            | Self::HerbCatnip
            | Self::HerbSlumbershade
            | Self::HerbOracleOrchid => 0.0003,

            Self::ShinyPebble | Self::GlassShard | Self::ColorfulShell | Self::ShadowBone => 0.0,

            Self::Barrel | Self::Crate | Self::Shelf => 0.0,

            Self::Wood | Self::Stone => 0.0,

            // Remedies are short-lived just-in-time craft outputs;
            // the plan chain consumes them within a few ticks of
            // preparation. Non-decaying keeps the edge case off the
            // table.
            Self::RemedyHealingPoultice | Self::RemedyEnergyTonic | Self::RemedyMoodTonic => 0.0,

            // Raw organ — perishable like other raw prey.
            Self::RawOrgan => 0.0005,

            // 367: Preserved food doesn't spoil — that's the whole
            // mechanical point of the Phase 1b preservation pipeline.
            // The source-meat corruption stamp at hunt time still
            // rides on the item via `ItemModifiers.corruption`, so a
            // tainted catch dries into a tainted dried fish; only the
            // condition-decay clock is frozen.
            Self::DriedFish | Self::SmokedMeat | Self::PreservedOrgan => 0.0,

            // 368: PolishedStone is inorganic (matches Stone). The
            // three behavioral tools are durable crafted objects —
            // their organic content (fiber, feather) decays in raw
            // form but is structurally bound in the finished tool.
            Self::PolishedStone
            | Self::GroomingBrush
            | Self::PlayBundle
            | Self::CourtshipGift => 0.0,

            // 369: Phase 2b warrior's-kit items are durable crafted
            // objects. Their organic content (bone, sinew, hide,
            // fiber) is structurally bound; degradation routes
            // through the Fragile snap-on-failed-strike branch in
            // the hunt-strike resolver, not per-tick decay.
            Self::BoneTipSpear
            | Self::BoneStiletto
            | Self::FlintBlade
            | Self::HideBracers
            | Self::HidePlatedWrap
            | Self::Sling
            | Self::WovenReedCloak
            | Self::ToothNotchedClub => 0.0,
        }
    }

    /// Bridge to the construction `Material` enum. Returns `Some(_)` for the
    /// item kinds that can be delivered to a `ConstructionSite`. Used by
    /// `resolve_pickup_material` and `resolve_deliver` to identify carried
    /// build-material units.
    pub fn material(self) -> Option<crate::components::task_chain::Material> {
        use crate::components::task_chain::Material;
        match self {
            Self::Wood => Some(Material::Wood),
            Self::Stone => Some(Material::Stone),
            _ => None,
        }
    }

    /// Extra item capacity granted when this item is stored in a building.
    /// Most items provide no bonus; storage upgrades add slots.
    pub fn capacity_bonus(self) -> usize {
        match self {
            Self::Barrel => 10,
            Self::Crate => 8,
            Self::Shelf => 15,
            _ => 0,
        }
    }

    /// Returns true if this item kind is a herb (mirrors `HerbKind` variants).
    pub fn is_herb(self) -> bool {
        matches!(
            self,
            Self::HerbHealingMoss
                | Self::HerbMoonpetal
                | Self::HerbCalmroot
                | Self::HerbThornbriar
                | Self::HerbDreamroot
                | Self::HerbCatnip
                | Self::HerbSlumbershade
                | Self::HerbOracleOrchid
        )
    }

    /// Returns true if this item can be eaten.
    pub fn is_food(self) -> bool {
        matches!(
            self,
            Self::RawMouse
                | Self::RawRat
                | Self::RawRabbit
                | Self::RawFish
                | Self::RawBird
                | Self::Berries
                | Self::Nuts
                | Self::Roots
                | Self::WildOnion
                | Self::Mushroom
                | Self::RawOrgan
                | Self::DriedFish
                | Self::SmokedMeat
                | Self::PreservedOrgan
        )
    }

    /// True iff this item is a Phase 1b preserved-food output
    /// (Dried Fish, Smoked Meat, Preserved Organ). Mirrors
    /// `is_food()` / `is_remedy()`.
    pub fn is_preserved_food(self) -> bool {
        matches!(
            self,
            Self::DriedFish | Self::SmokedMeat | Self::PreservedOrgan
        )
    }

    /// True iff this item is an organ in either raw or preserved
    /// form. Used by the eat path to grant the small mood bump
    /// stamped at hunt time via `ItemModifiers.from_organ`.
    pub fn is_organ(self) -> bool {
        matches!(self, Self::RawOrgan | Self::PreservedOrgan)
    }

    /// True iff this item is raw meat suitable for the smoking
    /// pipeline (mammals + birds). Fish go through the drying
    /// pipeline instead.
    pub fn is_raw_meat(self) -> bool {
        matches!(
            self,
            Self::RawMouse | Self::RawRat | Self::RawRabbit | Self::RawBird
        )
    }

    /// True iff this item can serve as smoking fuel. Currently only
    /// `Wood`; extended here so call sites don't hard-code the variant.
    pub fn is_fuel(self) -> bool {
        matches!(self, Self::Wood)
    }

    /// True iff this item is a crafted remedy (consumed by
    /// `resolve_apply_remedy`). Mirrors `is_herb()` / `is_food()`.
    pub fn is_remedy(self) -> bool {
        matches!(
            self,
            Self::RemedyHealingPoultice | Self::RemedyEnergyTonic | Self::RemedyMoodTonic
        )
    }

    /// Human-readable name for narrative output.
    pub fn name(self) -> &'static str {
        match self {
            Self::RawMouse => "mouse",
            Self::RawRat => "rat",
            Self::RawRabbit => "rabbit",
            Self::RawFish => "fish",
            Self::RawBird => "bird",
            Self::Berries => "berries",
            Self::Nuts => "nuts",
            Self::Roots => "roots",
            Self::WildOnion => "wild onion",
            Self::Mushroom => "mushrooms",
            Self::Moss => "moss",
            Self::DriedGrass => "dried grass",
            Self::Feather => "feathers",
            Self::HerbHealingMoss => "healing moss",
            Self::HerbMoonpetal => "moonpetal",
            Self::HerbCalmroot => "calmroot",
            Self::HerbThornbriar => "thornbriar",
            Self::HerbDreamroot => "dreamroot",
            Self::HerbCatnip => "catnip",
            Self::HerbSlumbershade => "slumbershade",
            Self::HerbOracleOrchid => "oracle orchid",
            Self::ShinyPebble => "shiny pebble",
            Self::GlassShard => "glass shard",
            Self::ColorfulShell => "colorful shell",
            Self::ShadowBone => "shadow bone",
            Self::Barrel => "barrel",
            Self::Crate => "crate",
            Self::Shelf => "shelf",
            Self::Wood => "wood",
            Self::Stone => "stone",
            Self::RemedyHealingPoultice => "healing poultice",
            Self::RemedyEnergyTonic => "energy tonic",
            Self::RemedyMoodTonic => "mood tonic",
            Self::RawOrgan => "organ",
            Self::DriedFish => "dried fish",
            Self::SmokedMeat => "smoked meat",
            Self::PreservedOrgan => "preserved organ",
            Self::Bone => "bone",
            Self::Sinew => "sinew",
            Self::Whisker => "whisker",
            Self::Hide => "hide",
            Self::FishScale => "fish scale",
            Self::Tallow => "tallow",
            // 368 Phase 2 inputs + behavioral tools.
            Self::Twig => "twig",
            Self::Bristle => "bristle",
            Self::Fiber => "fiber",
            Self::Flower => "flower",
            Self::PolishedStone => "polished stone",
            Self::GroomingBrush => "grooming brush",
            Self::PlayBundle => "play bundle",
            Self::CourtshipGift => "courtship gift",
            // 369 Phase 2b warrior's kit.
            Self::BoneTipSpear => "bone-tip spear",
            Self::BoneStiletto => "bone stiletto",
            Self::FlintBlade => "flint blade",
            Self::HideBracers => "hide bracers",
            Self::HidePlatedWrap => "hide-plated wrap",
            Self::Sling => "sling",
            Self::WovenReedCloak => "woven reed cloak",
            Self::ToothNotchedClub => "tooth-notched club",
        }
    }

    /// Whether `name()` returns a grammatically plural form.
    pub fn is_plural_name(self) -> bool {
        matches!(
            self,
            Self::Berries | Self::Nuts | Self::Roots | Self::Mushroom | Self::Feather
        )
    }

    /// Singular form of the item name for grammatical contexts like "every last X".
    pub fn singular_name(self) -> &'static str {
        match self {
            Self::Berries => "berry",
            Self::Nuts => "nut",
            Self::Roots => "root",
            Self::Mushroom => "mushroom",
            Self::Feather => "feather",
            _ => self.name(),
        }
    }

    /// Display category for UI rollups (190).
    ///
    /// Identity-keyed, derived from the variant itself. No new fields on
    /// items — categorization is display-layer-only, per the no-stat-sticks
    /// invariant. Categories grow as 016 crafting phases land (Tool /
    /// Wearable / Decoration arrive with phases 2/3/4).
    pub fn category(self) -> ItemCategory {
        match self {
            Self::RawMouse
            | Self::RawRat
            | Self::RawRabbit
            | Self::RawFish
            | Self::RawBird
            | Self::Berries
            | Self::Nuts
            | Self::Roots
            | Self::WildOnion
            | Self::Mushroom
            | Self::RawOrgan => ItemCategory::RawFood,

            Self::HerbHealingMoss
            | Self::HerbMoonpetal
            | Self::HerbCalmroot
            | Self::HerbThornbriar
            | Self::HerbDreamroot
            | Self::HerbCatnip
            | Self::HerbSlumbershade
            | Self::HerbOracleOrchid => ItemCategory::Herb,

            Self::ShinyPebble | Self::GlassShard | Self::ColorfulShell => ItemCategory::Curiosity,

            Self::Barrel | Self::Crate | Self::Shelf => ItemCategory::StorageUpgrade,

            Self::Moss
            | Self::DriedGrass
            | Self::Feather
            | Self::ShadowBone
            | Self::Wood
            | Self::Stone
            | Self::Bone
            | Self::Sinew
            | Self::Whisker
            | Self::Hide
            | Self::FishScale
            | Self::Tallow
            // 368 Phase 2 crafting inputs — raw and refined materials.
            | Self::Twig
            | Self::Bristle
            | Self::Fiber
            | Self::Flower
            | Self::PolishedStone => ItemCategory::Material,

            // 365 — first Phase 1a entry in the Remedy category. Phase 1b
            // (preservation outputs) lands as the PreservedFood category;
            // Phases 2/3/4 add Tool / Wearable / Decoration.
            Self::RemedyHealingPoultice | Self::RemedyEnergyTonic | Self::RemedyMoodTonic => {
                ItemCategory::Remedy
            }

            // 367 — preservation outputs from Drying Rack / Smoking Rack.
            Self::DriedFish | Self::SmokedMeat | Self::PreservedOrgan => {
                ItemCategory::PreservedFood
            }

            // 368 — Phase 2 behavioral tools (Grooming Brush /
            // Play Bundle / Courtship Gift). First Tool entry in
            // the 016 category list; Phase 3 (Wearable) and Phase 4
            // (Decoration) extend the same axis.
            Self::GroomingBrush | Self::PlayBundle | Self::CourtshipGift => ItemCategory::Tool,

            // 369 — Phase 2b warrior's kit. Distinct category from
            // Tool because the resolver-read shape is different:
            // weapons/armor compose via `weapon_class()` /
            // `armor_class()` / `noise_class()` in hunt-strike /
            // combat / detection resolvers, not via the fondness-
            // multiplier hook that the behavioral tools use.
            Self::BoneTipSpear
            | Self::BoneStiletto
            | Self::FlintBlade
            | Self::HideBracers
            | Self::HidePlatedWrap
            | Self::Sling
            | Self::WovenReedCloak
            | Self::ToothNotchedClub => ItemCategory::CombatGear,
        }
    }

    /// Hunger satisfaction provided when consumed (0.0–1.0 scale).
    /// Non-food items return 0.0.
    ///
    /// Tuned so a single hunt feeds a cat for days. Hunted prey is a real
    /// meal (0.5–0.8); foraged plants are snacks (0.20).
    pub fn food_value(self) -> f32 {
        match self {
            Self::RawRat => 0.8,
            Self::RawRabbit => 0.65,
            Self::RawMouse => 0.5,
            Self::RawFish => 0.7,
            Self::RawBird => 0.6,
            Self::Berries | Self::Nuts | Self::Roots | Self::Mushroom | Self::WildOnion => 0.2,
            // 367: Raw organ is a small meal — between a foraged
            // plant and a small carcass. Eats well fresh but
            // primarily exists as the input to the Preserved
            // Organ recipe.
            Self::RawOrgan => 0.4,
            // 367: Phase 1 preservation ratios from crafting.md
            // line 50-54. Dried Fish = 0.7× raw fish (0.7 × 0.7
            // ≈ 0.49). Smoked Meat = 0.8× the canonical raw-rat
            // value (0.8 × 0.8 = 0.64) — all smoked variants
            // share one output ItemKind, so the ratio applies
            // uniformly. Preserved Organ retains the mood bonus
            // (delivered separately via ItemModifiers.from_organ
            // in the eat path) rather than carrying a higher
            // hunger ratio.
            Self::DriedFish => 0.49,
            Self::SmokedMeat => 0.64,
            Self::PreservedOrgan => 0.3,
            _ => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// ItemCategory — UI display rollup
// ---------------------------------------------------------------------------

/// Display-layer grouping for items in inventory / stores overview UI.
///
/// Identity-keyed: every `ItemKind` maps to exactly one category via
/// `ItemKind::category()`. No numeric modifier fields on items; categories
/// are derived, not stored. New variants (Tool / Wearable / Decoration /
/// PreservedFood) land alongside their corresponding 016 crafting phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemCategory {
    RawFood,
    Herb,
    Material,
    StorageUpgrade,
    Curiosity,
    /// Crafted single-use consumables (poultices, tonics). Ticket
    /// 365 — first 016-phase entry in the category list. Phase 1b
    /// adds PreservedFood; later phases add Tool / Wearable /
    /// Decoration.
    Remedy,
    /// Crafted, non-spoiling food produced at the Drying Rack and
    /// Smoking Rack (ticket 367 — 016 Phase 1b). Dried Fish, Smoked
    /// Meat, Preserved Organ. Sorts alongside raw food in the UI
    /// rollup — both are "food" from a cat's planning perspective.
    PreservedFood,
    /// Crafted behavioral tools produced at the Workshop (ticket
    /// 368 — 016 Phase 2). Grooming Brush, Play Bundle, Courtship
    /// Gift. Their effect lives on the corresponding action
    /// resolver, not on a modifier field on the item type.
    Tool,
    /// Phase 2b warrior's kit (ticket 369): weapons + armor +
    /// stealth garments whose material properties are read by
    /// hunt-strike / combat / detection / noise resolvers. Distinct
    /// from `Tool` (which composes via action-magnitude multipliers
    /// rather than property-keyed resolver reads).
    CombatGear,
}

impl ItemCategory {
    /// Display label for the UI category header. Singular form — the panel
    /// appends a count separately.
    pub fn label(self) -> &'static str {
        match self {
            Self::RawFood => "Food",
            Self::PreservedFood => "Preserved food",
            Self::Herb => "Herbs",
            Self::Material => "Materials",
            Self::StorageUpgrade => "Storage upgrades",
            Self::Curiosity => "Curiosities",
            Self::Remedy => "Remedies",
            Self::Tool => "Tools",
            Self::CombatGear => "Combat gear",
        }
    }

    /// Stable display ordering for the panel — food first (most important),
    /// herbs next (healing/ward inputs), then materials, then the long tail.
    pub fn sort_key(self) -> u8 {
        match self {
            Self::RawFood => 0,
            // 367: Preserved food sorts directly after raw food — both
            // belong to the colony's food planning view.
            Self::PreservedFood => 1,
            Self::Herb => 2,
            Self::Remedy => 3,
            Self::Material => 4,
            Self::StorageUpgrade => 5,
            Self::Curiosity => 6,
            // 368: Tools sort after curiosities — distinct functional
            // class, lowest planning priority of the displayable
            // categories.
            Self::Tool => 7,
            // 369: Combat gear sorts last — equipped state is
            // long-lived and players rarely scan it for action.
            Self::CombatGear => 8,
        }
    }
}

// ---------------------------------------------------------------------------
// Item modifiers
// ---------------------------------------------------------------------------

/// Modifiers stamped onto an item at creation time. Corruption is captured from
/// the source tile when the item is first produced (hunt catch, forage, den
/// raid). Future modifiers (blessed, poisoned, shadow-touched) add fields here.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItemModifiers {
    /// Corruption level from the source tile, clamped to `[0.0, 1.0]`.
    pub corruption: f32,
    /// True if the item has been cooked at a Kitchen. Cooked food yields a
    /// hunger-restoration multiplier in `resolve_eat_at_stores`.
    #[serde(default)]
    pub cooked: bool,
    /// Ticket 367: stamped at hunt time on `ItemKind::RawOrgan` drops,
    /// preserved across the Drying Rack pipeline onto the resulting
    /// `ItemKind::PreservedOrgan`. The eat path reads this flag to
    /// grant a small mood bump — organ meat is the "rich find" of a
    /// kill in the cat-narrative sense, and the preservation
    /// pipeline carries that meaning forward. Defaults `false`; the
    /// `#[serde(default)]` attribute keeps savefile back-compat.
    #[serde(default)]
    pub from_organ: bool,
}

impl ItemModifiers {
    pub fn with_corruption(corruption: f32) -> Self {
        Self {
            corruption: corruption.clamp(0.0, 1.0),
            ..Self::default()
        }
    }

    /// True when the item has no negative modifiers (i.e. is not corrupted).
    /// Cooked items are still "clean" — cooking is a positive modifier.
    pub fn is_clean(&self) -> bool {
        self.corruption == 0.0
    }
}

/// Returns a display name combining quality, modifiers, and kind.
/// Examples: `"corrupted rat"`, `"exceptional corrupted rat"`, `"fine rabbit"`.
pub fn item_display_name(kind: ItemKind, quality: f32, modifiers: &ItemModifiers) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(ql) = quality_label(quality) {
        parts.push(ql);
    }
    if modifiers.corruption > 0.3 {
        parts.push("corrupted");
    }
    parts.push(kind.name());
    parts.join(" ")
}

// ---------------------------------------------------------------------------
// Quality tiers for narrative
// ---------------------------------------------------------------------------

/// Returns a narrative label for notable item quality. Common quality returns
/// `None` — only poor and above-average items are worth mentioning.
pub fn quality_label(quality: f32) -> Option<&'static str> {
    if quality < 0.2 {
        Some("poor")
    } else if quality >= 0.8 {
        Some("exceptional")
    } else if quality >= 0.5 {
        Some("fine")
    } else {
        None // common quality — not worth narrating
    }
}

// ---------------------------------------------------------------------------
// ItemLocation
// ---------------------------------------------------------------------------

/// Where an item currently resides.
///
/// Variants containing `Entity` are not serializable — entity handles are
/// runtime identifiers that cannot survive a save/load round-trip. The
/// `location` field in `Item` is therefore skipped during serialization and
/// defaults to `OnGround` on deserialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemLocation {
    /// Carried in a cat's inventory. The entity is the carrier.
    Carried(Entity),
    /// Lying on the ground; the item entity also has a `Position` component.
    OnGround,
    /// Stored inside a building. The entity is the containing structure.
    StoredIn(Entity),
}

impl ItemLocation {
    /// Default used by serde when deserializing items whose location cannot
    /// be restored from the save file.
    fn on_ground() -> Self {
        Self::OnGround
    }
}

// ---------------------------------------------------------------------------
// Build-material marker
// ---------------------------------------------------------------------------

/// Marker stamped on ground `Item` entities whose `kind` is a build
/// material (`Wood` / `Stone`). Used to make the planner's mutable
/// build-material query (`BuildingResolverParams::material_items`)
/// statically disjoint from the read-only `items_query` consumed by
/// food/herb resolvers (`eat_at_stores`, `deposit_at_stores`, etc).
/// Without it, both queries overlap on the same `Item` entities and
/// Bevy's borrow checker (B0001) rejects the system.
#[derive(Component, Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct BuildMaterialItem;

// ---------------------------------------------------------------------------
// Item component
// ---------------------------------------------------------------------------

/// A physical item entity in the world.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Item {
    pub kind: ItemKind,
    /// Overall quality, clamped to `[0.0, 1.0]` at construction.
    pub quality: f32,
    /// Current condition, starts at 1.0 and decays toward 0.0.
    pub condition: f32,
    /// Current location. Skipped during serialization because `Entity`
    /// handles are not stable across save/load boundaries; restored to
    /// `OnGround` on deserialization.
    #[serde(skip, default = "ItemLocation::on_ground")]
    pub location: ItemLocation,
    /// Modifiers stamped at creation (corruption, etc.). Defaults to clean
    /// for items created before this field existed.
    #[serde(default)]
    pub modifiers: ItemModifiers,
}

impl Item {
    /// Create a new item with quality clamped to `[0.0, 1.0]` and clean modifiers.
    pub fn new(kind: ItemKind, quality: f32, location: ItemLocation) -> Self {
        Self {
            kind,
            quality: quality.clamp(0.0, 1.0),
            condition: 1.0,
            location,
            modifiers: ItemModifiers::default(),
        }
    }

    /// Create a new item with explicit modifiers.
    pub fn with_modifiers(
        kind: ItemKind,
        quality: f32,
        location: ItemLocation,
        modifiers: ItemModifiers,
    ) -> Self {
        Self {
            kind,
            quality: quality.clamp(0.0, 1.0),
            condition: 1.0,
            location,
            modifiers,
        }
    }

    /// Advance decay by one tick.
    ///
    /// Returns `true` if the item should be destroyed (condition has reached
    /// or dropped below 0.0).
    pub fn tick_decay(&mut self) -> bool {
        self.condition -= self.kind.decay_rate();
        self.is_destroyed()
    }

    /// True when condition has reached 0.0 or below.
    pub fn is_destroyed(&self) -> bool {
        self.condition <= 0.0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_kind_has_a_category() {
        // Exhaustive over the 59 variants — extend this list when ItemKind grows.
        let all: [ItemKind; 59] = [
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
            // 368 Phase 2 inputs + behavioral tools.
            ItemKind::Twig,
            ItemKind::Bristle,
            ItemKind::Fiber,
            ItemKind::Flower,
            ItemKind::PolishedStone,
            ItemKind::GroomingBrush,
            ItemKind::PlayBundle,
            ItemKind::CourtshipGift,
            // 369 Phase 2b warrior's kit.
            ItemKind::BoneTipSpear,
            ItemKind::BoneStiletto,
            ItemKind::FlintBlade,
            ItemKind::HideBracers,
            ItemKind::HidePlatedWrap,
            ItemKind::Sling,
            ItemKind::WovenReedCloak,
            ItemKind::ToothNotchedClub,
        ];
        // Trivially exhaustive (the match in category() is total) — this test
        // exists to make ItemKind growth fail loudly if a future variant gets
        // added without extending this list.
        for kind in all {
            let _ = kind.category();
        }
        assert_eq!(all.len(), 59);
    }

    #[test]
    fn preserved_food_is_food_but_does_not_decay() {
        // 367: the whole point of the preservation pipeline.
        for k in [
            ItemKind::DriedFish,
            ItemKind::SmokedMeat,
            ItemKind::PreservedOrgan,
        ] {
            assert!(k.is_food(), "{k:?} should count as food");
            assert!(k.is_preserved_food(), "{k:?} should be preserved");
            assert_eq!(k.decay_rate(), 0.0, "{k:?} must not spoil");
            assert_eq!(k.category(), ItemCategory::PreservedFood);
            assert!(k.food_value() > 0.0);
        }
    }

    #[test]
    fn raw_organ_decays_and_categorizes_as_raw_food() {
        assert!(ItemKind::RawOrgan.is_food());
        assert!(ItemKind::RawOrgan.is_organ());
        assert!(!ItemKind::RawOrgan.is_preserved_food());
        assert_eq!(ItemKind::RawOrgan.category(), ItemCategory::RawFood);
        assert!(ItemKind::RawOrgan.decay_rate() > 0.0);
    }

    #[test]
    fn preservation_ratios_match_crafting_doc() {
        // crafting.md Phase 1 table:
        //   Dried Fish    = 0.7× fresh fish (0.7 × 0.7 ≈ 0.49)
        //   Smoked Meat   = 0.8× canonical raw-rat (0.8 × 0.8 = 0.64)
        assert!((ItemKind::DriedFish.food_value() - 0.49).abs() < 1e-3);
        assert!((ItemKind::SmokedMeat.food_value() - 0.64).abs() < 1e-3);
        // Preserved Organ doesn't claim a hunger ratio — it retains
        // the mood bonus via ItemModifiers.from_organ on the eat path.
        assert!(ItemKind::PreservedOrgan.food_value() > 0.0);
    }

    #[test]
    fn is_raw_meat_is_mammals_and_birds() {
        // Smoking pipeline reads `is_raw_meat`. Fish goes through
        // drying, not smoking — keep it out of the meat set.
        for k in [
            ItemKind::RawMouse,
            ItemKind::RawRat,
            ItemKind::RawRabbit,
            ItemKind::RawBird,
        ] {
            assert!(k.is_raw_meat(), "{k:?} should be raw meat");
        }
        for k in [
            ItemKind::RawFish,
            ItemKind::RawOrgan,
            ItemKind::Berries,
            ItemKind::DriedFish,
            ItemKind::SmokedMeat,
        ] {
            assert!(!k.is_raw_meat(), "{k:?} must not be classified as raw meat");
        }
    }

    #[test]
    fn from_organ_defaults_false_and_round_trips() {
        let default = ItemModifiers::default();
        assert!(!default.from_organ);
        let stamped = ItemModifiers {
            from_organ: true,
            ..ItemModifiers::default()
        };
        assert!(stamped.from_organ);
    }

    #[test]
    fn remedies_have_remedy_category() {
        assert_eq!(
            ItemKind::RemedyHealingPoultice.category(),
            ItemCategory::Remedy
        );
        assert_eq!(ItemKind::RemedyEnergyTonic.category(), ItemCategory::Remedy);
        assert_eq!(ItemKind::RemedyMoodTonic.category(), ItemCategory::Remedy);
    }

    #[test]
    fn remedies_are_not_food_or_herb() {
        for r in [
            ItemKind::RemedyHealingPoultice,
            ItemKind::RemedyEnergyTonic,
            ItemKind::RemedyMoodTonic,
        ] {
            assert!(r.is_remedy());
            assert!(!r.is_food());
            assert!(!r.is_herb());
            assert_eq!(r.food_value(), 0.0);
            assert_eq!(r.material(), None);
        }
    }

    #[test]
    fn category_buckets_match_intent() {
        assert_eq!(ItemKind::RawMouse.category(), ItemCategory::RawFood);
        assert_eq!(ItemKind::Berries.category(), ItemCategory::RawFood);
        assert_eq!(ItemKind::HerbCatnip.category(), ItemCategory::Herb);
        assert_eq!(ItemKind::Moss.category(), ItemCategory::Material);
        assert_eq!(ItemKind::Wood.category(), ItemCategory::Material);
        assert_eq!(ItemKind::Barrel.category(), ItemCategory::StorageUpgrade);
        assert_eq!(ItemKind::ShinyPebble.category(), ItemCategory::Curiosity);
        // 368: Phase 2 inputs are materials; behavioral tools are Tools.
        assert_eq!(ItemKind::Twig.category(), ItemCategory::Material);
        assert_eq!(ItemKind::Bristle.category(), ItemCategory::Material);
        assert_eq!(ItemKind::Fiber.category(), ItemCategory::Material);
        assert_eq!(ItemKind::Flower.category(), ItemCategory::Material);
        assert_eq!(ItemKind::PolishedStone.category(), ItemCategory::Material);
        assert_eq!(ItemKind::GroomingBrush.category(), ItemCategory::Tool);
        assert_eq!(ItemKind::PlayBundle.category(), ItemCategory::Tool);
        assert_eq!(ItemKind::CourtshipGift.category(), ItemCategory::Tool);
        // Behavioral tools are not food / herb / remedy.
        assert!(!ItemKind::GroomingBrush.is_food());
        assert!(!ItemKind::PlayBundle.is_food());
        assert!(!ItemKind::CourtshipGift.is_food());
    }

    #[test]
    fn category_sort_orders_food_first() {
        assert!(ItemCategory::RawFood.sort_key() < ItemCategory::Herb.sort_key());
        assert!(ItemCategory::Herb.sort_key() < ItemCategory::Material.sort_key());
        // 367: Preserved food sorts directly after raw food — both
        // belong to the colony's food planning view.
        assert!(ItemCategory::RawFood.sort_key() < ItemCategory::PreservedFood.sort_key());
        assert!(ItemCategory::PreservedFood.sort_key() < ItemCategory::Herb.sort_key());
        // Remedies sort between herbs and materials — closer to herbs
        // since they share the healing/medicine pool.
        assert!(ItemCategory::Herb.sort_key() < ItemCategory::Remedy.sort_key());
        assert!(ItemCategory::Remedy.sort_key() < ItemCategory::Material.sort_key());
        assert_eq!(ItemCategory::Curiosity.sort_key(), 6);
    }

    #[test]
    fn raw_prey_is_food() {
        assert!(ItemKind::RawMouse.is_food());
        assert!(ItemKind::RawRat.is_food());
        assert!(ItemKind::RawRabbit.is_food());
        assert!(ItemKind::RawFish.is_food());
        assert!(ItemKind::RawBird.is_food());
        assert!(ItemKind::Berries.is_food());
        assert!(ItemKind::Mushroom.is_food());

        assert!(!ItemKind::Moss.is_food());
        assert!(!ItemKind::Feather.is_food());
        assert!(!ItemKind::ShinyPebble.is_food());
        assert!(!ItemKind::HerbHealingMoss.is_food());

        // 375: prey byproducts are crafting materials, not food. Eat-DSE
        // and resolver paths gate on is_food(); a regression here would
        // pull bones / hide / etc. into the food pool.
        assert!(!ItemKind::Bone.is_food());
        assert!(!ItemKind::Sinew.is_food());
        assert!(!ItemKind::Whisker.is_food());
        assert!(!ItemKind::Hide.is_food());
        assert!(!ItemKind::FishScale.is_food());
        assert!(!ItemKind::Tallow.is_food());
    }

    #[test]
    fn item_decays_over_time() {
        let mut item = Item::new(ItemKind::RawFish, 1.0, ItemLocation::OnGround);
        // RawFish decay_rate = 0.0001; condition starts at 1.0, so ~10000 ticks
        // to fully decay. Allow up to 11000 to be float-safe.
        let mut destroyed = false;
        for _ in 0..11000 {
            if item.tick_decay() {
                destroyed = true;
                break;
            }
        }
        assert!(destroyed, "RawFish should be destroyed within 11000 ticks");
    }

    #[test]
    fn inorganic_items_do_not_decay() {
        let mut item = Item::new(ItemKind::ShinyPebble, 1.0, ItemLocation::OnGround);
        for _ in 0..1000 {
            assert!(!item.tick_decay(), "ShinyPebble should never decay");
        }
        assert!((item.condition - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn quality_is_clamped() {
        let over = Item::new(ItemKind::Nuts, 5.0, ItemLocation::OnGround);
        assert_eq!(over.quality, 1.0, "quality above 1.0 should clamp to 1.0");

        let under = Item::new(ItemKind::Nuts, -3.0, ItemLocation::OnGround);
        assert_eq!(under.quality, 0.0, "quality below 0.0 should clamp to 0.0");

        let mid = Item::new(ItemKind::Nuts, 0.7, ItemLocation::OnGround);
        assert_eq!(mid.quality, 0.7, "quality in range should be unchanged");
    }

    #[test]
    fn food_values_are_positive_for_food_items() {
        let food_items = [
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
        ];
        for kind in food_items {
            assert!(
                kind.food_value() > 0.0,
                "{kind:?} is food but has food_value == 0.0"
            );
        }
    }

    #[test]
    fn item_modifiers_default_is_clean() {
        let mods = ItemModifiers::default();
        assert!(mods.is_clean());
        assert_eq!(mods.corruption, 0.0);
    }

    #[test]
    fn with_corruption_clamps_to_unit_range() {
        let mods = ItemModifiers::with_corruption(2.5);
        assert_eq!(mods.corruption, 1.0);
        let mods = ItemModifiers::with_corruption(-0.3);
        assert_eq!(mods.corruption, 0.0);
    }

    #[test]
    fn corrupted_item_reduces_effective_food_value() {
        let penalty = 0.5;
        let base = ItemKind::RawRat.food_value(); // 0.8
        let mods = ItemModifiers::with_corruption(0.6);
        let freshness = 1.0 - mods.corruption * penalty;
        let effective = base * freshness;
        assert!(effective < base, "corrupted food should give less hunger");
        assert!((effective - 0.56).abs() < 0.01, "0.8 * 0.7 ≈ 0.56");
    }

    #[test]
    fn clean_item_gives_full_food_value() {
        let penalty = 0.5;
        let base = ItemKind::RawRat.food_value();
        let mods = ItemModifiers::default();
        let freshness = 1.0 - mods.corruption * penalty;
        assert_eq!(freshness, 1.0);
        assert_eq!(base * freshness, base);
    }

    #[test]
    fn item_display_name_reflects_corruption() {
        let mods = ItemModifiers::with_corruption(0.5);
        let name = item_display_name(ItemKind::RawRat, 0.4, &mods);
        assert_eq!(name, "corrupted rat");
    }

    #[test]
    fn item_display_name_clean_item() {
        let mods = ItemModifiers::default();
        let name = item_display_name(ItemKind::RawRat, 0.4, &mods);
        assert_eq!(name, "rat");
    }

    #[test]
    fn item_display_name_quality_and_corruption() {
        let mods = ItemModifiers::with_corruption(0.8);
        let name = item_display_name(ItemKind::RawRabbit, 0.85, &mods);
        assert_eq!(name, "exceptional corrupted rabbit");
    }

    #[test]
    fn item_new_has_clean_modifiers() {
        let item = Item::new(ItemKind::RawFish, 0.5, ItemLocation::OnGround);
        assert!(item.modifiers.is_clean());
    }

    #[test]
    fn item_with_modifiers_preserves_corruption() {
        let mods = ItemModifiers::with_corruption(0.7);
        let item = Item::with_modifiers(ItemKind::Berries, 0.5, ItemLocation::OnGround, mods);
        assert_eq!(item.modifiers.corruption, 0.7);
    }

    #[test]
    fn cooked_defaults_false_and_preserves_through_with_modifiers() {
        let default = ItemModifiers::default();
        assert!(!default.cooked);
        let with_corr = ItemModifiers::with_corruption(0.4);
        assert!(!with_corr.cooked);
    }

    #[test]
    fn cooked_item_yields_multiplier_on_hunger_math() {
        // Mirrors the formula in `resolve_eat_at_stores`.
        let cooked_food_multiplier = 1.3_f32;
        let penalty = 0.5_f32;
        let base = ItemKind::RawRat.food_value(); // 0.8
        let raw_mods = ItemModifiers::default();
        let cooked_mods = ItemModifiers {
            cooked: true,
            ..ItemModifiers::default()
        };
        let freshness = 1.0 - raw_mods.corruption * penalty;
        let raw_value = base * freshness;
        let cooked_value = base
            * freshness
            * if cooked_mods.cooked {
                cooked_food_multiplier
            } else {
                1.0
            };
        assert!(
            (cooked_value - raw_value * 1.3).abs() < 1e-4,
            "cooked item should yield 1.3× the raw food value"
        );
    }
}
