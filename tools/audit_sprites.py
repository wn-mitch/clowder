#!/usr/bin/env python3
"""
Sprite coverage audit for Fan-tasy Tileset migration.

Walks the Fan-tasy Art directories across all three packs, categorizes every
PNG by type using the naming conventions from the documentation, and
cross-references against a manifest of sprites currently loaded by the game.

Outputs a report showing mapped vs unmapped counts per category.

Usage:
    python3 tools/audit_sprites.py
"""
import re
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
PACKS = [
    REPO / "assets/new_sprites/The Fan-tasy Tileset (Premium)/Art",
    REPO / "assets/new_sprites/The Fan-tasy Tileset - Turning of the Seasons/Art",
    REPO / "assets/new_sprites/The Fan-tasy Tileset - Snow Adventures/Art",
]

# Categories based on naming conventions and folder structure.
# Order matters — first match wins.
CATEGORY_RULES = [
    ("Ground Tileset",  lambda p, n: "Ground Tilesets" in str(p) and n.startswith("Tileset_")),
    ("Road/Farm",       lambda p, n: "Ground Tilesets" in str(p)),
    ("Rock Slope",      lambda p, n: "Rock Slopes" in str(p)),
    ("Tree",            lambda p, n: "Trees and Bushes" in str(p) and any(n.startswith(t) for t in ["Birch_", "Oak_", "Pine_", "DeadTree_", "Stump_"])),
    ("Bush",            lambda p, n: "Trees and Bushes" in str(p) and n.startswith("Bush_")),
    ("Tree/Bush Other", lambda p, n: "Trees and Bushes" in str(p)),
    ("Building",        lambda p, n: "Buildings" in str(p) and not n.startswith("Atlas_") and not n.startswith("Animation_")),
    ("Building Atlas",  lambda p, n: "Buildings" in str(p) and n.startswith("Atlas_")),
    ("Building Anim",   lambda p, n: "Buildings" in str(p) and n.startswith("Animation_")),
    ("Fence/Wall",      lambda p, n: "Fences and Walls" in str(p)),
    ("Rock",            lambda p, n: "Rocks" in str(p) and "Rock Slopes" not in str(p)),
    ("Prop",            lambda p, n: "Props" in str(p)),
    ("Shadow",          lambda p, n: "Shadows" in str(p)),
    ("Water/Sand",      lambda p, n: "Water and Sand" in str(p)),
    ("Tall Grass",      lambda p, n: "Tall Grass" in str(p)),
    ("Character",       lambda p, n: "Characters" in str(p)),
    ("Map Background",  lambda p, n: "Map Backgrounds" in str(p)),
    ("Animation",       lambda p, n: n.startswith("Animation_")),
    ("Atlas",           lambda p, n: n.startswith("Atlas_")),
    ("Tileset",         lambda p, n: n.startswith("Tileset_")),
]

# Sprites currently loaded by the game (from sprite_assets.rs and tilemap_sync.rs).
# Updated as migration progresses.
MAPPED_PATTERNS = [
    # Ground (PR 1)
    re.compile(r"Tileset_Ground\.png"),
    re.compile(r"Tileset_Road\.png"),
    # Base tiles extracted by atlas builder
    re.compile(r"grass\.png$"),
    # (Add more as migration progresses)
]


def categorize(path, name):
    for cat_name, rule in CATEGORY_RULES:
        if rule(path, name):
            return cat_name
    return "Other"


def is_mapped(full_path):
    path_str = str(full_path)
    return any(pat.search(path_str) for pat in MAPPED_PATTERNS)


def main():
    # Collect all PNGs across all packs
    all_sprites = []
    for pack_dir in PACKS:
        if not pack_dir.exists():
            continue
        for png in pack_dir.rglob("*.png"):
            # Skip Tiled folder (TSX/TMX assets, not art)
            if "/Tiled/" in str(png):
                continue
            rel = png.relative_to(pack_dir.parent)
            name = png.name
            cat = categorize(str(png), name)
            mapped = is_mapped(png)
            all_sprites.append((cat, name, rel, mapped))

    # Aggregate by category
    categories = {}
    for cat, name, rel, mapped in all_sprites:
        if cat not in categories:
            categories[cat] = {"total": 0, "mapped": 0, "unmapped_samples": []}
        categories[cat]["total"] += 1
        if mapped:
            categories[cat]["mapped"] += 1
        elif len(categories[cat]["unmapped_samples"]) < 3:
            categories[cat]["unmapped_samples"].append(str(rel))

    # Print report
    total_all = sum(c["total"] for c in categories.values())
    mapped_all = sum(c["mapped"] for c in categories.values())

    print(f"Fan-tasy Tileset Sprite Coverage Report")
    print(f"{'=' * 60}")
    print(f"Total sprites: {total_all}  |  Mapped: {mapped_all}  |  Unmapped: {total_all - mapped_all}")
    print(f"Coverage: {mapped_all / total_all * 100:.1f}%")
    print()
    print(f"{'Category':<20} {'Mapped':>7} {'Total':>7} {'Coverage':>9}  Samples")
    print(f"{'-' * 20} {'-' * 7} {'-' * 7} {'-' * 9}  {'-' * 30}")

    for cat in sorted(categories.keys()):
        info = categories[cat]
        pct = info["mapped"] / info["total"] * 100 if info["total"] > 0 else 0
        samples = ", ".join(Path(s).name for s in info["unmapped_samples"][:2])
        if samples:
            samples = f"  e.g. {samples}"
        bar = "+" * int(pct / 10) + "-" * (10 - int(pct / 10))
        print(f"{cat:<20} {info['mapped']:>7} {info['total']:>7} {pct:>7.1f}%  [{bar}]{samples}")


if __name__ == "__main__":
    main()
