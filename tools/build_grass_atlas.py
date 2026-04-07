#!/usr/bin/env python3
"""
Build blob autotile atlases from Sprout Lands tilesets.

All terrain tilesets (Grass, Soil, Stone) share the same 11x7 tile layout.
This script extracts the 47 blob tiles + decorative variants into 8x8 atlases.

Each tile is extruded by 1px (edge pixels duplicated outward) to prevent
texture atlas bleeding at tile boundaries during GPU rendering.

Bitmask bits (clockwise): N=1, NE=2, E=4, SE=8, S=16, SW=32, W=64, NW=128
Diagonal bits only set when both adjacent cardinals are present.
"""
from PIL import Image
from pathlib import Path

TILE = 16
ATLAS_COLS = 8
PADDING = 0
STRIDE = TILE + 2 * PADDING  # 16 (no extrusion)

REPO = Path(__file__).resolve().parent.parent
SPROUT = REPO / "assets/sprites/Sprout Lands - Sprites - premium pack/Tilesets/ground tiles/New tiles"
OUTPUT_DIR = REPO / "assets/sprites"

# All 47 blob bitmask values → (tileset_col, tileset_row)
# Layout is identical across Grass, Soil, and Stone tilesets.
BLOB_TILES = [
    (  0, (3, 3), "Isolated"),
    (  1, (3, 2), "S endcap"),
    (  4, (0, 3), "W endcap"),
    (  5, (4, 3), "SW corner+NE inner"),
    (  7, (0, 2), "SW corner"),
    ( 16, (3, 0), "N endcap"),
    ( 17, (3, 1), "N+S strip"),
    ( 20, (4, 0), "NW corner+SE inner"),
    ( 21, (4, 4), "W edge+both inners"),
    ( 23, (4, 1), "W edge+SE inner"),
    ( 28, (0, 0), "NW corner"),
    ( 29, (4, 2), "W edge+NE inner"),
    ( 31, (0, 1), "W edge"),
    ( 64, (2, 3), "E endcap"),
    ( 65, (7, 3), "SE corner+NW inner"),
    ( 68, (1, 3), "E+W strip"),
    ( 69, (8, 3), "S edge+both inners"),
    ( 71, (6, 3), "S edge+NW inner"),
    ( 80, (7, 0), "NE corner+SW inner"),
    ( 81, (7, 4), "E edge+both inners"),
    ( 84, (8, 0), "N edge+both inners"),
    ( 85, (8, 4), "Cross"),
    ( 87, (9, 3), "3 inner (only NE)"),
    ( 92, (6, 0), "N edge+SW inner"),
    ( 93, (9, 2), "3 inner (only SE)"),
    ( 95, (6, 4), "SW+NW dbl inner"),
    (112, (2, 0), "NE corner"),
    (113, (7, 2), "E edge+NW inner"),
    (116, (5, 0), "N edge+SE inner"),
    (117, (10, 2), "3 inner (only SW)"),
    (119, (9, 1), "SE+NW opp inner"),
    (124, (1, 0), "N edge"),
    (125, (8, 2), "NE+NW dbl inner"),
    (127, (6, 2), "NW inner"),
    (193, (2, 2), "SE corner"),
    (197, (5, 3), "S edge+NE inner"),
    (199, (1, 2), "S edge"),
    (209, (7, 1), "E edge+SW inner"),
    (213, (10, 3), "3 inner (only NW)"),
    (215, (8, 1), "SE+SW dbl inner"),
    (221, (9, 0), "NE+SW opp inner"),
    (223, (6, 1), "SW inner"),
    (241, (2, 1), "E edge"),
    (245, (5, 4), "NE+SE dbl inner"),
    (247, (5, 1), "SE inner"),
    (253, (5, 2), "NE inner"),
    (255, (1, 1), "Center fill"),
]

# Decorative center variants (rows 5-6 of tileset)
DECORATIVE = [
    (0, 5), (1, 5), (2, 5), (3, 5), (4, 5), (5, 5),
    (0, 6), (1, 6), (2, 6), (3, 6), (4, 6), (5, 6),
]

# Tilesets to build atlases for
TILESETS = [
    ("grass", SPROUT / "Grass_tiles_v2.png", True),     # include decorative variants
    ("soil",  SPROUT / "Soil_Ground_Tiles.png", True),
    ("stone", SPROUT / "Stone_Ground_Tiles.png", True),
]

assert len(BLOB_TILES) == 47


def extrude_tile(atlas, tile, ox, oy):
    """Place a tile at (ox, oy) and extrude its edge pixels into the
    surrounding gap. Tiles are placed at stride-aligned positions (no
    padding offset) so bevy_ecs_tilemap UV calculations land correctly.
    Extrusion fills the gap AFTER the tile (right/bottom) and BEFORE it
    (left/top, clamped to atlas bounds)."""
    w, h = atlas.size
    atlas.paste(tile, (ox, oy))

    bot_row = tile.crop((0, TILE - 1, TILE, TILE))
    right_col = tile.crop((TILE - 1, 0, TILE, TILE))
    top_row = tile.crop((0, 0, TILE, 1))
    left_col = tile.crop((0, 0, 1, TILE))

    for p in range(1, PADDING + 1):
        # Right extrusion (into gap after tile)
        rx = ox + TILE - 1 + p
        if rx < w:
            atlas.paste(right_col, (rx, oy))
        # Bottom extrusion
        by = oy + TILE - 1 + p
        if by < h:
            atlas.paste(bot_row, (ox, by))
        # Left extrusion (into gap before tile, if space exists)
        lx = ox - p
        if lx >= 0:
            atlas.paste(left_col, (lx, oy))
        # Top extrusion
        ty = oy - p
        if ty >= 0:
            atlas.paste(top_row, (ox, ty))

    # Corners
    tl = tile.getpixel((0, 0))
    tr = tile.getpixel((TILE - 1, 0))
    bl = tile.getpixel((0, TILE - 1))
    br = tile.getpixel((TILE - 1, TILE - 1))
    for dp in range(1, PADDING + 1):
        for dq in range(1, PADDING + 1):
            # Bottom-right
            cx, cy = ox + TILE - 1 + dp, oy + TILE - 1 + dq
            if 0 <= cx < w and 0 <= cy < h:
                atlas.putpixel((cx, cy), br)
            # Bottom-left
            cx, cy = ox - dp, oy + TILE - 1 + dq
            if 0 <= cx < w and 0 <= cy < h:
                atlas.putpixel((cx, cy), bl)
            # Top-right
            cx, cy = ox + TILE - 1 + dp, oy - dq
            if 0 <= cx < w and 0 <= cy < h:
                atlas.putpixel((cx, cy), tr)
            # Top-left
            cx, cy = ox - dp, oy - dq
            if 0 <= cx < w and 0 <= cy < h:
                atlas.putpixel((cx, cy), tl)


def build_atlas(name, tileset_path, include_decorative):
    src = Image.open(tileset_path).convert("RGBA")
    atlas_px = ATLAS_COLS * STRIDE
    atlas = Image.new("RGBA", (atlas_px, atlas_px), (0, 0, 0, 0))

    for atlas_idx, (bitmask, (sx, sy), desc) in enumerate(BLOB_TILES):
        tile = src.crop((sx * TILE, sy * TILE, (sx + 1) * TILE, (sy + 1) * TILE))
        col = atlas_idx % ATLAS_COLS
        row = atlas_idx // ATLAS_COLS
        ox = col * STRIDE
        oy = row * STRIDE
        extrude_tile(atlas, tile, ox, oy)

    if include_decorative:
        for i, (sx, sy) in enumerate(DECORATIVE):
            atlas_idx = 47 + i
            tile = src.crop((sx * TILE, sy * TILE, (sx + 1) * TILE, (sy + 1) * TILE))
            col = atlas_idx % ATLAS_COLS
            row = atlas_idx // ATLAS_COLS
            ox = col * STRIDE + PADDING
            oy = row * STRIDE + PADDING
            extrude_tile(atlas, tile, ox, oy)

    output = OUTPUT_DIR / f"{name}_autotile_atlas.png"
    atlas.save(output)
    n_tiles = 47 + (12 if include_decorative else 0)
    print(f"  {name}: {output.name} ({atlas.size[0]}x{atlas.size[1]}, {n_tiles} tiles, {PADDING}px extrusion)")
    return output


def build_base_terrain_atlas():
    """Extrude the base terrain atlas (7 tiles in a single row).
    Reads from the original (tightly packed) source and writes to a
    separate extruded output so repeated runs don't corrupt the source."""
    src_path = OUTPUT_DIR / "base_terrain_atlas.png"
    out_path = OUTPUT_DIR / "base_terrain_atlas_extruded.png"
    src = Image.open(src_path).convert("RGBA")
    n_tiles = src.size[0] // TILE
    atlas = Image.new("RGBA", (n_tiles * STRIDE, STRIDE), (0, 0, 0, 0))
    for i in range(n_tiles):
        tile = src.crop((i * TILE, 0, (i + 1) * TILE, TILE))
        extrude_tile(atlas, tile, i * STRIDE, 0)
    atlas.save(out_path)
    print(f"  base: {out_path.name} ({atlas.size[0]}x{atlas.size[1]}, {n_tiles} tiles, {PADDING}px extrusion)")


def build_lookup_table():
    table = [0] * 256
    for atlas_idx, (bitmask, _, _) in enumerate(BLOB_TILES):
        table[bitmask] = atlas_idx
    return table


def main():
    print("Building blob autotile atlases...")
    for name, path, decorative in TILESETS:
        build_atlas(name, path, decorative)

    print("Extruding base terrain atlas...")
    build_base_terrain_atlas()

    # Print Rust lookup table (same for all terrain types — same atlas layout)
    table = build_lookup_table()
    print("\n// BLOB_TO_ATLAS lookup table (same for all terrain atlases):")
    print("const BLOB_TO_ATLAS: [u32; 256] = [")
    for row_start in range(0, 256, 16):
        vals = ", ".join(f"{table[i]:2d}" for i in range(row_start, row_start + 16))
        print(f"    {vals}, // {row_start}-{row_start+15}")
    print("];")

    print(f"\nAll atlases use same layout: 47 blob + 12 decorative = 59 tiles in 8x8 grid")
    print(f"Tile size: {TILE}x{TILE}, stride: {STRIDE}x{STRIDE} ({PADDING}px extrusion)")
    print(f"Set TilemapSpacing {{ x: {PADDING * 2}.0, y: {PADDING * 2}.0 }} in tilemap_sync.rs")


if __name__ == "__main__":
    main()
