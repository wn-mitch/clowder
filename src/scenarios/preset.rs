//! Builder for a single cat in a scenario. Mirrors the founder bundle so
//! scenarios produce cats indistinguishable from production-spawned ones.

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;

use crate::components::fulfillment::Fulfillment;
use crate::components::identity::{Appearance, Gender, Orientation};
use crate::components::personality::Personality;
use crate::components::physical::{Needs, Position};
use crate::components::skills::Skills;
use crate::components::zodiac::ZodiacSign;
use crate::world_gen::colony::CatBlueprint;

/// Markers a scenario can attach to a cat after spawn. Each variant maps
/// to a ZST `Component` insert. We materialize markers post-spawn rather
/// than threading them through the bundle because some are normally
/// authored by per-tick systems (e.g. life-stage markers in
/// `growth.rs`); attaching them at spawn time is a scenario-only override.
#[derive(Clone, Copy)]
pub enum MarkerKind {
    Parent,
    IsParentOfHungryKitten,
    CanHunt,
    CanForage,
    CanWard,
    CanCook,
    Adult,
    Kitten,
    Young,
    Elder,
}

#[derive(Clone, Default)]
pub struct MarkerSet(pub Vec<MarkerKind>);

impl MarkerSet {
    pub fn apply(&self, world: &mut World, entity: Entity) {
        for marker in &self.0 {
            let mut em = world.entity_mut(entity);
            match marker {
                MarkerKind::Parent => {
                    em.insert(crate::components::markers::Parent);
                }
                MarkerKind::IsParentOfHungryKitten => {
                    em.insert(crate::components::markers::IsParentOfHungryKitten);
                }
                MarkerKind::CanHunt => {
                    em.insert(crate::components::markers::CanHunt);
                }
                MarkerKind::CanForage => {
                    em.insert(crate::components::markers::CanForage);
                }
                MarkerKind::CanWard => {
                    em.insert(crate::components::markers::CanWard);
                }
                MarkerKind::CanCook => {
                    em.insert(crate::components::markers::CanCook);
                }
                MarkerKind::Adult => {
                    em.insert(crate::components::markers::Adult);
                }
                MarkerKind::Kitten => {
                    em.insert(crate::components::markers::Kitten);
                }
                MarkerKind::Young => {
                    em.insert(crate::components::markers::Young);
                }
                MarkerKind::Elder => {
                    em.insert(crate::components::markers::Elder);
                }
            }
        }
    }
}

/// Builder over the founder spawn bundle. Construct via [`Self::adult`],
/// [`Self::kitten`], or [`Self::fox`], then chain setters for the fields
/// the scenario cares about. Defaults match `build_new_world`'s spawn for
/// every field a scenario doesn't touch.
pub struct CatPreset {
    pub name: String,
    pub gender: Gender,
    pub orientation: Orientation,
    pub personality: Personality,
    pub appearance: Appearance,
    pub skills: Skills,
    pub magic_affinity: f32,
    pub zodiac_sign: ZodiacSign,
    pub position: Position,
    pub born_tick: u64,
    pub needs: Needs,
    pub fulfillment: Fulfillment,
    pub markers: MarkerSet,
}

/// Internal struct returned by [`CatPreset::into_blueprint`] to keep the
/// API surface narrow on the env-side spawn helpers.
pub(crate) struct PresetParts {
    pub position: Position,
    pub needs: Needs,
    pub fulfillment: Fulfillment,
    pub blueprint: CatBlueprint,
    pub markers: MarkerSet,
}

impl CatPreset {
    /// A balanced adult cat — all personality axes at 0.5, default needs
    /// (hunger=1.0 etc.), no markers. Scenarios chain setters to twist
    /// the dimensions they care about.
    pub fn adult(name: impl Into<String>, position: Position) -> Self {
        Self {
            name: name.into(),
            gender: Gender::Queen,
            orientation: Orientation::Straight,
            personality: balanced_personality(),
            appearance: default_appearance(),
            skills: Skills::default(),
            magic_affinity: 0.0,
            zodiac_sign: ZodiacSign::WarmDen,
            position,
            // Scenarios spawn at `start_tick = 60 * ticks_per_season`
            // (`env::init_scenario_world_with`). With the default
            // `SimConfig::ticks_per_season = 20_000`, `start_tick =
            // 1_200_000`. `Age::stage` bins `seasons = age_ticks /
            // ticks_per_season` as Kitten(0..=3) / Young(4..=11) /
            // Adult(12..=59) / Elder(>=60).
            //
            // Pre-2026-05-21 this was `born_tick: 0` with a stale
            // comment claiming `ticks_per_season = 1000` — combined
            // with the current default of 20_000, that placed the
            // "adult" preset at seasons=60 which is **Elder**, not
            // Adult. The discrepancy only surfaced when 367/436/437's
            // preservation scenarios hit `update_capability_markers`'s
            // strict `Has<Adult>` gate for `CanDry`/`CanCook` — other
            // scenarios that nominally relied on adulthood (Forage,
            // Pickup, etc.) never exercised an Adult-gated marker so
            // the misalignment stayed latent.
            //
            // Pin the preset 30 seasons before start_tick — mid-Adult
            // band (12..=59), well away from Young/Elder edges so a
            // change to `ticks_per_season` doesn't immediately re-bin
            // the cat. Computes via the default; scenarios that
            // override `ticks_per_season` need to also override
            // `born_tick` via `with_born_tick`.
            born_tick: 30 * 20_000,
            needs: Needs::default(),
            fulfillment: Fulfillment::default(),
            markers: MarkerSet::default(),
        }
    }

    /// A kitten — Needs override (hunger=0.5, energy=0.8, mating=1.0
    /// matching `pregnancy.rs:128-133`) is applied by `env::spawn_kitten`
    /// if the caller doesn't override needs explicitly. `born_tick` is
    /// the current tick (kitten just born).
    pub fn kitten(name: impl Into<String>, position: Position, current_tick: u64) -> Self {
        Self {
            name: name.into(),
            gender: Gender::Queen,
            orientation: Orientation::Straight,
            personality: balanced_personality(),
            appearance: kitten_appearance(),
            skills: Skills::default(),
            magic_affinity: 0.0,
            zodiac_sign: ZodiacSign::WarmDen,
            position,
            born_tick: current_tick,
            needs: Needs::default(),
            fulfillment: Fulfillment::default(),
            markers: MarkerSet(vec![MarkerKind::Kitten]),
        }
    }

    pub fn with_personality(mut self, f: impl FnOnce(&mut Personality)) -> Self {
        f(&mut self.personality);
        self
    }

    pub fn with_needs(mut self, f: impl FnOnce(&mut Needs)) -> Self {
        f(&mut self.needs);
        self
    }

    pub fn with_gender(mut self, gender: Gender) -> Self {
        self.gender = gender;
        self
    }

    pub fn with_marker(mut self, marker: MarkerKind) -> Self {
        self.markers.0.push(marker);
        self
    }

    pub fn with_born_tick(mut self, born_tick: u64) -> Self {
        self.born_tick = born_tick;
        self
    }

    pub fn with_magic_affinity(mut self, affinity: f32) -> Self {
        self.magic_affinity = affinity;
        self
    }

    pub(crate) fn into_blueprint(self) -> PresetParts {
        let blueprint = CatBlueprint {
            name: self.name,
            gender: self.gender,
            orientation: self.orientation,
            personality: self.personality,
            appearance: self.appearance,
            skills: self.skills,
            magic_affinity: self.magic_affinity,
            zodiac_sign: self.zodiac_sign,
            position: self.position,
            born_tick: self.born_tick,
        };
        PresetParts {
            position: self.position,
            needs: self.needs,
            fulfillment: self.fulfillment,
            blueprint,
            markers: self.markers,
        }
    }
}

/// All 18 axes at 0.5 — the bell-curve mean of `Personality::random`.
fn balanced_personality() -> Personality {
    Personality {
        boldness: 0.5,
        sociability: 0.5,
        curiosity: 0.5,
        diligence: 0.5,
        warmth: 0.5,
        spirituality: 0.5,
        ambition: 0.5,
        patience: 0.5,
        anxiety: 0.5,
        optimism: 0.5,
        temper: 0.5,
        stubbornness: 0.5,
        playfulness: 0.5,
        loyalty: 0.5,
        tradition: 0.5,
        compassion: 0.5,
        pride: 0.5,
        independence: 0.5,
    }
}

fn default_appearance() -> Appearance {
    Appearance {
        fur_color: "tabby brown".to_string(),
        pattern: "tabby".to_string(),
        eye_color: "amber".to_string(),
        distinguishing_marks: Vec::new(),
    }
}

fn kitten_appearance() -> Appearance {
    Appearance {
        fur_color: "tabby brown".to_string(),
        pattern: "tabby".to_string(),
        eye_color: "blue".to_string(),
        distinguishing_marks: Vec::new(),
    }
}
