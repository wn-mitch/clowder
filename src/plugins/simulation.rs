use bevy::prelude::*;

use crate::ai::eval::DseRegistry;
use crate::ai::methods::MethodRegistry;
use crate::ai::modifier::default_modifier_pipeline;
use crate::resources::recipe_registry::RecipeRegistry;
use crate::resources::sim_constants::ScoringConstants;
use crate::resources::SimConstants;
use crate::systems;
use crate::systems::influence_map::{
    CorruptionLens, InfluenceMap, InfluenceMapRegistry, PerSpeciesScentRef,
};

/// Populates a [`DseRegistry`] with the canonical 30 cat-DSE + 9
/// fox-DSE catalog plus all target-taking DSEs, using the supplied
/// [`ScoringConstants`].
///
/// Single source of truth for DSE catalog membership. Tests that
/// build a `DseRegistry` inline (`tests/integration.rs`) intentionally
/// do *not* call this function — they cherry-pick a subset.
pub fn populate_dse_registry(registry: &mut DseRegistry, scoring: &ScoringConstants) {
    use crate::ai::dses;

    // -----------------------------------------------------------------
    // Cat DSEs — auto-discovered via `linkme::distributed_slice`
    // (ticket 438). Each `src/ai/dses/*.rs` file declares its own
    // `CatDseRegistration` entry; `cat_dse_constructors` sorts by
    // declared `order` (load-bearing for seed-42 — sets the jitter
    // RNG sequence) and constructs them in dispatch order. Adding
    // a new cat DSE requires authoring one constructor + one
    // registration entry in its module; this central function does
    // not need to be touched. Eliminates the parallel hand-maintained
    // list that surfaced as the silent-failure class diagnosed by
    // tickets 436 / 437.
    // -----------------------------------------------------------------
    registry
        .cat_dses
        .extend(dses::cat_dse_constructors(scoring));

    // -----------------------------------------------------------------
    // Target-taking DSEs — separate dispatch path under §6.3. Order
    // here doesn't affect `score_actions` jitter sequence.
    // -----------------------------------------------------------------
    registry
        .target_taking_dses
        .push(dses::hunt_target_dse(scoring));
    registry.target_taking_dses.push(dses::fight_target_dse());
    registry
        .target_taking_dses
        .push(dses::socialize_target_dse());
    registry
        .target_taking_dses
        .push(dses::groom_other_target_dse());
    registry.target_taking_dses.push(dses::bury_target_dse());
    registry.target_taking_dses.push(dses::mentor_target_dse());
    registry
        .target_taking_dses
        .push(dses::caretake_target_dse());
    // 364: three sibling registrations for the rear_kitten HTN method's
    // primitive leaves. Maturity bands are mutually exclusive (Wean <
    // weaned ≤ Teach < teach_done ≤ Release), so at most one fires per
    // kitten per tick. Filter + scoring lives in dependent_kitten_target.rs.
    registry
        .target_taking_dses
        .push(dses::dependent_kitten_target::dependent_kitten_target_dse(
            crate::ai::Action::Wean,
        ));
    registry
        .target_taking_dses
        .push(dses::dependent_kitten_target::dependent_kitten_target_dse(
            crate::ai::Action::Teach,
        ));
    registry
        .target_taking_dses
        .push(dses::dependent_kitten_target::dependent_kitten_target_dse(
            crate::ai::Action::Release,
        ));
    registry.target_taking_dses.push(dses::mate_target_dse());
    registry.target_taking_dses.push(dses::build_target_dse());
    registry
        .target_taking_dses
        .push(dses::herbcraft_target_dse());
    registry
        .target_taking_dses
        .push(dses::apply_remedy_target_dse());
    registry.fox_dses.push(dses::fox_patrolling_dse(scoring));
    registry.fox_dses.push(dses::fox_hunting_dse(scoring));
    registry.fox_dses.push(dses::fox_raiding_dse());
    registry.fox_dses.push(dses::fox_fleeing_dse());
    registry.fox_dses.push(dses::fox_avoiding_dse());
    registry.fox_dses.push(dses::fox_den_defense_dse());
    registry.fox_dses.push(dses::fox_resting_dse(scoring));
    registry.fox_dses.push(dses::fox_feeding_dse());
    registry.fox_dses.push(dses::fox_dispersing_dse());
    // Ticket 025 Phase 2 — hawk + snake GOAP DSEs. Soaring is the
    // implicit hawk fallback (no DSE per Phase 1 design); the snake
    // fallback (Ambushing in scoring.rs) is registered as its own DSE.
    registry.hawk_dses.push(dses::hawk_hunting_dse());
    registry.hawk_dses.push(dses::hawk_fleeing_dse());
    registry.hawk_dses.push(dses::hawk_resting_dse());
    registry.snake_dses.push(dses::snake_ambushing_dse());
    registry.snake_dses.push(dses::snake_foraging_dse());
    registry.snake_dses.push(dses::snake_fleeing_dse());
    registry.snake_dses.push(dses::snake_basking_dse());
}

/// Startup system that populates [`DseRegistry`] and the §3.5
/// modifier pipeline from live [`SimConstants`]. Runs after
/// `setup_world_exclusive` so SimConstants is in place.
pub fn register_dses_at_startup(
    constants: Res<SimConstants>,
    mut registry: ResMut<DseRegistry>,
    mut commands: Commands,
) {
    let scoring = &constants.scoring;
    populate_dse_registry(&mut registry, scoring);
    // §075 — `default_modifier_pipeline` takes `&SimConstants` so the
    // `CommitmentTenure` modifier can reach `DispositionConstants`
    // (`oscillation_score_lift`).
    commands.insert_resource(default_modifier_pipeline(&constants));
}

/// Single source of truth for L1 trace coverage (ticket 207).
///
/// Every `impl InfluenceMap for X` in `src/` registers here; the
/// `emit_focal_trace` exclusive system walks `InfluenceMapRegistry`
/// blindly, so a missing entry silently drops a map from the focal
/// scrubber's L1 surface. `scripts/check_influence_map_registry.sh`
/// pairs this site against the trait-impl set at `just check` time
/// to catch the regression.
///
/// Resource-backed maps register via `register::<M>()`. Borrow-adapter
/// maps (`CorruptionLens` over `&TileMap`) register via
/// `register_with`; the closure constructs the adapter inline at
/// walk time so no wrapper Resource is needed. Per-species
/// adapters (ticket 062's `PerSpeciesScentRef`) follow the same
/// pattern — one `register_with` per species.
pub fn populate_influence_map_registry(registry: &mut InfluenceMapRegistry) {
    // Note (228): per-cat substrate (`RouteCostField`,
    // `escape_viability`, `fox_scent_level` at cat position) is
    // **not** registered here — this registry is world-keyed only.
    // Cat-keyed perception lives outside the registry; see
    // `src/components/route_cost_field.rs` for the cat-keyed family
    // and §4.7 of `docs/systems/ai-substrate-refactor.md` for the
    // substrate-vs-search-state boundary.
    use crate::resources::{
        BeautyMap, CarcassScentMap, CatPatrolDeterrentMap, CatScentMap, CleanlinessMap,
        ColonyDistrictMap, ComfortMap, ConstructionSiteMap, CorruptionInfluenceMap,
        CoverAvailabilityMap, ExplorationMap, FoodLocationMap, FoxApproachCorridorMap, FoxScentMap,
        GardenLocationMap, GraveAuraMap, HerbLocationMap, KittenCryMap, MysteryMap, PreyScentMaps,
        TileMap, TremorMap, WardCoverageMap, WardIntentMap, WardSiegeFearMap,
    };

    registry.register::<FoxScentMap>();
    // Ticket 062 — per-species prey scent. Five `PerSpeciesScentRef`
    // borrow-adapters over `PreyScentMaps`, one per `PreyKind`. The
    // aggregate `PreyScentMap` `Resource` is retired; `PreyScentMaps`
    // itself is **not** registered (no aggregate `InfluenceMap` impl).
    for kind in [
        crate::components::prey::PreyKind::Mouse,
        crate::components::prey::PreyKind::Rat,
        crate::components::prey::PreyKind::Rabbit,
        crate::components::prey::PreyKind::Fish,
        crate::components::prey::PreyKind::Bird,
    ] {
        registry.register_with(move |world, pos| {
            world.get_resource::<PreyScentMaps>().map(|maps| {
                let adapter = PerSpeciesScentRef(maps.for_kind(kind), kind);
                (adapter.metadata(), adapter.base_sample(pos))
            })
        });
    }
    registry.register::<CarcassScentMap>();
    // 100: aggregate substrate-vibration field. Faction::Neutral —
    // prey cannot discriminate cat vibration from fox vibration; the
    // map encodes "is anyone moving nearby?" not "who?". Written by
    // `tremor_tick`; read by `try_detect_cat` and the EngagePrey
    // approach phase's ambient opportunity-quality reads.
    registry.register::<TremorMap>();
    // 101: five-axis environmental quality maps. Tile-resolution
    // influence over comfort / cleanliness / beauty / mystery /
    // corruption-perception. Registered so the per-cell readings
    // surface in `trace-*.jsonl` for soak-trace verification.
    registry.register::<ComfortMap>();
    registry.register::<CleanlinessMap>();
    registry.register::<BeautyMap>();
    registry.register::<MysteryMap>();
    registry.register::<CorruptionInfluenceMap>();
    // Ticket 423: cover-availability map. Tile-resolution boolean
    // (within sprint_radius of any `Terrain::is_low_cover()` tile).
    // Consumer: `update_hide_eligible_markers`; trace emitter walks
    // it so soak-trace can verify the marker author reads the same
    // map the L1 trace reports.
    registry.register::<CoverAvailabilityMap>();
    // 312: fox-approach corridor traffic map. Dormant in scoring at
    // land (`ward_fox_approach_corridor_weight = 0.0`); registered so
    // its samples surface in `trace-*.jsonl` for soak-trace
    // verification at first-light activation.
    registry.register::<FoxApproachCorridorMap>();
    registry.register::<CatScentMap>();
    // 256 R5: cat patrol deterrent — read by fox A* as routing cost.
    registry.register::<CatPatrolDeterrentMap>();
    registry.register::<ExplorationMap>();
    registry.register::<WardCoverageMap>();
    // 470: per-tile siege-fear stamped each tick from
    // `WildlifeAiState::EncirclingWard`. Substrate ships ACTIVE (the
    // producer system always runs); consumer DSE weights stay dormant
    // at land (`ward_siege_fear_weight = 0.0`) per the 301 byte-
    // identical-at-land precedent. Registered here so the field
    // surfaces in `trace-*.jsonl` for soak-trace verification once
    // consumers activate.
    registry.register::<WardSiegeFearMap>();
    // 301: coordinator-stamped ward-placement intent. Substrate is
    // dormant at default `SimConstants` (semantics is
    // `SingleShotArgmax` so the populator short-circuits, and the
    // Path-B DSE weight is 0.0). Registered so the field surfaces in
    // `trace-*.jsonl` for soak-trace verification once activated.
    registry.register::<WardIntentMap>();
    registry.register::<FoodLocationMap>();
    registry.register::<GardenLocationMap>();
    registry.register::<ConstructionSiteMap>();
    registry.register::<KittenCryMap>();
    registry.register::<HerbLocationMap>();
    // 035: anti-corruption aura around buried graves.
    registry.register::<GraveAuraMap>();
    // 382: colony-district composite — frontier minus crowding minus
    // threat. Read by `compute_building_placement` to retire the
    // radius-16 spiral search; per-kind weighting happens at the
    // placement call-site via direct per-axis getters.
    registry.register::<ColonyDistrictMap>();

    // CorruptionLens is a borrow adapter over TileMap.corruption — not
    // a Resource itself, so it can't go through the generic
    // `register::<M>()`. The closure builds the lens inline.
    registry.register_with(|world, pos| {
        world.get_resource::<TileMap>().map(|t| {
            let lens = CorruptionLens(t);
            (lens.metadata(), lens.base_sample(pos))
        })
    });
}

/// Startup system that populates [`InfluenceMapRegistry`]. Independent
/// of `register_dses_at_startup` — registration only touches the
/// registry, not other Resources, so it can run any time after
/// `setup_world_exclusive` inserts the resources the walkers will
/// later look up.
pub fn register_influence_maps_at_startup(mut registry: ResMut<InfluenceMapRegistry>) {
    populate_influence_map_registry(&mut registry);
}

/// Single source of truth for HTN method catalog membership
/// (ticket 319 — child #1 of the 128 epic). Empty at landing of 319
/// by design — types and infrastructure first, methods authored in
/// tickets 320 onward. The `scripts/check_method_registry.sh` lint
/// passes vacuously on the empty registry; the first `PendingSubstrate`
/// method to land exercises the bidirectional check (method → open
/// ticket exists; ticket frontmatter `wires-method:` references the
/// method back).
///
/// Method-declaration convention (read before authoring methods): every
/// `Method` literal with `ApplicableWhen::PendingSubstrate` MUST sit in
/// a single multi-line struct-literal block under `src/ai/methods/`,
/// with `id: MethodId("<slug>")` and `blocker: "<ticket-id>"` each on
/// their own line. See `src/ai/methods/mod.rs` module doc for the full
/// contract.
/// Single source of truth for crafting recipe catalog
/// membership (ticket 365 — 016 Phase 1a).
///
/// Empty at landing of Commit 1 by design — types and registry
/// infrastructure first; recipe data is registered in Commits
/// 2 (remedy) and 3 (ward) of this ticket. Phase 1b (367) and
/// later phases add preservation, behavioral tools, wearables,
/// and decorations as additional entries.
///
/// Bespoke per-discipline resolvers (`resolve_prepare_remedy`,
/// `resolve_set_ward`, `resolve_cook`, …) look up recipe data
/// from this registry at runtime; HTN methods cite recipes by
/// `RecipeId` when emitting craft intentions. Cooking, ward
/// misfire rolls, herbcraft skill growth, and every other
/// runtime mechanic stay on their own resolvers per `crafting.md`'s
/// "crafting is an umbrella category" framing.
pub fn populate_recipe_registry(registry: &mut RecipeRegistry) {
    use crate::components::magic::{RemedyKind, WardKind};
    use crate::components::recipe::{
        DisciplineKind, ItemDestination, Recipe, RecipeDuration, RecipeId, RecipeInput,
        RecipeOutput, StationRequirement,
    };
    use crate::resources::sim_constants::MagicConstants;

    // 365 Commit 2 — herbcraft remedies. One Recipe per
    // RemedyKind variant. Inputs derive from
    // `RemedyKind::required_herb()`; the duration carries both
    // the default and at-workshop tick budgets so the planner /
    // future tooling can answer "how long does this take at /
    // away from the workshop?" without re-deriving from
    // MagicConstants. Output destination is Inventory — prepared
    // remedies are real ItemKind::Remedy* slots consumed by
    // resolve_apply_remedy. Discipline = Herbalism (the
    // ticket-155 split's home for remedy work).
    let m = MagicConstants::default();
    // Nominal ticks at canonical SimConfig — registry duration is
    // metadata for tooling / future planner introspection. Runtime
    // resolvers continue to read MagicConstants directly with the
    // live TimeScale, so a per-run variance in tick rate doesn't
    // require regenerating recipes.
    let time_scale = crate::resources::time::TimeScale::from_config(
        &crate::resources::time::SimConfig::default(),
        16.6667,
    );
    let default_ticks = m.prepare_remedy_duration_default.ticks(&time_scale);
    let workshop_ticks = m.prepare_remedy_duration_workshop.ticks(&time_scale);
    for remedy in [
        RemedyKind::HealingPoultice,
        RemedyKind::EnergyTonic,
        RemedyKind::MoodTonic,
    ] {
        registry.insert(Recipe {
            id: remedy.recipe_id(),
            discipline: DisciplineKind::Herbalism,
            inputs: vec![RecipeInput {
                kind: remedy.required_herb().to_item_kind(),
                count: 1,
            }],
            station: StationRequirement::Workshop,
            duration: RecipeDuration::AtStationFaster {
                default_ticks,
                at_station_ticks: workshop_ticks,
            },
            output: RecipeOutput {
                item_kind: remedy.to_item_kind(),
                destination: ItemDestination::Inventory,
            },
            skill_gate: None,
            // 463 — remedies aren't warrior's-kit; they ride the
            // existing Herbalism DSE chain (not Crafting). Affinity
            // stays Herbcraft so a future HaveItem(remedy) aspiration
            // would score against the herb-axis.
            is_warriors_kit: false,
            discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::Herbcraft),
        });
    }

    // 365 Commit 3 — wards. Thornward consumes one Thornbriar
    // herb at the actor's chosen perimeter position; DurableWard
    // is the magic-specialist variant (no herb input — magic
    // skill carries the cost). Both spawn world-position entities
    // tagged with CraftedItem. Output's item_kind reuses the
    // input herb for Thornward (the input determines the spell
    // type — there's no separate "Thornward" ItemKind because
    // the produced entity is a `Ward` Component, not an `Item`);
    // for DurableWard with no herb input we use a placeholder
    // ItemKind that the output-destination machinery doesn't
    // currently consult. Future tooling that queries recipe
    // outputs for "what entity to spawn" will key on the
    // destination + RecipeId, not the placeholder ItemKind.
    let set_ward_ticks = m.set_ward_duration.ticks(&time_scale);
    registry.insert(Recipe {
        id: crate::steps::magic::ward_recipe_id(WardKind::Thornward),
        discipline: DisciplineKind::Herbalism,
        inputs: vec![RecipeInput {
            kind: crate::components::items::ItemKind::HerbThornbriar,
            count: 1,
        }],
        station: StationRequirement::None,
        duration: RecipeDuration::Fixed {
            ticks: set_ward_ticks,
        },
        output: RecipeOutput {
            // Placeholder — Ward entities don't carry ItemKind today
            // (they're their own Component family). Reuses the input
            // herb as a stand-in for "this recipe consumes Thornbriar
            // and produces a thornward".
            item_kind: crate::components::items::ItemKind::HerbThornbriar,
            destination: ItemDestination::WorldPosition,
        },
        skill_gate: None,
        // 463 — Thornward isn't a warrior's-kit item (it's a placed
        // structure, not a worn/carried weapon). Affinity is Herbcraft.
        is_warriors_kit: false,
        discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::Herbcraft),
    });
    registry.insert(Recipe {
        id: crate::steps::magic::ward_recipe_id(WardKind::DurableWard),
        discipline: DisciplineKind::Witchcraft,
        inputs: vec![],
        station: StationRequirement::None,
        duration: RecipeDuration::Fixed {
            ticks: set_ward_ticks,
        },
        output: RecipeOutput {
            // Placeholder per the Thornward note above.
            item_kind: crate::components::items::ItemKind::HerbThornbriar,
            destination: ItemDestination::WorldPosition,
        },
        skill_gate: None,
        // 463 — DurableWard is the Witchcraft variant; magic axis.
        is_warriors_kit: false,
        discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::Magic),
    });

    // 367 Commit 5 — preservation recipes (Phase 1b).
    //
    // Six entries cover the Phase 1b pipeline: two drying recipes
    // (fish + organ) and four parallel smoking recipes (one per raw
    // meat kind). Smoked recipes are parallel rather than an
    // `AnyOf`-style consolidation because `RecipeInput` doesn't carry
    // a multi-kind variant yet (flagged at `recipe.rs:31-34` as a
    // follow-on need). The four-Recipe shape mirrors the existing
    // one-Recipe-per-Ward/Remedy precedent.
    //
    // Registry duration is **metadata** — runtime advancement happens
    // in `systems::preservation::advance_preservation_drying` (for
    // drying) and in tend-cycle completion (for smoking). The
    // resolvers read `CraftingConstants` directly, so this duration
    // field exists for future tooling ("how long does this take?")
    // not as a load-bearing budget. `Fixed` ticks are nominal at
    // canonical `SimConfig`.
    let crafting = crate::resources::sim_constants::CraftingConstants::default();
    let drying_fish_ticks = crafting.drying_dried_fish_total_ticks;
    let drying_organ_ticks = crafting.drying_preserved_organ_total_ticks;
    // Nominal smoking duration: tends_needed (3) * tend_cooldown (~416)
    // + tend-action ticks. Empirical bound at default constants is
    // ~1500-2500 ticks wall-clock; ~5000 ticks ≈ one sim-day is the
    // crafting.md target. Pick the design target as metadata so future
    // tooling sees the intended wall-clock budget.
    let smoking_ticks: u64 = 5_000;

    registry.insert(Recipe {
        id: RecipeId("preserve.dried_fish"),
        discipline: DisciplineKind::Preservation,
        inputs: vec![RecipeInput {
            kind: crate::components::items::ItemKind::RawFish,
            count: 1,
        }],
        station: StationRequirement::DryingRack,
        duration: RecipeDuration::Fixed {
            ticks: drying_fish_ticks,
        },
        output: RecipeOutput {
            item_kind: crate::components::items::ItemKind::DriedFish,
            // Spawns at the rack's position. `WorldPosition` is the
            // canonical destination for "spawn an `Item` entity on the
            // ground at a recipe-chosen tile" — the destination
            // machinery doesn't distinguish "at the crafter" from "at
            // the station" today.
            destination: ItemDestination::WorldPosition,
        },
        skill_gate: None,
        // 463 — preservation rides the Herbcraft axis (the same
        // discipline that handles herb preparation).
        is_warriors_kit: false,
        discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::Herbcraft),
    });

    registry.insert(Recipe {
        id: RecipeId("preserve.preserved_organ"),
        discipline: DisciplineKind::Preservation,
        inputs: vec![
            RecipeInput {
                kind: crate::components::items::ItemKind::RawOrgan,
                count: 1,
            },
            // Canonical herb input — `HerbHealingMoss` per the plan.
            // The load resolver consumes "any herb" (see `take_any_herb`
            // in `load_drying_rack.rs`) because `RecipeInput::AnyOf`
            // doesn't exist yet; this entry documents the canonical
            // intent until `AnyOf` lands.
            RecipeInput {
                kind: crate::components::items::ItemKind::HerbHealingMoss,
                count: 1,
            },
        ],
        station: StationRequirement::DryingRack,
        duration: RecipeDuration::Fixed {
            ticks: drying_organ_ticks,
        },
        output: RecipeOutput {
            item_kind: crate::components::items::ItemKind::PreservedOrgan,
            destination: ItemDestination::WorldPosition,
        },
        skill_gate: None,
        is_warriors_kit: false,
        discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::Herbcraft),
    });

    // Four parallel smoking recipes. All share `ItemKind::SmokedMeat`
    // as output; the source meat's identity rides through
    // `SmokingLoad::source_kind` for `CraftedItem` provenance. When
    // `RecipeInput::AnyOf` lands these collapse to one Recipe.
    for raw_meat_kind in [
        crate::components::items::ItemKind::RawMouse,
        crate::components::items::ItemKind::RawRat,
        crate::components::items::ItemKind::RawRabbit,
        crate::components::items::ItemKind::RawBird,
    ] {
        let recipe_slug: &'static str = match raw_meat_kind {
            crate::components::items::ItemKind::RawMouse => "preserve.smoked.mouse",
            crate::components::items::ItemKind::RawRat => "preserve.smoked.rat",
            crate::components::items::ItemKind::RawRabbit => "preserve.smoked.rabbit",
            crate::components::items::ItemKind::RawBird => "preserve.smoked.bird",
            _ => unreachable!("smoked-meat recipe loop is enumerated above"),
        };
        registry.insert(Recipe {
            id: RecipeId(recipe_slug),
            discipline: DisciplineKind::Preservation,
            inputs: vec![
                RecipeInput {
                    kind: raw_meat_kind,
                    count: 1,
                },
                RecipeInput {
                    kind: crate::components::items::ItemKind::Wood,
                    count: 1,
                },
            ],
            station: StationRequirement::SmokingRack,
            duration: RecipeDuration::Fixed {
                ticks: smoking_ticks,
            },
            output: RecipeOutput {
                item_kind: crate::components::items::ItemKind::SmokedMeat,
                destination: ItemDestination::WorldPosition,
            },
            skill_gate: None,
            is_warriors_kit: false,
            discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::Herbcraft),
        });
    }

    // 368 Phase 2 — behavioral-tool recipes (016 Phase 2).
    //
    // Six entries cover the Phase 2 Workshop pipeline: one polish
    // sub-recipe (Stone → PolishedStone), one Grooming Brush, one
    // Play Bundle, and three parallel Courtship Gift recipes
    // (PolishedStone / Feather / Flower → CourtshipGift). The three
    // gift recipes mirror the four smoking recipes' shape:
    // `RecipeInput::AnyOf` doesn't exist yet, so one Recipe per
    // input. When `AnyOf` lands the three gifts collapse to one.
    //
    // **Substrate completeness note**: these recipes register in the
    // registry but the Workshop-craft executor (DSE + plan template +
    // resolver) is not wired in 368. Cats won't autonomously craft
    // these tools in seed-42 until the follow-on ticket lands the
    // Workshop crafting pipeline. The three resolver branches in
    // groom_other / socialize / mate_with (commit 5) DO read the
    // tools' identity when the items are present in inventory —
    // honest at the action-execution layer, decoration at the
    // crafting-elect layer until the follow-on.
    registry.insert(Recipe {
        id: RecipeId("polish.polished_stone"),
        discipline: DisciplineKind::StonecraftCairn,
        inputs: vec![RecipeInput {
            kind: crate::components::items::ItemKind::Stone,
            count: 1,
        }],
        station: StationRequirement::Workshop,
        duration: RecipeDuration::Fixed {
            ticks: crafting.polish_polished_stone_ticks,
        },
        output: RecipeOutput {
            item_kind: crate::components::items::ItemKind::PolishedStone,
            destination: ItemDestination::Inventory,
        },
        skill_gate: None,
        // 463 — polished-stone polishing is a Cairn discipline
        // sub-recipe (StonecraftCairn → SkillKind::Cairn).
        is_warriors_kit: false,
        discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::Cairn),
    });

    registry.insert(Recipe {
        id: RecipeId("behavioral.grooming_brush"),
        discipline: DisciplineKind::BoneShellCraft,
        inputs: vec![
            RecipeInput {
                kind: crate::components::items::ItemKind::Twig,
                count: 1,
            },
            RecipeInput {
                kind: crate::components::items::ItemKind::Bristle,
                count: 1,
            },
        ],
        station: StationRequirement::Workshop,
        duration: RecipeDuration::Fixed {
            ticks: crafting.craft_grooming_brush_ticks,
        },
        output: RecipeOutput {
            item_kind: crate::components::items::ItemKind::GroomingBrush,
            destination: ItemDestination::Inventory,
        },
        skill_gate: None,
        // 463 — behavioral tool; BoneShellCraft discipline.
        is_warriors_kit: false,
        discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::BoneShaping),
    });

    registry.insert(Recipe {
        id: RecipeId("behavioral.play_bundle"),
        discipline: DisciplineKind::FiberWeaving,
        inputs: vec![
            RecipeInput {
                kind: crate::components::items::ItemKind::Fiber,
                count: 1,
            },
            RecipeInput {
                kind: crate::components::items::ItemKind::Feather,
                count: 1,
            },
        ],
        station: StationRequirement::Workshop,
        duration: RecipeDuration::Fixed {
            ticks: crafting.craft_play_bundle_ticks,
        },
        output: RecipeOutput {
            item_kind: crate::components::items::ItemKind::PlayBundle,
            destination: ItemDestination::Inventory,
        },
        skill_gate: None,
        // 463 — behavioral tool; FiberWeaving discipline.
        is_warriors_kit: false,
        discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::Weaving),
    });

    // Three parallel Courtship Gift recipes (one per acceptable input
    // material). Output is always `ItemKind::CourtshipGift`; the
    // narrative tier differentiates by `CraftedItem.recipe`.
    for (gift_slug, input_kind) in [
        (
            "behavioral.courtship_gift.polished_stone",
            crate::components::items::ItemKind::PolishedStone,
        ),
        (
            "behavioral.courtship_gift.feather",
            crate::components::items::ItemKind::Feather,
        ),
        (
            "behavioral.courtship_gift.flower",
            crate::components::items::ItemKind::Flower,
        ),
    ] {
        registry.insert(Recipe {
            id: RecipeId(gift_slug),
            discipline: DisciplineKind::AdornmentSetting,
            inputs: vec![RecipeInput {
                kind: input_kind,
                count: 1,
            }],
            station: StationRequirement::Workshop,
            duration: RecipeDuration::Fixed {
                ticks: crafting.craft_courtship_gift_ticks,
            },
            output: RecipeOutput {
                item_kind: crate::components::items::ItemKind::CourtshipGift,
                destination: ItemDestination::Inventory,
            },
            skill_gate: None,
            // 463 — Courtship gift; AdornmentSetting → Pigment.
            is_warriors_kit: false,
            discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::Pigment),
        });
    }

    // 369 Phase 2b — warrior's-kit recipes.
    //
    // Eight items split across four disciplines + three station
    // requirements (Workshop / TanningFrame / None). Inputs draw from
    // existing prey-byproducts (Bone, Sinew, Whisker, Hide) and
    // foraged stocks (Twig, Fiber, Stone) — no new input ItemKinds.
    // Recipe-id prefix `warriors_kit.` keeps the lex-order block
    // cohesive in the registry sort that drives recipe selection.
    use crate::components::items::ItemKind;

    registry.insert(Recipe {
        id: RecipeId("warriors_kit.bone_tip_spear"),
        discipline: DisciplineKind::BoneShellCraft,
        inputs: vec![
            RecipeInput {
                kind: ItemKind::Bone,
                count: 1,
            },
            RecipeInput {
                kind: ItemKind::Twig,
                count: 1,
            },
            RecipeInput {
                kind: ItemKind::Sinew,
                count: 1,
            },
        ],
        station: StationRequirement::Workshop,
        duration: RecipeDuration::Fixed {
            ticks: crafting.craft_bone_tip_spear_ticks,
        },
        output: RecipeOutput {
            item_kind: ItemKind::BoneTipSpear,
            // 017 — worn gear auto-equips on craft into its anatomical slot.
            destination: ItemDestination::EquippedSlot,
        },
        skill_gate: None,
        is_warriors_kit: true,
        discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::BoneShaping),
    });

    registry.insert(Recipe {
        id: RecipeId("warriors_kit.bone_stiletto"),
        discipline: DisciplineKind::BoneShellCraft,
        inputs: vec![RecipeInput {
            kind: ItemKind::Bone,
            count: 1,
        }],
        station: StationRequirement::Workshop,
        duration: RecipeDuration::Fixed {
            ticks: crafting.craft_bone_stiletto_ticks,
        },
        output: RecipeOutput {
            item_kind: ItemKind::BoneStiletto,
            // 017 — worn gear auto-equips on craft into its anatomical slot.
            destination: ItemDestination::EquippedSlot,
        },
        skill_gate: None,
        is_warriors_kit: true,
        discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::BoneShaping),
    });

    // Flint Blade — workshop-bench knapping for 369. crafting.md's
    // Phase 2b table calls for `open ground; no station`, but the
    // substrate to resolve a `StationRequirement::None` recipe (a
    // `CraftInPlaceDse` with no station-proximity gate) doesn't
    // exist yet — it would be a third sibling alongside the Workshop
    // / TanningFrame pair, and the design compromise (knap at the
    // workshop bench) is recoverable via a follow-on ticket. Per
    // 016's "follow-on tickets are non-optional" discipline, the
    // CraftInPlace substrate gets opened at landing.
    registry.insert(Recipe {
        id: RecipeId("warriors_kit.flint_blade"),
        discipline: DisciplineKind::Stonecraft,
        inputs: vec![RecipeInput {
            kind: ItemKind::Stone,
            count: 1,
        }],
        station: StationRequirement::Workshop,
        duration: RecipeDuration::Fixed {
            ticks: crafting.craft_flint_blade_ticks,
        },
        output: RecipeOutput {
            item_kind: ItemKind::FlintBlade,
            // 017 — worn gear auto-equips on craft into its anatomical slot.
            destination: ItemDestination::EquippedSlot,
        },
        skill_gate: None,
        // 463 — knapping rides BoneShaping today (Stonecraft
        // discipline; same affinity as the bone-shaping siblings).
        is_warriors_kit: true,
        discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::BoneShaping),
    });

    registry.insert(Recipe {
        id: RecipeId("warriors_kit.hide_bracers"),
        discipline: DisciplineKind::HidePeltWork,
        inputs: vec![
            RecipeInput {
                kind: ItemKind::Hide,
                count: 2,
            },
            RecipeInput {
                kind: ItemKind::Sinew,
                count: 1,
            },
        ],
        station: StationRequirement::TanningFrame,
        duration: RecipeDuration::Fixed {
            ticks: crafting.craft_hide_bracers_ticks,
        },
        output: RecipeOutput {
            item_kind: ItemKind::HideBracers,
            // 017 — worn gear auto-equips on craft into its anatomical slot.
            destination: ItemDestination::EquippedSlot,
        },
        skill_gate: None,
        is_warriors_kit: true,
        discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::Hidework),
    });

    registry.insert(Recipe {
        id: RecipeId("warriors_kit.hide_plated_wrap"),
        discipline: DisciplineKind::HidePeltWork,
        inputs: vec![
            RecipeInput {
                kind: ItemKind::Hide,
                count: 4,
            },
            RecipeInput {
                kind: ItemKind::Sinew,
                count: 2,
            },
        ],
        station: StationRequirement::TanningFrame,
        duration: RecipeDuration::Fixed {
            ticks: crafting.craft_hide_plated_wrap_ticks,
        },
        output: RecipeOutput {
            item_kind: ItemKind::HidePlatedWrap,
            // 017 — worn gear auto-equips on craft into its anatomical slot.
            destination: ItemDestination::EquippedSlot,
        },
        skill_gate: None,
        is_warriors_kit: true,
        discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::Hidework),
    });

    registry.insert(Recipe {
        id: RecipeId("warriors_kit.sling"),
        discipline: DisciplineKind::FiberWeaving,
        inputs: vec![
            RecipeInput {
                kind: ItemKind::Fiber,
                count: 2,
            },
            RecipeInput {
                kind: ItemKind::Hide,
                count: 1,
            },
        ],
        station: StationRequirement::Workshop,
        duration: RecipeDuration::Fixed {
            ticks: crafting.craft_sling_ticks,
        },
        output: RecipeOutput {
            item_kind: ItemKind::Sling,
            // 017 — worn gear auto-equips on craft into its anatomical slot.
            destination: ItemDestination::EquippedSlot,
        },
        skill_gate: None,
        is_warriors_kit: true,
        discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::Weaving),
    });

    registry.insert(Recipe {
        id: RecipeId("warriors_kit.woven_reed_cloak"),
        discipline: DisciplineKind::FiberWeaving,
        inputs: vec![RecipeInput {
            kind: ItemKind::Fiber,
            count: 4,
        }],
        station: StationRequirement::Workshop,
        duration: RecipeDuration::Fixed {
            ticks: crafting.craft_woven_reed_cloak_ticks,
        },
        output: RecipeOutput {
            item_kind: ItemKind::WovenReedCloak,
            // 017 — worn gear auto-equips on craft into its anatomical slot.
            destination: ItemDestination::EquippedSlot,
        },
        skill_gate: None,
        is_warriors_kit: true,
        discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::Weaving),
    });

    registry.insert(Recipe {
        id: RecipeId("warriors_kit.tooth_notched_club"),
        discipline: DisciplineKind::BoneShellCraft,
        inputs: vec![
            RecipeInput {
                kind: ItemKind::Twig,
                count: 1,
            },
            RecipeInput {
                kind: ItemKind::Whisker,
                count: 1,
            },
        ],
        station: StationRequirement::Workshop,
        duration: RecipeDuration::Fixed {
            ticks: crafting.craft_tooth_notched_club_ticks,
        },
        output: RecipeOutput {
            item_kind: ItemKind::ToothNotchedClub,
            // 017 — worn gear auto-equips on craft into its anatomical slot.
            destination: ItemDestination::EquippedSlot,
        },
        skill_gate: None,
        is_warriors_kit: true,
        discipline_skill_affinity: Some(crate::ai::aspirations::SkillKind::BoneShaping),
    });
}

/// Startup system that populates [`RecipeRegistry`]. Independent
/// of the other registries — registration only touches the
/// resource, no ordering constraint against `setup_world_exclusive`.
pub fn register_recipes_at_startup(mut registry: ResMut<RecipeRegistry>) {
    populate_recipe_registry(&mut registry);
}

pub fn populate_method_registry(registry: &mut MethodRegistry) {
    // 322: Tier-2 dormant methods. The remaining dormant entries
    // (`acquire_stealth_via_*`) carry `ApplicableWhen::PendingSubstrate
    // { blocker: "334" }` — wired when #334 (stealth-cloak crafting)
    // lands. `MethodRegistry::lookup` filters them out unconditionally
    // so they exist for the type-system + dormancy audit but never
    // run at runtime.
    //
    // 332/333: `mourn_at_grave` and `rear_kitten` flipped to
    // `ApplicableWhen::Live` here. Their `applicable_when` predicates
    // gate on the `Mourning` Component (332) and the
    // `KittenDependency.mother` reverse-lookup (333) so the methods
    // are selectable only for cats actually carrying the substrate.
    // **Dispatch wiring is pending** for both — the cat's
    // `chosen_action` is still picked by the per-tick DSE softmax,
    // not by HTN method primitive sub-goals. The dispatch follow-on
    // (named in #332's and #333's landing Logs) wires DSE /
    // GoapActionKind / plan template / resolver call site so the
    // cat's behavior advances each method's sub-goals.
    use crate::ai::methods::acquire_stealth::{
        acquire_stealth_via_commission, acquire_stealth_via_self_craft,
    };
    use crate::ai::methods::mourn_at_grave::mourn_at_grave;
    use crate::ai::methods::rear_kitten::rear_kitten;

    registry.push(mourn_at_grave());
    registry.push(rear_kitten());
    registry.push(acquire_stealth_via_self_craft());
    registry.push(acquire_stealth_via_commission());

    // 321: Tier-1 Live method — combine-and-test slice. Catches the
    // `hunt_prey` label that Hunting "First Blood"'s sole Emit row
    // produces. With `applicable_when: Live(always_true)` and one
    // primitive sub-goal, every Hunting cat at milestone 0 hits the
    // L2 wrap site this tick → picker emits → 320 frame-push gate
    // fires. The production gating + multi-step decomposition lands
    // with #325.
    registry.push(crate::ai::methods::hunt::hunt_method());

    // 327: Tier-1 Live methods — combine-and-test slice for the Combat
    // chain. `fight_method` catches `engage_threat` (Primary emit on
    // every WARRIORS_PATH milestone); `flee_method` catches
    // `flee_to_safety` (Tertiary survival fallback). Both carry
    // `applicable_when: Live(always_true)` and one primitive sub-goal
    // each (Action::Fight + TargetHint::Threat; Action::Flee +
    // TargetHint::SafeGround). Production gating (threat-in-range belief
    // check, wounded-cat predicate) lands as a follow-on balance pass.
    // SHADOW_FIGHTER (Patrol-based) emits are deferred to a follow-on
    // ticket alongside its Patrol primitive method.
    registry.push(crate::ai::methods::fight::fight_method());
    registry.push(crate::ai::methods::flee::flee_method());

    // 326: Tier-1 Live methods — combine-and-test slice for the Social
    // chain. `socialize_method` catches `socialize` (Primary emit on
    // every HEART_OF_THE_COLONY milestone); `groom_other_method` catches
    // `groom_other` (Primary emit on every THE_BELOVED milestone). Both
    // carry `applicable_when: Live(always_true)` and one primitive sub-
    // goal each (Action::Socialize + TargetHint::SocialPartner;
    // Action::GroomOther + TargetHint::GroomingTarget). Mentor wrappers
    // and bespoke applicability predicates (`has_eligible_apprentice`,
    // `has_friend_or_better_bond`, …) are deferred to a follow-on
    // balance pass per sibling 330 / 331 discipline.
    registry.push(crate::ai::methods::socialize::socialize_method());
    registry.push(crate::ai::methods::groom_other::groom_other_method());

    // 398 Phase 1a: Live single-primitive `caretake_kitten` method.
    // Catches the `caretake_kitten` label that
    // `RAISE_OFFSPRING_ASPIRATION`'s dormant emit row will fire once
    // Phase 1c/1d (unified softmax + per-tier persistence-bonus) ship.
    // At Phase 1a the chain's emit row guards with `always_false`, so
    // this method has no live emission path yet — registration here
    // exists so `MethodRegistry::lookup` resolves the label cleanly
    // when the row activates. See `src/ai/methods/caretake_kitten.rs`
    // module doc for the full architecture.
    registry.push(crate::ai::methods::caretake_kitten::caretake_kitten());

    // 323: Tier-1 Live HTN method — first method that mirrors a 127
    // `JointIntention` practice end-to-end. `courtship_method` catches
    // the `courtship_completed` label on any cat carrying
    // `JointIntention { practice: Courtship, .. }` and decomposes the
    // four `PracticeStage` variants (Approach → Courting → Mating →
    // Bonded) into four sub-goals. 127's `author_joint_intentions`
    // (wired above in the FixedUpdate schedule) is the source of truth
    // for stage transitions; this method's `sub_goal_index` advances
    // in step via the L2 evaluator's primitive-leaf completion contract
    // (320). #340 (below) upgrades the third sub-goal
    // (`mate_with_partner`) from `Primitive Action::Mate` to
    // `SubGoal::Goal(GoalState { label: "mating_event_completed" })`,
    // recursing into `mate_with_goal`.
    registry.push(crate::ai::methods::courtship::courtship_method());

    // 340: Tier-1 Live HTN method — worked-example landing of the 128
    // epic. `mate_with_goal` ports the legacy `build_mating_chain`
    // template (a hand-coded `[MoveTo, Socialize, GroomOther, MateWith]`
    // that lived in the unscheduled `disposition_to_chain` — dead code
    // at runtime; the live mating path runs through the GOAP planner's
    // `mating_actions`) onto the registry as three primitive sub-goals
    // (`socialize_with_partner` → `groom_partner` → `complete_mating`;
    // the `Action::Navigate` step from the htn-methods.md worked
    // example is implicit via `htn_primitive_actions`'s travel-action
    // injection). `courtship_method`'s third sub-goal recurses into
    // this method, so the `HeldGoalStack` on a courting cat in the
    // Mating stage carries a two-deep frame:
    // `courtship_method` (sub_goal_index=2) → `mate_with_goal`. That
    // recursion is the worked example screenshot for the 128 epic.
    registry.push(crate::ai::methods::mating::mate_with_goal());

    // 276: Tier-1 Live HTN method — second JointIntention practice
    // after `courtship_method`. `play_bout_method` catches the
    // `play_bout_completed` label on any cat carrying `JointIntention {
    // practice: PlayBout, .. }`. Three sub-goals
    // (`approach_play_partner` → `play_with_partner` →
    // `cool_down_after_play`), all dispatching to `Action::Socialize`.
    // Hosts the `play` continuity canary on JointIntention substrate
    // via `EventKind::JointPlayBoutCompleted` (emitted from
    // `author_joint_intentions` on `JointDropBranch::Completed`).
    registry.push(crate::ai::methods::play_bout::play_bout_method());

    // 472: Dormant HTN method — `seek_healing` decomposes the
    // `festering_wound_healed` compound goal into rest + accept-tending
    // sub-goals. `ApplicableWhen::PendingSubstrate { blocker: "473" }`
    // keeps it dormant; 473 wires the `TendFestering` cat-side DSE +
    // corrupted-kin perception map and flips the method to Live.
    registry.push(crate::ai::methods::seek_healing::seek_healing());
}

/// Startup system that populates [`MethodRegistry`]. Independent of
/// `register_dses_at_startup` and `register_influence_maps_at_startup`
/// — registration only touches the registry, so no ordering constraint
/// against `setup_world_exclusive`.
pub fn register_methods_at_startup(mut registry: ResMut<MethodRegistry>) {
    populate_method_registry(&mut registry);
}

/// Registers all simulation systems on `FixedUpdate` in the same order as the
/// original `build_schedule()`.
///
/// Four chained groups run sequentially:
///   1. World simulation (weather, corruption, wildlife, buildings, items)
///   2. Cat needs, mood, and decision-making
///   3. Action resolution
///   4. Social, combat, death, cleanup, narrative
///
/// Standalone systems (AI evaluation, fate, aspirations) run after the chains
/// but are unordered relative to each other.
pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        // Determinism: pin the simulation schedules to a single-threaded
        // executor. The standalone systems group below is unordered relative
        // to itself, and Bevy's MultiThreadedExecutor picks a topological
        // order that varies across processes when the conflict graph admits
        // alternatives — that shifts the SimRng-consumption sequence and
        // breaks same-seed replay (verified: two seed-42 runs of the same
        // binary diverged at the first SystemActivation tick). Single-
        // threaded execution forces a stable order; the throughput cost is
        // negligible for a ~50-cat headless sim. Pinning Startup as well
        // covers worldgen, even though its current systems are explicitly
        // chained.
        use bevy::ecs::schedule::ExecutorKind;
        app.edit_schedule(Startup, |s| {
            s.set_executor_kind(ExecutorKind::SingleThreaded);
        });
        app.edit_schedule(FixedUpdate, |s| {
            s.set_executor_kind(ExecutorKind::SingleThreaded);
        });

        // World construction — terrain, cats, all sim resources. Owned
        // by the plugin so any host (windowed App, headless App in
        // ticket 030) gets the simulation populated by adding the
        // single plugin. The system reads `AppArgs` (seed, load_path,
        // …) which the host inserts before `add_plugins`.
        app.add_systems(Startup, crate::plugins::setup::setup_world_exclusive);

        // Register personality event observers (cascade handlers).
        systems::personality_events::register_observers(app);

        // Ticket 138 — author MovementBudget on freshly-spawned
        // wildlife (cats are authored in `cat_bundle`). See
        // `crate::systems::movement_budget` for the per-tick
        // accumulator + the lazy-insert fallback for save-loaded
        // entities.
        app.add_observer(systems::movement_budget::on_wild_animal_added);
        // 140 step 10 — prey sibling: Velocity/DesiredVelocity +
        // species speed cap + Flying for burst-flight birds.
        app.add_observer(systems::movement_budget::on_prey_animal_added);

        // Register messages.
        app.add_message::<crate::components::prey::PreyKilled>();
        app.add_message::<crate::components::prey::DenRaided>();
        app.add_message::<crate::components::goap_plan::PlanNarrative>();
        app.add_message::<crate::systems::magic::CorruptionPushback>();
        // Ticket 127 Commit B — bias-reader call sites emit this when
        // their resolver target matches the actor's
        // `JointIntention { Courtship }.partner`. Consumed by
        // `author_joint_intentions` to bump `last_interaction_tick`.
        app.add_message::<crate::ai::joint_intention::JointInteractionObserved>();
        // Ticket 276 Commit B — PlayBout `PlayBoutApproach → PlayBoutBouting`
        // stage transition. `author_joint_intentions` emits this from
        // the lower-`Entity::index()` side of the pair; consumed by
        // `cascade_play_bout_bouting` to apply the Bouting-stage
        // mood-lift cascade + narrative entry (the substrate replacement
        // for the legacy `on_play_initiated` observer).
        app.add_message::<crate::ai::joint_intention::PlayBoutBoutingEntered>();
        // 258 — observable side-effects consumed by `belief_integrator` to
        // update per-cat mental models. Action resolvers emit variants at
        // completion; the integrator finds witnesses by sensing-range query.
        app.add_message::<crate::messages::witnessable_event::WitnessableEvent>();
        // 050 — fox-lifecycle mechanical events (no observer-side
        // semantics). Consumed by `fox_spatial`'s §4 marker authors so
        // `HasDen` / `HasCubs` author from event signals instead of
        // (purely) per-tick scans. Emitted at fox-spawn / den-claim /
        // cub-birth / den-loss sites in `wildlife.rs`.
        app.add_message::<crate::messages::fox_lifecycle::DenClaimed>();
        app.add_message::<crate::messages::fox_lifecycle::DenLost>();
        app.add_message::<crate::messages::fox_lifecycle::CubsBorn>();
        // 025 Phase 2 — hawk/snake GOAP messages. `*DiveLanded` /
        // `*StrikeLanded` are emitted by the dive/strike step resolvers
        // (the *attempt* event); kill-attribution stays in
        // `predator_hunt_prey` per ticket §12. Lifecycle messages are
        // emitted by `hawk_lifecycle_tick` / `snake_lifecycle_tick`.
        app.add_message::<crate::components::wildlife::HawkDiveLanded>();
        app.add_message::<crate::components::wildlife::SnakeStrikeLanded>();
        app.add_message::<crate::components::wildlife::HawkDied>();
        app.add_message::<crate::components::wildlife::SnakeDied>();
        // 095 Phase 1 — anatomical injury substrate. Emitted by
        // `damage_to_body_part` (combat.rs) alongside the legacy `Injury`
        // push during Stage A; becomes the sole signal at Stage B.
        app.add_message::<crate::messages::body_part_injury::BodyPartInjury>();
        // 471 — per-event magic-misfire stream. Emitted by `apply_misfire`
        // (magic.rs) for every resolved misfire outcome; consumed by the
        // event-log replay path and by the festering-wound authoring path
        // (ticket 472, on the WoundTransfer arm).
        app.add_message::<crate::messages::misfire_effect::MisfireEffect>();
        // 431 Stage A — cat-movement substrate. Emitted by
        // `emit_cat_moved_messages` (cat_movement.rs) once per FixedUpdate
        // after every cat-stepping resolver. Substrate for event-driven
        // cache invalidation (Stage B: NearPairCache; Stage C: per-cat
        // RouteCostCache).
        app.add_message::<crate::messages::cat_moved::CatMoved>();

        // L2 substrate resources (§9 faction + §L2.10). FactionRelations
        // is a constant lookup — fine to insert at build time.
        // DseRegistry starts empty; populated by `register_dses_at_startup`
        // (Startup-after-`setup_world_exclusive`) so fox DSEs etc. read
        // live `SimConstants` instead of `ScoringConstants::default()`.
        // The §3.5 modifier pipeline is also built by that Startup
        // system. Single-site registration — eliminates the prior
        // three-mirror burden flagged in CLAUDE.md.
        app.insert_resource(crate::ai::faction::FactionRelations::canonical());
        // 431 Stage B — cache of cat pairs within passive_familiarity_range.
        // Built incrementally on CatMoved by `update_near_pair_cache`; read
        // by `passive_familiarity` for the per-tick delta application.
        app.init_resource::<crate::resources::near_pair_cache::NearPairCache>();
        // Ticket 279 — per-pair sustained-co-presence accumulator. Consumes
        // NearPairCache; emits WitnessableEvent::SustainedCoPresence on
        // threshold. See `systems::sustained_copresence`.
        app.init_resource::<crate::systems::sustained_copresence::SustainedCoPresenceTracker>();
        // Ticket 433 — cross-system per-tick aggregates (colony markers +
        // food snapshot). Populated by `populate_world_snapshots`; read by
        // `evaluate_and_plan`. See `docs/systems/world-snapshots.md`.
        app.init_resource::<crate::resources::world_snapshots::WorldSnapshots>();
        app.init_resource::<DseRegistry>();
        // Ticket 207 — InfluenceMapRegistry replaces the hand-bundled
        // `L1Maps` SystemParam in `trace_emit.rs`. Empty at build time;
        // populated by `register_influence_maps_at_startup` below.
        app.init_resource::<InfluenceMapRegistry>();
        // Ticket 319 — HTN MethodRegistry (child #1 of the 128 epic).
        // Empty at build time and at 319 landing; populated by
        // `register_methods_at_startup`. Tickets 320 onward author
        // methods into `populate_method_registry`.
        app.init_resource::<MethodRegistry>();
        // Ticket 365 — crafting RecipeRegistry (016 Phase 1a).
        // Empty at build time and at Commit 1 of 365; populated
        // by `register_recipes_at_startup`. Commits 2/3 of this
        // ticket and follow-on phases (367 / 368 / 369 / 370 /
        // 371 / 372) author recipes into
        // `populate_recipe_registry`.
        app.init_resource::<RecipeRegistry>();
        // 176: chronicity tracker for `ColonyStoresChronicallyFull`.
        // Updated by `update_colony_building_markers` once per
        // `ScoringConstants::chronicity_window_ticks` ticks.
        app.init_resource::<crate::resources::stores_pressure::StoresPressureTracker>();
        // 084 Commit 3: chronicity tracker for
        // `ColonyThornbriarChronicallyLow`. Same cadence as
        // StoresPressureTracker but inverts polarity (samples stash
        // *state* against a low-water threshold).
        app.init_resource::<crate::resources::thornbriar_pressure::ThornbriarPressureTracker>();
        // Ticket 400 — per-cat parenting-bias scalar map. Populated each
        // tick by `parenting_activity::populate_parenting_scalars`; read
        // at `ScoringContext` build time and by `ParentingActivityModifier`.
        app.init_resource::<crate::systems::parenting_activity::ParentingScalars>();
        // Ticket 427 Step 1 — pre-allocated scratch for target-taking DSE
        // resolvers. Threaded through `PlanResources::dse_scratchpad`;
        // each `resolve_*_target` clears + writes into its own slots so
        // capacity persists across cat-ticks (~355 MB/soak saved at the
        // 500-cat projection).
        app.init_resource::<crate::resources::DseTargetScratchpad>();
        // 487 — colony-self directive queue, populated by
        // `assess_colony_needs` when no coordinator-tagged cat exists
        // (day-1 founder phase). Drained by `dispatch_urgent_directives`
        // alongside per-coordinator queues.
        app.init_resource::<crate::components::coordination::ColonySelfDirectiveQueue>();
        app.add_systems(
            Startup,
            register_dses_at_startup.after(crate::plugins::setup::setup_world_exclusive),
        );
        // Registry registration is independent of resource setup —
        // walkers look up resources at *call* time, not at registration
        // time — so this can run alongside DSE registration without an
        // ordering constraint on `setup_world_exclusive`.
        app.add_systems(Startup, register_influence_maps_at_startup);
        // 319 — HTN method registry. Same independence as the
        // InfluenceMap registry above: registration only touches the
        // resource, no setup ordering required.
        app.add_systems(Startup, register_methods_at_startup);
        // 365 — recipe registry. Same independence as the
        // InfluenceMap / method registries above: registration
        // only touches the resource, no setup ordering required.
        app.add_systems(Startup, register_recipes_at_startup);

        // Snapshot positions before any simulation system moves entities.
        // The rendering layer interpolates between PreviousPosition and Position.
        app.add_systems(
            FixedUpdate,
            crate::rendering::entity_sprites::snapshot_previous_positions
                .before(systems::time::advance_time),
        );

        app.add_systems(
            FixedUpdate,
            (
                // Chain 1: World simulation
                (
                    systems::time::advance_time.run_if(systems::time::not_paused),
                    systems::weather::update_weather,
                    systems::wind::update_wind,
                    systems::time::emit_weather_transitions,
                    systems::magic::corruption_spread,
                    // Ward decay → coverage rebuild: rebuild reads
                    // post-decay strength so the L1 `ward_coverage`
                    // map is always one tick fresh.
                    (
                        systems::magic::ward_decay,
                        systems::magic::update_ward_coverage_map,
                        // 470: per-tile siege-fear stamp from the live
                        // `WildlifeAiState::EncirclingWard` set. Sits
                        // next to `update_ward_coverage_map` so the
                        // two ward-side maps recompute together each
                        // tick (consumers sample both at the same
                        // freshness).
                        systems::magic::update_ward_siege_fear_map,
                        // 382: sliding ColonyCenter. Runs before
                        // `update_colony_district_map` so the district
                        // populator sees the current anchor (the
                        // district populator doesn't read ColonyCenter
                        // directly, but consumer ordering downstream
                        // expects center to be up-to-date by the time
                        // any L1/L2/L3 system reads it this tick).
                        systems::coordination::update_colony_center,
                        // 382: ColonyDistrictMap populator. Threaded
                        // into the existing ward-coverage chain rather
                        // than added as a new top-level sibling — the
                        // chain extension keeps the relative order of
                        // every other system unchanged. Reads
                        // CatScentMap / FoxScentMap /
                        // FoxApproachCorridorMap / TileMap corruption
                        // and emits the three-axis colony-district
                        // composite consumed by
                        // `compute_building_placement`.
                        systems::coordination::update_colony_district_map,
                    )
                        .chain(),
                    // Herb/flavor growth sub-chain: seasonal check resets stage,
                    // then growth advances, then flavors advance.
                    //
                    // Ticket 061 note — `update_herb_location_map`
                    // (defined in `magic.rs`) is intentionally NOT
                    // scheduled here. Adding it shifts Bevy's
                    // topological sort enough to collapse Hunting and
                    // Foraging dispositions to zero on a seed-42 soak,
                    // matching the `reconsider_held_intentions`
                    // precedent documented at `simulation.rs:425-433`.
                    // The producer is registered separately (along
                    // with the marker cutover and the
                    // `herbcraft_target_dse` consumer wiring) in a
                    // follow-on that absorbs the scheduling shift via
                    // wider verification (likely ticket 052's
                    // spatial-consideration sweep).
                    (
                        systems::magic::herb_seasonal_check,
                        systems::magic::advance_herb_growth,
                        systems::magic::advance_flavor_growth,
                        systems::magic::herb_regrowth,
                    )
                        .chain(),
                    systems::magic::corruption_tile_effects,
                    systems::magic::apply_corruption_pushback,
                    // §L2.10.7 — recompute the territory corruption
                    // centroid after spread + tile effects so AI
                    // consumers (ColonyCleanseDse via
                    // LandmarkAnchor::TerritoryCorruptionCentroid)
                    // read the post-mutation centroid next frame.
                    systems::magic::update_corruption_landmarks,
                    systems::magic::spawn_shadow_fox_from_corruption,
                    (
                        // 140 step 13 — save-load lazy-insert pass
                        // (pre-138 saves lack `MovementBudget`;
                        // pre-140 saves lack `Velocity` /
                        // `DesiredVelocity`). The ticket-138 per-tick
                        // accumulator loop that used to live here is
                        // retired with the accumulator itself; the
                        // remaining `Without<…>` queries are
                        // archetype-pruned to empty after the first
                        // tick, so the steady-state cost is nil.
                        systems::movement_budget::insert_missing_movement_components,
                        // Ticket 023 shadow-fox decision sub-chain —
                        // grouped to keep the wildlife `.chain()` under
                        // Bevy's 20-element limit while preserving
                        // strict order: coherence → motivation →
                        // haunting drain.
                        (
                            // Ticket 023 Phase A — coherence tick must run
                            // before `wildlife_ai` so a dissolving shadow-fox
                            // gets despawned (well, queued) before downstream
                            // shadowfox-bearing systems take decisions. Lives
                            // inside the existing wildlife `.chain()` block
                            // to avoid creating a new top-level schedule edge
                            // (ticket 061 precedent).
                            systems::wildlife::shadowfox_coherence_tick,
                            // Ticket 023 Phase B — motivation tick re-elects
                            // each shadow-fox's WildlifeAiState every
                            // `shadow_fox_motivation_tick_cadence` ticks
                            // (default 16). Runs after coherence so a
                            // shadow-fox that dissolves this tick won't be
                            // assigned a state it can't act on, and before
                            // `wildlife_ai` so the new state takes effect
                            // immediately.
                            systems::wildlife::shadowfox_motivation_tick,
                            // Ticket 023 Phase C — haunting-drain runs every
                            // tick to apply per-tick mood/safety drain on
                            // nearby cats and to tick the haunting-to-stalk
                            // escalation counter. Runs after motivation_tick
                            // (which writes the Haunting state) and before
                            // wildlife_ai (which executes the orbit-at-edge
                            // movement).
                            systems::wildlife::shadowfox_haunting_drain,
                        )
                            .chain(),
                        systems::wildlife::spawn_wildlife,
                        systems::wildlife::wildlife_ai,
                        // 140 step 9 — `fox_movement` retired: fox motion is
                        // desire-driven via the fox GOAP travel resolvers; the
                        // legacy phase-mirror mover double-drove every travel
                        // step and its Fleeing arm walked hurt foxes into water.
                        // Ticket 025 Phase 2 — per-species per-tick
                        // needs decay, scheduled as a nested sub-chain
                        // to keep the outer wildlife tuple at exactly
                        // 20 entries (Bevy's `.chain()` limit). Pre-025
                        // the chain had 20 entries already after the
                        // ticket-023 shadow-fox systems landed; adding
                        // 10 hawk/snake systems without nesting would
                        // overflow. Per-species ordering inside each
                        // sub-chain follows needs → sync → evaluate →
                        // resolve, matching the fox precedent.
                        (
                            systems::wildlife::fox_needs_tick,
                            systems::fox_goap::sync_fox_needs,
                            systems::fox_goap::fox_evaluate_and_plan,
                            systems::fox_goap::fox_resolve_goap_plans,
                        )
                            .chain(),
                        (
                            systems::hawk_goap::hawk_needs_tick,
                            systems::hawk_goap::sync_hawk_needs,
                            systems::hawk_goap::hawk_evaluate_and_plan,
                            systems::hawk_goap::hawk_resolve_goap_plans,
                        )
                            .chain(),
                        (
                            systems::snake_goap::snake_needs_tick,
                            systems::snake_goap::sync_snake_needs,
                            systems::snake_goap::snake_evaluate_and_plan,
                            systems::snake_goap::snake_resolve_goap_plans,
                        )
                            .chain(),
                        systems::fox_goap::feed_cubs_at_dens,
                        systems::fox_goap::resolve_paired_confrontations,
                        systems::wildlife::fox_ai_decision,
                        systems::wildlife::fox_scent_tick,
                        // 312: corridor-traffic populator + decay,
                        // scheduled alongside `fox_scent_tick` inside
                        // the existing wildlife `.chain()` block to
                        // avoid creating a new top-level schedule edge
                        // (ticket 061 precedent — adding an unordered
                        // sibling perturbs Bevy's topological sort and
                        // collapsed Hunting/Foraging on seed-42).
                        systems::wildlife::update_fox_approach_corridor_map,
                        systems::wildlife::predator_hunt_prey,
                        systems::wildlife::carcass_decay,
                        systems::wildlife::carcass_scent_tick,
                        systems::wildlife::predator_stalk_cats,
                        // Hawk + snake lifecycle (starvation death +
                        // age tick + HawkDied/SnakeDied messages).
                        // Sub-chain to stay within the 20-tuple limit
                        // for the outer wildlife chain.
                        (
                            systems::hawk_goap::hawk_lifecycle_tick,
                            systems::snake_goap::snake_lifecycle_tick,
                        )
                            .chain(),
                    )
                        .chain(),
                    systems::prey::prey_population,
                    systems::prey::prey_hunger,
                    systems::prey::prey_ai,
                    // Substrate-vibration + scent influence-map writers.
                    // Sub-chain to stay under Bevy's 20-tuple limit on
                    // the outer chain. 100: `tremor_tick` runs after
                    // `prey_ai` (which reads it) so the read sees
                    // last-tick deposits — preserves the cause-and-
                    // effect ordering "the cat moved, prey alerts next
                    // tick" rather than "prey alerts during the same
                    // tick the cat first stepped."
                    (
                        systems::prey::prey_scent_tick,
                        systems::sensing::tremor_tick,
                    )
                        .chain(),
                    systems::prey::prey_den_lifecycle,
                    systems::wildlife::detect_threats,
                    // Building-side sub-chain: passive effects, decay,
                    // and the §5.6.3 colony-faction influence-map
                    // writers (ticket 006). Nested to stay under
                    // Bevy's 20-system tuple limit on the outer chain.
                    // Map writers run *after* `decay_building_condition`
                    // so effectiveness gates read post-decay values.
                    (
                        systems::buildings::apply_building_effects,
                        systems::buildings::decay_building_condition,
                        systems::buildings::update_colony_landmarks,
                        systems::buildings::update_food_location_map,
                        systems::buildings::update_garden_location_map,
                        systems::buildings::update_construction_site_map,
                        // 101: env-quality influence-map sweep + feature
                        // emission. Runs after `decay_building_condition`
                        // so `structure.condition` / `structure.cleanliness`
                        // reads in the building sweep see post-decay
                        // values. `emit_env_quality_features` reads the
                        // four mood-relevant maps the sweep just
                        // populated and records the per-soak canaries.
                        systems::env_quality::update_env_quality_maps,
                        systems::env_quality::emit_env_quality_features,
                    )
                        .chain(),
                    systems::items::decay_items,
                )
                    .chain(),
                // Item pruning, food sync, den pressure/raids, orphan prey.
                (
                    systems::items::prune_stored_items,
                    systems::items::sync_food_stores,
                    // 308 — ground-truth colony reserves aggregator
                    // (thornbriar / remedy-herb counts across cat
                    // inventories + Stores buildings). Sibling to
                    // sync_food_stores; runs each tick so downstream
                    // observability and balance canaries can compare
                    // ground truth against per-cat
                    // ColonyReservesBelief.
                    systems::items::sync_colony_reserves,
                    systems::prey::update_den_pressure,
                    systems::prey::apply_den_raids,
                    systems::prey::orphan_prey_adopt_or_found,
                )
                    .chain(),
                // Chain 2: Cat needs, markers, mood, coordination.
                // Split into 2a/2b sub-chains to stay under Bevy's
                // 20-system tuple limit on `.chain()`.
                (
                    // Chain 2a: needs + marker authors + reproduction + growth
                    (
                        systems::needs::decay_needs,
                        // §4 marker authors — run before the GOAP/scoring
                        // pipeline so consumers see freshly-authored
                        // markers. Grouped as a nested sub-tuple to keep
                        // the outer Chain 2a under Bevy's 20-system tuple
                        // limit; sub-chain order matches the dependency
                        // chain (life-stage / injury / inventory /
                        // directive feed into capability + mate
                        // eligibility).
                        (
                            systems::incapacitation::update_incapacitation,
                            systems::growth::update_life_stage_markers,
                            systems::needs::update_injury_marker,
                            // Ticket 087 — interoceptive perception.
                            // Authors LowHealth / SevereInjury /
                            // BodyDistressed from Health + Needs. Runs
                            // adjacent to update_injury_marker (same
                            // data sources, different markers); both
                            // run before the GOAP/scoring pipeline so
                            // DSE eligibility filters see fresh state.
                            systems::interoception::author_self_markers,
                            systems::items::update_inventory_markers,
                            systems::coordination::update_directive_markers,
                            // §4 batch — Mate eligibility marker. Reads
                            // the full `mating::has_eligible_mate`
                            // predicate (season + sated/happy + fertility
                            // + Partners bond + orientation compat) and
                            // writes `HasEligibleMate`.
                            // `MateDse::eligibility()` requires this
                            // marker, so the DSE returns 0.0 for cats
                            // whose gate is closed.
                            crate::ai::mating::update_mate_eligibility_markers,
                            // Ticket 127 — JointIntention author with
                            // matchmaker + drop predicate + stage
                            // progression + cascade detection +
                            // mismatch tracking. Subsumes the prior
                            // §7.M L2 PairingActivity author (tickets
                            // 027b / 082 / 083 / 257). The substrate
                            // shift was activated post-Wave-2 hardening
                            // and post-272 mating-cadence stabilization
                            // so the food-economy lift (pair-socializing
                            // bias raising median food_fraction) stays
                            // in band; Farm dormancy under abundant
                            // food remains intended per ticket 084.
                            // Ticket 276 Commit B — wrap the JI author +
                            // PlayBout cascade in a 2-tuple sub-chain
                            // to stay under Bevy's 20-system outer-tuple
                            // limit. The cascade drains
                            // `PlayBoutBoutingEntered` messages emitted
                            // by the author and applies the Bouting-
                            // stage mood-lift + play_social narrative
                            // (substrate replacement for the retired
                            // `on_play_initiated` observer).
                            (
                                crate::ai::joint_intention::author_joint_intentions,
                                crate::ai::joint_intention::cascade_play_bout_bouting,
                            )
                                .chain(),
                            // §4 batch 2: capability markers — reads
                            // life-stage, injury, inventory markers
                            // authored above.
                            crate::ai::capabilities::update_capability_markers,
                            // §4.2 State markers — InCombat reads
                            // CurrentAction; OnCorruptedTile and
                            // OnSpecialTerrain read TileMap. Independent
                            // of each other and of the upstream marker
                            // authors, but registered here so the
                            // MarkerSnapshot population in the GOAP /
                            // disposition scoring loops sees them.
                            systems::combat::update_combat_marker,
                            systems::magic::update_corrupted_tile_markers,
                            systems::sensing::update_terrain_markers,
                            // Ticket 014 Mentoring batch — Mentor /
                            // Apprentice authored from `Training`;
                            // HasMentoringTarget from skill-gap
                            // sensing predicate.
                            systems::aspirations::update_training_markers,
                            systems::aspirations::update_mentoring_target_markers,
                            // Ticket 014 Parent marker — active
                            // parenthood authored from
                            // `KittenDependency` references.
                            systems::growth::update_parent_markers,
                            // Ticket 398 — event-driven adoption of
                            // RAISE_OFFSPRING_ASPIRATION when Parent
                            // marker is first set. Runs after
                            // update_parent_markers so newly-set
                            // markers trigger adoption the same tick.
                            // The passive aspiration adoption picker
                            // explicitly skips Kinship (see
                            // `select_aspirations` / `adopt_new_aspirations`).
                            //
                            // Ticket 400 — 398's mother-only gate
                            // widened to all parents (mother OR father)
                            // now that ParentingActivityModifier
                            // handles dispersion personality-conditionally.
                            systems::aspirations::adopt_kinship_aspiration,
                            // Ticket 400 — ParentingActivity + InLaw
                            // adoption bundle. Sync the Biological-kind
                            // entries (mirrors update_parent_markers's
                            // KittenDependency-presence pattern), apply
                            // the InLaw rule on fresh CourtshipBonded
                            // transitions (depends on
                            // author_joint_intentions having set
                            // stage_entered_tick = now_tick earlier in
                            // this chain), then tick engagement
                            // gradients and populate the per-cat
                            // scalar bundle for ScoringContext. Grouped
                            // to stay under Bevy's 20-system
                            // outer-tuple limit.
                            (
                                systems::parenting_activity::update_parenting_activity_biological,
                                crate::ai::joint_intention::apply_inlaw_adoption_on_bonded,
                                systems::parenting_activity::tick_parental_engagement,
                                systems::parenting_activity::populate_parenting_scalars,
                            )
                                .chain(),
                            // Ticket 014 §4 sensing batch — broad-phase
                            // target-existence: HasThreatNearby,
                            // HasSocialTarget, HasHerbsNearby, PreyNearby,
                            // CarcassNearby. Single author owns five
                            // markers to amortize the per-cat sensing scans.
                            //
                            // Ticket 170 — `HideEligible` author chained
                            // after target-existence (it reads the
                            // just-authored `HasThreatNearby`).
                            //
                            // Ticket 423 — `CoverAvailabilityMap` rebuild
                            // chained BEFORE the HideEligible author so
                            // the marker reads the same tick's fresh
                            // cover map. The map is dirty-flag-gated;
                            // steady-state ticks pay zero cost. Wrapped
                            // as a sub-tuple to keep the outer chain
                            // under Bevy's 20-system limit.
                            (
                                crate::resources::update_cover_availability_map,
                                systems::sensing::update_target_existence_markers,
                                systems::sensing::update_hide_eligible_markers,
                            )
                                .chain(),
                            // Ticket 014 §4 fox markers — 7 authors
                            // grouped into a sub-tuple so the outer
                            // chain stays under Bevy's 20-system tuple
                            // limit. Authors are independent of each
                            // other; chain ordering is informational.
                            (
                                systems::fox_spatial::update_store_awareness_markers,
                                systems::fox_spatial::update_den_threat_markers,
                                systems::fox_spatial::update_ward_detection_markers,
                                systems::fox_spatial::update_cub_marker,
                                systems::fox_spatial::update_cub_hunger_markers,
                                systems::fox_spatial::update_juvenile_dispersal_markers,
                                systems::fox_spatial::update_den_marker,
                            )
                                .chain(),
                            // Ticket 049 §9.2 BefriendedAlly author —
                            // toggles the marker on cats and wildlife
                            // when their cross-species familiarity
                            // crosses the threshold (no production
                            // signal source today; runs as a no-op
                            // until trade or a non-hostile-contact
                            // accumulator lands).
                            systems::social::befriend_wildlife,
                        )
                            .chain(),
                        systems::needs::decay_grooming,
                        // Ticket 080 — clear `Reserved` markers whose
                        // `expires_tick` has lapsed.
                        crate::systems::plan_substrate::expire_reservations,
                        // Ticket 073 — bound per-cat `RecentTargetFailures`
                        // map size by expiring entries older than
                        // `target_failure_cooldown_ticks`.
                        systems::plan_substrate::sensors::prune_recent_target_failures,
                        systems::needs::eat_from_inventory,
                        systems::needs::decay_exploration,
                        systems::needs::stamp_passive_exploration,
                        systems::needs::update_exploration_centroid,
                        systems::needs::bond_proximity_social,
                        systems::fulfillment::decay_fulfillment,
                        systems::fulfillment::bond_proximity_social_warmth,
                        systems::fulfillment::update_body_condition,
                        systems::pregnancy::tick_pregnancy,
                        // Fertility transitions (§7.M.7) — run after
                        // tick_pregnancy so `RemovedComponents<Pregnant>`
                        // from the birth path reaches
                        // `handle_post_partum_reinsert` in the same frame.
                        systems::fertility::handle_post_partum_reinsert,
                        systems::fertility::update_fertility_phase,
                        systems::growth::tick_kitten_growth,
                        systems::growth::kitten_mood_aura,
                        // Ticket 006 §5.6.3 row #13 — re-stamp the
                        // kitten-cry influence map after growth so
                        // matured kittens (KittenDependency removed in
                        // tick_kitten_growth) drop out of the same
                        // frame. Ticket 156 repurposed the map from
                        // Sight to Hearing channel.
                        //
                        // Ticket 161: this system also authors
                        // `IsParentOfHungryKitten` (merged from a
                        // separate Chain 2a author). Both subsystems
                        // share the same `&Needs` access on kittens
                        // and the same hunger-threshold predicate, so
                        // co-locating them avoids adding a new
                        // schedule conflict edge to Bevy's parallel
                        // scheduler — ticket 158's standalone author
                        // shifted the seed-42 trajectory at tick
                        // 1201300 by introducing such an edge.
                        systems::growth::update_kitten_cry_map,
                    )
                        .chain(),
                    // Chain 2b: mood + memory + coordination
                    (
                        systems::mood::update_mood,
                        systems::mood::mood_contagion,
                        systems::mood::bond_proximity_mood,
                        systems::memory::decay_memories,
                        // 308 — broadcast each cat's inventory snapshot
                        // on its stagger tick. Writes
                        // `WitnessableEvent::InventoryObserved` so
                        // `integrate_beliefs` Pass A can consume the
                        // event in the same tick (writer → reader
                        // within-tick is valid when writer is chained
                        // before reader).
                        systems::belief_integrator::gossip_inventory_observations,
                        // 374 — shelter belief substrate. Four
                        // per-stagger systems that own the home_den
                        // claim lifecycle (claim/lose), continuity
                        // accrual/decay, and emit DenDamaged /
                        // DenRepaired / DenSieged / DenSiegeBroken
                        // events for `integrate_beliefs` to consume
                        // same-tick. Chained before `integrate_beliefs`
                        // so this stagger's events land in this tick's
                        // Pass A. Schedule-edge perturbation is
                        // expected at land — Phase C reader cutover
                        // re-baselines `welfare.shelter` and
                        // `pressure.shelter` against the post-substrate
                        // signal shape.
                        systems::shelter_beliefs::claim_home_dens,
                        systems::shelter_beliefs::update_shelter_continuity,
                        systems::shelter_beliefs::emit_den_condition_events,
                        systems::shelter_beliefs::detect_den_sieges,
                        // 258 — C3 belief substrate integrator. Pass A
                        // consumes WitnessableEvent messages → EMA updates
                        // on per-cat mental models; pass B implants species
                        // priors for nearby predators and decays facets
                        // toward priors on each cat's stagger tick.
                        systems::belief_integrator::integrate_beliefs,
                        // 293 — derive `ColonyHuntingMap` from per-cat
                        // `LocationBeliefs.prey_yield`. Replaces the
                        // legacy social-transmission `absorb` pathway
                        // (socialize / groom_other) and the direct
                        // `colony_map.beliefs.record_catch` writes
                        // retired this same commit. Chained after
                        // `integrate_beliefs` so the aggregation sees
                        // this tick's freshly-updated facets.
                        systems::colony_hunting_map::rebuild_colony_hunting_map,
                        // 261 — ActionAffordances substrate writer. Reads
                        // facets the integrator authored this tick (within
                        // a single `.chain()` block so the ordering is
                        // strict). Lands behavior-neutral: the resource
                        // populates but no DSE reads from it. Folded into
                        // Chain 2b (not a new top-level sibling) per the
                        // schedule-edge perturbation memory — adding a
                        // sibling can reshuffle Bevy's topological sort
                        // and perturb seed-42 on unrelated systems.
                        systems::affordance_writer::affordance_writer,
                        // 308 — author per-cat `HasLowWardReserve` from
                        // the just-updated `ColonyReservesBelief`. Runs
                        // after `integrate_beliefs` so the marker
                        // reflects same-tick belief state.
                        systems::items::update_low_ward_reserve_markers,
                        // 487 — alignment EWMA runs BEFORE
                        // evaluate_coordinators so the election score
                        // reads this tick's freshly-decayed scores.
                        systems::coordination::update_colony_alignment_scores,
                        systems::coordination::evaluate_coordinators,
                        systems::coordination::assess_colony_needs,
                        systems::coordination::dispatch_urgent_directives,
                        systems::coordination::accumulate_build_pressure,
                        systems::coordination::spawn_construction_sites,
                    )
                        .chain(),
                )
                    .chain(),
                // Chain 3: Action resolution (disposition system handles all action selection)
                (
                    systems::task_chains::resolve_task_chains,
                    systems::magic::resolve_magic_task_chains,
                    systems::magic::apply_remedy_effects,
                    systems::buildings::process_gates,
                    systems::buildings::tidy_buildings,
                    // 367 Commit 5 — drying-rack per-tick advancement.
                    // Sits in Chain 3 next to building update systems
                    // because the work it does ("if loaded under
                    // Clear sky, advance progress; spawn output at
                    // 100%") is structurally a structure-state-update
                    // pass. The query is filtered to DryingRack
                    // archetypes (typically 0-3 entities), so the
                    // per-tick cost is bounded by colony rack count.
                    systems::preservation::advance_preservation_drying,
                )
                    .chain(),
                // Chain 4: Social, combat, death, cleanup, narrative
                (
                    // 431 Stage A — emit CatMoved messages for cats whose
                    // Position changed during Chain 3's resolvers. Runs at
                    // the head of Chain 4 so all per-tick subscribers
                    // (Stage B: NearPairCache; Stage C: per-cat
                    // RouteCostCache) consume up-to-date deltas.
                    // 140 step 6 — velocity integrator: consumes
                    // DesiredVelocity written by Chain-3 resolvers,
                    // applies steer + Euclidean speed cap + sub-stepped
                    // wall-slide motion. MUST run before
                    // emit_cat_moved_messages so CatMoved subscribers
                    // see post-move positions the same tick.
                    (
                        // 140 step 7 — personal-space pass feeds the
                        // integrator the same tick.
                        systems::movement::apply_separation,
                        systems::movement::integrate_velocities,
                        systems::cat_movement::emit_cat_moved_messages,
                    )
                        .chain(),
                    // 431 Stage B — rebuild the near-pair cache from the
                    // CatMoved deltas (with first-tick bootstrap from the
                    // cats query). Replaces the legacy per-tick O(N²)
                    // pair sweep that ran inside `passive_familiarity`
                    // (64.43% inclusive CPU at the 2026-05-20 baseline).
                    systems::social::update_near_pair_cache,
                    systems::social::passive_familiarity,
                    // 279 — play-engagement perception cues. Nested as one
                    // chained sub-group (keeps the outer Chain-4 tuple within
                    // Bevy's 20-element `.chain()` arity). All three run after
                    // `update_near_pair_cache` so they read a fresh pair set
                    // and the head-of-Chain-4 CatMoved stream. They emit
                    // WitnessableEvent variants consumed by `integrate_beliefs`
                    // on the following tick (same one-tick latency as the
                    // goap.rs / disposition.rs emitters, which also fire after
                    // the integrator).
                    (
                        systems::playbow_emitter::emit_play_bows,
                        systems::playbow_emitter::emit_reciprocal_advances,
                        systems::sustained_copresence::track_sustained_copresence,
                        // 472 — broadcast `CarriesFesteringWound` from
                        // cats with a festering wound, throttled per
                        // cat by `festering_observation_interval_ticks`.
                        // Sits next to the 279 affiliation emitters
                        // because both feed `belief_integrator` via
                        // `WitnessableEvent`.
                        systems::festering_authoring::emit_festering_observations,
                    )
                        .chain(),
                    systems::personality_friction::personality_friction,
                    systems::social::check_bonds,
                    systems::colony_knowledge::update_colony_knowledge,
                    systems::combat::resolve_combat,
                    systems::combat::heal_injuries,
                    systems::wildlife::fox_lifecycle_tick,
                    systems::wildlife::fox_confrontation_tick,
                    systems::wildlife::fox_store_raid_tick,
                    systems::magic::personal_corruption_effects,
                    // 471 / 472 — telemetry-then-attribution sub-chain.
                    // `author_festering_from_misfire` (472) drains the
                    // MisfireEffect stream emitted by resolve_magic_
                    // task_chains and resolve_goap_plans earlier in the
                    // tick; for WoundTransfer arms it rolls
                    // `misfire_festering_chance` and authors a
                    // WoundKind::Festering wound on a random body part,
                    // emitting a BodyPartInjury that `cache_last_body_
                    // part_injury` (471) then drains for death
                    // attribution before `check_death` reads the cache.
                    // Nested sub-chain (same arity-20 accommodation as
                    // the 279 group above).
                    (
                        systems::festering_authoring::author_festering_from_misfire,
                        systems::injury_cache::cache_last_body_part_injury,
                        systems::death::check_death,
                    )
                        .chain(),
                    // 487 — `flag_coordinator_death` +
                    // `flag_coordinator_incapacitated` bundled as a sub-
                    // chain to stay under Bevy's 20-element outer-chain
                    // arity limit. Both write into the same
                    // `CoordinatorDied` resource (re-eval trigger); the
                    // incapacitated path also strips the Coordinator
                    // marker since holding the role while downed would
                    // keep `assess_colony_needs`'s no-coordinator gate
                    // false.
                    (
                        systems::coordination::flag_coordinator_death,
                        systems::coordination::flag_coordinator_incapacitated,
                    )
                        .chain(),
                    systems::coordination::expire_directives,
                    systems::death::cleanup_dead,
                    // 035: rebuild the grave-aura InfluenceMap from
                    // live `Grave` entities each tick. Lives in the
                    // late-tick batch alongside `cleanup_dead` and
                    // `cleanup_wildlife` because graves are spawned
                    // by `resolve_goap_plans`'s post-loop drain
                    // earlier in the tick — the rebuild must run
                    // after all spawns so the next tick's L1 trace
                    // sees the freshly-spawned aura.
                    systems::death::update_grave_aura_map,
                    systems::wildlife::cleanup_wildlife,
                    systems::narrative::generate_narrative,
                )
                    .chain(),
            )
                .chain(),
        );

        // GOAP systems — ordered pipeline replacing the old disposition systems.
        // check_modifier_preemption → evaluate_and_plan →
        // resolve_goap_plans → emit_plan_narrative.
        //
        // `check_modifier_preemption` and `evaluate_and_plan` must run
        // AFTER sync_food_stores so that food_available reflects the
        // current tick's item state, not a stale default of 0.0.
        //
        // Ticket 230 — the legacy `check_anxiety_interrupts` system
        // and its lone surviving `ThreatDetected` arm are retired.
        // Tickets 106/107/108/119 retired the four sibling arms in
        // favor of substrate-driven modifiers; 230 replaces the last
        // arm with `DispositionKind::Fleeing` (plan template
        // `[PickFleeTarget, Flee, HoldUntilSafe]`, commitment-aware
        // modifier guard via the disposition-tier early-skip in
        // `try_preempt_with_modifier_lurch`). The substrate-driven
        // preempt path (`check_modifier_preemption`) is now the sole
        // tier-1-acute interrupt surface.
        app.add_systems(
            FixedUpdate,
            systems::goap::check_modifier_preemption.after(systems::items::sync_food_stores),
        );
        // §7.2 commitment gate (Phase 6a) is not a stand-alone system —
        // it's inlined into `resolve_goap_plans`'s per-cat loop
        // prologue via `crate::ai::commitment::{strategy_for_disposition,
        // proxies_for_plan, should_drop_intention, record_drop}`. The
        // 2026-04-23 PM attempt registered a `reconsider_held_intentions`
        // system between `check_anxiety_interrupts` and
        // `evaluate_and_plan`; its schedule presence reshuffled
        // ordering enough to starve the colony (see
        // `docs/open-work.md` #5). The inlined form shifts the gate's
        // effect by one tick (replacement next tick instead of same
        // tick) without new scheduler edges.
        // Ticket 168 — colony-marker author chain. Runs after
        // sync_food_stores (so HasStoredFood reflects the current tick's
        // food state) and before evaluate_and_plan (so the snapshot
        // population reads up-to-date markers). Chained among themselves
        // for deterministic ordering — the same `reconsider_held_intentions`
        // schedule-edge perturbation that bit the 2026-04-23 attempt
        // (see comment at line 492 above) is the reason these are
        // sequentially chained rather than registered as siblings.
        app.add_systems(
            FixedUpdate,
            (
                systems::buildings::update_colony_building_markers,
                systems::magic::update_herb_availability_markers,
                systems::magic::update_ward_coverage_markers,
                systems::magic::update_ward_siege_marker,
            )
                .chain()
                .after(systems::items::sync_food_stores)
                .before(systems::goap::evaluate_and_plan),
        );
        // Flush the singleton `.insert()/.remove()` writes so the
        // `WorldSnapshots` populator + `evaluate_and_plan`'s
        // `Has<MarkerN>` reads see them within the same tick. Chained
        // with `populate_world_snapshots` so the snapshot reads the
        // freshly-flushed marker state and downstream consumers
        // (`evaluate_and_plan`) see the cached snapshot.
        // Ticket 433 — `populate_world_snapshots` lives in the same
        // chain block (rather than as an independent `.before` /
        // `.after` constraint) so the topological order is explicit:
        // ApplyDeferred → populate_world_snapshots → evaluate_and_plan.
        app.add_systems(
            FixedUpdate,
            (
                bevy::ecs::schedule::ApplyDeferred,
                systems::world_snapshots::populate_world_snapshots,
            )
                .chain()
                .after(systems::magic::update_ward_siege_marker)
                .before(systems::goap::evaluate_and_plan),
        );
        // Ticket 321 — L1→L2 aspiration emission picker. Exclusive
        // system (needs `&World` for `Method.applicable_when` and
        // `MethodRegistry.lookup`). Runs after the L1 marker authors
        // flush (the `ApplyDeferred` above) and before
        // `evaluate_and_plan`, so its `AspirationEmissions`
        // Component is visible to the L2 wrap site within the same
        // tick. Per memory `learning_bevy_schedule_edge_perturbation`
        // the Chain-sibling addition can perturb seed-42; at 321
        // land emissions are sparse (Hunting cats at "First Blood"
        // only), so drift is bounded.
        app.add_systems(
            FixedUpdate,
            systems::aspiration_picker::pick_aspiration_emissions
                .after(systems::magic::update_ward_siege_marker)
                .before(systems::goap::evaluate_and_plan),
        );
        app.add_systems(
            FixedUpdate,
            systems::goap::evaluate_and_plan
                .after(systems::goap::check_modifier_preemption)
                .after(systems::items::sync_food_stores)
                .after(systems::aspiration_picker::pick_aspiration_emissions)
                // Ticket 400 — ensure ParentingScalars is populated for
                // this tick before scoring reads it via ColonyContext.
                // Without this, Bevy's scheduler can run the modifier
                // pipeline before `populate_parenting_scalars` writes
                // the per-cat bundle — the nested SystemParam access
                // path through `ColonyContext` doesn't surface the
                // ResMut<ParentingScalars> dependency to the scheduler
                // strongly enough.
                //
                // Schedule-edge note (per memory:
                // `learning_bevy_schedule_edge_perturbation`): adding
                // this constraint perturbs seed-42 RNG via the
                // re-ordered FixedUpdate sequence. Affected scenario
                // tests (`disposal_election`, `picking_up_scavenging`)
                // see different tick-1 softmax outcomes despite
                // structurally-identical scoring — they assert on
                // probabilistic L3 wins under specific seed behavior.
                // Updated to use sufficiently long tick budgets that
                // re-elections smooth out the RNG variance.
                .after(systems::parenting_activity::populate_parenting_scalars),
        );
        // Flush commands so GoapPlan inserted by evaluate_and_plan is
        // visible to resolve_goap_plans in the same tick.
        app.add_systems(
            FixedUpdate,
            bevy::ecs::schedule::ApplyDeferred
                .after(systems::goap::evaluate_and_plan)
                .before(systems::goap::resolve_goap_plans),
        );
        app.add_systems(
            FixedUpdate,
            systems::goap::resolve_goap_plans
                .after(systems::goap::evaluate_and_plan)
                .before(systems::task_chains::resolve_task_chains),
        );
        app.add_systems(
            FixedUpdate,
            systems::goap::emit_plan_narrative.after(systems::goap::resolve_goap_plans),
        );
        // Ticket 108 — write back current `safety_deficit` to
        // `PrevSafetyDeficit` *after* the scoring pass so next tick's
        // `evaluate_and_plan` / `evaluate_dispositions` see last tick's
        // value as `prev` and compute a non-zero rising-derivative
        // when safety drops over the tick boundary. If this ran
        // before scoring, the derivative would always be zero.
        app.add_systems(
            FixedUpdate,
            systems::plan_substrate::update_prev_safety_deficit
                .after(systems::goap::evaluate_and_plan)
                .after(systems::goap::resolve_goap_plans),
        );

        // Standalone systems — registered after the chains but unordered
        // relative to each other. These exceed Bevy's chain param limit.
        app.add_systems(
            FixedUpdate,
            (
                systems::disposition::cat_scent_tick.after(systems::goap::resolve_goap_plans),
                // 256 R5 — runs alongside cat_scent_tick (both
                // depend on resolve_goap_plans having set the cat's
                // current_action for this tick). Fox AI in the same
                // schedule (further down) consumes the deterrent
                // map in its A* call.
                systems::disposition::cat_patrol_deterrent_tick
                    .after(systems::goap::resolve_goap_plans),
                systems::personality_events::emit_personality_events,
                systems::ai::emit_periodic_events,
                systems::snapshot::emit_cat_snapshots.after(systems::goap::resolve_goap_plans),
                systems::snapshot::emit_position_traces.after(systems::goap::resolve_goap_plans),
                systems::snapshot::emit_spatial_snapshots,
                systems::colony_score::emit_colony_score,
                systems::fate::assign_fated_connections,
                systems::fate::awaken_fated_connections,
                systems::aspirations::select_aspirations,
                systems::aspirations::check_second_aspiration_slot,
                systems::aspirations::check_aspiration_abandonment,
                systems::aspirations::track_milestones,
            ),
        );

        // §11 trace emitter — headless-only in practice. Gated on
        // FocalTraceTarget + TraceLog resources; neither is inserted by
        // the interactive setup path, so this system never fires outside
        // headless runs that pass --focal-cat. Registered here (not just
        // in build_schedule) to satisfy the manual-mirror invariant in
        // CLAUDE.md's Headless Mode section.
        app.add_systems(
            FixedUpdate,
            systems::trace_emit::emit_focal_trace
                .after(systems::goap::resolve_goap_plans)
                .run_if(bevy_ecs::prelude::resource_exists::<crate::resources::FocalTraceTarget>)
                .run_if(bevy_ecs::prelude::resource_exists::<crate::resources::TraceLog>)
                .run_if(bevy_ecs::prelude::resource_exists::<crate::resources::FocalScoreCapture>),
        );
    }
}
