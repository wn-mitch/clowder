use crate::resources::map::{Terrain, TileMap};

/// Generate a hand-crafted test map for visual debugging of the tilemap
/// rendering pipeline. Every terrain-group boundary combination is laid out
/// in a predictable grid so rendering artifacts are immediately attributable
/// to a specific terrain pairing.
///
/// Layout (80 wide × 60 tall, grass background):
///
///   Rows  2–9:  Isolated 6×6 patches of each terrain group
///   Rows 13–22: Boundary-pair rectangles (A on top, B on bottom)
///   Rows 26–35: Complex multi-layer overlap scenarios
///   Rows 40–59: Open grass (colony spawns here)
///
/// Also writes an annotated legend to /tmp/clowder_test_map_legend.txt.
pub fn generate_test_map() -> TileMap {
    let mut map = TileMap::new(80, 60, Terrain::Grass);

    // ── Section A: Isolated patches (rows 2–9) ──────────────────────────
    // Each terrain group as a 6×6 island on the grass background.
    // Shows what each overlay looks like in isolation.
    let patches: &[(i32, Terrain, &str)] = &[
        (2, Terrain::Water, "Water"),
        (11, Terrain::Mud, "Dirt (Mud)"),
        (20, Terrain::Sand, "Sand"),
        (29, Terrain::Rock, "Rock"),
        (38, Terrain::Wall, "Stone (Wall)"),
        (47, Terrain::Den, "Building (Den)"),
        (56, Terrain::FairyRing, "Special (FairyRing)"),
        (65, Terrain::DenseForest, "Grass (DenseForest)"),
    ];
    for &(start_x, terrain, _) in patches {
        fill_rect(&mut map, start_x, 2, 6, 6, terrain);
    }

    // ── Section B: Boundary pairs (rows 13–22) ─────────────────────────
    // Horizontal split: top 4 rows = type A, bottom 4 rows = type B.
    // Each pair is 8 wide × 8 tall. These target multi-layer overlaps.
    let pairs: &[(i32, Terrain, Terrain, &str)] = &[
        // Grass overlay (z=3) meets Stone overlay (z=2)
        (2, Terrain::Grass, Terrain::Rock, "Grass/Rock (z3+z2)"),
        // Grass overlay (z=3) meets Stone overlay (z=2) — Wall variant
        (12, Terrain::Grass, Terrain::Wall, "Grass/Wall (z3+z2)"),
        // Grass overlay (z=3) meets Soil overlay (z=1), asymmetric friendliness
        (22, Terrain::Grass, Terrain::Mud, "Grass/Mud (z3+z1, asymm)"),
        // Grass overlay (z=3) meets Soil overlay (z=1)
        (32, Terrain::Grass, Terrain::Sand, "Grass/Sand (z3+z1)"),
        // Same z=3 overlay layer, NOT friendly to each other
        (42, Terrain::Grass, Terrain::FairyRing, "Grass/Special (z3, !friend)"),
        // Stone overlay (z=2) meets Soil overlay (z=1)
        (52, Terrain::Rock, Terrain::Mud, "Rock/Mud (z2+z1)"),
        // Same z=1 overlay layer, different groups
        (62, Terrain::Mud, Terrain::Sand, "Mud/Sand (z1, same layer)"),
    ];
    for &(start_x, top, bottom, _) in pairs {
        fill_rect(&mut map, start_x, 13, 8, 4, top);
        fill_rect(&mut map, start_x, 17, 8, 4, bottom);
    }

    // ── Section C: Complex overlap scenarios (rows 26–35) ───────────────

    // C1: Rock island in grass — full autotile border ring (stone z=2 + grass z=3)
    fill_rect(&mut map, 2, 26, 6, 6, Terrain::Rock);

    // C2: Nested layers — Wall(2×2) inside Rock(6×6) inside Grass
    // Tests Rock/Stone friendliness AND the triple-layer boundary
    fill_rect(&mut map, 12, 26, 8, 8, Terrain::Rock);
    fill_rect(&mut map, 15, 29, 2, 2, Terrain::Wall);

    // C3: Colony-like building cluster — Den + Hearth + Stores in grass
    // Tests Building group (no overlay) surrounded by Grass overlay edges
    fill_rect(&mut map, 24, 27, 3, 3, Terrain::Den);
    fill_rect(&mut map, 28, 27, 3, 3, Terrain::Hearth);
    fill_rect(&mut map, 32, 27, 3, 3, Terrain::Stores);

    // C4: Triple boundary — Dirt|Rock|Grass meeting at one point
    // Three overlay layers (z=1, z=2, z=3) all rendering edges at the junction
    fill_rect(&mut map, 38, 26, 4, 10, Terrain::Mud);
    fill_rect(&mut map, 42, 26, 4, 10, Terrain::Rock);
    // Grass already fills 46+ from background

    // C5: Checkerboard Dirt/Sand — same overlay layer (z=1), different groups
    for y in 26..34 {
        for x in 50..58 {
            let terrain = if (x + y) % 2 == 0 { Terrain::Mud } else { Terrain::Sand };
            map.set(x, y, terrain);
        }
    }

    // C6: Single-tile isolates — bitmask = 0, each type alone in grass
    let isolates: &[(i32, Terrain)] = &[
        (62, Terrain::Water),
        (64, Terrain::Mud),
        (66, Terrain::Sand),
        (68, Terrain::Rock),
        (70, Terrain::Wall),
        (72, Terrain::FairyRing),
    ];
    for &(x, terrain) in isolates {
        map.set(x, 30, terrain);
    }

    // C7: 1-tile-wide corridors — tests narrow autotile paths
    // Horizontal rock corridor
    for x in 62..74 {
        map.set(x, 26, Terrain::Rock);
    }
    // Vertical rock corridor
    for y in 26..35 {
        map.set(75, y, Terrain::Rock);
    }

    dump_legend(patches, pairs);
    map
}

fn fill_rect(map: &mut TileMap, x: i32, y: i32, w: i32, h: i32, terrain: Terrain) {
    for dy in 0..h {
        for dx in 0..w {
            let tx = x + dx;
            let ty = y + dy;
            if map.in_bounds(tx, ty) {
                map.set(tx, ty, terrain);
            }
        }
    }
}

fn dump_legend(
    patches: &[(i32, Terrain, &str)],
    pairs: &[(i32, Terrain, Terrain, &str)],
) {
    use std::io::Write;
    let Ok(mut f) = std::fs::File::create("/tmp/clowder_test_map_legend.txt") else {
        return;
    };
    let _ = writeln!(f, "=== Clowder Test Map Legend ===\n");

    let _ = writeln!(f, "Section A — Isolated patches (rows 2–9, 6×6 each on grass):");
    for &(x, _, label) in patches {
        let _ = writeln!(f, "  x={x:>2}..{end}: {label}", end = x + 5);
    }

    let _ = writeln!(f, "\nSection B — Boundary pairs (rows 13–22, top=A / bottom=B):");
    for &(x, _, _, label) in pairs {
        let _ = writeln!(f, "  x={x:>2}..{end}: {label}", end = x + 7);
    }

    let _ = writeln!(f, "\nSection C — Complex scenarios (rows 26–35):");
    let _ = writeln!(f, "  x= 2: Rock island (6×6) in grass");
    let _ = writeln!(f, "  x=12: Nested Wall(2×2) in Rock(8×8) in grass");
    let _ = writeln!(f, "  x=24: Colony buildings (Den+Hearth+Stores) in grass");
    let _ = writeln!(f, "  x=38: Triple boundary (Mud|Rock|Grass)");
    let _ = writeln!(f, "  x=50: Dirt/Sand checkerboard");
    let _ = writeln!(f, "  x=62: Single-tile isolates (row 30)");
    let _ = writeln!(f, "  x=62: 1-wide rock corridors (h@row26, v@col75)");

    let _ = writeln!(f, "\nRows 40–59: Open grass for colony placement");
    eprintln!("Test map legend → /tmp/clowder_test_map_legend.txt");
}
