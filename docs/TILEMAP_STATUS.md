# Tilemap Rendering — Current Status & Issues

## What Works

- **Two-layer rendering**: base terrain (solid colors) + grass overlay (autotile sprites)
- **bevy_ecs_tilemap 0.18**: spawns 80x60 tiles per layer, renders with frustum culling
- **4-bit cardinal bitmask autotiling**: checks N/E/S/W neighbors, selects from 16 grass variants
- **Custom atlas** (`assets/sprites/grass_autotile_atlas.png`): built from individual Sprout Lands v2 cutout tiles, 4x4 grid (64x64px)
- **Y-axis flip**: TileMap is Y-down, bevy is Y-up — corrected in `tilemap_sync.rs`
- **Camera**: 2D orthographic, WASD/arrow pan, scroll zoom
- **Auto-screenshot**: 2-second timer saves to `/tmp/clowder_screenshot.png`
- **Terrain dump**: writes `/tmp/clowder_terrain.txt` on startup
- **All 328 tests pass**

## Known Issues

### 1. Sprout Lands naming convention uncertainty
The cutout tile filenames (Flat_North, Flat_South, Corner_NorthEast, etc.) have ambiguous meaning:
- Does "Flat_North" mean "the edge faces north" or "the grass IS on the north side"?
- First attempt had edges backwards; swapped N↔S and E↔W in the atlas rebuild
- Current mapping may still have some tiles oriented wrong — needs zoomed-in visual verification against the terrain dump

**Key files:**
- Atlas build script: inline Python in conversation (needs to be saved as a script)
- Atlas image: `assets/sprites/grass_autotile_atlas.png`
- Index mapping: `src/rendering/terrain_sprites.rs` → `grass_overlay_atlas_index()`
- Cutout source: `assets/sprites/Sprout Lands - Sprites - premium pack/Tilesets/ground tiles/New tiles/simpel versions/Grass tiles v2 simple cutout/`

**How to verify:** Run the app, zoom into a grass-water boundary, compare the grass edge direction with expected behavior. The scalloped edge should protrude FROM the grass tile INTO the adjacent non-grass tile's space.

### 2. Base layer uses solid colors, not sprites
`assets/sprites/base_terrain_atlas.png` is a programmatically generated 7x1 atlas with flat-colored 16x16 tiles. Works but looks flat — should eventually use actual Sprout Lands terrain sprites.

Current colors: grass-green (index 0), water blue (1), mud brown (2), sand tan (3), rock gray (4), stone light-gray (5), building brown (6).

### 3. Non-grass terrain has no edge treatment
Mud, sand, rock, water — all render as flat colored squares with no edge blending. Where grass meets water, the grass overlay provides scalloped edges on the grass side, but the water side is just a blue rectangle. Ideally water would have its own autotile overlay.

### 4. Missing Flat_East cutout
The Sprout Lands pack has no `Flat_East.png`. The atlas uses `Flat_West` flipped horizontally. This might be correct or might produce subtle visual artifacts.

### 5. No tree/object sprites
Forest tiles (LightForest, DenseForest) render as plain grass with no tree objects on top. The spec calls for a third rendering layer (z=12) with tree/rock/decoration sprites.

### 6. No water animation
Water is a static blue square. The Sprout Lands Water.png has 4 animation frames. bevy_ecs_tilemap supports `AnimatedTile`.

### 7. Diagonal boundary artifacts
The 4-bit cardinal bitmask has only 16 states. Diagonal-heavy boundaries (stair-step patterns) don't look as smooth as they would with an 8-bit (full neighbor) bitmask. The inner corner tiles (Edge_NE, Edge_NW, etc.) exist in the cutout directory but aren't properly utilized.

## Architecture Reference

```
src/rendering/
  mod.rs              — RenderingPlugin, CameraPlugin
  terrain_sprites.rs  — TerrainGroup, cardinal_bitmask(), atlas index mapping, tests
  tilemap_sync.rs     — create_tilemap() startup system, TILE_SCALE/TILE_PX constants
  camera.rs           — GameCamera, setup_camera(), camera_controls(), auto_screenshot()
```

**Rendering order:**
- z=0: Base terrain layer (solid colors) — `BaseTerrainLayer` marker
- z=1: Grass overlay layer (autotile sprites) — `GrassOverlayLayer` marker

**Bitmask convention:**
- N=0x1, E=0x2, S=0x4, W=0x8
- Bit SET = neighbor IS same TerrainGroup
- 0b1111 = surrounded by same = center fill
- 0b0000 = isolated = all edges

**Coordinate mapping:**
- TileMap: (0,0) = top-left, Y increases downward
- bevy_ecs_tilemap: (0,0) = bottom-left, Y increases upward
- Conversion: `TilePos { x, y: map_height - 1 - y }`
