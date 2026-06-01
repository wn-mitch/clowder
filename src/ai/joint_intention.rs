//! Ticket 127 — JointIntention author system. Commit B.
//!
//! # Commit B — full author with stage progression + cascade
//!
//! Per-tick maintenance of `JointIntention { practice: Courtship, .. }`:
//!
//! 1. **Pre-tick partner snapshot.** Build
//!    `HashMap<Entity, (PracticeKind, Entity)>` from every existing JI.
//!    The snapshot is the pre-flush state — partner-removal happens via
//!    buffered `Commands` so cascade detection lags by one tick (cat A
//!    drops on tick T → cat B's snapshot still has A's JI on tick T →
//!    cat B drops on tick T+1 with `PartnerLeftPractice`). This meets
//!    the §Exit criterion 3 "within 1 tick" budget.
//!
//! 2. **Pre-tick interaction message drain.** Read
//!    [`JointInteractionObserved`] messages emitted by bias-reader call
//!    sites on the prior tick. For each message, bump the cat's
//!    `JointIntention.last_interaction_tick` to the message tick. This
//!    is the structural feed for the `Approach → Courting` stage-
//!    advance predicate.
//!
//! 3. **Per-cat drop + stage advance.** Build proxies and evaluate
//!    [`should_drop_joint`] / [`next_stage`]; remove the JI on drop
//!    and advance stage on transition. Fires `JointIntentionDropped`
//!    or `JointStageAdvanced` respectively.
//!
//! 4. **Mismatch-tick accrual.** Per pair, the lower-Entity-index side
//!    compares its own stage to partner's; on mismatch fires
//!    `JointStageMismatchTickAccrued { practice }`. Lower-index reports
//!    to avoid double-counting symmetric pairs.
//!
//! 5. **Pass 3 — mirror PA emission.** For each cat with
//!    `PairingActivity` but no `JointIntention` (PA author just emitted
//!    in the same tick), insert a fresh JI starting at
//!    `PracticeStage::CourtshipApproach`. Fire
//!    `JointIntentionEmitted { practice }`. **Once Commit C deletes PA,
//!    this pass is replaced by a full matchmaker** — for now we
//!    re-use PA's matchmaker via the lockstep mirror, which preserves
//!    migration parity mechanically.
//!
//! # Schedule
//!
//! Registered after `author_pairing_intentions` on the same chain so
//! the apply_deferred boundary makes Pass 3 see the freshly-inserted
//! PAs from this tick. Reads `MessageReader<JointInteractionObserved>`
//! to consume bias-reader interaction events from the prior tick (Bevy
//! Messages buffer between system runs; read order is intra-tick LIFO
//! against write order, which doesn't matter for our tick-bump
//! semantics).

use bevy_ecs::prelude::*;

use crate::ai::mating::{MatingFitness, MatingFitnessParams};
use crate::ai::{Action, CurrentAction};
use crate::components::identity::{Age, Gender, LifeStage, Name, Orientation};
use crate::components::joint_intention::{
    next_stage, should_drop_joint, JointDropBranch, JointIntention, JointIntentionDropConfig,
    JointIntentionProxies, PracticeKind, PracticeStage, StageAdvanceProxies,
};
use crate::components::mental::{Mood, MoodModifier, MoodSource};
use crate::components::personality::Personality;
use crate::components::physical::Dead;
use crate::components::physical::{Needs, Position};
use crate::components::pregnancy::Pregnant;
use crate::resources::event_log::{EventKind, EventLog};
use crate::resources::map::TileMap;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::narrative_templates::{
    emit_event_narrative, MoodBucket, TemplateContext, TemplateRegistry, VariableContext,
};
use crate::resources::relationships::{BondType, Relationships};
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::{
    CourtshipPracticeConstants, PlayBoutPracticeConstants, SimConstants,
};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{DayPhase, Season, SimConfig, TimeState};
use crate::resources::weather::WeatherState;
use crate::systems::mood::patience_extend;
use crate::systems::social::are_orientation_compatible;
use std::collections::{HashMap, HashSet};

/// Ticket 127 — Bias-reader call sites emit this when their resolver
/// target matches the actor's `JointIntention { Courtship }.partner`.
/// The author system reads the batch on the following tick and bumps
/// `JointIntention.last_interaction_tick` to the message tick, which
/// gates the `Approach → Courting` stage advance.
///
/// Decouples reads from writes — bias readers iterate `&JointIntention`
/// queries; the per-tick `last_interaction_tick` bump happens via
/// `&mut JointIntention` here. Bevy Messages are buffered between
/// system runs, so the bump lags by one tick.
#[derive(Message, Debug, Clone, Copy)]
pub struct JointInteractionObserved {
    pub entity: Entity,
    pub partner: Entity,
    pub practice: PracticeKind,
    pub tick: u64,
}

/// Ticket 276 Commit B — emitted by `author_joint_intentions` when a
/// PlayBout JointIntention transitions from `PlayBoutApproach` into
/// `PlayBoutBouting`. Drives [`cascade_play_bout_bouting`], which
/// applies the Bouting-stage mood-lift to nearby witnesses and emits
/// the play_social narrative — the substrate replacement for the legacy
/// `on_play_initiated` observer's cascade.
///
/// Emitted from the lower-`Entity::index()` side only, mirroring the
/// mismatch-tracking convention, so symmetric stage transitions don't
/// double-fire the cascade.
#[derive(Message, Debug, Clone, Copy)]
pub struct PlayBoutBoutingEntered {
    pub actor: Entity,
    pub partner: Entity,
    pub tick: u64,
}

/// Bundled message channels so `author_joint_intentions` stays under
/// Bevy's 16-param SystemParam ceiling. Reads `JointInteractionObserved`
/// (Pass 0 last_interaction_tick bump) and writes `PlayBoutBoutingEntered`
/// (ticket 276 Commit B, stage_advances loop).
#[derive(bevy_ecs::system::SystemParam)]
pub struct JointMessageStreams<'w, 's> {
    pub interactions: MessageReader<'w, 's, JointInteractionObserved>,
    pub bouting_entered: MessageWriter<'w, PlayBoutBoutingEntered>,
}

/// Ticket 127 Commit B — full author / drop / stage-advance / cascade
/// system. Replaces the Commit A `mirror_joint_intentions` mirror.
///
/// See module docs for the per-tick contract.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn author_joint_intentions(
    mut commands: Commands,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    constants: Res<SimConstants>,
    relationships: Res<Relationships>,
    mut activation: ResMut<SystemActivation>,
    mut messages: JointMessageStreams,
    mut event_log: Option<ResMut<EventLog>>,
    mating: MatingFitnessParams,
    // Mutable JI query — needed for `last_interaction_tick` bump and
    // stage advance. Disjoint from `Without<JointIntention>` query below
    // via marker disjointness.
    mut joints: Query<(Entity, &mut JointIntention), Without<Dead>>,
    // Eligible cats lacking a JointIntention — Pass 3 matchmaker
    // candidates. Carries position for range filter.
    needs_emit: Query<(Entity, &Position), (Without<JointIntention>, Without<Dead>)>,
    // All-cat positions for matchmaker peer scan (includes JI-holding
    // cats so the matchmaker can re-pair after a drop+rematch on the
    // same tick).
    all_positions: Query<(Entity, &Position), Without<Dead>>,
    // Partner-validity probe used by `PartnerInvalid` branch.
    invalidity: Query<(
        Has<Dead>,
        Has<crate::components::markers::Banished>,
        Has<crate::components::markers::Incapacitated>,
    )>,
    pregnant_q: Query<(), With<Pregnant>>,
    // Ticket 276 — PlayBout matchmaker reads playfulness + mood +
    // current action + name (for event emit on completion). Dead cats
    // get filtered at iteration time.
    names: Query<&Name>,
    playbout_q: Query<(Entity, &Personality, &Mood, Option<&CurrentAction>), Without<Dead>>,
) {
    let now_tick = time.tick;
    let season = time.season(&config);
    let drop_config = JointIntentionDropConfig {
        romantic_floor: constants.practices.courtship.romantic_floor,
        fondness_floor: constants.practices.courtship.fondness_floor,
        stage_stall_ticks: constants.practices.courtship.stage_stall_ticks,
        playbout_cooldown_ticks: constants.practices.play_bout.cooldown_duration_ticks,
    };
    let playbout_approach_duration_ticks = constants.practices.play_bout.approach_duration_ticks;
    let playbout_bouting_duration_ticks = constants.practices.play_bout.bouting_duration_ticks;

    // -----------------------------------------------------------------
    // Pass 0: drain interaction messages → bump last_interaction_tick.
    //
    // Ticket 276 removed the Courtship-only filter — PlayBout
    // interactions also drive `Approach → Bouting`. The author's drop
    // gate still gates per-practice on `joint.practice`, so a stray
    // message would only bump `last_interaction_tick` on a JI whose
    // actor matched both message and component.
    // -----------------------------------------------------------------
    let mut interaction_tick_for: HashMap<Entity, u64> = HashMap::new();
    for msg in messages.interactions.read() {
        // Keep the latest tick per entity, regardless of practice — the
        // bump applies whether the JI is Courtship or PlayBout.
        let entry = interaction_tick_for.entry(msg.entity).or_insert(msg.tick);
        if msg.tick > *entry {
            *entry = msg.tick;
        }
    }
    for (entity, mut joint) in joints.iter_mut() {
        if let Some(&tick) = interaction_tick_for.get(&entity) {
            if tick > joint.last_interaction_tick {
                joint.last_interaction_tick = tick;
            }
        }
    }

    // -----------------------------------------------------------------
    // Pass 1: partner-state snapshot (post-Pass-0, pre-drop).
    // -----------------------------------------------------------------
    let partner_snapshot: HashMap<Entity, (PracticeKind, Entity)> = joints
        .iter()
        .map(|(e, j)| (e, (j.practice, j.partner)))
        .collect();
    let pregnant_snapshot: HashMap<Entity, bool> = joints
        .iter()
        .map(|(e, j)| (e, pregnant_q.get(j.partner).is_ok()))
        .collect();
    // Self-pregnant snapshot — used by Approach→Mating→Bonded gate.
    let self_pregnant_snapshot: HashMap<Entity, bool> = joints
        .iter()
        .map(|(e, _)| (e, pregnant_q.get(e).is_ok()))
        .collect();

    // -----------------------------------------------------------------
    // Pass 2: per-cat drop + stage advance + mismatch tracking.
    //
    // Snapshot of (entity, joint-copy) so we can use `commands` for
    // removal without aliasing the mutable JI query. Stage-advance
    // mutations write back through `joints` after the drop decisions
    // are made.
    // -----------------------------------------------------------------
    let fitness = mating.snapshot();
    // Ticket 276 — drop decisions carry the branch + practice so the
    // mutation pass can emit `EventKind::JointPlayBoutCompleted` when
    // PlayBout cooldown elapses (the canary's continuity-tally site).
    let mut drop_decisions: Vec<(Entity, PracticeKind, Entity, Option<JointDropBranch>)> =
        Vec::new();
    let mut stage_advances: Vec<(Entity, PracticeKind, PracticeStage)> = Vec::new();
    let mut mismatch_emissions: Vec<PracticeKind> = Vec::new();

    for (entity, joint) in joints.iter() {
        let Some(self_fit) = fitness.get(&entity).copied() else {
            // Cat is dead / not in fitness snapshot — drop defensively
            // without a branch (no event emit). Practice is recorded so
            // the per-practice Feature still attributes correctly.
            drop_decisions.push((entity, joint.practice, joint.partner, None));
            continue;
        };

        // Partner-validity probe.
        let partner_invalid = match invalidity.get(joint.partner) {
            Ok((dead, banished, incapacitated)) => dead || banished || incapacitated,
            Err(_) => true, // Despawned.
        };
        let bond = relationships
            .get(entity, joint.partner)
            .and_then(|r| r.bond);
        let (romantic, fondness) = relationships
            .get(entity, joint.partner)
            .map(|r| (r.romantic, r.fondness))
            .unwrap_or((0.0, 0.0));
        let self_is_pregnant = self_pregnant_snapshot
            .get(&entity)
            .copied()
            .unwrap_or(false)
            || self_fit.is_pregnant;
        let partner_is_pregnant = pregnant_snapshot.get(&entity).copied().unwrap_or(false);
        // Partner-in-practice cascade detection. Snapshot is pre-Pass-2,
        // so a partner who hasn't yet been removed appears in the
        // snapshot — the cascade fires on the FOLLOWING tick.
        let partner_in_practice = partner_snapshot
            .get(&joint.partner)
            .is_some_and(|(kind, p_partner)| *kind == joint.practice && *p_partner == entity);

        let proxies = JointIntentionProxies {
            self_stage: self_fit.stage,
            self_orientation: self_fit.orientation,
            self_gender: self_fit.gender,
            self_is_pregnant,
            self_fertility_phase: self_fit.fertility_phase,
            partner_invalid,
            partner_in_practice,
            partner_is_pregnant,
            bond,
            romantic,
            fondness,
            season,
            practice: joint.practice,
            current_stage: joint.stage,
            stage_entered_tick: joint.stage_entered_tick,
            now_tick,
            still_compatible: is_practice_compatible_now(
                joint.practice,
                self_fit.stage,
                self_fit.orientation,
                self_is_pregnant,
            ),
        };

        if let Some(branch) = should_drop_joint(&proxies, &drop_config) {
            drop_decisions.push((entity, joint.practice, joint.partner, Some(branch)));
            continue;
        }

        // Stage advance — only when drop didn't fire.
        let stage_proxies = StageAdvanceProxies {
            current_stage: joint.stage,
            last_interaction_tick: joint.last_interaction_tick,
            adopted_tick: joint.adopted_tick,
            bond,
            self_gender: self_fit.gender,
            season,
            self_fertility_phase: self_fit.fertility_phase,
            self_is_pregnant,
            partner_is_pregnant,
            now_tick,
            stage_entered_tick: joint.stage_entered_tick,
            playbout_approach_duration_ticks,
            playbout_bouting_duration_ticks,
        };
        if let Some(new_stage) = next_stage(&stage_proxies) {
            stage_advances.push((entity, joint.practice, new_stage));
        }

        // Mismatch tracking — lower-Entity-index side reports to avoid
        // double-counting. Only when partner still has a JI and stages
        // differ.
        if let Some((p_kind, p_partner_entity)) = partner_snapshot.get(&joint.partner) {
            if *p_kind == joint.partner_practice() && *p_partner_entity == entity {
                // Look up partner's stage from the joints query.
                if let Some((_, partner_joint)) = joints.iter().find(|(pe, _)| *pe == joint.partner)
                {
                    if entity.index() < joint.partner.index() && partner_joint.stage != joint.stage
                    {
                        mismatch_emissions.push(joint.practice);
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------
    // Apply mutations.
    //
    // Ticket 276 — when a PlayBout drops on `JointDropBranch::Completed`
    // (Cooldown elapsed), emit `EventKind::JointPlayBoutCompleted` to
    // increment `continuity_tallies["play"]`. The event is the canary's
    // structural-replacement for the legacy `EventKind::PlayFired`
    // direct-emit (`personality_events.rs:80-90`); both feed the same
    // tally key during migration.
    // -----------------------------------------------------------------
    for (entity, practice, partner, branch) in drop_decisions {
        commands.entity(entity).remove::<JointIntention>();
        activation.record(Feature::JointIntentionDropped { practice });
        if let Some(JointDropBranch::Completed) = branch {
            if practice == PracticeKind::PlayBout {
                if let Some(ref mut events) = event_log {
                    let actor = names
                        .get(entity)
                        .map(|n| n.0.clone())
                        .unwrap_or_else(|_| "<unknown>".to_string());
                    let partner_name = names
                        .get(partner)
                        .map(|n| n.0.clone())
                        .unwrap_or_else(|_| "<unknown>".to_string());
                    events.push(
                        now_tick,
                        EventKind::JointPlayBoutCompleted {
                            actor,
                            partner: partner_name,
                        },
                    );
                }
            }
        }
    }
    for (entity, practice, new_stage) in stage_advances {
        if let Ok((_, mut joint)) = joints.get_mut(entity) {
            joint.stage = new_stage;
            joint.stage_entered_tick = now_tick;
            activation.record(Feature::JointStageAdvanced { practice });
            // Ticket 276 Commit B — PlayBout Approach→Bouting is the
            // substrate equivalent of "play just began". Emit the
            // Bouting-entry message so `cascade_play_bout_bouting`
            // applies the mood-lift + narrative cascade migrated from
            // the legacy `on_play_initiated` observer.
            //
            // Lower-`Entity::index()` side emits — mirrors the
            // mismatch-tracking convention above so symmetric pair
            // transitions don't double-fire the cascade.
            if practice == PracticeKind::PlayBout
                && new_stage == PracticeStage::PlayBoutBouting
                && entity.index() < joint.partner.index()
            {
                messages.bouting_entered.write(PlayBoutBoutingEntered {
                    actor: entity,
                    partner: joint.partner,
                    tick: now_tick,
                });
            }
        }
    }
    for practice in mismatch_emissions {
        activation.record(Feature::JointStageMismatchTickAccrued { practice });
    }

    // -----------------------------------------------------------------
    // Pass 3: matchmaker — emit JointIntention for eligible cats
    // lacking one.
    //
    // Two-phase: Courtship picks first (its eligibility is narrower —
    // gated on fertility / orientation / reproductive life-stage), then
    // PlayBout picks from remaining cats. Each cat emits at most one
    // JI per tick (a cat can only hold one JointIntention Component);
    // the `claimed_this_tick` set tracks Courtship picks so PlayBout
    // doesn't try to pair a Courtship-just-matched cat.
    //
    // Order matters: Courtship is the higher-stakes practice; PlayBout
    // is the background social practice. Picking PlayBout first would
    // starve Courtship of candidates.
    // -----------------------------------------------------------------
    let courtship_constants = &constants.practices.courtship;
    let playbout_constants = &constants.practices.play_bout;
    let positions: Vec<(Entity, Position)> = all_positions.iter().map(|(e, p)| (e, *p)).collect();
    let mut claimed_this_tick: HashSet<Entity> = HashSet::new();

    // ---------- Courtship pass ----------
    for (entity, position) in needs_emit.iter() {
        let Some(self_fit) = fitness.get(&entity).copied() else {
            continue;
        };
        if !is_reproductive_for_courtship(&self_fit) {
            continue;
        }
        let Some(partner) = pick_courtship_partner(
            entity,
            position,
            self_fit,
            &positions,
            &fitness,
            &relationships,
            courtship_constants,
        ) else {
            continue;
        };

        commands.entity(entity).insert(JointIntention::new(
            PracticeKind::Courtship,
            partner,
            now_tick,
        ));
        activation.record(Feature::JointIntentionEmitted {
            practice: PracticeKind::Courtship,
        });
        claimed_this_tick.insert(entity);
    }

    // ---------- PlayBout pass ----------
    //
    // Ticket 276 — PlayBout matchmaker. Eligibility: both cats
    // `Personality.playfulness > playfulness_floor`, both
    // `Mood.valence > mood_valence_floor`, both current action is
    // `Socialize` / `Idle` / `Wander` (light-bandwidth coexistence),
    // both alive and within `candidate_range` tiles.
    //
    // Per CLAUDE.md design pillar #2, this substrate replaces the
    // direct-emit at `personality_events.rs:80-90` (the four-AND × RNG
    // gate that collapsed to 0–13 play events/soak post-066). Hosting
    // the canary on JointIntention means "playing together" becomes
    // mutually-public practice state rather than a softmax coincidence.
    let mut playbout_q_cache: HashMap<Entity, (f32, f32, Action)> = HashMap::new();
    for (entity, personality, mood, current_action) in playbout_q.iter() {
        playbout_q_cache.insert(
            entity,
            (
                personality.playfulness,
                mood.valence,
                current_action.map(|c| c.action).unwrap_or(Action::Idle),
            ),
        );
    }

    for (entity, position) in needs_emit.iter() {
        if claimed_this_tick.contains(&entity) {
            continue;
        }
        let Some(&(self_play, self_mood, self_action)) = playbout_q_cache.get(&entity) else {
            continue;
        };
        if !is_playbout_eligible(self_play, self_mood, self_action, playbout_constants) {
            continue;
        }
        let Some(partner) = pick_playbout_partner(
            entity,
            position,
            &positions,
            &playbout_q_cache,
            &claimed_this_tick,
            playbout_constants,
        ) else {
            continue;
        };

        commands.entity(entity).insert(JointIntention::new(
            PracticeKind::PlayBout,
            partner,
            now_tick,
        ));
        activation.record(Feature::JointIntentionEmitted {
            practice: PracticeKind::PlayBout,
        });
        // Intentionally NOT adding `entity` to `claimed_this_tick`.
        // Mirrors Courtship's symmetric matchmaker (both partners
        // iterate independently and pick each other when each is the
        // other's best match). Self-claiming would force asymmetric
        // pairing (A.JI=B but B.JI=C), and the `PartnerLeftPractice`
        // cascade would drop every fresh JI on tick T+1. The set
        // remains used for cross-practice exclusion: Courtship-matched
        // cats in `claimed_this_tick` are skipped by both the PlayBout
        // top-loop guard and `pick_playbout_partner`'s candidate
        // filter.
    }
}

// ---------------------------------------------------------------------------
// PlayBoutBouting cascade (ticket 276 Commit B)
// ---------------------------------------------------------------------------

/// Ticket 276 Commit B — drains [`PlayBoutBoutingEntered`] messages and
/// applies the Bouting-stage cascade: a +0.1 / 15-tick Social mood-lift
/// to every non-dead cat within manhattan-4 of the actor (the legacy
/// `on_play_initiated` cascade's bystander reach), plus a
/// template-driven `play_social` narrative entry naming actor + partner.
///
/// **Replaces** the legacy `on_play_initiated` observer that fired at
/// PlayInitiated time on the four-AND × RNG·0.1 gate. The substrate
/// equivalent of "play just began" is the `PlayBoutApproach →
/// PlayBoutBouting` transition; the cascade fires once per bout from
/// the lower-`Entity::index()` side (per the message emit guard in
/// `author_joint_intentions`'s stage_advances loop), preserving the
/// legacy one-fire-per-bout shape.
///
/// Note: `EventKind::JointPlayBoutCompleted` is emitted separately from
/// `author_joint_intentions`'s drop loop at `JointDropBranch::Completed`
/// (Cooldown elapsed); that's the canary tally site. The cascade here
/// only handles the cosmetic / mood side-effects.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn cascade_play_bout_bouting(
    mut messages: MessageReader<PlayBoutBoutingEntered>,
    cats: Query<(Entity, &Position, &Personality, &Name, &Gender, &Age), Without<Dead>>,
    needs_q: Query<&Needs, Without<Dead>>,
    mut moods: Query<&mut Mood, Without<Dead>>,
    mut narrative_log: ResMut<NarrativeLog>,
    config: Res<SimConfig>,
    constants: Res<SimConstants>,
    weather: Res<WeatherState>,
    map: Res<TileMap>,
    registry: Option<Res<TemplateRegistry>>,
    mut rng: ResMut<SimRng>,
) {
    for msg in messages.read() {
        let Ok((_, actor_pos, actor_pers, actor_name, actor_gender, actor_age)) =
            cats.get(msg.actor)
        else {
            // Actor died / despawned between stage_advance and cascade
            // drain (cross-system race). Skip silently.
            continue;
        };
        let actor_pos = *actor_pos;
        let actor_name_str = actor_name.0.clone();
        let actor_gender = *actor_gender;
        let life_stage = actor_age.stage(msg.tick, config.ticks_per_season);

        // Mood-lift sweep: every non-dead cat within manhattan-4 of the
        // actor (including the partner; legacy observer's behavior).
        // Collect (entity, patience) under the read-only `cats` query
        // then push modifiers via the disjoint `moods` write query —
        // single Mood query avoids the B0001 conflict that would arise
        // from holding `&Mood` and `&mut Mood` in overlapping queries.
        let mut nearby: Vec<(Entity, f32)> = Vec::new();
        for (other, other_pos, other_pers, _, _, _) in cats.iter() {
            if other == msg.actor {
                continue;
            }
            if actor_pos.manhattan_distance(other_pos) > 4 {
                continue;
            }
            nearby.push((other, other_pers.patience));
        }
        for (other, patience) in nearby {
            if let Ok(mut other_mood) = moods.get_mut(other) {
                let mut modifier =
                    MoodModifier::new(0.1, 15, "watched play nearby").with_kind(MoodSource::Social);
                patience_extend(&mut modifier, patience, &constants.mood);
                other_mood.modifiers.push_back(modifier);
            }
        }

        // Partner name for the narrative — JointIntention guarantees a
        // partner, but the partner may have despawned. Fall back to a
        // placeholder so the template still resolves.
        let partner_name = cats
            .get(msg.partner)
            .map(|(_, _, _, n, _, _)| n.0.clone())
            .unwrap_or_else(|_| "their playmate".to_string());

        // Template context — matches the legacy observer's
        // `play_social` event tag (JointIntention guarantees a partner;
        // there is no solo branch).
        let day_phase = DayPhase::from_tick(msg.tick, &config);
        let season = Season::from_tick(msg.tick, &config);
        let terrain = if map.in_bounds(actor_pos.x(), actor_pos.y()) {
            map.get(actor_pos.x(), actor_pos.y()).terrain
        } else {
            crate::resources::map::Terrain::Grass
        };
        let mood_bucket = moods
            .get(msg.actor)
            .map(|m| MoodBucket::from_valence(m.valence))
            .unwrap_or(MoodBucket::Neutral);
        let needs = needs_q
            .get(msg.actor)
            .cloned()
            .unwrap_or_else(|_| Needs::default());

        let ctx = TemplateContext {
            action: Action::Socialize,
            day_phase,
            season,
            weather: weather.current,
            mood_bucket,
            life_stage,
            has_target: true,
            terrain,
            event: Some("play_social".into()),
        };
        let var_ctx = VariableContext {
            name: &actor_name_str,
            gender: actor_gender,
            weather: weather.current,
            day_phase,
            season,
            life_stage,
            fur_color: "unknown",
            other: Some(&partner_name),
            prey: None,
            item: None,
            item_singular: None,
            quality: None,
        };

        let fallback =
            format!("A game breaks out. {actor_name_str} bats a pinecone toward {partner_name}.");

        emit_event_narrative(
            registry.as_deref(),
            &mut narrative_log,
            msg.tick,
            fallback,
            NarrativeTier::Action,
            &ctx,
            &var_ctx,
            actor_pers,
            &needs,
            &mut rng.rng,
        );
    }
}

// ---------------------------------------------------------------------------
// Courtship matchmaker helpers (moved from `crate::ai::pairing` in 127 Commit C)
// ---------------------------------------------------------------------------

/// L1-equivalent reproductive-eligibility predicate for Courtship.
/// Mirrors the prior `crate::ai::pairing::is_reproductive` 1:1.
fn is_reproductive_for_courtship(f: &MatingFitness) -> bool {
    matches!(
        f.stage,
        crate::components::identity::LifeStage::Adult
            | crate::components::identity::LifeStage::Elder
    ) && f.orientation != crate::components::identity::Orientation::Asexual
        && !f.is_pregnant
}

/// Map a bond tier to a graduated scalar for the courtship-quality
/// score. Mirrors `socialize_target::bond_score`'s vocabulary so the
/// L2 emission decision uses the same scale the bias readers do.
fn bond_tier_score(bond: Option<BondType>) -> f32 {
    match bond {
        Some(BondType::Mates | BondType::Partners) => 1.0,
        Some(BondType::Friends) => 0.5,
        None => 0.0,
    }
}

/// Pick the best Courtship partner for `self_entity` from within
/// `candidate_range`. Returns `None` when no candidate clears
/// `emission_threshold`. Stable tie-break via `Entity::index()` asc.
///
/// Moved from `crate::ai::pairing::pick_partner` in 127 Commit C.
/// Future practices declare their own matchmaker via per-practice
/// dispatch.
fn pick_courtship_partner(
    self_entity: Entity,
    self_position: &Position,
    self_fit: MatingFitness,
    positions: &[(Entity, Position)],
    fitness: &HashMap<Entity, MatingFitness>,
    relationships: &Relationships,
    practice_constants: &CourtshipPracticeConstants,
) -> Option<Entity> {
    // Ticket 453: Mates-exclusivity perception gate. A cat already in a
    // `BondType::Mates` bond does not emit new Courtship JointIntentions
    // toward third parties under the current substrate shape.
    if relationships
        .iter_for(self_entity)
        .any(|(_, rel)| rel.bond == Some(BondType::Mates))
    {
        return None;
    }

    let range = practice_constants.candidate_range;
    let weights = (
        practice_constants.quality_fondness_weight,
        practice_constants.quality_romantic_weight,
        practice_constants.quality_bond_weight,
    );

    let mut best: Option<(Entity, f32)> = None;
    for (other, other_pos) in positions.iter() {
        if *other == self_entity {
            continue;
        }
        let manhattan =
            (self_position.x() - other_pos.x()).abs() + (self_position.y() - other_pos.y()).abs();
        if manhattan > range {
            continue;
        }
        let Some(other_fit) = fitness.get(other) else {
            continue;
        };
        if !is_reproductive_for_courtship(other_fit) {
            continue;
        }
        if !are_orientation_compatible(
            self_fit.gender,
            self_fit.orientation,
            other_fit.gender,
            other_fit.orientation,
        ) {
            continue;
        }
        let Some(rel) = relationships.get(self_entity, *other) else {
            continue;
        };
        let bond_score = bond_tier_score(rel.bond);
        if bond_score == 0.0 {
            continue;
        }
        // Ticket 453: skip candidates already Mates-bonded to a third
        // party. (Self-Mates already excluded by the early return above.)
        if relationships.iter_for(*other).any(|(third, third_rel)| {
            third != self_entity && third_rel.bond == Some(BondType::Mates)
        }) {
            continue;
        }
        let fondness = rel.fondness.max(0.0);
        let romantic = rel.romantic.max(0.0);
        let score = weights.0 * fondness + weights.1 * romantic + weights.2 * bond_score;
        if score < practice_constants.emission_threshold {
            continue;
        }
        let candidate = (*other, score);
        match best {
            Some((_, best_score)) if best_score >= score => {}
            _ => best = Some(candidate),
        }
    }
    best.map(|(e, _)| e)
}

/// Inline practice-compatibility recheck. Mirrors the eligibility
/// predicate `crate::ai::pairing::is_reproductive` for Courtship:
/// life-stage Adult/Elder + non-Asexual + not pregnant.
///
/// Distinct from the `AspirationCascade` drop branch which fires on
/// `self_stage / self_orientation / self_is_pregnant`: this captures
/// the practice's own compat predicate, which today happens to be the
/// same for Courtship. Future practices declare their own.
fn is_practice_compatible_now(
    practice: PracticeKind,
    stage: LifeStage,
    orientation: Orientation,
    self_is_pregnant: bool,
) -> bool {
    match practice {
        PracticeKind::Courtship => {
            matches!(stage, LifeStage::Adult | LifeStage::Elder)
                && orientation != Orientation::Asexual
                && !self_is_pregnant
        }
        // Ticket 276 — PlayBout is broadly compatible. Kittens get
        // their own play substrate via Caretake; adults / elders play
        // together. Orientation / pregnancy don't gate play. The
        // playfulness / mood floors live in the matchmaker, not here
        // — `still_compatible` only captures predicates that should
        // *drop a held JI* if they flip. A cat losing playfulness
        // mid-bout shouldn't cascade-drop; a stage transition off
        // Adult/Elder should.
        PracticeKind::PlayBout => matches!(stage, LifeStage::Adult | LifeStage::Elder),
    }
}

/// Ticket 276 — PlayBout per-cat eligibility predicate. Both partners
/// must satisfy this for the matchmaker to emit a `PlayBout`
/// JointIntention.
fn is_playbout_eligible(
    playfulness: f32,
    mood_valence: f32,
    current_action: Action,
    constants: &PlayBoutPracticeConstants,
) -> bool {
    playfulness > constants.playfulness_floor
        && mood_valence > constants.mood_valence_floor
        && matches!(
            current_action,
            Action::Socialize | Action::Idle | Action::Wander
        )
}

/// Ticket 276 — pick the best PlayBout partner for `self_entity` from
/// within `candidate_range`. Returns `None` when no candidate passes
/// the eligibility predicate. Stable tie-break via `Entity::index()`
/// asc; quality score = `playfulness_avg + mood_valence_avg`.
///
/// Skips cats already picked this tick (Courtship-claimed or earlier
/// PlayBout-claimed) to prevent the matchmaker from emitting a JI
/// pointing at a cat that already holds one.
fn pick_playbout_partner(
    self_entity: Entity,
    self_position: &Position,
    positions: &[(Entity, Position)],
    cache: &HashMap<Entity, (f32, f32, Action)>,
    claimed_this_tick: &HashSet<Entity>,
    constants: &PlayBoutPracticeConstants,
) -> Option<Entity> {
    let &(self_play, self_mood, _) = cache.get(&self_entity)?;
    let range = constants.candidate_range;
    let mut best: Option<(Entity, f32)> = None;
    for (other, other_pos) in positions.iter() {
        if *other == self_entity {
            continue;
        }
        if claimed_this_tick.contains(other) {
            continue;
        }
        let manhattan =
            (self_position.x() - other_pos.x()).abs() + (self_position.y() - other_pos.y()).abs();
        if manhattan > range {
            continue;
        }
        let Some(&(other_play, other_mood, other_action)) = cache.get(other) else {
            continue;
        };
        if !is_playbout_eligible(other_play, other_mood, other_action, constants) {
            continue;
        }
        let score = (self_play + other_play) * 0.5 + (self_mood + other_mood) * 0.5;
        if score < constants.emission_threshold {
            continue;
        }
        match best {
            // Strict-better keeps the lowest-Entity-index winner on ties.
            Some((best_e, best_score))
                if best_score > score
                    || (best_score == score && best_e.index() < other.index()) => {}
            _ => best = Some((*other, score)),
        }
    }
    best.map(|(e, _)| e)
}

// Helper trait extension — keeps the mismatch comparison readable.
trait JointIntentionExt {
    fn partner_practice(&self) -> PracticeKind;
}
impl JointIntentionExt for JointIntention {
    fn partner_practice(&self) -> PracticeKind {
        // For symmetric practices, the partner's expected practice is
        // the same as self's. Asymmetric practices (future) may differ;
        // resolved by a per-practice dispatch.
        self.practice
    }
}

// ---------------------------------------------------------------------------
// InLaw adoption rule — ticket 400
// ---------------------------------------------------------------------------

/// Ticket 400 — InLaw adoption on `PracticeStage::CourtshipBonded`
/// transition. When a `JointIntention { practice: Courtship }` advances
/// to `CourtshipBonded`, each partner's biological parents gain an
/// `InLaw`-kind `RelationshipTo` entry on their `ParentingActivity`
/// pointing at the other partner. Mirrored both directions: my parents
/// get InLaw entries toward my new mate; my mate's parents get InLaw
/// entries toward me.
///
/// **Detection.** Runs after [`author_joint_intentions`] (which sets
/// `joint.stage_entered_tick = now_tick` on advance). A cat with
/// `joint.stage == CourtshipBonded && joint.stage_entered_tick == now_tick`
/// just transitioned this tick.
///
/// **Reverse-lookup.** Biological-parent identification scans all
/// `ParentingActivity` Components for entries with `kind = Biological`
/// and `target = X`: each such owner is one of X's biological parents.
/// `KittenDependency` is unusable for this — adults shed the Component
/// when they mature.
///
/// **Idempotency.** `ParentingActivity::has_kind` guards against
/// duplicate InLaw entries. If a JointIntention briefly drops and
/// re-enters Bonded, the rule fires again but does not duplicate.
///
/// **No alloparenting yet.** Only `Biological`-kind parents propagate
/// InLaw status. `BondFormed` and `Adopted` parental adoption rules
/// land in follow-on tickets 403 / 404 and will participate naturally
/// in this same rule (any cat carrying a parental stance toward X
/// gains InLaw toward X's new mate).
#[allow(clippy::type_complexity)]
pub fn apply_inlaw_adoption_on_bonded(
    time: Res<TimeState>,
    joints: Query<(Entity, &JointIntention), Without<Dead>>,
    mut parenting: bevy_ecs::system::ParamSet<(
        Query<(
            Entity,
            &crate::components::parenting_activity::ParentingActivity,
        )>,
        Query<&mut crate::components::parenting_activity::ParentingActivity>,
    )>,
) {
    use crate::components::parenting_activity::{ParentalKind, RelationshipTo};

    let now_tick = time.tick;

    // 1. Find cats that just entered CourtshipBonded this tick.
    let fresh_bonds: Vec<(Entity, Entity)> = joints
        .iter()
        .filter_map(|(entity, joint)| {
            (joint.stage == PracticeStage::CourtshipBonded && joint.stage_entered_tick == now_tick)
                .then_some((entity, joint.partner))
        })
        .collect();
    if fresh_bonds.is_empty() {
        return;
    }

    // 2. Build reverse-lookup: child -> Vec<parent_entity> via
    // ParentingActivity scan. Only Biological-kind entries propagate
    // InLaw status in 400.
    let mut parents_of: HashMap<Entity, Vec<Entity>> = HashMap::new();
    {
        let read = parenting.p0();
        for (cat, pa) in read.iter() {
            for rel in &pa.relationships {
                if rel.kind == ParentalKind::Biological {
                    parents_of.entry(rel.target).or_default().push(cat);
                }
            }
        }
    }

    // 3. Determine InLaw insertions per (parent_to_modify, target_inlaw).
    // Mirror in both directions per the design.
    let mut insertions: Vec<(Entity, Entity)> = Vec::new();
    for (entity, partner) in &fresh_bonds {
        if let Some(my_parents) = parents_of.get(entity) {
            for &p in my_parents {
                insertions.push((p, *partner));
            }
        }
        if let Some(partner_parents) = parents_of.get(partner) {
            for &p in partner_parents {
                insertions.push((p, *entity));
            }
        }
    }
    if insertions.is_empty() {
        return;
    }

    // 4. Apply insertions via the mutable arm of the ParamSet.
    let mut write = parenting.p1();
    for (parent_entity, in_law_target) in insertions {
        if let Ok(mut pa) = write.get_mut(parent_entity) {
            if pa.has_kind(in_law_target, ParentalKind::InLaw) {
                continue;
            }
            pa.relationships.push(RelationshipTo::new(
                in_law_target,
                ParentalKind::InLaw,
                None,
                now_tick,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::fertility::{Fertility, FertilityPhase};
    use crate::components::identity::{Age, Gender, Name, Orientation};
    use crate::components::mental::Mood;
    use crate::components::physical::{Health, Needs, Position};
    use crate::resources::relationships::BondType;
    use crate::resources::time::{SimConfig, TimeState};
    use crate::resources::SimConstants;
    use bevy_ecs::schedule::Schedule;

    /// Spawn an Adult cat with all per-cat fertility / sated-and-happy
    /// gates open. Adapted from `pairing::tests::spawn_eligible_adult`.
    fn spawn_eligible_adult(
        world: &mut World,
        name: &str,
        gender: Gender,
        orientation: Orientation,
        position: Position,
    ) -> Entity {
        let mut needs = Needs::default();
        needs.hunger = 0.9;
        needs.energy = 0.9;
        needs.mating = 0.3;
        let mut mood = Mood::default();
        mood.valence = 0.5;
        let fertility = if matches!(gender, Gender::Tom) {
            None
        } else {
            Some(Fertility {
                phase: FertilityPhase::Estrus,
                cycle_offset: 0,
                post_partum_remaining_ticks: 0,
            })
        };
        let mut entity = world.spawn((
            Name(name.to_string()),
            Age { born_tick: 0 },
            gender,
            orientation,
            mood,
            needs,
            position,
            Health::default(),
        ));
        if let Some(f) = fertility {
            entity.insert(f);
        }
        entity.id()
    }

    fn author_world() -> World {
        let mut world = World::new();
        let mut time = TimeState::default();
        time.tick = 20_000 * 13;
        world.insert_resource(time);
        world.insert_resource(SimConfig::default());
        world.insert_resource(SimConstants::default());
        world.insert_resource(Relationships::default());
        world.insert_resource(SystemActivation::default());
        world.insert_resource(bevy_ecs::message::Messages::<JointInteractionObserved>::default());
        world.insert_resource(bevy_ecs::message::Messages::<PlayBoutBoutingEntered>::default());
        world
    }

    fn run_author(world: &mut World) {
        let mut schedule = Schedule::default();
        schedule.add_systems(author_joint_intentions);
        schedule.run(world);
    }

    fn make_paired_cats(world: &mut World) -> (Entity, Entity) {
        let a = spawn_eligible_adult(
            world,
            "Fern",
            Gender::Queen,
            Orientation::Straight,
            Position::new(0, 0),
        );
        let b = spawn_eligible_adult(
            world,
            "Reed",
            Gender::Tom,
            Orientation::Straight,
            Position::new(1, 0),
        );
        let mut rels = world.resource_mut::<Relationships>();
        let rel = rels.get_or_insert(a, b);
        rel.bond = Some(BondType::Friends);
        rel.fondness = 0.5;
        (a, b)
    }

    #[test]
    fn author_emits_joint_intention_for_compatible_paired_adults() {
        let mut world = author_world();
        let (a, b) = make_paired_cats(&mut world);

        run_author(&mut world);

        for (entity, expected_partner) in [(a, b), (b, a)] {
            let ji = world
                .get::<JointIntention>(entity)
                .expect("JI authored by Commit C in-tree matchmaker");
            assert_eq!(ji.partner, expected_partner);
            assert_eq!(ji.practice, PracticeKind::Courtship);
            assert_eq!(ji.stage, PracticeStage::CourtshipApproach);
        }
        let activation = world.resource::<SystemActivation>();
        assert_eq!(
            activation
                .counts
                .get(&Feature::JointIntentionEmitted {
                    practice: PracticeKind::Courtship,
                })
                .copied(),
            Some(2),
            "symmetric pair → both cats emit one JI each"
        );
    }

    #[test]
    fn stage_advances_approach_to_courting_after_interaction() {
        let mut world = author_world();
        let (a, b) = make_paired_cats(&mut world);
        run_author(&mut world);
        // Sanity — both at Approach.
        assert_eq!(
            world.get::<JointIntention>(a).unwrap().stage,
            PracticeStage::CourtshipApproach
        );

        // Emit a JointInteractionObserved for `a` at a later tick.
        let now = world.resource::<TimeState>().tick;
        let later = now + 100;
        world
            .resource_mut::<bevy_ecs::message::Messages<JointInteractionObserved>>()
            .write(JointInteractionObserved {
                entity: a,
                partner: b,
                practice: PracticeKind::Courtship,
                tick: later,
            });

        // Advance time so the author sees the message + bumps the tick.
        world.resource_mut::<TimeState>().tick = later;
        run_author(&mut world);

        let ji_a = world.get::<JointIntention>(a).unwrap();
        assert_eq!(ji_a.last_interaction_tick, later);
        assert_eq!(
            ji_a.stage,
            PracticeStage::CourtshipCourting,
            "Approach→Courting on first paired-resolver interaction"
        );
        let activation = world.resource::<SystemActivation>();
        assert!(
            activation
                .counts
                .get(&Feature::JointStageAdvanced {
                    practice: PracticeKind::Courtship,
                })
                .copied()
                .unwrap_or(0)
                >= 1,
            "JointStageAdvanced fires on the transition"
        );
    }

    #[test]
    fn partner_left_practice_cascade_fires_within_one_tick() {
        // §Exit criterion 3 — drop cascade works within 1 tick.
        let mut world = author_world();
        let (a, b) = make_paired_cats(&mut world);
        run_author(&mut world);
        assert!(world.get::<JointIntention>(a).is_some());
        assert!(world.get::<JointIntention>(b).is_some());

        // Force-remove a's JI (simulating a desire-drift drop on a).
        // Also drop the relationship bond so the matchmaker can't
        // re-emit JI for a or b in the same tick's Pass 3.
        world.entity_mut(a).remove::<JointIntention>();
        let mut rels = world.resource_mut::<Relationships>();
        rels.get_or_insert(a, b).bond = None;

        // Tick T+1 — partner snapshot reflects a's removal; b's drop
        // gate fires PartnerLeftPractice.
        run_author(&mut world);

        assert!(
            world.get::<JointIntention>(b).is_none(),
            "b's JointIntention dropped via PartnerLeftPractice cascade"
        );
        let activation = world.resource::<SystemActivation>();
        assert!(
            activation
                .counts
                .get(&Feature::JointIntentionDropped {
                    practice: PracticeKind::Courtship,
                })
                .copied()
                .unwrap_or(0)
                >= 1,
            "JointIntentionDropped fires on the cascade"
        );
    }

    // -----------------------------------------------------------------
    // Ticket 453 — Mates-exclusivity perception gates on
    // `pick_courtship_partner`. The matchmaker refuses to emit a
    // Courtship intention when the actor is already Mates-bonded, and
    // skips candidates already Mates-bonded with a third party.
    // -----------------------------------------------------------------

    fn default_eligible_fitness(gender: Gender) -> MatingFitness {
        MatingFitness {
            stage: LifeStage::Adult,
            gender,
            orientation: Orientation::Straight,
            mood_valence: 0.5,
            hunger: 0.9,
            energy: 0.9,
            is_pregnant: false,
            fertility_phase: if matches!(gender, Gender::Tom) {
                None
            } else {
                Some(FertilityPhase::Estrus)
            },
            body_condition: 1.0,
        }
    }

    #[test]
    fn pick_courtship_partner_returns_none_when_self_already_mated() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();

        let mut fitness: HashMap<Entity, MatingFitness> = HashMap::new();
        fitness.insert(a, default_eligible_fitness(Gender::Queen));
        fitness.insert(b, default_eligible_fitness(Gender::Tom));
        fitness.insert(c, default_eligible_fitness(Gender::Tom));

        let mut rels = Relationships::default();
        // A is already Mates with B — exclusivity precondition.
        let ab = rels.get_or_insert(a, b);
        ab.bond = Some(BondType::Mates);
        ab.fondness = 0.8;
        ab.romantic = 0.8;
        // A and C also share a strong courtship-bonded relationship that
        // would otherwise qualify C as a target.
        let ac = rels.get_or_insert(a, c);
        ac.bond = Some(BondType::Partners);
        ac.fondness = 0.8;
        ac.romantic = 0.8;

        let positions = vec![
            (a, Position::new(0, 0)),
            (b, Position::new(1, 0)),
            (c, Position::new(2, 0)),
        ];
        let constants = SimConstants::default();
        let picked = pick_courtship_partner(
            a,
            &Position::new(0, 0),
            default_eligible_fitness(Gender::Queen),
            &positions,
            &fitness,
            &rels,
            &constants.practices.courtship,
        );
        assert!(
            picked.is_none(),
            "actor already Mates-bonded must not emit new courtship; got {picked:?}"
        );
    }

    #[test]
    fn pick_courtship_partner_skips_candidates_mated_elsewhere() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let c = world.spawn_empty().id();
        let d = world.spawn_empty().id();

        let mut fitness: HashMap<Entity, MatingFitness> = HashMap::new();
        fitness.insert(a, default_eligible_fitness(Gender::Queen));
        fitness.insert(b, default_eligible_fitness(Gender::Tom));
        fitness.insert(c, default_eligible_fitness(Gender::Tom));
        fitness.insert(d, default_eligible_fitness(Gender::Queen));

        let mut rels = Relationships::default();
        // A's nearby targets: B and C, both with Partners bonds —
        // ordinarily both would be eligible. But C is Mates-bonded
        // with D elsewhere, so C should be skipped.
        let ab = rels.get_or_insert(a, b);
        ab.bond = Some(BondType::Partners);
        ab.fondness = 0.8;
        ab.romantic = 0.8;
        let ac = rels.get_or_insert(a, c);
        ac.bond = Some(BondType::Partners);
        ac.fondness = 0.9; // higher quality — would win without the gate
        ac.romantic = 0.9;
        let cd = rels.get_or_insert(c, d);
        cd.bond = Some(BondType::Mates);
        cd.fondness = 0.9;
        cd.romantic = 0.9;

        let positions = vec![
            (a, Position::new(0, 0)),
            (b, Position::new(1, 0)),
            (c, Position::new(1, 1)),
            (d, Position::new(5, 5)),
        ];
        let constants = SimConstants::default();
        let picked = pick_courtship_partner(
            a,
            &Position::new(0, 0),
            default_eligible_fitness(Gender::Queen),
            &positions,
            &fitness,
            &rels,
            &constants.practices.courtship,
        );
        assert_eq!(
            picked,
            Some(b),
            "Mates-bonded candidate C must be filtered out, leaving B; got {picked:?}"
        );
    }
}
