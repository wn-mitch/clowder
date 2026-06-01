//! Ticket 321 — L1→L2 aspiration emission picker.
//!
//! Per `docs/systems/htn-methods.md` §H, this is the system that
//! produces per-cat `Intention::Goal` candidates from each cat's
//! active aspirations. Spec anchor:
//! `docs/systems/ai-substrate-refactor.md` §7.7 (aspirations are
//! "long-horizon Intentions that emit short-horizon Intentions") +
//! §L2.10.6 (softmax-over-Intentions).
//!
//! # Four-step contract (§H)
//!
//! For each cat per tick, per `ActiveAspiration`:
//!
//! 1. **Already-in-flight check.** If the cat's `HeldGoalStack`'s
//!    top frame's `source == AspirationEmitted { chain }` matching
//!    this aspiration, the picker skips emission. Commitment momentum
//!    (§7.4) carries the existing arc. An L1Aspiration trace record
//!    fires with empty `emit_walk`.
//! 2. **Emits walk.** Walk the current milestone's
//!    [`crate::ai::aspirations::Emit`] table in `Priority` order
//!    (Primary first, then registration order within tier). The
//!    first row where (a) `MethodRegistry::lookup(label, world,
//!    entity).is_some()` AND (b) `applicable_when(world, entity)`
//!    wins. Emits an [`EmissionRow`] tagged with the matched
//!    chain / milestone / label / strategy / priority.
//! 3. **Domain-affinity fallback.** If no `Emit` row matched, walk
//!    `MethodRegistry` for any `Live` method whose
//!    `Method::domain == Some(chain.domain)` and whose
//!    `applicable_when` predicate holds. First match wins; the
//!    emission carries `fallback_used: true` so trace surfaces can
//!    distinguish the path.
//! 4. **Silent quiet.** No row produced — the aspiration emits
//!    nothing this tick. Multiple quiet ticks escalate to §7.7.e
//!    stagnation-abandon (existing `track_milestones` /
//!    `check_aspiration_abandonment` path).
//!
//! # Exclusive system
//!
//! `applicable_when: fn(&World, Entity) -> bool` and
//! `MethodRegistry::lookup(..., world, entity)` both take `&World`,
//! so the picker is an exclusive system. The implementation snapshots
//! the cat list first (Entity + active aspirations + in-flight chain
//! name + display name) to free the per-cat `Aspirations` borrow
//! before calling the predicates; results are buffered and applied as
//! a single mutation pass at the end.
//!
//! # L1Aspiration trace
//!
//! Per §11.5 registry-walk discipline, the picker emits a
//! `TraceRecord::L1Aspiration` per active aspiration per focal-cat
//! tick. At 321 land the `emit_walk` field is populated only for
//! milestones with non-empty `emits` (currently only Hunting's
//! "First Blood" combine-and-test slice); all other milestones emit
//! `emit_walk: vec![]`. Ticket #338 enriches the record.

use bevy_ecs::prelude::*;

use crate::ai::aspirations::{AspirationChain, Emit, Priority};
use crate::ai::dse::CommitmentStrategy;
use crate::ai::methods::{ApplicableWhen, MethodRegistry};
use crate::components::aspiration_emission::{AspirationEmissions, EmissionRow};
use crate::components::aspirations::{ActiveAspiration, Aspirations};
use crate::components::held_goal_stack::HeldGoalStack;
use crate::components::held_intention::IntentionSource;
use crate::components::identity::Name;
use crate::components::physical::Dead;
use crate::resources::aspiration_registry::AspirationRegistry;
use crate::resources::time::TimeState;
use crate::resources::trace_log::{
    EmitWalkRow, FocalTraceTarget, TraceEntry, TraceLog, TraceRecord,
};

/// Ticket 364 — reactive emit sentinel chain name. Folded into
/// `IntentionSource::AspirationEmitted { chain: REACTIVE_CHAIN }`
/// when a marker-gated demand authors an emission row. The picker's
/// in-flight check distinguishes reactive vs aspirational frames by
/// matching the sentinel here.
pub const REACTIVE_CHAIN: &str = "<reactive>";

/// 364 — marker-gated reactive emit. Unlike aspiration milestones, a
/// reactive emit fires whenever the cat's substrate state satisfies the
/// `applicable_when` predicate; there's no chain / milestone bookkeeping
/// behind it. The Live method matching `label` provides the structure
/// the leaf-primitive arc walks.
struct ReactiveEmit {
    label: &'static str,
    applicable_when: fn(&World, Entity) -> bool,
    strategy: CommitmentStrategy,
    priority: Priority,
}

/// 364 — registry of reactive emits walked after the per-aspiration
/// loop. Order is registration order; `Priority` decides winner via
/// `AspirationEmissions::winner()`. `kitten_reared` is the only entry
/// at 364 land; `process_grief` follows when §7.7.b ships the
/// `Mourning` writer.
const REACTIVE_EMITS: &[ReactiveEmit] = &[ReactiveEmit {
    label: "kitten_reared",
    applicable_when: crate::ai::methods::rear_kitten::has_dependent_kitten,
    strategy: CommitmentStrategy::SingleMinded,
    priority: Priority::Primary,
}];

/// One cat's picker snapshot — captured under the `Aspirations`
/// query borrow so subsequent `&World`-taking predicates can run
/// without holding it.
struct CatSnapshot {
    entity: Entity,
    name: String,
    active: Vec<ActiveAspiration>,
    /// Chain name from `HeldGoalStack.frames[0].source` if it is
    /// `AspirationEmitted { chain }`. Step 1 compares each
    /// aspiration's `chain_name` against this.
    in_flight_chain: Option<&'static str>,
}

/// Ticket 463 — colony-level Stores aggregate, computed once per
/// picker tick before the per-cat compute pass. Each entry pre-
/// aggregates one Stores building's `Vec<Entity>` of contained items
/// into a `HashMap<ItemKind, u32>` so the per-cat recipe-scoring
/// loop in `step2_craft_item_recipe_scoring` doesn't re-walk every
/// store entity. Stores positions are static, so the cost is
/// O(total_stored_items) once per tick (not once per cat).
struct StoresSnapshot {
    stores: Vec<(crate::components::physical::Position, ItemAggregate)>,
}

type ItemAggregate = std::collections::HashMap<crate::components::items::ItemKind, u32>;

/// Computed per-cat picker result — buffered before the mutation pass.
struct CatOutcome {
    entity: Entity,
    emissions: AspirationEmissions,
    traces: Vec<TraceEntry>,
}

/// Exclusive system. Authored per htn-methods.md §H.
///
/// Scheduling: added to the L1 chain in `src/plugins/simulation.rs`
/// as a sibling of `update_training_markers` /
/// `update_mentoring_target_markers`, before `evaluate_and_plan`. Per
/// memory `learning_bevy_schedule_edge_perturbation` the Chain-sibling
/// addition can perturb seed-42; at 321 land the picker is observably
/// no-op for any cat whose only milestone has empty `emits` and whose
/// chain has no Live method tagged with the chain's domain, so the
/// expected drift is bounded to (a) Hunting cats at "First Blood" (one
/// `Emit` row, `Live` `hunt_method`) and (b) cat-tick walking cost.
pub fn pick_aspiration_emissions(world: &mut World) {
    // -------- Snapshot pass --------
    let snapshot = collect_snapshot(world);
    let stores_snapshot = collect_stores_snapshot(world);

    // -------- Compute pass --------
    let tick = world.resource::<TimeState>().tick;
    let focal_entity = world
        .get_resource::<FocalTraceTarget>()
        .and_then(|f| f.entity);

    let mut outcomes: Vec<CatOutcome> = Vec::with_capacity(snapshot.len());
    for snap in &snapshot {
        let outcome = compute_outcome(world, snap, tick, focal_entity, &stores_snapshot);
        outcomes.push(outcome);
    }

    // -------- Mutation pass --------
    for outcome in outcomes {
        apply_outcome(world, outcome);
    }
}

/// Ticket 463 — collect the per-store ItemAggregate for the
/// CraftItemAspiration recipe-scoring loop. Walks every Stores
/// building's `StoredItems.items` vec and resolves each entity's
/// `Item.kind` into a per-store `HashMap<ItemKind, u32>`. Excludes
/// `BuildMaterialItem` entries (Stores carry only food / craft
/// inputs, not building materials).
fn collect_stores_snapshot(world: &mut World) -> StoresSnapshot {
    use crate::components::building::{StoredItems, Structure, StructureType};
    use crate::components::items::Item;
    use crate::components::physical::Position;
    // Materialize all `Item.kind` entries in one pass so the
    // per-store loop avoids re-fetching them per store.
    let mut item_kinds: std::collections::HashMap<Entity, crate::components::items::ItemKind> =
        std::collections::HashMap::new();
    {
        let mut q = world.query::<(Entity, &Item)>();
        for (entity, item) in q.iter(world) {
            item_kinds.insert(entity, item.kind);
        }
    }
    let mut stores: Vec<(Position, ItemAggregate)> = Vec::new();
    {
        let mut q = world.query_filtered::<(&Structure, &Position, &StoredItems), ()>();
        for (structure, pos, stored) in q.iter(world) {
            if structure.kind != StructureType::Stores {
                continue;
            }
            let mut agg: ItemAggregate = std::collections::HashMap::new();
            for &item_entity in &stored.items {
                if let Some(&kind) = item_kinds.get(&item_entity) {
                    *agg.entry(kind).or_insert(0) += 1;
                }
            }
            stores.push((*pos, agg));
        }
    }
    StoresSnapshot { stores }
}

fn collect_snapshot(world: &mut World) -> Vec<CatSnapshot> {
    let mut snapshot = Vec::new();
    let mut q = world
        .query_filtered::<(Entity, &Aspirations, Option<&HeldGoalStack>, &Name), Without<Dead>>();
    for (entity, asps, stack, name) in q.iter(world) {
        let in_flight_chain =
            stack
                .and_then(|s| s.frames.first())
                .and_then(|frame| match &frame.source {
                    IntentionSource::AspirationEmitted { chain } => Some(*chain),
                    _ => None,
                });
        snapshot.push(CatSnapshot {
            entity,
            name: name.0.clone(),
            active: asps.active.clone(),
            in_flight_chain,
        });
    }
    snapshot
}

fn compute_outcome(
    world: &World,
    snap: &CatSnapshot,
    tick: u64,
    focal_entity: Option<Entity>,
    stores_snapshot: &StoresSnapshot,
) -> CatOutcome {
    let mut emissions = AspirationEmissions::empty();
    let mut traces: Vec<TraceEntry> = Vec::new();
    let is_focal = focal_entity == Some(snap.entity);

    // Resolve the registry once per cat — the per-chain lookup is
    // hot (every active aspiration) and the registry is read-only.
    let aspiration_registry = world.resource::<AspirationRegistry>();

    for asp in &snap.active {
        let Some(chain) = aspiration_registry.chain_by_name(&asp.chain_name) else {
            // Stale chain name (chain retired after the cat adopted
            // it); skip. `check_aspiration_abandonment` will clean
            // the active list via the stagnation path eventually.
            continue;
        };
        if asp.current_milestone >= chain.milestones.len() {
            // Chain already complete; `track_milestones` will move it
            // to `completed` on its next pass. No emission.
            continue;
        }

        let already_in_flight = snap.in_flight_chain == Some(chain.name);

        let (row, walk, fallback_used) = if already_in_flight {
            // Step 1: skip emission, momentum carries the arc.
            (None, Vec::new(), false)
        } else if chain.name == crate::ai::aspirations::crafting::CRAFT_ITEM_ASPIRATION.name {
            // Ticket 463 — CraftItemAspiration bypasses the static
            // step-2 method-live gate. The chain's emit row carries a
            // sentinel label that no MethodRegistry entry matches; the
            // *real* emission picks a recipe per cat per tick and
            // attaches a dynamic `goal_kind = Some(HaveItem(_))` so the
            // L2 wrap (goap.rs:3192) constructs a typed Goal Intention
            // directly. The HTN frame-push special-case for HaveItem
            // (goap.rs ~3279) builds the 3-step craft plan
            // (RetrieveCraftInputs → TravelTo(station) → CraftAt).
            let (step_row, step_walk) =
                step2_craft_item_recipe_scoring(world, snap.entity, chain, asp, stores_snapshot);
            (step_row, step_walk, false)
        } else {
            // Step 2: emits walk.
            let (step2_row, step2_walk) = step2_emits_walk(world, snap.entity, chain, asp);
            if step2_row.is_some() {
                (step2_row, step2_walk, false)
            } else {
                // Step 3: domain-affinity fallback. Step-4 silent
                // quiet is the natural empty-Option fall-through.
                let (step3_row, step3_walk) = step3_domain_fallback(world, snap.entity, chain, asp);
                let fallback_used = step3_row.is_some();
                // Combine the two walks: step2's authored-emit rows
                // (empty when the milestone has no emits) followed
                // by step3's registry-walked domain-affinity
                // candidates. Concatenation preserves the "no
                // authored emit matched, so tried these" narrative
                // in the L1Aspiration trace record.
                let combined_walk = step2_walk.into_iter().chain(step3_walk).collect();
                (step3_row, combined_walk, fallback_used)
            }
        };

        if let Some(row) = row {
            emissions.rows.push(row);
        }

        if is_focal {
            traces.push(TraceEntry {
                tick,
                cat: snap.name.clone(),
                record: TraceRecord::L1Aspiration {
                    aspiration: asp.chain_name.clone(),
                    milestone: asp.current_milestone,
                    emit_walk: walk,
                    fallback_used,
                },
            });
        }
    }

    // 463 — implicit CRAFT_ITEM_ASPIRATION pass. The chain is always
    // available to every cat (no adoption needed) — analogous to
    // reactive emits, but driven by the per-cat recipe-scoring loop
    // rather than a marker-gated predicate. Skip if the cat already
    // has Crafting in its active list (the loop above already ran
    // it) or if the chain is in-flight (commitment momentum carries
    // it).
    let crafting_in_active = snap
        .active
        .iter()
        .any(|a| a.chain_name == crate::ai::aspirations::crafting::CRAFT_ITEM_ASPIRATION.name);
    let crafting_in_flight =
        snap.in_flight_chain == Some(crate::ai::aspirations::crafting::CRAFT_ITEM_ASPIRATION.name);
    if !crafting_in_active && !crafting_in_flight {
        // Synthesize an ActiveAspiration on the fly. The chain is
        // implicit — the cat's Aspirations Component doesn't carry
        // it. milestone 0, no real progress tracking yet (commit 7+
        // wires progress via ActionCount when adoption surface
        // matures).
        let synthetic_asp = ActiveAspiration {
            chain_name: crate::ai::aspirations::crafting::CRAFT_ITEM_ASPIRATION
                .name
                .to_string(),
            domain: crate::components::aspirations::AspirationDomain::Crafting,
            current_milestone: 0,
            progress: 0,
            adopted_tick: tick,
            last_progress_tick: tick,
            misaligned_since_tick: None,
        };
        let chain = &crate::ai::aspirations::crafting::CRAFT_ITEM_ASPIRATION;
        let (row, walk) = step2_craft_item_recipe_scoring(
            world,
            snap.entity,
            chain,
            &synthetic_asp,
            stores_snapshot,
        );
        if let Some(row) = row {
            emissions.rows.push(row);
        }
        if is_focal {
            traces.push(TraceEntry {
                tick,
                cat: snap.name.clone(),
                record: TraceRecord::L1Aspiration {
                    aspiration: synthetic_asp.chain_name,
                    milestone: 0,
                    emit_walk: walk,
                    fallback_used: false,
                },
            });
        }
    }

    // 364 step 1.5 — reactive emits. Marker-gated demands (active
    // dependent kitten, future mourning) that don't ride on an
    // aspiration milestone. Skip when ANY frame is in flight (today's
    // policy is non-preemptive — the cat finishes their current arc
    // before adopting a new one; revisit if welfare canaries degrade).
    if snap.in_flight_chain.is_none() {
        for reactive in REACTIVE_EMITS {
            if !(reactive.applicable_when)(world, snap.entity) {
                continue;
            }
            // Live method existence check — mirrors step 2's discipline.
            let method_registry = world.resource::<MethodRegistry>();
            if method_registry
                .lookup(reactive.label, world, snap.entity)
                .is_none()
            {
                continue;
            }
            emissions.rows.push(EmissionRow {
                chain: REACTIVE_CHAIN,
                milestone_index: 0, // reactive emits have no milestone
                label: reactive.label,
                strategy: reactive.strategy,
                priority: reactive.priority,
                fallback_used: false,
                goal_kind: None,
            });
            // First match wins per ordering — Primary reactive emits
            // dominate any later Secondary/Tertiary reactive entry.
            break;
        }
    }

    CatOutcome {
        entity: snap.entity,
        emissions,
        traces,
    }
}

/// Step 2 — walk the milestone's `emits` table in Priority order.
/// Returns the chosen `EmissionRow` (if any) plus the full walk for
/// trace emission.
fn step2_emits_walk(
    world: &World,
    entity: Entity,
    chain: &'static AspirationChain,
    asp: &ActiveAspiration,
) -> (Option<EmissionRow>, Vec<EmitWalkRow>) {
    let milestone = &chain.milestones[asp.current_milestone];
    let method_registry = world.resource::<MethodRegistry>();

    // Stable sort by priority, preserving registration order within
    // tier (`sort_by_key` is stable).
    let mut indexed: Vec<(usize, Emit)> = milestone.emits.iter().copied().enumerate().collect();
    indexed.sort_by_key(|(_, e)| e.priority as u8);

    let mut walk = Vec::with_capacity(indexed.len());
    let mut chosen: Option<EmissionRow> = None;

    for (_, emit) in indexed {
        let method_live = method_registry.lookup(emit.label, world, entity).is_some();
        let applicable = (emit.applicable_when)(world, entity);
        let emitted = chosen.is_none() && method_live && applicable;
        walk.push(EmitWalkRow {
            label: emit.label.to_string(),
            applicable,
            method_live,
            emitted,
        });
        if emitted {
            chosen = Some(EmissionRow {
                chain: chain.name,
                milestone_index: asp.current_milestone,
                label: emit.label,
                strategy: emit.strategy,
                priority: emit.priority,
                fallback_used: false,
                goal_kind: None,
            });
        }
    }

    (chosen, walk)
}

/// Step 3 — domain-affinity fallback. Walks the `MethodRegistry` for
/// any method whose `domain == Some(chain.domain)`. Returns the first
/// `Live`-and-applicable method as the chosen emission (or `None` when
/// no method matches — step 4 silent quiet). The second return value
/// is the full walk of every domain-matching registry entry, with
/// per-entry `method_live` / `applicable` / `emitted` fields — this
/// is the §11.5 registry-walk trace that #338 adds so
/// `L1Aspiration.emit_walk` is populated even for milestones with no
/// authored `emits[]` rows.
///
/// The fallback emits with `Priority::Tertiary` and
/// `CommitmentStrategy::OpenMinded` as a defensive default — the
/// fallback is the "we don't have a specific authored row, so
/// reach broadly toward the domain" path; `OpenMinded` matches the
/// aspiration layer's default per §7.7.
fn step3_domain_fallback(
    world: &World,
    entity: Entity,
    chain: &'static AspirationChain,
    asp: &ActiveAspiration,
) -> (Option<EmissionRow>, Vec<EmitWalkRow>) {
    let method_registry = world.resource::<MethodRegistry>();
    let mut walk: Vec<EmitWalkRow> = Vec::new();
    let mut chosen: Option<EmissionRow> = None;

    for method in method_registry.iter() {
        if method.domain != Some(chain.domain) {
            continue;
        }
        let method_live = matches!(&method.applicable_when, ApplicableWhen::Live(_));
        let applicable = match &method.applicable_when {
            ApplicableWhen::Live(check) => check(world, entity),
            ApplicableWhen::PendingSubstrate { .. } => false,
        };
        let emitted = chosen.is_none() && method_live && applicable;
        walk.push(EmitWalkRow {
            label: method.goal_label.to_string(),
            applicable,
            method_live,
            emitted,
        });
        if emitted {
            chosen = Some(EmissionRow {
                chain: chain.name,
                milestone_index: asp.current_milestone,
                label: method.goal_label,
                strategy: CommitmentStrategy::OpenMinded,
                priority: Priority::Tertiary,
                fallback_used: true,
                goal_kind: None,
            });
        }
    }

    (chosen, walk)
}

/// Ticket 463 — variation-gate scoring weights (commit 7 lift).
/// First-light (commit 6) shipped at W_THREAT=0.05, W_SKILL=0.05,
/// W_RECENT=1.0 to verify the L1 emit walk fires without perturbing
/// behavior. With the substrate verified (29405 L1Aspiration entries
/// on the seed-42 soak), commit 7 raises the weights so the
/// per-recipe softmax actually shapes the cat's recipe selection:
/// - W_THREAT rises to 0.30 so warrior's-kit recipes win under
///   recent-threat conditions.
/// - W_SKILL rises to 0.20 so the discipline-skill-affinity axis
///   becomes a meaningful tie-breaker between same-station recipes.
/// - W_RECENT drops to 0.50 so anti-monotony still pressures variety
///   but doesn't dominate the recent-craft penalty curve.
const CRAFT_ITEM_W_THREAT: f32 = 0.30;
const CRAFT_ITEM_W_SKILL: f32 = 0.20;
const CRAFT_ITEM_W_RECENT: f32 = 0.50;

/// Ticket 463 — Euclidean reach threshold (tile-domain) for a cat's
/// Stores. Cats within this distance of a Stores building can pull its
/// inputs into inventory via `RetrieveCraftInputs`. Matches the
/// proximity used by other Stores-walking resolvers (cooking / drying /
/// smoking). Ticket 492 switched from Manhattan to Euclidean.
const CRAFT_ITEM_STORES_REACH: f32 = 64.0;

/// Ticket 463 — per-cat recipe-scoring loop for `CraftItemAspiration`.
/// Walks every recipe in the registry, computes the per-cat reachable-
/// input aggregate once (inventory + nearby Stores), scores each
/// satisfiable recipe along three axes, and emits one
/// `EmissionRow { goal_kind: Some(GoalKind::HaveItem(winner.output)) }`.
/// Returns `(None, walk)` when no recipe is satisfiable for this cat.
///
/// Score formula (per the user's "downwind of the stat" framing):
///   score = W_threat * (recipe.is_warriors_kit as f32)
///                    * hide_recency_of_threat_cue(cat)
///         + W_skill  * recipe.discipline_skill_affinity
///                       .map(|s| 1.0 - s.value(skills))
///                       .unwrap_or(0.0)
///         - W_recent / (1.0 + ticks_since(recipe.id))
///
/// First-light weights are damped (Tertiary priority + small W's) so
/// the row doesn't dominate adoption. Commit 7 lifts after the
/// variation gate confirms emission.
fn step2_craft_item_recipe_scoring(
    world: &World,
    entity: Entity,
    chain: &'static AspirationChain,
    asp: &ActiveAspiration,
    stores_snapshot: &StoresSnapshot,
) -> (Option<EmissionRow>, Vec<EmitWalkRow>) {
    use crate::ai::dse::GoalKind;
    use crate::components::beliefs::{ContextBeliefs, PredatorBeliefs};
    use crate::components::items::ItemKind;
    use crate::components::magic::Inventory;
    use crate::components::physical::Position;
    use crate::components::recent_crafts::CatRecentCrafts;
    use crate::components::recipe::Recipe;
    use crate::components::skills::Skills;
    use crate::resources::recipe_registry::RecipeRegistry;

    let mut walk: Vec<EmitWalkRow> = Vec::new();

    let Some(pos) = world.get::<Position>(entity) else {
        return (None, walk);
    };
    let Some(inventory) = world.get::<Inventory>(entity) else {
        return (None, walk);
    };
    let Some(skills) = world.get::<Skills>(entity) else {
        return (None, walk);
    };
    let now = world.resource::<TimeState>().tick;
    let recipes = world.resource::<RecipeRegistry>();

    // Threat-cue scalar: max over PredatorBeliefs[nearest] +
    // ContextBeliefs[HereNow] recency_of_threat_cue facets. Mirrors
    // the read in ai::scoring.rs:563. Both Components carry
    // `models: HashMap<_, MentalModel>` and MentalModel has a
    // `recency_of_threat_cue: Facet` field; we read `.value`.
    let threat_cue = {
        let predator_max = world
            .get::<PredatorBeliefs>(entity)
            .map(|pb| {
                pb.models
                    .values()
                    .map(|m| m.recency_of_threat_cue.value)
                    .fold(0.0_f32, f32::max)
            })
            .unwrap_or(0.0);
        let context_now = world
            .get::<ContextBeliefs>(entity)
            .and_then(|cb| {
                cb.models
                    .get(&crate::components::beliefs::EnvironmentalContextKey::HereNow)
                    .map(|m| m.recency_of_threat_cue.value)
            })
            .unwrap_or(0.0);
        predator_max.max(context_now)
    };
    let recent_crafts = world.get::<CatRecentCrafts>(entity);

    // Build the cat's reachable-input aggregate ONCE (mirroring the
    // ticket-431 "default to event-driven, justify per-tick" pillar:
    // bounded ≈ O(stores × recipes × inputs) per cat per tick,
    // dwarfed by the picker's existing per-cat costs).
    let mut reachable: ItemAggregate = std::collections::HashMap::new();
    for slot in &inventory.pouch {
        *reachable.entry(slot.kind).or_insert(0) += 1;
    }
    for (store_pos, store_agg) in &stores_snapshot.stores {
        if pos.distance_to(store_pos) > CRAFT_ITEM_STORES_REACH {
            continue;
        }
        for (&kind, &count) in store_agg {
            *reachable.entry(kind).or_insert(0) += count;
        }
    }

    // Score each satisfiable recipe; track the winner. Filter to
    // recipes that `craft_have_item_actions` (commit 2) can actually
    // decompose — Workshop / TanningFrame only. Other stations
    // (Kitchen / DryingRack / SmokingRack / None) belong to their
    // own dispositions (Cooking / DryingFood / SmokingMeat / herbal
    // ward-placement) and aren't valid HaveItem aspiration targets.
    let mut best: Option<(&Recipe, f32)> = None;
    for recipe in recipes.iter() {
        let craft_decomposable = matches!(
            recipe.station,
            crate::components::recipe::StationRequirement::Workshop
                | crate::components::recipe::StationRequirement::TanningFrame
        );
        if !craft_decomposable {
            // Don't surface non-craft-decomposable recipes in the
            // walk — they aren't candidates the picker considers.
            continue;
        }
        if !recipe_inputs_reachable(recipe, &reachable) {
            walk.push(EmitWalkRow {
                label: recipe.id.0.to_string(),
                applicable: false,
                method_live: false,
                emitted: false,
            });
            continue;
        }
        let threat_term = CRAFT_ITEM_W_THREAT * (recipe.is_warriors_kit as i32 as f32) * threat_cue;
        let skill_term = CRAFT_ITEM_W_SKILL
            * recipe
                .discipline_skill_affinity
                .map(|s| (1.0 - s.value(skills)).max(0.0))
                .unwrap_or(0.0);
        let recent_penalty = match recent_crafts.and_then(|rc| rc.ticks_since(recipe.id, now)) {
            Some(elapsed) => CRAFT_ITEM_W_RECENT / (1.0 + elapsed as f32),
            None => 0.0,
        };
        let score = threat_term + skill_term - recent_penalty;
        walk.push(EmitWalkRow {
            label: recipe.id.0.to_string(),
            applicable: true,
            method_live: true,
            emitted: false, // patched after the winner pass
        });
        match best {
            Some((_, bs)) if score <= bs => {}
            _ => best = Some((recipe, score)),
        }
    }

    let Some((winner, _)) = best else {
        return (None, walk);
    };
    // Patch the walk to mark the winner's row `emitted: true`. Linear
    // scan over `recipes.iter()` order; cheap (≤ 20 entries).
    if let Some(row) = walk.iter_mut().find(|r| r.label == winner.id.0) {
        row.emitted = true;
    }
    // Sentinel label for the EmissionRow; the L2 wrap reads
    // goal_kind directly and derives the trace label from ItemKind.
    let row = EmissionRow {
        chain: chain.name,
        milestone_index: asp.current_milestone,
        label: chain.milestones[asp.current_milestone].emits[0].label,
        strategy: chain.milestones[asp.current_milestone].emits[0].strategy,
        priority: chain.milestones[asp.current_milestone].emits[0].priority,
        fallback_used: false,
        goal_kind: Some(GoalKind::HaveItem(winner.output.item_kind)),
    };
    let _ = ItemKind::Bone; // touch the import so unused-warning stays clean
    (Some(row), walk)
}

/// Helper: returns `true` iff every `RecipeInput { kind, count }` is
/// present in the reachable-aggregate at sufficient count. Same shape
/// as `recipe_inputs_satisfied` in `craft_at_workshop.rs` but reads
/// the aggregated map rather than a `Vec<Slot>`.
fn recipe_inputs_reachable(
    recipe: &crate::components::recipe::Recipe,
    reachable: &ItemAggregate,
) -> bool {
    for input in &recipe.inputs {
        if reachable.get(&input.kind).copied().unwrap_or(0) < input.count {
            return false;
        }
    }
    true
}

fn apply_outcome(world: &mut World, outcome: CatOutcome) {
    // Insert/remove the AspirationEmissions Component.
    if outcome.emissions.rows.is_empty() {
        if let Ok(mut e) = world.get_entity_mut(outcome.entity) {
            e.remove::<AspirationEmissions>();
        }
    } else if let Ok(mut e) = world.get_entity_mut(outcome.entity) {
        e.insert(outcome.emissions);
    }
    // Push trace records (gated on FocalTraceTarget upstream, so
    // `outcome.traces` is empty for non-focal cats).
    if !outcome.traces.is_empty() {
        let mut log = world.resource_mut::<TraceLog>();
        for entry in outcome.traces {
            log.push(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reactive_emits_table_contains_kitten_reared() {
        // 364 — the kitten_reared reactive emit is the only registered
        // entry at 364 land. process_grief follows when §7.7.b ships.
        assert_eq!(REACTIVE_EMITS.len(), 1);
        assert_eq!(REACTIVE_EMITS[0].label, "kitten_reared");
        assert_eq!(REACTIVE_EMITS[0].priority, Priority::Primary);
        assert_eq!(REACTIVE_EMITS[0].strategy, CommitmentStrategy::SingleMinded);
    }

    #[test]
    fn reactive_chain_sentinel_is_stable() {
        // The sentinel string round-trips through
        // `IntentionSource::AspirationEmitted { chain }`. Stable constant
        // so the in-flight check (snap.in_flight_chain == REACTIVE_CHAIN)
        // matches frames authored from a reactive emit.
        assert_eq!(REACTIVE_CHAIN, "<reactive>");
    }
}
