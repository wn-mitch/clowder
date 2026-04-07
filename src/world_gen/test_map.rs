use crate::resources::map::{Terrain, TileMap};

/// Generate a hand-crafted test map for visual debugging of the tilemap
/// rendering pipeline. Every terrain-group boundary combination is laid out
/// in a predictable grid so rendering artifacts are immediately attributable
/// to a specific terrain pairing.
///
/// All test patterns are placed OUTSIDE the colony-eligible zone (margin=15
/// from each edge) so `find_colony_site` always lands in the open grass
/// center and never overwrites test terrain.
///
/// Sections A and B share a 7-column grid (x=2,12,22,32,42,52,62, each 8
/// wide with a 2-tile grass gap) so columns align vertically.
///
/// Layout (80 wide × 60 tall, grass background):
///
///   Rows  1– 5: Section A — isolated 8×5 patches (one per terrain group)
///   Rows  7–14: Section B — boundary-pair rectangles (A top, B bottom)
///   Rows 15–44: Open grass (colony-eligible zone)
///   Rows 45–53: Section C — complex multi-layer overlap scenarios
///   Rows 55–59: Section D — bug reproduction patterns (irregular boundaries)
///
/// Also writes an annotated legend to /tmp/clowder_test_map_legend.txt.
pub fn generate_test_map() -> TileMap {
    let mut map = TileMap::new(80, 60, Terrain::Grass);

    // Column grid shared by Sections A and B: 7 slots, each 8 wide, gap 2.
    const COLS: [i32; 7] = [2, 12, 22, 32, 42, 52, 62];
    const SLOT_W: i32 = 8;

    // ── Section A: Isolated patches (rows 1–5) ─────────────────────────
    // One terrain group per column. 8 wide × 5 tall on grass background.
    let patches: &[(Terrain, &str)] = &[
        (Terrain::Water, "Water"),
        (Terrain::Mud, "Dirt (Mud)"),
        (Terrain::Sand, "Sand"),
        (Terrain::Rock, "Rock"),
        (Terrain::Wall, "Stone (Wall)"),
        (Terrain::Den, "Building (Den)"),
        (Terrain::FairyRing, "Special (FairyRing)"),
    ];
    for (i, &(terrain, _)) in patches.iter().enumerate() {
        fill_rect(&mut map, COLS[i], 1, SLOT_W, 5, terrain);
    }

    // ── Section B: Boundary pairs (rows 7–14) ──────────────────────────
    // Same column grid. Top 4 rows = type A, bottom 4 rows = type B.
    // All pairs use two visually distinct non-grass terrains. The grass
    // background already tests every terrain's boundary with grass.
    let pairs: &[(Terrain, Terrain, &str)] = &[
        // No overlay meets Stone overlay (z=2)
        (Terrain::Water, Terrain::Rock, "Water/Rock (none+z2)"),
        // No overlay meets Stone overlay (z=2) — Wall variant
        (Terrain::Water, Terrain::Wall, "Water/Wall (none+z2)"),
        // Stone overlay (z=2) meets Soil overlay (z=1)
        (Terrain::Rock, Terrain::Mud, "Rock/Mud (z2+z1)"),
        // Stone overlay (z=2) meets Soil overlay (z=1) — Sand variant
        (Terrain::Rock, Terrain::Sand, "Rock/Sand (z2+z1)"),
        // Soil overlay (z=1) meets Grass overlay (z=3, Special, !friendly)
        (Terrain::Sand, Terrain::FairyRing, "Sand/Special (z1+z3)"),
        // Stone variant meets Soil overlay
        (Terrain::Wall, Terrain::Mud, "Wall/Mud (z2+z1)"),
        // Building (no overlay) meets Stone overlay (z=2)
        (Terrain::Den, Terrain::Rock, "Den/Rock (none+z2)"),
    ];
    for (i, &(top, bottom, _)) in pairs.iter().enumerate() {
        fill_rect(&mut map, COLS[i], 7, SLOT_W, 4, top);
        fill_rect(&mut map, COLS[i], 11, SLOT_W, 4, bottom);
    }

    // ── Section C: Complex overlap scenarios (rows 45–53) ───────────────

    // C1: Rock island in grass — full autotile border ring (stone z=2 + grass z=3)
    fill_rect(&mut map, 2, 45, 6, 6, Terrain::Rock);

    // C2: Nested layers — Wall(2×2) inside Rock(8×8) inside Grass
    // Tests Rock/Stone friendliness AND the triple-layer boundary
    fill_rect(&mut map, 11, 45, 8, 8, Terrain::Rock);
    fill_rect(&mut map, 14, 48, 2, 2, Terrain::Wall);

    // C3: Colony-like building cluster — Den + Hearth + Stores in grass
    // Tests Building group (no overlay) surrounded by Grass overlay edges
    fill_rect(&mut map, 22, 46, 3, 3, Terrain::Den);
    fill_rect(&mut map, 26, 46, 3, 3, Terrain::Hearth);
    fill_rect(&mut map, 30, 46, 3, 3, Terrain::Stores);

    // C4: Triple boundary — Dirt|Rock|Grass meeting at one point
    // Three overlay layers (z=1, z=2, z=3) all rendering edges at the junction
    fill_rect(&mut map, 36, 45, 4, 8, Terrain::Mud);
    fill_rect(&mut map, 40, 45, 4, 8, Terrain::Rock);
    // Grass already fills x=44+ from background

    // C5: Checkerboard Dirt/Sand — same overlay layer (z=1), different groups
    for y in 45..53 {
        for x in 48..56 {
            let terrain = if (x + y) % 2 == 0 { Terrain::Mud } else { Terrain::Sand };
            map.set(x, y, terrain);
        }
    }

    // C6: Single-tile isolates — bitmask = 0, each type alone in grass
    let isolates: &[(i32, Terrain)] = &[
        (60, Terrain::Water),
        (62, Terrain::Mud),
        (64, Terrain::Sand),
        (66, Terrain::Rock),
        (68, Terrain::Wall),
        (70, Terrain::FairyRing),
        (72, Terrain::Den),
    ];
    for &(x, terrain) in isolates {
        map.set(x, 49, terrain);
    }

    // C7: 1-tile-wide corridors — tests narrow autotile paths
    for x in 60..74 {
        map.set(x, 45, Terrain::Rock); // horizontal
    }
    for y in 45..53 {
        map.set(75, y, Terrain::Rock); // vertical
    }

    // ── Section D: Bug reproduction (rows 55–59) ────────────────────────
    // Patterns that mimic what Perlin noise produces near colony centers,
    // where the original "double layered" artifact was observed.

    // D1: Rock pocket with buildings adjacent — the likely bug trigger.
    // Buildings have no overlay; Rock has stone overlay (z=2); surrounding
    // Grass has overlay (z=3). Three rendering regimes meet.
    fill_rect(&mut map, 2, 55, 5, 4, Terrain::Rock);
    fill_rect(&mut map, 7, 55, 3, 4, Terrain::Den);
    fill_rect(&mut map, 10, 55, 3, 4, Terrain::Hearth);
    // Grass surrounds all of it

    // D2: Irregular (non-rectangular) rock boundary in grass.
    // L-shape: horizontal bar + vertical arm. Tests bitmask at concave corner.
    for x in 16..26 {
        map.set(x, 55, Terrain::Rock);
        map.set(x, 56, Terrain::Rock);
    }
    for y in 55..60 {
        map.set(16, y, Terrain::Rock);
        map.set(17, y, Terrain::Rock);
    }

    // D3: Rock island with 1-tile grass moat before buildings.
    // Tests whether a narrow grass gap between Rock and Building
    // creates overlapping autotile edges from both sides.
    fill_rect(&mut map, 30, 55, 3, 4, Terrain::Rock);
    // 1-tile grass gap at x=33 (already grass)
    fill_rect(&mut map, 34, 55, 3, 4, Terrain::Den);

    // D4: Water-Rock-Grass striped sequence — three groups in a row,
    // each pair producing overlapping overlay edges.
    fill_rect(&mut map, 40, 55, 3, 4, Terrain::Water);
    fill_rect(&mut map, 43, 55, 3, 4, Terrain::Rock);
    // Grass continues at x=46+

    // D5: Dense scatter — alternating Rock/Grass single tiles.
    // Maximizes edge density to stress-test overlay rendering.
    for y in 55..59 {
        for x in 50..60 {
            if (x + y) % 2 == 0 {
                map.set(x, y, Terrain::Rock);
            }
        }
    }

    // D6: Building cluster surrounded by rock, surrounded by grass.
    // Two concentric borders: stone overlay (z=2) + grass overlay (z=3).
    fill_rect(&mut map, 63, 55, 5, 4, Terrain::Rock);
    fill_rect(&mut map, 64, 56, 3, 2, Terrain::Den);

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
    patches: &[(Terrain, &str)],
    pairs: &[(Terrain, Terrain, &str)],
) {
    use std::io::Write;
    let Ok(mut f) = std::fs::File::create("/tmp/clowder_test_map_legend.txt") else {
        return;
    };
    const COLS: [i32; 7] = [2, 12, 22, 32, 42, 52, 62];

    let _ = writeln!(f, "=== Clowder Test Map Legend ===");
    let _ = writeln!(f, "Colony-safe: test patterns are in rows 0-14 and 45-59.");
    let _ = writeln!(f, "Colony spawns in rows 15-44 (open grass).");
    let _ = writeln!(f, "Column grid: x=2,12,22,32,42,52,62 (8 wide, 2 gap)\n");

    let _ = writeln!(f, "Section A — Isolated patches (rows 1–5, 8×5 each on grass):");
    for (i, &(_, label)) in patches.iter().enumerate() {
        let x = COLS[i];
        let _ = writeln!(f, "  col {i} x={x:>2}..{end}: {label}", end = x + 7);
    }

    let _ = writeln!(f, "\nSection B — Boundary pairs (rows 7–14, top=A / bottom=B):");
    for (i, &(_, _, label)) in pairs.iter().enumerate() {
        let x = COLS[i];
        let _ = writeln!(f, "  col {i} x={x:>2}..{end}: {label}", end = x + 7);
    }

    let _ = writeln!(f, "\nSection C — Complex scenarios (rows 45–53):");
    let _ = writeln!(f, "  x= 2: Rock island (6×6) in grass");
    let _ = writeln!(f, "  x=11: Nested Wall(2×2) in Rock(8×8) in grass");
    let _ = writeln!(f, "  x=22: Colony buildings (Den+Hearth+Stores) in grass");
    let _ = writeln!(f, "  x=36: Triple boundary (Mud|Rock|Grass)");
    let _ = writeln!(f, "  x=48: Dirt/Sand checkerboard");
    let _ = writeln!(f, "  x=60: Single-tile isolates (row 49)");
    let _ = writeln!(f, "  x=60: 1-wide rock corridors (h@row45, v@col75)");

    let _ = writeln!(f, "\nSection D — Bug reproduction (rows 55–59):");
    let _ = writeln!(f, "  x= 2: Rock pocket + adjacent buildings in grass");
    let _ = writeln!(f, "  x=16: Irregular L-shaped rock boundary");
    let _ = writeln!(f, "  x=30: Rock + 1-tile grass moat + buildings");
    let _ = writeln!(f, "  x=40: Water|Rock|Grass stripe sequence");
    let _ = writeln!(f, "  x=50: Dense Rock/Grass checkerboard");
    let _ = writeln!(f, "  x=63: Building cluster inside rock ring");
    eprintln!("Test map legend → /tmp/clowder_test_map_legend.txt");
}
