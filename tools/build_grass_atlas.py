#!/usr/bin/env python3
"""
Build blob autotile atlases from Fan-tasy Tileset ground sheets.

The Fan-tasy Tileset (Premium) uses a single mega-sheet (Tileset_Ground.png,
768x1520px = 48 cols x 95 rows of 16x16 tiles) with multiple terrain types
stacked vertically. Each terrain occupies a 10-row band (8 blob rows + 2
decorative rows) separated by 4-row transition gaps (14 rows per band total).

Within each band, the 47 blob tiles + 1 isolated tile occupy a 6-column x 8-row
block at (cols 0-5, rows 0-7 relative to band start). The 12 decorative center
variants occupy (cols 0-5, rows 8-9).

The blob tile coordinates were derived from the TSX Wang Set definitions in:
  Tiled/Tilesets/Tileset_Ground.tsx

Bitmask bits (clockwise): N=1, NE=2, E=4, SE=8, S=16, SW=32, W=64, NW=128
Diagonal bits only set when both adjacent cardinals are present.

Transition tiles (terrain A blending into terrain B) live at cols 6+ in each
band but are NOT extracted here — those are handled separately by the overlay
layer system.
"""
from pathlib import Path

from PIL import Image

TILE = 16
ATLAS_COLS = 8
PADDING = 0
STRIDE = TILE + 2 * PADDING  # 16 (no extrusion)

REPO = Path(__file__).resolve().parent.parent
FANTASY = REPO / "assets/new_sprites/The Fan-tasy Tileset (Premium)/Art/Ground Tilesets"
SEASONS = REPO / "assets/new_sprites/The Fan-tasy Tileset - Turning of the Seasons/Art/Ground Tilesets"
SNOW = REPO / "assets/new_sprites/The Fan-tasy Tileset - Snow Adventures/Art/Ground Tilesets"
OUTPUT_DIR = REPO / "assets/sprites"

# ---------------------------------------------------------------------------
# Blob tile coordinates within each 6x8 band.
# (bitmask, (col_in_band, row_in_band), description)
# Derived from Tileset_Ground.tsx Wang Set, color=1 (Grass).
# Coordinates are identical for every terrain band in the mega-sheet.
# ---------------------------------------------------------------------------
BLOB_TILES = [
    (  0, (0, 3), "Isolated"),
    (  1, (0, 2), "N endcap"),
    (  4, (1, 3), "E endcap"),
    (  5, (2, 5), "NE corner+SW inner"),
    (  7, (1, 2), "NE corner"),
    ( 16, (0, 0), "S endcap"),
    ( 17, (0, 1), "N+S strip"),
    ( 20, (2, 4), "SE corner+NW inner"),
    ( 21, (3, 6), "W edge+both inners"),
    ( 23, (4, 1), "W edge+NW inner"),
    ( 28, (1, 0), "SE corner"),
    ( 29, (4, 2), "W edge+SE inner"),
    ( 31, (1, 1), "W edge"),
    ( 64, (3, 3), "W endcap"),
    ( 65, (3, 5), "SW corner+NE inner"),
    ( 68, (2, 3), "E+W strip"),
    ( 69, (2, 6), "S edge+both inners"),
    ( 71, (4, 3), "S edge+NE inner"),
    ( 80, (3, 4), "NW corner+SE inner"),
    ( 81, (2, 7), "E edge+both inners"),
    ( 84, (3, 7), "N edge+both inners"),
    ( 85, (4, 7), "Cross"),
    ( 87, (0, 7), "3 inner (only NE)"),
    ( 92, (4, 0), "N edge+SW inner"),
    ( 93, (0, 6), "3 inner (only SE)"),
    ( 95, (4, 5), "SE+NE dbl inner"),
    (112, (3, 0), "NW corner"),
    (113, (5, 0), "NW corner alt"),
    (116, (5, 2), "N edge+SE inner"),
    (117, (1, 6), "3 inner (only SW)"),
    (119, (4, 6), "NE+SW opp inner"),
    (124, (2, 0), "N edge"),
    (125, (4, 4), "NE+NW dbl inner"),
    (127, (1, 5), "NW inner"),
    (193, (3, 2), "SE corner"),
    (197, (5, 1), "S edge+NE inner"),
    (199, (2, 2), "S edge"),
    (209, (5, 3), "E edge+SW inner"),
    (213, (1, 7), "3 inner (only NW)"),
    (215, (5, 5), "SE+SW dbl inner"),
    (221, (5, 6), "NE+SW opp inner alt"),
    (223, (1, 4), "SW inner"),
    (241, (3, 1), "E edge"),
    (245, (5, 4), "NE+SE dbl inner"),
    (247, (0, 4), "SE inner"),
    (253, (0, 5), "NE inner"),
    (255, (2, 1), "Center fill"),
]

# Decorative center variants: rows 8-9 of each band, cols 0-5.
DECORATIVE = [
    (0, 8), (1, 8), (2, 8), (3, 8), (4, 8), (5, 8),
    (0, 9), (1, 9), (2, 9), (3, 9), (4, 9), (5, 9),
]

# ---------------------------------------------------------------------------
# Terrain band offsets (row in mega-sheet where each band starts).
# Band structure: rows 0-7 = blob tiles, rows 8-9 = decorative, rows 10-13 = gap/transitions.
# Total: 14 rows per band.
# ---------------------------------------------------------------------------
BANDS = {
    "grass":             0,
    "light_grass":      14,
    "dark_grass":       28,
    "winter_grass":     42,
    "autumn_grass":     56,
    "dark_autumn_grass":70,
    "cherry_grass":     84,
}

# ---------------------------------------------------------------------------
# Tileset configurations: (name, source_image, band_name, include_decorative)
# Multiple bands can be extracted from the same source image.
# ---------------------------------------------------------------------------
ROAD_BANDS = {
    "road":           0,
    "brick_road":    14,
    "dark_brick":    28,
}

TILESETS = [
    # PR 1: Core ground overlays
    ("grass",  FANTASY / "Tileset_Ground.png", BANDS, "grass", True),
    ("soil",   FANTASY / "Tileset_Road.png",   ROAD_BANDS, "road", True),
    ("stone",  FANTASY / "Tileset_Road.png",   ROAD_BANDS, "brick_road", True),
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


def build_atlas(name, tileset_path, bands_dict, band_name, include_decorative):
    src = Image.open(tileset_path).convert("RGBA")
    atlas_px = ATLAS_COLS * STRIDE
    atlas = Image.new("RGBA", (atlas_px, atlas_px), (0, 0, 0, 0))

    row_offset = bands_dict[band_name]

    for atlas_idx, (bitmask, (sx, sy), desc) in enumerate(BLOB_TILES):
        src_col = sx
        src_row = row_offset + sy
        tile = src.crop((
            src_col * TILE,
            src_row * TILE,
            (src_col + 1) * TILE,
            (src_row + 1) * TILE,
        ))
        col = atlas_idx % ATLAS_COLS
        row = atlas_idx // ATLAS_COLS
        ox = col * STRIDE
        oy = row * STRIDE
        extrude_tile(atlas, tile, ox, oy)

    if include_decorative:
        for i, (sx, sy) in enumerate(DECORATIVE):
            atlas_idx = 47 + i
            src_col = sx
            src_row = row_offset + sy
            tile = src.crop((
                src_col * TILE,
                src_row * TILE,
                (src_col + 1) * TILE,
                (src_row + 1) * TILE,
            ))
            col = atlas_idx % ATLAS_COLS
            row = atlas_idx // ATLAS_COLS
            ox = col * STRIDE + PADDING
            oy = row * STRIDE + PADDING
            extrude_tile(atlas, tile, ox, oy)

    output = OUTPUT_DIR / f"{name}_autotile_atlas.png"
    atlas.save(output)
    n_tiles = 47 + (12 if include_decorative else 0)
    print(f"  {name}: {output.name} ({atlas.size[0]}x{atlas.size[1]}, {n_tiles} tiles, band '{band_name}' row {row_offset})")
    return output


def build_base_terrain_tiles():
    """Extract base terrain fill tiles from the Fan-tasy mega-sheet.

    Each terrain band has a center-fill tile at (2, 1) — the bitmask 255 tile.
    We extract these as individual PNGs for the base terrain layer.
    """
    src = Image.open(FANTASY / "Tileset_Ground.png").convert("RGBA")

    base_tiles = {
        "grass":   ("grass",  0),
        "dirt":    None,  # Dirt uses the Road tileset, handled separately
        "rock":    None,  # Rock uses stone_autotile, handled separately
    }

    # Extract the center-fill tile (col=2, row=1 within each band) as the base tile
    for name, (band_name, row_offset) in [(k, (k, v)) for k, v in BANDS.items() if k == "grass"]:
        cx, cy = 2, 1  # center fill position
        tile = src.crop((
            cx * TILE,
            (row_offset + cy) * TILE,
            (cx + 1) * TILE,
            (row_offset + cy + 1) * TILE,
        ))
        out_path = OUTPUT_DIR / "tiles" / f"{name}.png"
        out_path.parent.mkdir(parents=True, exist_ok=True)
        tile.save(out_path)
        print(f"  base tile: {out_path.relative_to(REPO)}")


def build_base_terrain_atlas():
    """Build a base terrain atlas from the extracted tiles."""
    tiles_dir = OUTPUT_DIR / "tiles"
    tile_names = ["grass", "water", "dirt", "sand", "rock", "stone", "building"]
    tiles = []
    for name in tile_names:
        path = tiles_dir / f"{name}.png"
        if path.exists():
            tiles.append(Image.open(path).convert("RGBA"))
        else:
            # Placeholder: transparent tile
            tiles.append(Image.new("RGBA", (TILE, TILE), (0, 0, 0, 0)))

    n = len(tiles)
    atlas = Image.new("RGBA", (n * STRIDE, STRIDE), (0, 0, 0, 0))
    for i, tile in enumerate(tiles):
        extrude_tile(atlas, tile, i * STRIDE, 0)

    out_path = OUTPUT_DIR / "base_terrain_atlas.png"
    atlas.save(out_path)
    print(f"  base atlas: {out_path.name} ({atlas.size[0]}x{atlas.size[1]}, {n} tiles)")


def build_lookup_table():
    table = [0] * 256
    for atlas_idx, (bitmask, _, _) in enumerate(BLOB_TILES):
        table[bitmask] = atlas_idx
    return table


def main():
    print("Building blob autotile atlases (Fan-tasy Tileset)...")
    for name, path, bands_dict, band, decorative in TILESETS:
        build_atlas(name, path, bands_dict, band, decorative)

    print("\nExtracting base terrain tiles...")
    build_base_terrain_tiles()
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


if __name__ == "__main__":
    main()
