//! Shared data types for UI display — used by both the TUI and Bevy UI renderers.

use bevy::prelude::Vec2;
use bevy_ecs::prelude::*;

use crate::ai::CurrentAction;
use crate::components::building::{ConstructionSite, CropState, GateState, Structure};
use crate::components::identity::LifeStage;
use crate::components::physical::Needs;
use crate::components::skills::Skills;
use crate::resources::map::Terrain;
use crate::resources::relationships::BondType;

// ---------------------------------------------------------------------------
// Inspection state (Bevy resource for the graphical UI)
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
pub struct InspectionState {
    pub mode: InspectionMode,
    pub last_selected_cat: Option<Entity>,
    pub cursor_world_pos: Option<Vec2>,
    pub cursor_grid_pos: Option<(i32, i32)>,
    /// Screen-space position of the last click, used for tile popup placement.
    pub click_screen_pos: Option<Vec2>,
}

#[derive(Default, PartialEq)]
pub enum InspectionMode {
    #[default]
    None,
    CatInspect(Entity),
    TileInspect {
        x: i32,
        y: i32,
    },
    WildlifeInspect(Entity),
}

// ---------------------------------------------------------------------------
// Cat inspect data
// ---------------------------------------------------------------------------

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
    pub fated_love: Option<(String, bool)>,
    pub fated_rival: Option<(String, bool)>,
    pub aspirations: Vec<AspirationDisplay>,
    pub completed_aspirations: Vec<String>,
    pub likes: Vec<String>,
    pub dislikes: Vec<String>,
    pub disposition: Option<String>,
    pub action_history: Vec<ActionHistoryEntry>,
}

pub struct ActionHistoryEntry {
    pub action: String,
    pub tick: u64,
    pub outcome: String,
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
    pub temperature: f32,
    pub safety: f32,
    pub social: f32,
    /// §7.W social_warmth fulfillment axis (not a Maslow need, but
    /// displayed alongside needs for inspection convenience).
    pub social_warmth: f32,
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
    pub fn from_needs(
        needs: &Needs,
        fulfillment: Option<&crate::components::fulfillment::Fulfillment>,
    ) -> Self {
        Self {
            hunger: needs.hunger,
            energy: needs.energy,
            temperature: needs.temperature,
            safety: needs.safety,
            social: needs.social,
            social_warmth: fulfillment.map_or(0.6, |f| f.social_warmth),
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
// Cat inspect builder
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn build_inspect_data(
    name: &str,
    life_stage: LifeStage,
    needs: &Needs,
    fulfillment: Option<&crate::components::fulfillment::Fulfillment>,
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
    disposition: Option<&crate::components::goap_plan::GoapPlan>,
    action_history: Option<&crate::components::disposition::ActionHistory>,
) -> CatInspectData {
    let disposition_str = disposition.map(|d| {
        if d.target_trips == u32::MAX {
            d.kind.label().to_string()
        } else {
            format!("{} ({}/{})", d.kind.label(), d.trips_done, d.target_trips)
        }
    });

    let history_entries = action_history
        .map(|h| {
            h.entries
                .iter()
                .rev()
                .take(10)
                .map(|r| ActionHistoryEntry {
                    action: format!("{:?}", r.action),
                    tick: r.tick,
                    outcome: match r.outcome {
                        crate::components::disposition::ActionOutcome::Success => "ok".into(),
                        crate::components::disposition::ActionOutcome::Failure => "fail".into(),
                        crate::components::disposition::ActionOutcome::Interrupted => {
                            "interrupted".into()
                        }
                    },
                })
                .collect()
        })
        .unwrap_or_default();

    CatInspectData {
        name: name.to_string(),
        life_stage,
        needs: NeedsSnapshot::from_needs(needs, fulfillment),
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
        disposition: disposition_str,
        action_history: history_entries,
    }
}

// ---------------------------------------------------------------------------
// Tile / building inspect data
// ---------------------------------------------------------------------------

pub struct BuildingInfo {
    pub structure: Structure,
    pub construction_site: Option<ConstructionSite>,
    pub crop_state: Option<CropState>,
    pub gate_state: Option<GateState>,
}

pub fn terrain_label(terrain: Terrain) -> &'static str {
    match terrain {
        Terrain::Grass => "Grass",
        Terrain::LightForest => "Light Forest",
        Terrain::DenseForest => "Dense Forest",
        Terrain::Water => "Water",
        Terrain::Rock => "Rock",
        Terrain::Mud => "Mud",
        Terrain::Sand => "Sand",
        Terrain::Den => "Den",
        Terrain::Hearth => "Hearth",
        Terrain::Kitchen => "Kitchen",
        Terrain::Stores => "Stores",
        Terrain::Workshop => "Workshop",
        Terrain::Garden => "Garden",
        Terrain::Watchtower => "Watchtower",
        Terrain::WardPost => "Ward Post",
        Terrain::Wall => "Wall",
        Terrain::Gate => "Gate",
        Terrain::FairyRing => "Fairy Ring",
        Terrain::StandingStone => "Standing Stone",
        Terrain::DeepPool => "Deep Pool",
        Terrain::AncientRuin => "Ancient Ruin",
    }
}
