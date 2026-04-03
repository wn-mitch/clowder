use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders};

use crate::components::identity::LifeStage;
use crate::components::physical::Position;
use crate::components::wildlife::WildSpecies;
use crate::resources::map::{Terrain, TileMap};

/// Lightweight ward info for TUI rendering (avoids passing ECS queries into TUI).
pub struct WardDisplay {
    pub pos: Position,
    pub inverted: bool,
}

/// Lightweight herb info for TUI rendering.
pub struct HerbDisplay {
    pub pos: Position,
}

/// Lightweight zone info for TUI rendering.
pub struct ZoneDisplay {
    pub pos: Position,
    pub kind: crate::components::zone::ZoneKind,
}

/// Wildlife behavior for TUI rendering — simplified from WildlifeAiState.
#[derive(Debug, Clone, Copy)]
pub enum WildlifeBehavior {
    /// Patrolling or circling — just moving around.
    Roaming,
    /// Stationary ambush — lying in wait.
    Ambushing,
    /// Fleeing after a fight.
    Fleeing,
}

/// Lightweight cat info for TUI rendering.
pub struct CatDisplay {
    pub name: String,
    pub pos: Position,
    pub life_stage: LifeStage,
    pub fur_color: String,
    pub is_dead: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn render_map(
    frame: &mut Frame,
    area: Rect,
    map: &TileMap,
    cats: &[CatDisplay],
    wildlife: &[(WildSpecies, Position, WildlifeBehavior)],
    wards: &[WardDisplay],
    herbs: &[HerbDisplay],
    zones: &[ZoneDisplay],
    inspect_cursor: Option<Position>,
    tick: u64,
) {
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

            // Check if any entity is at this tile (priority: cat > wildlife > ward > herb > terrain).
            let cat_here = cats.iter().find(|c| c.pos.x == map_x && c.pos.y == map_y);
            let wildlife_here = wildlife.iter().find(|(_, pos, _)| pos.x == map_x && pos.y == map_y);
            let ward_here = wards.iter().find(|w| w.pos.x == map_x && w.pos.y == map_y);
            let herb_here = herbs.iter().find(|h| h.pos.x == map_x && h.pos.y == map_y);
            let zone_here = zones.iter().find(|z| z.pos.x == map_x && z.pos.y == map_y);

            let is_cursor = inspect_cursor.is_some_and(|c| c.x == map_x && c.y == map_y);

            let tile = map.get(map_x, map_y);

            let (symbol, mut style) = if let Some(cat) = cat_here {
                let (ch, color) = if cat.is_dead {
                    ('x', Color::DarkGray)
                } else {
                    (life_stage_symbol(cat.life_stage), fur_terminal_color(&cat.fur_color, tick))
                };
                let mut s = Style::default().fg(color);
                if !cat.is_dead {
                    s = s.add_modifier(Modifier::BOLD);
                }
                (ch, s)
            } else if let Some((species, _, behavior)) = wildlife_here {
                let color = wildlife_color(*species);
                let (ch, style) = match behavior {
                    WildlifeBehavior::Roaming => {
                        (species.symbol(), Style::default().fg(color))
                    }
                    WildlifeBehavior::Ambushing => {
                        // Uppercase + BOLD — visually alarming
                        let upper = species.symbol().to_ascii_uppercase();
                        (upper, Style::default().fg(color).add_modifier(Modifier::BOLD))
                    }
                    WildlifeBehavior::Fleeing => {
                        (species.symbol(), Style::default().fg(color).add_modifier(Modifier::DIM))
                    }
                };
                (ch, style)
            } else if let Some(ward) = ward_here {
                let color = if ward.inverted { Color::Red } else { Color::Cyan };
                ('+', Style::default().fg(color).add_modifier(Modifier::BOLD))
            } else if herb_here.is_some() {
                ('h', Style::default().fg(Color::Green))
            } else if let Some(zone) = zone_here {
                let color = match zone.kind {
                    crate::components::zone::ZoneKind::BuildHere => Color::LightBlue,
                    crate::components::zone::ZoneKind::FarmHere => Color::LightGreen,
                    crate::components::zone::ZoneKind::Avoid => Color::LightRed,
                };
                (zone.kind.symbol(), Style::default().fg(color))
            } else {
                let ch = tile.terrain.symbol();
                let color = terrain_color(tile.terrain);
                (ch, Style::default().fg(color))
            };

            // Corruption tint: tiles with corruption > 0.3 get a dark magenta bg.
            if tile.corruption > 0.3 {
                style = style.bg(Color::Indexed(53)); // dark magenta
            }

            if is_cursor {
                style = style.add_modifier(Modifier::REVERSED);
            }

            let cell = frame.buffer_mut().cell_mut((cell_x, cell_y));
            if let Some(c) = cell {
                c.set_char(symbol);
                c.set_style(style);
            }
        }
    }
}

fn life_stage_symbol(stage: LifeStage) -> char {
    match stage {
        LifeStage::Kitten => 'k',
        LifeStage::Young => 'c',
        LifeStage::Adult => 'C',
        LifeStage::Elder => 'e',
    }
}

fn fur_terminal_color(fur: &str, tick: u64) -> Color {
    match fur {
        "ginger" => Color::Indexed(208),
        "black" => Color::DarkGray,
        "white" => Color::White,
        "gray" => Color::Gray,
        "tabby brown" => Color::Indexed(130),
        "calico" => {
            const CALICO: [Color; 3] = [Color::Indexed(208), Color::DarkGray, Color::White];
            CALICO[(tick % 3) as usize]
        }
        "tortoiseshell" => Color::Indexed(166),
        "cream" => Color::LightYellow,
        "silver" => Color::Indexed(250),
        "russet" => Color::Red,
        _ => Color::Yellow,
    }
}

fn wildlife_color(species: WildSpecies) -> Color {
    match species {
        WildSpecies::Fox => Color::Red,
        WildSpecies::Hawk => Color::LightYellow,
        WildSpecies::Snake => Color::LightGreen,
        WildSpecies::ShadowFox => Color::Magenta,
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
        Terrain::Watchtower => Color::White,
        Terrain::WardPost => Color::Magenta,
        Terrain::Wall => Color::Gray,
        Terrain::Gate => Color::LightYellow,
        Terrain::FairyRing => Color::Magenta,
        Terrain::StandingStone => Color::White,
        Terrain::DeepPool | Terrain::AncientRuin => Color::DarkGray,
    }
}
