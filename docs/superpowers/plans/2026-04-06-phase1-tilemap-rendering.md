# Phase 1: Tile Map Rendering — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render the 80x60 TileMap as Sprout Lands pixel art sprites with bitmask autotiled grass edges, layered terrain, and a 2D camera with zoom/pan.

**Architecture:** Two-layer tilemap (base ground + grass overlay) using bevy_ecs_tilemap. A sync system reads the ECS `TileMap` resource each frame and updates sprite indices. Autotiling computes 4-bit cardinal bitmasks for grass edges. A 2D orthographic camera provides scroll/zoom.

**Tech Stack:** bevy 0.18, bevy_ecs_tilemap 0.18, Sprout Lands v2 tilesets (premium pack)

---

## File Structure

| File | Responsibility |
|------|---------------|
| `Cargo.toml` | Add bevy_ecs_tilemap dependency |
| `src/rendering/mod.rs` | Module declarations + RenderingPlugin |
| `src/rendering/terrain_sprites.rs` | Terrain→sprite index mapping, autotile bitmask calculation |
| `src/rendering/tilemap_sync.rs` | Startup tilemap creation + runtime sync system |
| `src/rendering/camera.rs` | 2D camera setup, zoom/pan controls |
| `src/lib.rs` | Add `pub mod rendering` |
| `src/main.rs` | Register RenderingPlugin + CameraPlugin, configure pixel-perfect rendering |

---

## Task 1: Add bevy_ecs_tilemap dependency and pixel-perfect rendering

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs`

- [ ] **Step 1: Add bevy_ecs_tilemap to Cargo.toml**

```toml
# Add to [dependencies]
bevy_ecs_tilemap = "0.18"
```

- [ ] **Step 2: Configure pixel-perfect rendering in main.rs**

In `src/main.rs`, modify the `DefaultPlugins` setup to use nearest-neighbor filtering (critical for pixel art — without this, sprites will be blurry):

```rust
// Replace:
.add_plugins(DefaultPlugins.set(WindowPlugin {
// With:
.add_plugins(DefaultPlugins
    .set(ImagePlugin::default_nearest())
    .set(WindowPlugin {
```

Add to imports:
```rust
use bevy::prelude::ImagePlugin;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: Compiles with no errors.

- [ ] **Step 4: Commit**

```bash
jj describe -m "chore: add bevy_ecs_tilemap and configure pixel-perfect rendering"
jj new
```

---

## Task 2: Create terrain sprite mapping with autotile bitmask

**Files:**
- Create: `src/rendering/mod.rs`
- Create: `src/rendering/terrain_sprites.rs`
- Modify: `src/lib.rs`

This task implements the core logic: given a terrain type and its neighbors, which sprite index should we use?

The Sprout Lands v2 Grass_tiles_v2.png atlas (176x112) is NOT a uniform grid. Instead, we use the individual cutout files from the premium pack to understand the tile positions, then map them to atlas coordinates.

**Autotile approach:** The Sprout Lands system uses layered rendering:
- **Base layer**: Solid fill tile (dirt, sand, water, stone)
- **Grass overlay**: Edge/corner variants painted on top, selected by checking which neighbors are also "grassy"

For the 4-bit cardinal bitmask, we check N/E/S/W neighbors. Each bit indicates whether that neighbor is the same terrain group (e.g., "grassy"). This gives 16 combinations:
- 0b0000 (isolated) → full corner piece
- 0b1111 (surrounded) → center fill
- 0b0001 (only north) → south edge
- etc.

- [ ] **Step 1: Create module structure**

`src/rendering/mod.rs`:
```rust
pub mod terrain_sprites;
pub mod tilemap_sync;
pub mod camera;

use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TilemapPlugin);
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, camera::setup_camera);
        app.add_systems(Update, camera::camera_controls);
    }
}
```

Add to `src/lib.rs`:
```rust
pub mod rendering;
```

- [ ] **Step 2: Implement terrain classification and bitmask calculation**

`src/rendering/terrain_sprites.rs`:
```rust
use crate::resources::map::{Terrain, TileMap};

/// Terrain rendering category — determines which sprite set to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainGroup {
    /// Grass overlay rendered on top of dirt base.
    Grass,
    /// Water tiles (autotiled edges, animated).
    Water,
    /// Bare dirt/soil — used as the base layer.
    Dirt,
    /// Sandy ground.
    Sand,
    /// Rocky ground.
    Rock,
    /// Stone building floor.
    Stone,
    /// Building interior (den, hearth, stores, etc.).
    Building,
    /// Special/magical terrain.
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
///
/// Bit layout: N=0x1, E=0x2, S=0x4, W=0x8
pub fn cardinal_bitmask(map: &TileMap, x: i32, y: i32, group: TerrainGroup) -> u8 {
    let mut mask = 0u8;
    let same = |dx: i32, dy: i32| -> bool {
        let nx = x + dx;
        let ny = y + dy;
        if !map.in_bounds(nx, ny) {
            // Out-of-bounds neighbors count as same group (avoids edge artifacts).
            return true;
        }
        map.get(nx, ny).terrain.group() == group
    };
    if same(0, -1) { mask |= 0x1; } // North (y-1 in row-major)
    if same(1, 0)  { mask |= 0x2; } // East
    if same(0, 1)  { mask |= 0x4; } // South
    if same(-1, 0) { mask |= 0x8; } // West
    mask
}

/// Sprite indices into the grass overlay atlas (Grass_tiles_v2.png).
///
/// The atlas is 176x112 with irregular tile placement. These indices
/// are column-major positions in an 11x7 grid (16px cells), mapping
/// bitmask values to specific cells. Indices are (col, row) in the atlas.
///
/// We map the 16 cardinal bitmask values to atlas positions.
/// For now, use a lookup table that maps bitmask → (atlas_col, atlas_row).
pub fn grass_overlay_atlas_index(bitmask: u8) -> u32 {
    // This maps 4-bit cardinal bitmask → index into a flat atlas.
    // The indices correspond to the Grass_tiles_v2.png layout.
    //
    // Bitmask bits: N=0x1, E=0x2, S=0x4, W=0x8
    //
    // These atlas indices will be determined by examining the actual
    // sprite sheet layout. For now, use the center tile (index 0) as
    // the default and map known configurations.
    //
    // Index encoding: row * columns + col, where the atlas is treated
    // as a uniform 11-column grid.
    match bitmask {
        0b1111 => 0,  // NESW all present → center fill
        0b1110 => 1,  // ESW (no north) → north edge
        0b1101 => 2,  // NSW (no east) → east edge
        0b1011 => 3,  // NEW (no south) → south edge
        0b0111 => 4,  // NES (no west) → west edge
        0b1100 => 5,  // SW (no north/east) → NE corner
        0b1001 => 6,  // NW (no east/south) → SE corner
        0b0110 => 7,  // ES (no north/west) → NW corner
        0b0011 => 8,  // NE (no south/west) → SW corner
        0b1000 => 9,  // W only → east/north/south edges
        0b0100 => 10, // S only
        0b0010 => 11, // E only
        0b0001 => 12, // N only
        0b0000 => 13, // isolated → full border
        0b1010 => 14, // N+S (vertical strip)
        0b0101 => 15, // E+W (horizontal strip)
        _ => 0,
    }
}

/// Base layer tile index for non-grass terrain.
pub fn base_tile_index(terrain: &Terrain) -> u32 {
    match terrain.group() {
        TerrainGroup::Grass | TerrainGroup::Special => 0, // Dirt base under grass
        TerrainGroup::Water => 1,    // Water tile (animated separately)
        TerrainGroup::Dirt => 2,     // Mud/dirt
        TerrainGroup::Sand => 3,     // Sand
        TerrainGroup::Rock => 4,     // Rock
        TerrainGroup::Stone => 5,    // Stone floor
        TerrainGroup::Building => 6, // Building floor
    }
}

/// Whether this terrain should have a grass overlay rendered on top.
pub fn has_grass_overlay(terrain: &Terrain) -> bool {
    matches!(
        terrain.group(),
        TerrainGroup::Grass | TerrainGroup::Special
    )
}

/// Whether this terrain should display a tree/object sprite on top.
pub fn tree_object_index(terrain: &Terrain) -> Option<u32> {
    match terrain {
        Terrain::LightForest => Some(0), // Small tree
        Terrain::DenseForest => Some(1), // Large tree
        Terrain::Rock => Some(2),        // Rock cluster
        Terrain::FairyRing => Some(3),   // Fairy ring decoration
        Terrain::StandingStone => Some(4),
        Terrain::AncientRuin => Some(5),
        _ => None,
    }
}
```

- [ ] **Step 3: Write tests for bitmask calculation**

Add to the bottom of `src/rendering/terrain_sprites.rs`:
```rust
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
        let mask = cardinal_bitmask(&map, 1, 1, TerrainGroup::Grass);
        assert_eq!(mask, 0b1111);
    }

    #[test]
    fn isolated_grass_has_zero_bitmask() {
        let map = make_map(&[
            &[Terrain::Water, Terrain::Water, Terrain::Water],
            &[Terrain::Water, Terrain::Grass, Terrain::Water],
            &[Terrain::Water, Terrain::Water, Terrain::Water],
        ]);
        let mask = cardinal_bitmask(&map, 1, 1, TerrainGroup::Grass);
        assert_eq!(mask, 0b0000);
    }

    #[test]
    fn north_neighbor_sets_bit_0() {
        let map = make_map(&[
            &[Terrain::Water, Terrain::Grass, Terrain::Water],
            &[Terrain::Water, Terrain::Grass, Terrain::Water],
            &[Terrain::Water, Terrain::Water, Terrain::Water],
        ]);
        let mask = cardinal_bitmask(&map, 1, 1, TerrainGroup::Grass);
        assert_eq!(mask, 0b0001); // N only
    }

    #[test]
    fn out_of_bounds_counts_as_same_group() {
        let map = make_map(&[
            &[Terrain::Grass],
        ]);
        // All 4 neighbors are out of bounds → treated as same → 0b1111
        let mask = cardinal_bitmask(&map, 0, 0, TerrainGroup::Grass);
        assert_eq!(mask, 0b1111);
    }

    #[test]
    fn forest_counts_as_grass_group() {
        let map = make_map(&[
            &[Terrain::LightForest, Terrain::DenseForest],
            &[Terrain::Grass, Terrain::Garden],
        ]);
        // All are TerrainGroup::Grass → full bitmask for center check
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
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib rendering::terrain_sprites`
Expected: All 6 tests pass.

- [ ] **Step 5: Commit**

```bash
jj describe -m "feat: add terrain sprite mapping with autotile bitmask calculation"
jj new
```

---

## Task 3: Create tilemap startup and sync systems

**Files:**
- Create: `src/rendering/tilemap_sync.rs`
- Modify: `src/rendering/mod.rs`

This creates the bevy_ecs_tilemap entities at startup and syncs them with the simulation's TileMap resource.

- [ ] **Step 1: Implement tilemap creation and sync**

`src/rendering/tilemap_sync.rs`:
```rust
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use crate::rendering::terrain_sprites::{
    base_tile_index, cardinal_bitmask, grass_overlay_atlas_index, has_grass_overlay,
};
use crate::resources::map::TileMap;

/// Marker component for the base terrain layer.
#[derive(Component)]
pub struct BaseTerrainLayer;

/// Marker component for the grass overlay layer.
#[derive(Component)]
pub struct GrassOverlayLayer;

/// Tile scale factor: 16px sprites rendered at this multiplier.
pub const TILE_SCALE: f32 = 3.0;
/// Pixel size of each tile in the sprite sheet.
pub const TILE_PX: f32 = 16.0;

/// Startup system: creates the two-layer tilemap from the TileMap resource.
pub fn create_tilemap(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    map: Res<TileMap>,
) {
    let map_width = map.width as u32;
    let map_height = map.height as u32;
    let map_size = TilemapSize { x: map_width, y: map_height };
    let tile_size = TilemapTileSize { x: TILE_PX, y: TILE_PX };
    let grid_size = TilemapGridSize { x: TILE_PX, y: TILE_PX };

    // --- Base terrain layer (z=0.0) ---
    let base_texture: Handle<Image> =
        asset_server.load("sprites/Sprout Lands - Sprites - premium pack/Tilesets/ground tiles/New tiles/Soil_Ground_Tiles.png");

    let base_tilemap_entity = commands.spawn_empty().id();
    let mut base_storage = TileStorage::empty(map_size);

    for y in 0..map_height {
        for x in 0..map_width {
            let tile_pos = TilePos { x, y };
            let terrain = &map.get(x as i32, y as i32).terrain;
            let tile_entity = commands
                .spawn(TileBundle {
                    position: tile_pos,
                    texture_index: TileTextureIndex(base_tile_index(terrain)),
                    tilemap_id: TilemapId(base_tilemap_entity),
                    ..Default::default()
                })
                .id();
            base_storage.set(&tile_pos, tile_entity);
        }
    }

    commands.entity(base_tilemap_entity).insert((
        TilemapBundle {
            grid_size,
            map_type: TilemapType::Square,
            size: map_size,
            spacing: TilemapSpacing::zero(),
            storage: base_storage,
            texture: TilemapTexture::Single(base_texture),
            tile_size,
            transform: Transform::from_scale(Vec3::splat(TILE_SCALE)),
            ..Default::default()
        },
        BaseTerrainLayer,
    ));

    // --- Grass overlay layer (z=1.0) ---
    let grass_texture: Handle<Image> =
        asset_server.load("sprites/Sprout Lands - Sprites - premium pack/Tilesets/ground tiles/New tiles/Grass_tiles_v2.png");

    let grass_tilemap_entity = commands.spawn_empty().id();
    let mut grass_storage = TileStorage::empty(map_size);

    for y in 0..map_height {
        for x in 0..map_width {
            let tile_pos = TilePos { x, y };
            let terrain = &map.get(x as i32, y as i32).terrain;

            if !has_grass_overlay(terrain) {
                continue; // Skip non-grass tiles — leave storage empty at this position
            }

            let bitmask = cardinal_bitmask(&map, x as i32, y as i32, terrain.group());
            let atlas_index = grass_overlay_atlas_index(bitmask);

            let tile_entity = commands
                .spawn(TileBundle {
                    position: tile_pos,
                    texture_index: TileTextureIndex(atlas_index),
                    tilemap_id: TilemapId(grass_tilemap_entity),
                    ..Default::default()
                })
                .id();
            grass_storage.set(&tile_pos, tile_entity);
        }
    }

    commands.entity(grass_tilemap_entity).insert((
        TilemapBundle {
            grid_size,
            map_type: TilemapType::Square,
            size: map_size,
            spacing: TilemapSpacing::zero(),
            storage: grass_storage,
            texture: TilemapTexture::Single(grass_texture),
            tile_size,
            transform: Transform {
                translation: Vec3::new(0.0, 0.0, 1.0),
                scale: Vec3::splat(TILE_SCALE),
                ..Default::default()
            },
            ..Default::default()
        },
        GrassOverlayLayer,
    ));
}
```

- [ ] **Step 2: Register the tilemap startup system in RenderingPlugin**

Update `src/rendering/mod.rs`:
```rust
pub mod terrain_sprites;
pub mod tilemap_sync;
pub mod camera;

use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TilemapPlugin)
            .add_systems(Startup, tilemap_sync::create_tilemap);
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, camera::setup_camera)
            .add_systems(Update, camera::camera_controls);
    }
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: Compiles (camera module doesn't exist yet — create a stub).

Create stub `src/rendering/camera.rs`:
```rust
use bevy::prelude::*;

pub fn setup_camera(_commands: Commands) {}
pub fn camera_controls() {}
```

- [ ] **Step 4: Commit**

```bash
jj describe -m "feat: create two-layer tilemap from TileMap resource at startup"
jj new
```

---

## Task 4: Implement 2D camera with zoom and pan

**Files:**
- Modify: `src/rendering/camera.rs`

- [ ] **Step 1: Implement camera setup and controls**

`src/rendering/camera.rs`:
```rust
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

use crate::rendering::tilemap_sync::{TILE_PX, TILE_SCALE};
use crate::resources::map::TileMap;

/// Marker for the main game camera.
#[derive(Component)]
pub struct GameCamera;

/// Startup system: spawns a 2D camera centered on the colony.
pub fn setup_camera(mut commands: Commands, map: Res<TileMap>) {
    let world_px = TILE_PX * TILE_SCALE;
    let center_x = (map.width as f32 / 2.0) * world_px;
    let center_y = (map.height as f32 / 2.0) * world_px;

    commands.spawn((
        Camera2d,
        Transform::from_xyz(center_x, center_y, 999.0),
        OrthographicProjection {
            scale: 1.0,
            ..OrthographicProjection::default_2d()
        },
        GameCamera,
    ));
}

/// Update system: scroll wheel zooms, arrow keys / WASD pans.
pub fn camera_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut scroll_events: EventReader<MouseWheel>,
    mut query: Query<(&mut Transform, &mut OrthographicProjection), With<GameCamera>>,
    time: Res<Time>,
) {
    let Ok((mut transform, mut projection)) = query.single_mut() else {
        return;
    };

    // Zoom with scroll wheel.
    for event in scroll_events.read() {
        let zoom_delta = -event.y * 0.1;
        projection.scale = (projection.scale + zoom_delta).clamp(0.2, 5.0);
    }

    // Pan with arrow keys or WASD.
    let pan_speed = 500.0 * projection.scale * time.delta_secs();
    let mut direction = Vec2::ZERO;

    if keyboard.pressed(KeyCode::ArrowLeft) || keyboard.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }
    if keyboard.pressed(KeyCode::ArrowDown) || keyboard.pressed(KeyCode::KeyS) {
        direction.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::ArrowUp) || keyboard.pressed(KeyCode::KeyW) {
        direction.y += 1.0;
    }

    if direction != Vec2::ZERO {
        direction = direction.normalize();
        transform.translation.x += direction.x * pan_speed;
        transform.translation.y += direction.y * pan_speed;
    }
}
```

- [ ] **Step 2: Fix S key conflict in main.rs**

The `handle_input` system in `main.rs` uses `KeyCode::KeyS` for speed cycling. The camera uses S for panning. Remove speed cycling from S — use a different key:

In `src/main.rs`, change:
```rust
if keyboard.just_pressed(KeyCode::KeyS) {
    time.speed = time.speed.cycle();
}
```
to:
```rust
if keyboard.just_pressed(KeyCode::BracketRight) {
    time.speed = time.speed.cycle();
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
jj describe -m "feat: add 2D camera with scroll zoom and WASD/arrow pan"
jj new
```

---

## Task 5: Wire everything together in main.rs and test visually

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Register RenderingPlugin and CameraPlugin**

In `src/main.rs`, add imports and register plugins:

```rust
// Add import:
use clowder::rendering::{RenderingPlugin, CameraPlugin};

// In the App builder, add after SimulationPlugin:
.add_plugins(RenderingPlugin)
.add_plugins(CameraPlugin)
```

- [ ] **Step 2: Run the application**

Run: `cargo run`

Expected: A window opens showing the tile map rendered with sprites. Grass areas should have edge variants. Arrow keys/WASD pan, scroll wheel zooms.

The initial rendering won't be perfect — the atlas index mapping in `grass_overlay_atlas_index` uses placeholder indices that need tuning to match the actual Grass_tiles_v2.png layout. That's Task 6.

- [ ] **Step 3: Run all tests to verify nothing broke**

Run: `cargo test`
Expected: All existing tests pass + new terrain_sprites tests pass.

- [ ] **Step 4: Commit**

```bash
jj describe -m "feat: wire up tile map rendering with camera controls"
jj new
```

---

## Task 6: Tune atlas indices to match Sprout Lands sprite layout

**Files:**
- Modify: `src/rendering/terrain_sprites.rs`

This is the visual tuning step. Run the app, see which sprites appear for which terrain configurations, and adjust the index mapping until it looks correct.

- [ ] **Step 1: Examine the Grass_tiles_v2.png atlas layout**

The atlas is 176x112 pixels. At 16px per tile, that's an 11x7 grid. However, the tiles are NOT uniformly placed in this grid — some positions are empty or contain larger composite sprites.

Read the atlas image and map each cell to its visual meaning. Update the `grass_overlay_atlas_index` function to return correct indices.

Also verify: does bevy_ecs_tilemap index tiles left-to-right, top-to-bottom? The index formula is typically `row * columns + col`.

- [ ] **Step 2: Update base_tile_index for the Soil_Ground_Tiles atlas**

Similarly, the Soil_Ground_Tiles.png atlas has specific positions for different ground types. Map them correctly.

- [ ] **Step 3: Iterate visually**

Run: `cargo run`

Pan around the map. Check:
- Grass areas show correct edges where they meet water/rock/sand
- Buildings show their correct floor tiles
- No misaligned or obviously wrong sprites

Adjust indices as needed. This step may require several iterations.

- [ ] **Step 4: Commit**

```bash
jj describe -m "fix: tune sprite atlas indices to match Sprout Lands layout"
jj new
```

---

## Task 7: Run full verification

- [ ] **Step 1: Run all tests**

Run: `cargo test`
Expected: All tests pass (322 existing + new terrain_sprites tests).

- [ ] **Step 2: Run clippy**

Run: `cargo clippy`
Expected: No warnings.

- [ ] **Step 3: Visual verification**

Run: `cargo run`

Check:
- Tile map renders with Sprout Lands sprites
- Grass edges autotile correctly
- Camera zooms and pans smoothly
- Simulation still ticks (check narrative log or press speed key)
- Escape quits cleanly
- Headless mode still works: `cargo run -- --headless --duration 5`

- [ ] **Step 4: Final commit**

```bash
jj describe -m "test: verify Phase 1 tile map rendering"
jj new
```
