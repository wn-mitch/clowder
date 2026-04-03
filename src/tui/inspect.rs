use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::ai::CurrentAction;
use crate::components::identity::LifeStage;
use crate::components::physical::Needs;
use crate::components::skills::Skills;
use crate::resources::relationships::BondType;

// ---------------------------------------------------------------------------
// CatInspectData
// ---------------------------------------------------------------------------

/// Data needed to render the cat inspect panel.
pub struct CatInspectData {
    pub name: String,
    pub life_stage: LifeStage,
    pub needs: NeedsSnapshot,
    pub mood_valence: f32,
    pub action: String,
    pub skills: SkillsSnapshot,
    pub relationships: Vec<RelationshipEntry>,
    pub is_coordinator: bool,
    pub active_directive: Option<String>,
    pub zodiac: Option<String>,
    pub fated_love: Option<(String, bool)>,  // (partner name, awakened)
    pub fated_rival: Option<(String, bool)>, // (rival name, awakened)
    pub aspirations: Vec<AspirationDisplay>,
    pub completed_aspirations: Vec<String>,
    pub likes: Vec<String>,
    pub dislikes: Vec<String>,
}

pub struct AspirationDisplay {
    pub chain_name: String,
    pub milestone_name: String,
    pub progress: u32,
    pub target: u32,
}

pub struct NeedsSnapshot {
    pub hunger: f32,
    pub energy: f32,
    pub warmth: f32,
    pub safety: f32,
    pub social: f32,
}

pub struct SkillsSnapshot {
    pub hunting: f32,
    pub foraging: f32,
    pub herbcraft: f32,
    pub building: f32,
    pub combat: f32,
    pub magic: f32,
}

pub struct RelationshipEntry {
    pub name: String,
    pub fondness: f32,
    pub bond: Option<BondType>,
}

impl NeedsSnapshot {
    pub fn from_needs(needs: &Needs) -> Self {
        Self {
            hunger: needs.hunger,
            energy: needs.energy,
            warmth: needs.warmth,
            safety: needs.safety,
            social: needs.social,
        }
    }
}

impl SkillsSnapshot {
    pub fn from_skills(skills: &Skills) -> Self {
        Self {
            hunting: skills.hunting,
            foraging: skills.foraging,
            herbcraft: skills.herbcraft,
            building: skills.building,
            combat: skills.combat,
            magic: skills.magic,
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn need_bar(label: &str, value: f32, width: usize) -> Line<'static> {
    let filled = (value * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    let pct = (value * 100.0).round() as u8;

    let color = if value < 0.2 {
        Color::Red
    } else if value < 0.5 {
        Color::Yellow
    } else {
        Color::Green
    };

    Line::from(vec![
        Span::styled(format!(" {label:<8}"), Style::default().fg(Color::White)),
        Span::styled("[", Style::default().fg(Color::DarkGray)),
        Span::styled("#".repeat(filled), Style::default().fg(color)),
        Span::styled("-".repeat(empty), Style::default().fg(Color::DarkGray)),
        Span::styled("]", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" {pct:>3}%"), Style::default().fg(Color::White)),
    ])
}

fn skill_line(label: &str, value: f32) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {label:<10}"), Style::default().fg(Color::White)),
        Span::styled(format!("{value:.2}"), Style::default().fg(Color::Cyan)),
    ])
}

fn relationship_line(entry: &RelationshipEntry, width: usize) -> Line<'static> {
    let bar_width: usize = 6;
    let filled = ((entry.fondness.clamp(-1.0, 1.0) + 1.0) / 2.0 * bar_width as f32).round() as usize;
    let empty = bar_width.saturating_sub(filled);
    let bond_str = match entry.bond {
        Some(BondType::Friends) => " Friends",
        Some(BondType::Partners) => " Partners",
        Some(BondType::Mates) => " Mates",
        None => "",
    };
    let _ = width; // used for future formatting if needed

    Line::from(vec![
        Span::styled(format!("   {:<10}", entry.name), Style::default().fg(Color::White)),
        Span::styled("[", Style::default().fg(Color::DarkGray)),
        Span::styled("#".repeat(filled), Style::default().fg(Color::Magenta)),
        Span::styled("-".repeat(empty), Style::default().fg(Color::DarkGray)),
        Span::styled("]", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" {:.2}", entry.fondness), Style::default().fg(Color::White)),
        Span::styled(bond_str.to_string(), Style::default().fg(Color::LightYellow)),
    ])
}

pub fn render_inspect(frame: &mut Frame, area: Rect, data: &CatInspectData) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", data.name));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    // Life stage and mood
    let stage_str = match data.life_stage {
        LifeStage::Kitten => "Kitten",
        LifeStage::Young => "Young",
        LifeStage::Adult => "Adult",
        LifeStage::Elder => "Elder",
    };
    let mood_str = if data.mood_valence > 0.3 {
        "happy"
    } else if data.mood_valence > -0.3 {
        "neutral"
    } else {
        "unhappy"
    };
    let mut stage_spans = vec![
        Span::styled(format!(" {stage_str}"), Style::default().fg(Color::White)),
    ];
    if data.is_coordinator {
        stage_spans.push(Span::styled(
            " [Coordinator]",
            Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD),
        ));
    }
    stage_spans.push(Span::styled(" — ", Style::default().fg(Color::DarkGray)));
    stage_spans.push(Span::styled(
        format!("mood: {mood_str} ({:.1})", data.mood_valence),
        Style::default().fg(Color::White),
    ));
    lines.push(Line::from(stage_spans));

    // Zodiac sign
    if let Some(ref zodiac) = data.zodiac {
        lines.push(Line::from(Span::styled(
            format!(" Born under {zodiac}"),
            Style::default().fg(Color::LightMagenta),
        )));
    }

    // Current action
    lines.push(Line::from(Span::styled(
        format!(" Action: {}", data.action),
        Style::default().fg(Color::Cyan),
    )));

    // Active directive (from a coordinator)
    if let Some(ref directive) = data.active_directive {
        lines.push(Line::from(Span::styled(
            format!(" Directed: {directive}"),
            Style::default().fg(Color::LightYellow),
        )));
    }

    // Fated connections
    if let Some((ref name, awakened)) = data.fated_love {
        let status = if awakened { "awakened" } else { "dormant" };
        lines.push(Line::from(Span::styled(
            format!(" Fated Love: {name} ({status})"),
            Style::default().fg(Color::LightRed),
        )));
    }
    if let Some((ref name, awakened)) = data.fated_rival {
        let status = if awakened { "awakened" } else { "dormant" };
        lines.push(Line::from(Span::styled(
            format!(" Fated Rival: {name} ({status})"),
            Style::default().fg(Color::LightBlue),
        )));
    }

    lines.push(Line::from(""));

    // Needs
    lines.push(Line::from(Span::styled(
        " Needs",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    )));
    let w = 10;
    lines.push(need_bar("Hunger", data.needs.hunger, w));
    lines.push(need_bar("Energy", data.needs.energy, w));
    lines.push(need_bar("Warmth", data.needs.warmth, w));
    lines.push(need_bar("Safety", data.needs.safety, w));
    lines.push(need_bar("Social", data.needs.social, w));

    lines.push(Line::from(""));

    // Skills
    lines.push(Line::from(Span::styled(
        " Skills",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    )));
    lines.push(skill_line("Hunting", data.skills.hunting));
    lines.push(skill_line("Foraging", data.skills.foraging));
    lines.push(skill_line("Herbcraft", data.skills.herbcraft));
    lines.push(skill_line("Building", data.skills.building));
    lines.push(skill_line("Combat", data.skills.combat));
    lines.push(skill_line("Magic", data.skills.magic));

    lines.push(Line::from(""));

    // Relationships
    if !data.relationships.is_empty() {
        lines.push(Line::from(Span::styled(
            " Relationships",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )));
        for entry in &data.relationships {
            lines.push(relationship_line(entry, inner.width as usize));
        }
    }

    // Aspirations
    if !data.aspirations.is_empty() || !data.completed_aspirations.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Aspirations",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )));
        for asp in &data.aspirations {
            let bar_width: usize = 10;
            let target = asp.target.max(1);
            let filled = ((asp.progress as f32 / target as f32) * bar_width as f32).round() as usize;
            let empty = bar_width.saturating_sub(filled);
            lines.push(Line::from(vec![
                Span::styled(format!("   {}: ", asp.chain_name), Style::default().fg(Color::White)),
                Span::styled(&asp.milestone_name, Style::default().fg(Color::Cyan)),
                Span::styled(" [", Style::default().fg(Color::DarkGray)),
                Span::styled("#".repeat(filled), Style::default().fg(Color::Green)),
                Span::styled("-".repeat(empty), Style::default().fg(Color::DarkGray)),
                Span::styled("]", Style::default().fg(Color::DarkGray)),
                Span::styled(format!(" {}/{}", asp.progress, target), Style::default().fg(Color::White)),
            ]));
        }
        for name in &data.completed_aspirations {
            lines.push(Line::from(Span::styled(
                format!("   {name} (complete)"),
                Style::default().fg(Color::LightGreen),
            )));
        }
    }

    // Likes / Dislikes
    if !data.likes.is_empty() || !data.dislikes.is_empty() {
        lines.push(Line::from(""));
        if !data.likes.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(" Likes: {}", data.likes.join(", ")),
                Style::default().fg(Color::LightGreen),
            )));
        }
        if !data.dislikes.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(" Dislikes: {}", data.dislikes.join(", ")),
                Style::default().fg(Color::LightRed),
            )));
        }
    }

    // Truncate to available height
    lines.truncate(inner.height as usize);

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Render the cat selection list (when in Selecting mode).
pub fn render_cat_list(
    frame: &mut Frame,
    area: Rect,
    cats: &[(bevy_ecs::entity::Entity, String)],
    selected_index: usize,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Select Cat ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    for (i, (_entity, name)) in cats.iter().enumerate() {
        let style = if i == selected_index {
            Style::default().fg(Color::Black).bg(Color::White)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(Span::styled(format!(" {name}"), style)));
    }

    lines.truncate(inner.height as usize);
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Build a `CatInspectData` from ECS components.
#[allow(clippy::too_many_arguments)]
pub fn build_inspect_data(
    name: &str,
    life_stage: LifeStage,
    needs: &Needs,
    mood_valence: f32,
    current: &CurrentAction,
    skills: &Skills,
    relationships: Vec<RelationshipEntry>,
    is_coordinator: bool,
    active_directive: Option<String>,
    zodiac: Option<String>,
    fated_love: Option<(String, bool)>,
    fated_rival: Option<(String, bool)>,
    aspirations: Vec<AspirationDisplay>,
    completed_aspirations: Vec<String>,
    likes: Vec<String>,
    dislikes: Vec<String>,
) -> CatInspectData {
    CatInspectData {
        name: name.to_string(),
        life_stage,
        needs: NeedsSnapshot::from_needs(needs),
        mood_valence,
        action: format!("{:?}", current.action),
        skills: SkillsSnapshot::from_skills(skills),
        relationships,
        is_coordinator,
        active_directive,
        zodiac,
        fated_love,
        fated_rival,
        aspirations,
        completed_aspirations,
        likes,
        dislikes,
    }
}
