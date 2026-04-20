#!/usr/bin/env python3
"""
Visual verification grid for blob autotile atlases.

For each of the 47 blob tiles, renders:
  - The atlas index and bitmask value (decimal + binary)
  - A 3x3 mini-diagram showing which neighbors are "same" (the expected pattern)
  - The actual tile sprite from the atlas (scaled 4x for visibility)

Outputs atlas_verification.png for eyeball validation.

Usage:
    python3 tools/verify_atlas.py [atlas_path]

Defaults to assets/sprites/grass_autotile_atlas.png if no argument given.
Also generates verification images for all three core atlases.
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

TILE = 16
ATLAS_COLS = 8
SCALE = 4  # Scale tiles up for visibility

REPO = Path(__file__).resolve().parent.parent
OUTPUT_DIR = REPO / "assets/sprites"

# The 47 valid blob bitmask values and their atlas indices (0-46).
# Bitmask bits: N=1, NE=2, E=4, SE=8, S=16, SW=32, W=64, NW=128
BLOB_MASKS = [
    0, 1, 4, 5, 7, 16, 17, 20, 21, 23, 28, 29, 31,
    64, 65, 68, 69, 71, 80, 81, 84, 85, 87, 92, 93, 95,
    112, 113, 116, 117, 119, 124, 125, 127,
    193, 197, 199, 209, 213, 215, 221, 223,
    241, 245, 247, 253, 255,
]

# Colors for the 3x3 mini-diagram
TERRAIN_COLOR = (100, 200, 120)   # Green = same terrain
EMPTY_COLOR = (60, 60, 60)        # Dark grey = different terrain
CENTER_COLOR = (200, 80, 80)      # Red = the tile itself
GRID_COLOR = (40, 40, 40)


def bitmask_to_neighbors(mask):
    """Return a 3x3 grid of booleans: True = same terrain neighbor."""
    # Grid positions: [row][col] where (1,1) is center
    # N=1, NE=2, E=4, SE=8, S=16, SW=32, W=64, NW=128
    grid = [[False] * 3 for _ in range(3)]
    grid[1][1] = True   # Center is always "self"
    if mask & 1:   grid[0][1] = True   # N
    if mask & 2:   grid[0][2] = True   # NE
    if mask & 4:   grid[1][2] = True   # E
    if mask & 8:   grid[2][2] = True   # SE
    if mask & 16:  grid[2][1] = True   # S
    if mask & 32:  grid[2][0] = True   # SW
    if mask & 64:  grid[1][0] = True   # W
    if mask & 128: grid[0][0] = True   # NW
    return grid


def draw_mini_diagram(draw, x, y, mask, cell_size=8):
    """Draw a 3x3 mini-diagram at (x, y)."""
    grid = bitmask_to_neighbors(mask)
    for row in range(3):
        for col in range(3):
            cx = x + col * cell_size
            cy = y + row * cell_size
            if row == 1 and col == 1:
                color = CENTER_COLOR
            elif grid[row][col]:
                color = TERRAIN_COLOR
            else:
                color = EMPTY_COLOR
            draw.rectangle([cx, cy, cx + cell_size - 1, cy + cell_size - 1], fill=color)
            draw.rectangle([cx, cy, cx + cell_size - 1, cy + cell_size - 1], outline=GRID_COLOR)


def build_verification(atlas_path, output_path):
    atlas = Image.open(atlas_path).convert("RGBA")

    # Layout: 8 columns x 6 rows of verification cells
    # Each cell: tile (scaled) + diagram + label
    cols_per_row = 8
    n_rows = (len(BLOB_MASKS) + cols_per_row - 1) // cols_per_row

    tile_scaled = TILE * SCALE  # 64px
    diagram_size = 3 * 8        # 24px
    cell_w = tile_scaled + diagram_size + 60  # tile + diagram + text
    cell_h = max(tile_scaled, diagram_size + 20) + 16  # room for label
    padding = 4

    img_w = cols_per_row * cell_w + padding * 2
    img_h = n_rows * cell_h + padding * 2 + 24  # header room

    img = Image.new("RGBA", (img_w, img_h), (30, 30, 30, 255))
    draw = ImageDraw.Draw(img)

    # Header
    draw.text((padding, 2), f"Atlas: {atlas_path.name}  ({len(BLOB_MASKS)} blob tiles)", fill=(255, 255, 255))

    for idx, mask in enumerate(BLOB_MASKS):
        col = idx % cols_per_row
        row = idx // cols_per_row

        cx = padding + col * cell_w
        cy = padding + 24 + row * cell_h

        # Extract tile from atlas
        atlas_col = idx % ATLAS_COLS
        atlas_row = idx // ATLAS_COLS
        tile = atlas.crop((
            atlas_col * TILE,
            atlas_row * TILE,
            (atlas_col + 1) * TILE,
            (atlas_row + 1) * TILE,
        ))
        # Scale up
        tile_big = tile.resize((tile_scaled, tile_scaled), Image.NEAREST)
        img.paste(tile_big, (cx, cy), tile_big)

        # Draw mini-diagram to the right of the tile
        draw_mini_diagram(draw, cx + tile_scaled + 4, cy + 4, mask)

        # Label: index and bitmask
        label = f"#{idx} m={mask}"
        bits = f"{mask:08b}"
        draw.text((cx + tile_scaled + 4, cy + 30), label, fill=(200, 200, 200))
        draw.text((cx + tile_scaled + 4, cy + 42), bits, fill=(150, 150, 150))

    img.save(output_path)
    print(f"  Verification: {output_path.name} ({img_w}x{img_h})")


def main():
    print("Building atlas verification grids...")

    if len(sys.argv) > 1:
        atlas_path = Path(sys.argv[1])
        output = atlas_path.with_name(atlas_path.stem + "_verification.png")
        build_verification(atlas_path, output)
    else:
        # Build for all three core atlases
        for name in ["grass", "soil", "stone"]:
            atlas_path = OUTPUT_DIR / f"{name}_autotile_atlas.png"
            if atlas_path.exists():
                output = REPO / f"{name}_atlas_verification.png"
                build_verification(atlas_path, output)
            else:
                print(f"  SKIP: {atlas_path.name} not found")


if __name__ == "__main__":
    main()
