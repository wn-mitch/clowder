use bevy_ecs::prelude::Resource;

// ---------------------------------------------------------------------------
// Terrain
// ---------------------------------------------------------------------------

/// The terrain type of a map tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Terrain {
    // Natural
    Grass,
    LightForest,
    DenseForest,
    Water,
    Rock,
    Mud,
    Sand,
    // Settlement
    Den,
    Hearth,
    Stores,
    Workshop,
    Garden,
    // Defensive
    Watchtower,
    WardPost,
    Wall,
    Gate,
    // Special
    FairyRing,
    StandingStone,
    DeepPool,
    AncientRuin,
}

impl Terrain {
    /// Movement cost in abstract ticks. `u32::MAX` means impassable.
    pub fn movement_cost(self) -> u32 {
        match self {
            Self::Grass | Self::Sand | Self::Den | Self::Hearth | Self::Stores | Self::Workshop
            | Self::Watchtower | Self::WardPost | Self::Gate => 1,
            Self::LightForest | Self::Mud | Self::Garden => 2,
            Self::Wall => u32::MAX,
            Self::DenseForest => 3,
            Self::Rock => 4,
            Self::Water => u32::MAX,
            Self::FairyRing | Self::StandingStone | Self::DeepPool | Self::AncientRuin => 2,
            // Wall handled above (u32::MAX)
        }
    }

    /// Single-character map symbol.
    pub fn symbol(self) -> char {
        match self {
            Self::Grass => '.',
            Self::LightForest => 't',
            Self::DenseForest => 'T',
            Self::Water => '~',
            Self::Rock => '#',
            Self::Mud => ',',
            Self::Sand => ':',
            Self::Den => 'D',
            Self::Hearth => 'H',
            Self::Stores => 'S',
            Self::Workshop => 'W',
            Self::Garden => 'G',
            Self::Watchtower => '^',
            Self::WardPost => '+',
            Self::Wall => '=',
            Self::Gate => '|',
            Self::FairyRing => '*',
            Self::StandingStone => '!',
            Self::DeepPool => 'O',
            Self::AncientRuin => '?',
        }
    }

    /// How much shelter from weather and danger this terrain provides (0.0–1.0).
    pub fn shelter_value(self) -> f32 {
        match self {
            Self::Den => 1.0,
            Self::DenseForest => 0.6,
            Self::LightForest => 0.3,
            Self::Hearth | Self::Stores | Self::Workshop => 0.8,
            Self::Watchtower => 0.3,
            Self::AncientRuin => 0.5,
            _ => 0.0,
        }
    }

    /// Expected food yield per forage action on this terrain (0.0–1.0).
    pub fn foraging_yield(self) -> f32 {
        match self {
            Self::DenseForest => 0.5,
            Self::LightForest => 0.3,
            Self::Garden => 0.8,
            Self::Grass => 0.1,
            _ => 0.0,
        }
    }

    /// Whether a creature can move onto this terrain at all.
    pub fn is_passable(self) -> bool {
        self.movement_cost() != u32::MAX
    }

    /// Whether wildlife can move onto this terrain.
    ///
    /// Walls and gates block wildlife (gate entity state is checked separately
    /// for open gates, but the terrain-level check blocks by default).
    pub fn is_wildlife_passable(self) -> bool {
        !matches!(self, Self::Wall | Self::Gate) && self.is_passable()
    }
}

// ---------------------------------------------------------------------------
// Tile
// ---------------------------------------------------------------------------

/// A single cell in the world map.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Tile {
    pub terrain: Terrain,
    /// Accumulated magical corruption in this cell (0.0 = none, 1.0 = fully corrupted).
    pub corruption: f32,
    /// Ambient mystery / magical resonance (0.0 = mundane, 1.0 = saturated).
    pub mystery: f32,
}

impl Tile {
    pub fn new(terrain: Terrain) -> Self {
        Self {
            terrain,
            corruption: 0.0,
            mystery: 0.0,
        }
    }

    pub fn new_with(terrain: Terrain, corruption: f32, mystery: f32) -> Self {
        Self {
            terrain,
            corruption,
            mystery,
        }
    }
}

// ---------------------------------------------------------------------------
// TileMap
// ---------------------------------------------------------------------------

/// The world map. Stored as a flat `Vec<Tile>` in row-major order.
#[derive(Resource, serde::Serialize, serde::Deserialize)]
pub struct TileMap {
    pub width: i32,
    pub height: i32,
    tiles: Vec<Tile>,
}

impl TileMap {
    /// Create a new map filled entirely with `default_terrain`.
    pub fn new(width: i32, height: i32, default_terrain: Terrain) -> Self {
        assert!(width > 0 && height > 0, "map dimensions must be positive");
        let capacity = (width * height) as usize;
        let tiles = (0..capacity).map(|_| Tile::new(default_terrain)).collect();
        Self {
            width,
            height,
            tiles,
        }
    }

    /// Construct from a pre-built tile vec (used by save/load).
    pub fn from_raw(width: i32, height: i32, tiles: Vec<Tile>) -> Self {
        assert_eq!(
            tiles.len(),
            (width * height) as usize,
            "tile count must match dimensions"
        );
        Self {
            width,
            height,
            tiles,
        }
    }

    /// Returns `true` if `(x, y)` is within the map bounds.
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.width && y >= 0 && y < self.height
    }

    fn index(&self, x: i32, y: i32) -> usize {
        assert!(self.in_bounds(x, y), "({x}, {y}) is out of bounds");
        (y * self.width + x) as usize
    }

    /// Immutable tile access.
    pub fn get(&self, x: i32, y: i32) -> &Tile {
        let idx = self.index(x, y);
        &self.tiles[idx]
    }

    /// Mutable tile access.
    pub fn get_mut(&mut self, x: i32, y: i32) -> &mut Tile {
        let idx = self.index(x, y);
        &mut self.tiles[idx]
    }

    /// Replace the terrain type of a tile without touching other fields.
    pub fn set(&mut self, x: i32, y: i32, terrain: Terrain) {
        self.get_mut(x, y).terrain = terrain;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_map_dimensions() {
        let map = TileMap::new(80, 60, Terrain::Grass);
        assert_eq!(map.width, 80);
        assert_eq!(map.height, 60);
    }

    #[test]
    fn tile_map_get_set() {
        let mut map = TileMap::new(10, 10, Terrain::Grass);
        map.set(3, 4, Terrain::Water);
        assert_eq!(map.get(3, 4).terrain, Terrain::Water);
        assert_eq!(map.get(0, 0).terrain, Terrain::Grass);
    }

    #[test]
    fn tile_map_bounds() {
        let map = TileMap::new(10, 10, Terrain::Grass);
        assert!(map.in_bounds(0, 0));
        assert!(map.in_bounds(9, 9));
        assert!(!map.in_bounds(10, 0));
        assert!(!map.in_bounds(0, 10));
        assert!(!map.in_bounds(-1, 0));
    }

    #[test]
    fn terrain_movement_cost() {
        assert_eq!(Terrain::Grass.movement_cost(), 1);
        assert_eq!(Terrain::DenseForest.movement_cost(), 3);
        assert_eq!(Terrain::Water.movement_cost(), u32::MAX);
    }

    #[test]
    fn terrain_symbol() {
        assert_eq!(Terrain::Grass.symbol(), '.');
        assert_eq!(Terrain::Water.symbol(), '~');
        assert_eq!(Terrain::DenseForest.symbol(), 'T');
    }
}
