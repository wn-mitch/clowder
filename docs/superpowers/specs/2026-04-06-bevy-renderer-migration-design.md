# Bevy Renderer Migration

Migrate clowder from ratatui terminal rendering to full Bevy 2D with pixel art
sprites. The simulation is a "digital aquarium" — zero player control, pure
observation — so visual appeal is load-bearing.

## Context

The current TUI renders each tile as a single terminal character. Most of the
screen is static terrain with a handful of cat characters moving in a small
area. For a passive observation experience, this doesn't create the "watching an
aquarium" feeling the project needs.

The simulation already runs on bevy_ecs 0.18 standalone (manual World +
Schedule, ~200 systems). Upgrading to full Bevy adds rendering infrastructure
while preserving all simulation code.

## Decisions

- **Renderer:** Full Bevy 0.18 (upgrade from bevy_ecs standalone)
- **Art style:** Cozy pixel art using Sprout Lands asset packs (terrain,
  buildings, UI) from CupNooble. 16x16 base tiles, rendered at 3x scale (48px
  per tile on screen) for visibility on modern displays.
- **Cat sprites:** Sprout Lands character sprites (48x48, 3x3 tiles) as base,
  palette-swapped for fur colors. Already somewhat cat-like.
- **UI:** Pixel art panels using Sprout Lands UI pack with bevy_ui 9-slice
  rendering
- **Camera:** Ambient drift with activity-aware lingering and manual override
- **Platform:** Desktop native (Mac/Windows/Linux)

## Architecture

### App Structure

```
App::new()
  .add_plugins(DefaultPlugins)       // window, rendering, input, assets
  .add_plugins(SimulationPlugin)     // existing systems → FixedUpdate
  .add_plugins(RenderingPlugin)      // tile map, entity sprites, effects
  .add_plugins(CameraPlugin)         // ambient drift camera
  .add_plugins(UiPlugin)             // pixel art panels
  .add_plugins(InputPlugin)          // observer controls
  .run();
```

Simulation systems move to `FixedUpdate` (replaces the manual tick
accumulator). Rendering runs on `Update` at display refresh rate. Sim speed
changes adjust the FixedUpdate timestep.

### Rendering Layers

Three z-layers, bottom to top:

| Layer | z | Contents |
|-------|---|----------|
| Tile map | 10 | Terrain, buildings, gates, farm plots |
| Entities | 20 | Cats, wildlife, herbs, wards, items |
| Effects | 30 | Weather particles, corruption overlay, magic auras, day/night tint |

**Tile map:** `bevy_ecs_tilemap` for GPU-accelerated chunked rendering. A sync
system reads the `TileMap` resource and maps each `Terrain` variant to a sprite
index in the Sprout Lands atlas.

**Entities:** Each cat/wildlife/herb entity gets `Transform` + `Sprite`
components. A `sync_positions` system converts grid `Position` → pixel
`Transform` each frame. Sprite appearance driven by existing components
(LifeStage, Appearance, WildSpecies, WildlifeBehavior).

**Effects:** Visual overlays independent of ECS entities. Corruption as
semi-transparent dark magenta tile overlay. Weather as particle systems. Magic
as glow/particle effects. Day/night as a global color tint.

### Camera System

State machine with four modes:

- **Drift** (default): Slow pan across colony (~0.5-1 tile/sec), Perlin noise
  on velocity for organic direction changes. Medium zoom showing ~20x15 tiles.
- **Linger**: When drift approaches active entities (eating, fighting,
  crafting, playing), camera decelerates. Resumes drift after ~5-10 sec of
  inactivity.
- **Follow**: Significant events (birth, death, raid, feast, corruption surge)
  trigger a smooth 2-second pan to the event location. Lingers, then resumes
  drift.
- **Override**: Observer takes manual control via scroll wheel (zoom), click-drag
  or WASD (pan), or click-entity (follow). Escape or idle timeout returns to
  drift.

All modes write to a `camera_target: Vec2`. A single system lerps the actual
camera `Transform` toward the target each frame, ensuring smooth transitions
between all modes.

### UI Panels

Screen-space overlays using `bevy_ui` with Sprout Lands UI pack sprites:

| Panel | Purpose | Implementation |
|-------|---------|----------------|
| Narrative log | Scrolling event text | 9-slice parchment border, pixel font text |
| Cat inspect | Stats/needs/personality for selected cat | Portrait frame + icon-based stat bars |
| Status bar | Speed, season/day, population | Bottom strip with icon sprites |
| Tile inspect | Terrain/building info on hover | Small popup panel near cursor |

Panels use `UiImage` nodes with `ImageScaleMode::Sliced` for resizable borders.
Default to minimal visibility (log only) with panels appearing on
hover/click — lean into the aquarium vibe.

### Asset Pipeline

Assets are already in `assets/sprites/` organized by pack.

**Terrain (Sprout Lands - Sprites - Basic pack/Tilesets/):**
- `Grass.png` (176x112) — bitmask autotile system. Tile selection is
  neighbor-aware, not a simple enum lookup. Bitmask reference images included.
- `Water.png` (64x16) — 4 animation frames for animated water.
- `Hills.png` — elevated terrain with cliff edges, paths, stairs.
- `Fences.png` — fence segments in different configurations.
- `Wooden House.png`, `Doors.png` — building components.
- `Tilled_Dirt.png` — farmland tiles.

**Objects (Sprout Lands - Sprites - Basic pack/Objects/):**
- `Basic_Grass_Biom_things.png` — trees, mushrooms, rocks, flowers, pumpkins.
- `Basic_Plants.png` — crops at different growth stages.

**Characters (Sprout Lands - Sprites - Basic pack/Characters/):**
- `Basic Charakter Spritesheet.png` (192x192) — 48x48 frames (3x3 tiles per
  character), 4 directions. Already somewhat cat-like — usable as a base with
  palette swapping for fur colors.
- `Basic Charakter Actions.png` — 6+ action animations (chop, water, etc.) in
  4 directions.
- `Free Cow Sprites.png`, `Free Chicken Sprites.png` — animals (potential
  wildlife bases).

**UI (Sprout Lands - UI Pack/):**
- `dialog box.png` (48x48) — 9-slice ready, 16x16 corner/edge tiles.
- `Sprite sheet for Basic Pack.png` — buttons, inventory slots, progress bars,
  settings icons, play buttons.
- `All Icons.png` — tools, currency, hearts, settings, checkmarks.
- `pixel-letters-7-8x14.png` — bitmap font (upper, lower, numbers, punctuation).
- `Emoji_Spritesheet_Free.png` — emotes for speech bubbles.
- `Teemo Basic emote animations sprite sheet.png` — cat emotes.
- Cat paw mouse cursor sprites.
- `Weather_UI_Free.png`, `Weather_Icons_smal_freel.png` — weather indicators.
- Premium pack adds colored button variants and more panel styles.

**Key implication:** Terrain uses bitmask autotiling, which means the tile map
sync system needs to check each tile's neighbors and select the correct variant.
This is more complex than a simple Terrain→sprite index mapping but produces
smooth terrain transitions.

### What Changes vs What Stays

**Unchanged (simulation):**
- All ~200 simulation systems (needs, AI, combat, food, weather, buildings, magic, etc.)
- All components and resources
- ECS schedule ordering and system dependencies
- RON data files
- Game logic, personality model, utility scoring

**Changed (shell):**
- Main loop: manual World+Schedule → App::run() with plugins
- Rendering: ratatui direct buffer → Bevy 2D sprites + camera
- Input: crossterm event polling → Bevy ButtonInput
- UI: ratatui Paragraph/Block → bevy_ui with 9-slice sprites
- Frame timing: manual 33ms sleep → Bevy render loop (vsync)

**Removed (after migration):**
- `src/tui/` directory (map.rs, log.rs, inspect.rs, status.rs, tile_inspect.rs, mod.rs)
- ratatui and crossterm dependencies
- Terminal rendering code in main.rs

## Migration Phases

Each phase produces a working build. Tests verify simulation correctness at
every step.

### Phase 0 — Bevy Scaffold

Add full `bevy` dependency. Create `App::new()` with `DefaultPlugins`. Move all
simulation systems into `SimulationPlugin` on `FixedUpdate`. Window opens,
screen is blank, sim ticks underneath.

**Risk:** Main loop restructuring — this is the highest-risk phase.
**Gate:** All existing tests pass. Sim behavior matches old version.

### Phase 1 — Tile Map Rendering

Load Sprout Lands terrain sprite sheet. Add `bevy_ecs_tilemap`. Sync `TileMap`
resource to tilemap sprites. Map each `Terrain` variant to a sprite index. Add
2D camera with basic scroll/zoom.

**Gate:** All terrain types rendered. Camera navigable.

### Phase 2 — Entity Sprites

Add `Transform` + `Sprite` to cats, wildlife, herbs, wards. Implement
`sync_positions` system. Cats use the Sprout Lands character sprite (48x48)
with palette swapping for fur colors. Wildlife uses cow/chicken sprites as
bases where possible, colored shapes otherwise.

**Gate:** Entities visible and tracking correctly on the map.

### Phase 3 — Camera System

Implement Drift/Linger/Follow/Override state machine. Perlin noise drift path.
Activity detection for lingering. Event detection for following. Smooth lerp
interpolation on all transitions. Manual override via mouse/keyboard.

**Gate:** Camera feels like watching an aquarium.

### Phase 4 — UI Panels

Load Sprout Lands UI sprite sheet. Implement 9-slice panel rendering. Port
narrative log, cat inspect panel, status bar, tile inspect popup. Toggle
visibility, default to minimal.

**Gate:** All information from current TUI accessible in pixel art panels.

### Phase 5 — Animation and Polish

Sprite sheet animations for entities (when real art is ready). Weather particle
effects. Day/night tint cycle. Corruption overlay. Magic/ward glow effects.

**Gate:** Visually delightful.

### Phase 6 — Cleanup

Remove ratatui and crossterm dependencies. Delete `src/tui/` directory. Remove
old rendering code from main.rs. Update headless mode to use `MinimalPlugins`
(task scheduling only, no window/rendering) instead of `DefaultPlugins`.

**Gate:** No terminal rendering dependencies remain.

## Key Files to Modify

- `Cargo.toml` — add bevy, bevy_ecs_tilemap; eventually remove ratatui, crossterm
- `src/main.rs` — replace manual loop with App::new().run()
- `src/plugins/` — new directory for SimulationPlugin, RenderingPlugin, CameraPlugin, UiPlugin, InputPlugin
- `src/rendering/` — new directory for tile map sync, entity sprite sync, effects
- `src/camera.rs` — new: drift/linger/follow/override state machine
- `src/tui/` — deleted in Phase 6
- `assets/sprites/` — new directory for sprite sheets and atlas descriptors

## Verification

- **Phase 0:** Run `just test` — all existing tests must pass. Compare sim output (narrative log) against old version for identical seed.
- **Phases 1-2:** Visual inspection — terrain types match old symbols, entities appear at correct positions.
- **Phase 3:** Subjective — does the camera feel good? Iterate on drift speed, linger threshold, lerp rate.
- **Phase 4:** All information from the old TUI is accessible through the new panels.
- **Phase 5-6:** `just ci` passes. No ratatui/crossterm imports remain. Headless mode works.
