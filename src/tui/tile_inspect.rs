use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::components::physical::Position;
use crate::resources::map::TileMap;

// Re-export shared data types so existing TUI code continues to compile.
pub use crate::ui_data::{terrain_label, BuildingInfo};

// ---------------------------------------------------------------------------
// Tile detail rendering
// ---------------------------------------------------------------------------

pub fn render_tile_inspect(
    frame: &mut Frame,
    area: Rect,
    map: &TileMap,
    cursor: Position,
    cat_names_at_cursor: &[&str],
    building_info: Option<&BuildingInfo>,
) {
    let title = format!(" Tile ({}, {}) ", cursor.x, cursor.y);
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    if !map.in_bounds(cursor.x, cursor.y) {
        lines.push(Line::from(Span::styled(
            " Out of bounds",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let tile = map.get(cursor.x, cursor.y);

        // Terrain type
        let terrain_name = terrain_label(tile.terrain);
        lines.push(Line::from(vec![
            Span::styled(" Terrain: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} {}", tile.terrain.symbol(), terrain_name),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        ]));

        lines.push(Line::from(""));

        // Properties
        lines.push(Line::from(Span::styled(
            " Properties",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )));

        let cost = tile.terrain.movement_cost();
        let cost_str = if cost == u32::MAX {
            "impassable".to_string()
        } else {
            format!("{cost}")
        };
        lines.push(property_line("Move cost", &cost_str));
        lines.push(property_line(
            "Shelter",
            &format!("{:.0}%", tile.terrain.shelter_value() * 100.0),
        ));
        lines.push(property_line(
            "Forage yield",
            &format!("{:.1}", tile.terrain.foraging_yield()),
        ));
        lines.push(property_line(
            "Passable",
            if tile.terrain.is_passable() { "yes" } else { "no" },
        ));

        // Corruption / mystery (only show if non-zero)
        if tile.corruption > 0.0 {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(" Corruption: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:.2}", tile.corruption),
                    Style::default().fg(Color::Red),
                ),
            ]));
        }
        if tile.mystery > 0.0 {
            if tile.corruption == 0.0 {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(vec![
                Span::styled(" Mystery: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:.2}", tile.mystery),
                    Style::default().fg(Color::Magenta),
                ),
            ]));
        }

        // Building info
        if let Some(info) = building_info {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " Building",
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )));
            lines.push(property_line(
                "Condition",
                &format!("{:.0}%", info.structure.condition * 100.0),
            ));
            if let Some(ref site) = info.construction_site {
                lines.push(property_line(
                    "Progress",
                    &format!("{:.0}%", site.progress * 100.0),
                ));
                let mats_done = site.materials_complete();
                lines.push(property_line(
                    "Materials",
                    if mats_done { "complete" } else { "needed" },
                ));
            }
            if let Some(ref crop) = info.crop_state {
                lines.push(property_line(
                    "Crop growth",
                    &format!("{:.0}%", crop.growth * 100.0),
                ));
            }
            if let Some(ref gate) = info.gate_state {
                lines.push(property_line(
                    "Gate",
                    if gate.open { "open" } else { "closed" },
                ));
            }
        }

        // Entities present
        if !cat_names_at_cursor.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " Occupants",
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )));
            for name in cat_names_at_cursor {
                lines.push(Line::from(Span::styled(
                    format!("   {name}"),
                    Style::default().fg(Color::Yellow),
                )));
            }
        }
    }

    lines.truncate(inner.height as usize);
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn property_line<'a>(label: &str, value: &str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("   {label:<14}"), Style::default().fg(Color::DarkGray)),
        Span::styled(value.to_string(), Style::default().fg(Color::White)),
    ])
}
