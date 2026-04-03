use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders};

use crate::components::physical::Position;
use crate::resources::map::{Terrain, TileMap};

pub fn render_map(frame: &mut Frame, area: Rect, map: &TileMap, cats: &[(&str, Position)]) {
    let block = Block::default().borders(Borders::ALL).title(" Map ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let view_w = inner.width as i32;
    let view_h = inner.height as i32;

    // Center the view on the map center.
    let map_cx = map.width / 2;
    let map_cy = map.height / 2;
    let start_x = map_cx - view_w / 2;
    let start_y = map_cy - view_h / 2;

    for screen_y in 0..view_h {
        for screen_x in 0..view_w {
            let map_x = start_x + screen_x;
            let map_y = start_y + screen_y;

            if !map.in_bounds(map_x, map_y) {
                continue;
            }

            let cell_x = inner.x + screen_x as u16;
            let cell_y = inner.y + screen_y as u16;

            // Check if any cat is at this tile.
            let cat_here = cats.iter().find(|(_, pos)| pos.x == map_x && pos.y == map_y);

            let (symbol, style) = if let Some((name, _)) = cat_here {
                let ch = name.chars().next().unwrap_or('?');
                (ch, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            } else {
                let tile = map.get(map_x, map_y);
                let ch = tile.terrain.symbol();
                let color = terrain_color(tile.terrain);
                (ch, Style::default().fg(color))
            };

            let cell = frame.buffer_mut().cell_mut((cell_x, cell_y));
            if let Some(c) = cell {
                c.set_char(symbol);
                c.set_style(style);
            }
        }
    }
}

fn terrain_color(terrain: Terrain) -> Color {
    match terrain {
        Terrain::Grass => Color::Green,
        Terrain::LightForest | Terrain::DenseForest => Color::DarkGray, // approximation for DarkGreen
        Terrain::Water => Color::Blue,
        Terrain::Rock => Color::Gray,
        Terrain::Mud => Color::DarkGray,
        Terrain::Sand => Color::Yellow,
        Terrain::Den => Color::LightMagenta,
        Terrain::Hearth => Color::LightRed,
        Terrain::Stores => Color::LightCyan,
        Terrain::Workshop => Color::Cyan,
        Terrain::Garden => Color::LightGreen,
        Terrain::FairyRing => Color::Magenta,
        Terrain::StandingStone => Color::White,
        Terrain::DeepPool | Terrain::AncientRuin => Color::DarkGray,
    }
}
