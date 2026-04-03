use bevy_ecs::prelude::*;

use crate::resources::map::Terrain;
use crate::resources::time::Season;

// ---------------------------------------------------------------------------
// Herb system
// ---------------------------------------------------------------------------

/// The species of herb that can be gathered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum HerbKind {
    /// Healing poultice ingredient. Near water and forest.
    HealingMoss,
    /// Energy restorative ingredient. Light forest and grass.
    Moonpetal,
    /// Mood tonic ingredient. Any non-water terrain.
    Calmroot,
    /// Ward material. Forest edges.
    Thornbriar,
    /// Visions and narrative events. Fairy rings and standing stones only.
    Dreamroot,
}

impl HerbKind {
    /// Terrain types where this herb can spawn.
    pub fn spawn_terrains(self) -> &'static [Terrain] {
        match self {
            Self::HealingMoss => &[Terrain::Water, Terrain::LightForest, Terrain::DenseForest],
            Self::Moonpetal => &[Terrain::LightForest, Terrain::Grass],
            Self::Calmroot => &[
                Terrain::Grass,
                Terrain::LightForest,
                Terrain::DenseForest,
                Terrain::Rock,
                Terrain::Mud,
                Terrain::Sand,
            ],
            Self::Thornbriar => &[Terrain::LightForest, Terrain::DenseForest],
            Self::Dreamroot => &[Terrain::FairyRing, Terrain::StandingStone],
        }
    }

    /// Seasons during which this herb can be harvested.
    pub fn available_seasons(self) -> &'static [Season] {
        match self {
            Self::HealingMoss => &[Season::Spring, Season::Summer, Season::Autumn],
            Self::Moonpetal => &[Season::Summer],
            Self::Calmroot => &[Season::Spring, Season::Summer],
            Self::Thornbriar => &[Season::Spring, Season::Summer, Season::Autumn, Season::Winter],
            Self::Dreamroot => &[Season::Autumn, Season::Winter],
        }
    }

    /// TUI map symbol for this herb.
    pub fn symbol(self) -> char {
        'h'
    }

    /// Spawn density: probability that an eligible tile actually gets this herb.
    pub fn spawn_density(self) -> f32 {
        match self {
            Self::HealingMoss => 0.15,
            Self::Moonpetal => 0.10,
            Self::Calmroot => 0.08,
            Self::Thornbriar => 0.12,
            Self::Dreamroot => 1.0,
        }
    }
}

/// An herb entity in the world.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Herb {
    pub kind: HerbKind,
    /// True if growing on a tile with high mystery.
    pub magical: bool,
    /// True if corrupted — cannot be harvested and may cause negative effects.
    pub twisted: bool,
}

/// Marker: this herb can be harvested right now (correct season, not twisted).
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Harvestable;

/// Tracks which seasons an herb entity is available for harvest.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Seasonal {
    pub available: Vec<Season>,
}

// ---------------------------------------------------------------------------
// Inventory
// ---------------------------------------------------------------------------

/// A cat's carried herb pouch. Capacity-limited.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Inventory {
    pub herbs: Vec<HerbKind>,
}

impl Inventory {
    pub const MAX_HERBS: usize = 5;

    pub fn is_full(&self) -> bool {
        self.herbs.len() >= Self::MAX_HERBS
    }

    pub fn has_herb(&self, kind: HerbKind) -> bool {
        self.herbs.contains(&kind)
    }

    /// Remove one instance of `kind` from inventory. Returns true if found.
    pub fn take_herb(&mut self, kind: HerbKind) -> bool {
        if let Some(idx) = self.herbs.iter().position(|h| *h == kind) {
            self.herbs.swap_remove(idx);
            true
        } else {
            false
        }
    }

    /// Add a herb. Returns false if inventory is full.
    pub fn add_herb(&mut self, kind: HerbKind) -> bool {
        if self.is_full() {
            return false;
        }
        self.herbs.push(kind);
        true
    }

    /// Whether the inventory has any herb usable for a remedy.
    pub fn has_remedy_herb(&self) -> bool {
        self.herbs.iter().any(|h| {
            matches!(
                h,
                HerbKind::HealingMoss | HerbKind::Moonpetal | HerbKind::Calmroot
            )
        })
    }

    /// Whether the inventory has thornbriar for ward-setting.
    pub fn has_ward_herb(&self) -> bool {
        self.has_herb(HerbKind::Thornbriar)
    }
}

// ---------------------------------------------------------------------------
// Wards
// ---------------------------------------------------------------------------

/// The kind of magical ward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WardKind {
    /// Basic herb-based ward. ~200 tick lifespan.
    Thornward,
    /// Trained-magic ward. ~1000 tick lifespan.
    DurableWard,
}

/// A magical ward entity placed in the world.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Ward {
    pub kind: WardKind,
    /// Current strength, 1.0 at creation. Decays per tick.
    pub strength: f32,
    /// Per-tick decay rate.
    pub decay_rate: f32,
    /// True for misfire-created wards that attract instead of repel.
    pub inverted: bool,
}

impl Ward {
    pub fn thornward() -> Self {
        Self {
            kind: WardKind::Thornward,
            strength: 1.0,
            decay_rate: 0.005,
            inverted: false,
        }
    }

    pub fn durable() -> Self {
        Self {
            kind: WardKind::DurableWard,
            strength: 1.0,
            decay_rate: 0.001,
            inverted: false,
        }
    }

    pub fn inverted_at(pos_kind: WardKind) -> Self {
        let mut w = match pos_kind {
            WardKind::Thornward => Self::thornward(),
            WardKind::DurableWard => Self::durable(),
        };
        w.inverted = true;
        w
    }

    /// Effective repulsion radius (tiles). Proportional to strength.
    pub fn repel_radius(&self) -> f32 {
        3.0 * self.strength
    }
}

// ---------------------------------------------------------------------------
// Remedies
// ---------------------------------------------------------------------------

/// The kind of herbal remedy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RemedyKind {
    /// HealingMoss → +0.05 health/tick for 20 ticks.
    HealingPoultice,
    /// Moonpetal → +0.03 energy/tick for 15 ticks.
    EnergyTonic,
    /// Calmroot → +0.2 mood modifier for 50 ticks.
    MoodTonic,
}

impl RemedyKind {
    /// Total ticks the remedy effect lasts.
    pub fn duration(self) -> u64 {
        match self {
            Self::HealingPoultice => 20,
            Self::EnergyTonic => 15,
            Self::MoodTonic => 50,
        }
    }

    /// Which herb is consumed to prepare this remedy.
    pub fn required_herb(self) -> HerbKind {
        match self {
            Self::HealingPoultice => HerbKind::HealingMoss,
            Self::EnergyTonic => HerbKind::Moonpetal,
            Self::MoodTonic => HerbKind::Calmroot,
        }
    }
}

/// Active remedy buff on a cat.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemedyEffect {
    pub kind: RemedyKind,
    pub ticks_remaining: u64,
    /// The cat who applied this remedy (for gratitude tracking).
    #[serde(skip)]
    pub healer: Option<Entity>,
}

// ---------------------------------------------------------------------------
// Misfires
// ---------------------------------------------------------------------------

/// Possible outcomes when a magic attempt goes wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MisfireEffect {
    /// Nothing happens. Mild embarrassment.
    Fizzle,
    /// Caster gains +0.1 personal corruption.
    CorruptionBacksplash,
    /// Ward spawned with inverted effect (attracts predators).
    InvertedWard,
    /// Caster takes the injury instead of healing the target.
    WoundTransfer,
    /// Caster's position revealed to dark creatures.
    LocationReveal,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_add_take() {
        let mut inv = Inventory::default();
        assert!(inv.add_herb(HerbKind::HealingMoss));
        assert!(inv.has_herb(HerbKind::HealingMoss));
        assert!(!inv.has_herb(HerbKind::Thornbriar));

        assert!(inv.take_herb(HerbKind::HealingMoss));
        assert!(!inv.has_herb(HerbKind::HealingMoss));
        assert!(!inv.take_herb(HerbKind::HealingMoss)); // already taken
    }

    #[test]
    fn inventory_full() {
        let mut inv = Inventory::default();
        for _ in 0..Inventory::MAX_HERBS {
            assert!(inv.add_herb(HerbKind::Calmroot));
        }
        assert!(inv.is_full());
        assert!(!inv.add_herb(HerbKind::Thornbriar));
    }

    #[test]
    fn inventory_has_remedy_herb() {
        let mut inv = Inventory::default();
        assert!(!inv.has_remedy_herb());
        inv.add_herb(HerbKind::Thornbriar);
        assert!(!inv.has_remedy_herb()); // thornbriar is ward material
        inv.add_herb(HerbKind::HealingMoss);
        assert!(inv.has_remedy_herb());
    }

    #[test]
    fn ward_constructors() {
        let thorn = Ward::thornward();
        assert_eq!(thorn.kind, WardKind::Thornward);
        assert_eq!(thorn.strength, 1.0);
        assert!(!thorn.inverted);

        let inv = Ward::inverted_at(WardKind::DurableWard);
        assert!(inv.inverted);
        assert_eq!(inv.kind, WardKind::DurableWard);
    }

    #[test]
    fn remedy_required_herbs() {
        assert_eq!(
            RemedyKind::HealingPoultice.required_herb(),
            HerbKind::HealingMoss
        );
        assert_eq!(
            RemedyKind::EnergyTonic.required_herb(),
            HerbKind::Moonpetal
        );
        assert_eq!(RemedyKind::MoodTonic.required_herb(), HerbKind::Calmroot);
    }

    #[test]
    fn herb_seasonal_availability() {
        // Thornbriar available all seasons
        assert_eq!(HerbKind::Thornbriar.available_seasons().len(), 4);
        // Moonpetal only summer
        assert_eq!(HerbKind::Moonpetal.available_seasons(), &[Season::Summer]);
        // Dreamroot autumn/winter
        assert_eq!(
            HerbKind::Dreamroot.available_seasons(),
            &[Season::Autumn, Season::Winter]
        );
    }
}
