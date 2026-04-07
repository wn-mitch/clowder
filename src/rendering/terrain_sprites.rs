use crate::resources::map::{Terrain, TileMap};

/// Terrain rendering category — determines which sprite set to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainGroup {
    Grass,
    Water,
    Dirt,
    Sand,
    Rock,
    Stone,
    Building,
    Special,
}

impl Terrain {
    pub fn group(&self) -> TerrainGroup {
        match self {
            Terrain::Grass | Terrain::LightForest | Terrain::DenseForest
            | Terrain::Garden => TerrainGroup::Grass,
            Terrain::Water => TerrainGroup::Water,
            Terrain::Mud => TerrainGroup::Dirt,
            Terrain::Sand => TerrainGroup::Sand,
            Terrain::Rock => TerrainGroup::Rock,
            Terrain::Den | Terrain::Hearth | Terrain::Stores
            | Terrain::Workshop => TerrainGroup::Building,
            Terrain::Wall | Terrain::Gate | Terrain::Watchtower
            | Terrain::WardPost => TerrainGroup::Stone,
            Terrain::FairyRing | Terrain::StandingStone
            | Terrain::DeepPool | Terrain::AncientRuin => TerrainGroup::Special,
        }
    }
}

/// 4-bit bitmask for cardinal neighbors. Bit set = neighbor is same group.
/// Bit layout: N=0x1, E=0x2, S=0x4, W=0x8
pub fn cardinal_bitmask(map: &TileMap, x: i32, y: i32, group: TerrainGroup) -> u8 {
    let mut mask = 0u8;
    let same = |dx: i32, dy: i32| -> bool {
        let nx = x + dx;
        let ny = y + dy;
        if !map.in_bounds(nx, ny) {
            return true;
        }
        map.get(nx, ny).terrain.group() == group
    };
    // Note: TileMap is Y-down but bevy renders Y-up, so visual north = TileMap y-1.
    // The Sprout Lands tiles have edges labeled in visual (Y-up) convention,
    // so we map: visual N (y-1 in TileMap) = bit 0, visual S (y+1 in TileMap) = bit 2.
    if same(0, -1) { mask |= 0x1; } // Visual North (TileMap y-1)
    if same(1, 0)  { mask |= 0x2; } // East
    if same(0, 1)  { mask |= 0x4; } // Visual South (TileMap y+1)
    if same(-1, 0) { mask |= 0x8; } // West
    mask
}

/// Maps 4-bit cardinal bitmask to grass overlay atlas index.
/// Bitmask bits: N=0x1, E=0x2, S=0x4, W=0x8
pub fn grass_overlay_atlas_index(bitmask: u8) -> u32 {
    match bitmask {
        0b1111 => 0,  // NESW all present → center fill
        0b1110 => 1,  // ESW (no north) → north edge
        0b1101 => 2,  // NSW (no east) → east edge
        0b1011 => 3,  // NEW (no south) → south edge
        0b0111 => 4,  // NES (no west) → west edge
        0b1100 => 5,  // SW → NE corner
        0b1001 => 6,  // NW → SE corner
        0b0110 => 7,  // ES → NW corner
        0b0011 => 8,  // NE → SW corner
        0b1000 => 9,  // W only
        0b0100 => 10, // S only
        0b0010 => 11, // E only
        0b0001 => 12, // N only
        0b0000 => 13, // isolated
        0b1010 => 14, // N+S vertical strip
        0b0101 => 15, // E+W horizontal strip
        _ => 0,
    }
}

/// Base layer tile index for terrain.
pub fn base_tile_index(terrain: &Terrain) -> u32 {
    match terrain.group() {
        TerrainGroup::Grass | TerrainGroup::Special => 0,
        TerrainGroup::Water => 1,
        TerrainGroup::Dirt => 2,
        TerrainGroup::Sand => 3,
        TerrainGroup::Rock => 4,
        TerrainGroup::Stone => 5,
        TerrainGroup::Building => 6,
    }
}

/// Whether this terrain should have a grass overlay rendered on top.
pub fn has_grass_overlay(terrain: &Terrain) -> bool {
    matches!(terrain.group(), TerrainGroup::Grass | TerrainGroup::Special)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_map(terrain: &[&[Terrain]]) -> TileMap {
        let height = terrain.len() as i32;
        let width = terrain[0].len() as i32;
        let mut map = TileMap::new(width, height, Terrain::Water);
        for (y, row) in terrain.iter().enumerate() {
            for (x, &t) in row.iter().enumerate() {
                map.set(x as i32, y as i32, t);
            }
        }
        map
    }

    #[test]
    fn surrounded_grass_has_full_bitmask() {
        let map = make_map(&[
            &[Terrain::Grass, Terrain::Grass, Terrain::Grass],
            &[Terrain::Grass, Terrain::Grass, Terrain::Grass],
            &[Terrain::Grass, Terrain::Grass, Terrain::Grass],
        ]);
        assert_eq!(cardinal_bitmask(&map, 1, 1, TerrainGroup::Grass), 0b1111);
    }

    #[test]
    fn isolated_grass_has_zero_bitmask() {
        let map = make_map(&[
            &[Terrain::Water, Terrain::Water, Terrain::Water],
            &[Terrain::Water, Terrain::Grass, Terrain::Water],
            &[Terrain::Water, Terrain::Water, Terrain::Water],
        ]);
        assert_eq!(cardinal_bitmask(&map, 1, 1, TerrainGroup::Grass), 0b0000);
    }

    #[test]
    fn north_neighbor_sets_bit_0() {
        let map = make_map(&[
            &[Terrain::Water, Terrain::Grass, Terrain::Water],
            &[Terrain::Water, Terrain::Grass, Terrain::Water],
            &[Terrain::Water, Terrain::Water, Terrain::Water],
        ]);
        assert_eq!(cardinal_bitmask(&map, 1, 1, TerrainGroup::Grass), 0b0001);
    }

    #[test]
    fn out_of_bounds_counts_as_same_group() {
        let map = make_map(&[&[Terrain::Grass]]);
        assert_eq!(cardinal_bitmask(&map, 0, 0, TerrainGroup::Grass), 0b1111);
    }

    #[test]
    fn forest_counts_as_grass_group() {
        assert_eq!(Terrain::LightForest.group(), TerrainGroup::Grass);
        assert_eq!(Terrain::DenseForest.group(), TerrainGroup::Grass);
        assert_eq!(Terrain::Garden.group(), TerrainGroup::Grass);
    }

    #[test]
    fn terrain_group_classification() {
        assert_eq!(Terrain::Water.group(), TerrainGroup::Water);
        assert_eq!(Terrain::Rock.group(), TerrainGroup::Rock);
        assert_eq!(Terrain::Den.group(), TerrainGroup::Building);
        assert_eq!(Terrain::Wall.group(), TerrainGroup::Stone);
        assert_eq!(Terrain::Sand.group(), TerrainGroup::Sand);
    }
}
