# Sprite Pass — Remaining Visual Upgrades

## Purpose
Replace all remaining placeholder visuals with real sprite art. This covers building entity sprites and weather VFX overlays — the two major rendering gaps after wildlife and prey sprites were integrated.

## Current State
- **Buildings** (Den, Hearth, Stores, Workshop, Garden, Watchtower, WardPost, Wall, Gate): rendered as terrain types only, no distinct entity sprites. The colony well is the sole building with a real sprite.
- **Weather**: 8 weather states cycle and affect gameplay (warmth drain, building decay) but have zero visual representation. No rain, snow, fog, or atmospheric overlays exist.

---

## 1. Building Sprites — Fan-tasy Tileset (Ventilatore)

### Asset Source
Three purchased packs in `assets/new_sprites/`:
- **The Fan-tasy Tileset (Premium)** — base/default, blue/green wood, 11 faction colors
- **The Fan-tasy Tileset - Turning of the Seasons** — hay/rustic theme (closest to Sprout Lands tone)
- **The Fan-tasy Tileset - Snow Adventures** — winter/snow-covered variants

All use 16x16 base tile grid. Buildings are free-placed object sprites at non-grid dimensions.

### Building → Asset Mapping

| Clowder Building | Fan-tasy Asset | Variant | Dimensions | Notes |
|------------------|---------------|---------|-----------|-------|
| Den | `House_Hay_1.png` – `House_Hay_3.png` | Seasons | ~86x103 | Small houses, pick per-den |
| Hearth | `House_Hay_4_*.png` – `House_Hay_6_*.png` | Seasons | ~86x103 | Larger houses with color, chimney feel |
| Stores | `MarketStand_1_*.png`, `MarketStand_2_*.png` | Seasons | varies | Animated door variant available |
| Workshop | Anvil + `ToolsStand_1.png` + `Woodcutter_Table_*.png` | Premium | ~24x19 | Composite: prop cluster on tile |
| Garden | `Basket_*.png` + farming props | Premium | ~32x32 | Decorative prop cluster |
| Watchtower | `Watchtower_1_Hay_*.png` | Seasons | 68x149 | Tall — needs careful z-sort |
| WardPost | `Banner_Stick_1_*.png` | Seasons/Premium | small | Faction color per ward type |
| Wall | `CityWall_*.png` (directional segments) | Premium | varies | Up/Down/Left/Right + corners + gate |
| Gate | `CityWall_Gate_1.png` | Premium | varies | Integrated with wall segments |
| Well | `Well_Hay_1.png` | Seasons | 56x74 | Replaces current Sprout Lands well |

### Integration Approach
- Buildings render as **entity sprites** on a z-layer above terrain (z=13–14), below items/herbs (z=15+). Same pattern as the colony well today.
- Non-grid dimensions mean sizing via `custom_size` scaled relative to `world_px`. A 86x103 house at roughly 2x tile scale.
- Match on `Structure` component variant in `attach_entity_sprites` to select the sprite.
- Faction colors (11 variants per building) could map to colony identity or be randomized at spawn.

### Seasonal Variant Swaps
When winter weather is active, swap building textures from Seasons → Snow Adventures variants. The naming convention is identical (`House_Hay_*` → `House_Snow_*`), so a runtime texture swap keyed on season is straightforward.

### Wall/Gate Segments
`CityWall_*` sprites use directional naming (`Up`, `Down`, `Left`, `Right`, `UpLeft`, `DownRight`, etc.). Building a colony perimeter means selecting the correct segment based on neighbor connectivity — same concept as the existing blob autotile system but simpler (4-neighbor cardinal + corners rather than 8-neighbor blob).

### Props
The Premium pack has 700+ props. Relevant subset for colony flavor:
- Barrels (empty, fish, meat, water) — near Stores
- Crates and chests — near Workshop
- Torches — near Watchtower, along walls
- Benches, tables — near Hearth
- Tool stands, woodcutter tables — near Workshop
- Tombstones — potential graveyard/memorial area

---

## 2. Weather VFX Overlays — Pixel Art Atmospheric (Alenia Studios)

### Asset Source
`assets/new_sprites/Pixel Art Atmospheric/` — 10 atmospheric effects, CC BY 4.0 (attribution: "Assets by Nox - Alenia Studios").

### Effect Inventory

| Effect | File | Resolution | Frames | Clowder Use |
|--------|------|-----------|--------|-------------|
| Cozy Snow | `clima_nieve_cozy` | 320x180 | 48 | Snow weather |
| Aesthetic Rain | `clima_lluvia_estetica` | 320x180 | 48 | LightRain, HeavyRain |
| Aesthetic Wind | `clima_viento_estetico` | 320x180 | 48 | Wind weather, Storm |
| Autumn Leaves | `clima_hojas_autumn` | 320x180 | 48 | Autumn seasonal ambient |
| Fireflies/Spores | `clima_luciernagas_cozy` | 320x180 | 48 | Fairy rings, nighttime |
| God Rays | `clima_godrays` | 320x180 | 48 | Dawn/dusk, Clear weather |
| Fire Embers | `clima_chispas_fuego` | 320x180 | 48 | Hearth proximity, fire events |
| Sakura Petals | `clima_sakura` | 320x180 | 48 | Spring seasonal ambient |
| Meteor Shower | `clima_meteoritos` | 320x180 | 48 | Special events (The Calling?) |
| Epic Tornado | `clima_tornado_epico` | 320x180 | 48 | Storm escalation (rare) |

### Integration Approach

**Rendering:** Each effect is a full-screen overlay rendered as a camera-child sprite on a high z-layer (z=50+, above all game entities). The spritesheet is a horizontal strip (15360x180) with 48 columns of 320x180 frames.

**Atlas layout:**
```rust
TextureAtlasLayout::from_grid(UVec2::new(320, 180), 48, 1, None, None)
```

**Tiling:** At the existing 3x scale, each frame covers 960x540 logical pixels. Tile 2x2 (or as needed) to fill the viewport. Alternatively, scale the overlay sprite to match the camera's visible area.

**Animation:** Cycle through 48 frames on a timer. At 12 FPS the loop completes in 4 seconds. Adjust per-effect for feel (snow slower, rain faster).

**Weather state mapping:**
```
Clear (day)       → God Rays (low alpha)
Clear (night)     → Fireflies (near fairy rings only, or global at low density)
Overcast          → none
LightRain         → Rain (low alpha)
HeavyRain         → Rain (full alpha)
Snow              → Snow
Fog               → none (fog is a separate tint/blur effect)
Wind              → Wind + Autumn Leaves (if autumn)
Storm             → Rain + Wind layered
```

**Seasonal ambient overlays** (independent of weather state):
- Spring: Sakura petals (low alpha, intermittent)
- Autumn: Autumn leaves (low alpha, always on)
- Night + fairy ring proximity: Fireflies

**Layering multiple effects:** Spawn multiple overlay entities at different z-offsets. Rain at z=50, wind at z=51. Each can have independent alpha and animation speed.

**Alpha modulation:** Use `Sprite.color` alpha to control intensity. HeavyRain = alpha 0.8, LightRain = alpha 0.4. Transition smoothly when weather changes.

---

## 3. Attribution Requirements

| Pack | License | Attribution |
|------|---------|------------|
| Fan-tasy Tileset | Commercial use allowed, no redistribution of raw assets | Credit: Ventilatore / The Fan-tasy Tileset |
| Pixel Art Atmospheric | CC BY 4.0 | "Assets by Nox - Alenia Studios" |
| Sprout Lands (existing) | Commercial use, attribution required | "Assets - From: Sprout Lands - By: Cup Nooble" |
| Minifolks Forest Animals | Per pack license | Credit: LYASeeK |
| Animal Packs (Basic/Supporter/Premium) | Per pack license | Credit per pack creator |

Ensure credits file is updated before any public release.

## Tuning Notes
_Record observations and adjustments here during iteration._
