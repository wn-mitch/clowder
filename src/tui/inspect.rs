use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::components::identity::LifeStage;

// Re-export shared data types so existing TUI code continues to compile.
pub use crate::ui_data::{
    AspirationDisplay, CatInspectData, NeedsSnapshot, RelationshipEntry, SkillsSnapshot,
    build_inspect_data,
};

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
    use crate::resources::relationships::BondType;

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

    // Current action and disposition
    if let Some(ref disp) = data.disposition {
        lines.push(Line::from(Span::styled(
            format!(" Disposition: {disp}"),
            Style::default().fg(Color::LightGreen),
        )));
    }
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

    // Action history
    if !data.action_history.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Recent Actions",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )));
        for entry in &data.action_history {
            let color = match entry.outcome.as_str() {
                "ok" => Color::DarkGray,
                "fail" => Color::Red,
                "interrupted" => Color::Yellow,
                _ => Color::DarkGray,
            };
            lines.push(Line::from(vec![
                Span::styled(format!("   t{}: ", entry.tick), Style::default().fg(Color::DarkGray)),
                Span::styled(&entry.action, Style::default().fg(color)),
                Span::styled(format!(" ({})", entry.outcome), Style::default().fg(color)),
            ]));
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
