#!/usr/bin/env python3
"""
Build the ancient-ruin rune atlas from the Fan-tasy Tileset animation sheet.

Source: Animation_Rock_Brown_EmeraldGrass.png (320x336, 20 cols x 21 rows of
16x16 tiles). Row 2 (0-indexed) holds the rune-pair animation used by the
AncientRuin terrain. Tiles 41 and 42 are the left/right halves of the pair;
the remaining 17 tiles on that row are the 9 animation frames for each half.

Per Animation_Rock_Brown_EmeraldGrass.tsx, each half cycles 9 unique frames
in an 18-step sequence of uniform 250ms. The 18-step sequence is encoded in
the Rust rendering code, not in the atlas. This tool just extracts the 9
frames per half into a compact 9-wide x 2-tall strip:

  row 0: left-half frames  at source row 2, cols [1, 3, 5, 7, 9, 11, 13, 15, 17]
  row 1: right-half frames at source row 2, cols [2, 4, 6, 8, 10, 12, 14, 16, 18]

Output: assets/sprites/rune_rock_gray_atlas.png (144x32, 9x2 cells of 16x16).
"""
from pathlib import Path

from PIL import Image

TILE = 16
FRAMES_PER_HALF = 9

REPO = Path(__file__).resolve().parent.parent
SOURCE = REPO / (
    "assets/new_sprites/The Fan-tasy Tileset (Premium)"
    "/Art/Rocks/Animations/Animation_Rock_Brown_EmeraldGrass.png"
)
OUTPUT = REPO / "assets/sprites/rune_rock_gray_atlas.png"

SOURCE_ROW = 2
LEFT_COLS = [1, 3, 5, 7, 9, 11, 13, 15, 17]
RIGHT_COLS = [2, 4, 6, 8, 10, 12, 14, 16, 18]


def extract_frame(src, col, row):
    x0, y0 = col * TILE, row * TILE
    return src.crop((x0, y0, x0 + TILE, y0 + TILE))


def main():
    if not SOURCE.exists():
        raise SystemExit(f"source not found: {SOURCE}")

    src = Image.open(SOURCE).convert("RGBA")
    assert len(LEFT_COLS) == FRAMES_PER_HALF
    assert len(RIGHT_COLS) == FRAMES_PER_HALF

    atlas = Image.new("RGBA", (FRAMES_PER_HALF * TILE, 2 * TILE), (0, 0, 0, 0))

    for i, col in enumerate(LEFT_COLS):
        atlas.paste(extract_frame(src, col, SOURCE_ROW), (i * TILE, 0))
    for i, col in enumerate(RIGHT_COLS):
        atlas.paste(extract_frame(src, col, SOURCE_ROW), (i * TILE, TILE))

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    atlas.save(OUTPUT)
    print(f"  rune atlas: {OUTPUT.relative_to(REPO)} ({atlas.size[0]}x{atlas.size[1]})")


if __name__ == "__main__":
    main()
