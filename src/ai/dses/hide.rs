//! `Hide` — ticket 104. The third predator-avoidance valence
//! ("remain still and hope") alongside Flee and Fight. Real cat
//! ethology shows freeze as a distinct response — body flat, eyes
//! averted, breath held — when fleeing is too risky and combat
//! unwinnable.
//!
//! **Phase 1 dormancy contract.** This DSE ships behind the
//! `HideEligible` eligibility marker, which has **no authoring system
//! at landing**. The marker is defined in `markers.rs` so the gate
//! compiles, but never fires until a future ticket lands the
//! authoring system alongside the lift activation in modifiers 105
//! (`AcuteHealthAdrenalineFreeze`) and 142
//! (`IntraspeciesConflictResponseFreeze`). With the marker dormant
//! the eligibility filter rejects Hide on every cat every tick, so
//! the IAUS contest never sees a non-zero Hide score and the colony
//! is bit-identical to pre-Wave-1 baseline.
//!
//! **Future awakening.** Phase 2/3 of ticket 105 (and parallel work
//! on 142) lands a `HideEligible` authoring system with predicate:
//! threat in sight AND a low-cover tile within 2 tiles AND no fight
//! allies in range. With the marker authored, Hide becomes eligible;
//! 105's modifier (lift defaults 0.0 today, swept-validated 0.70)
//! pushes its score above competing actions when the cornered-and-
//! overmatched gate trips.
//!
//! **Sensing coupling — deferred.** The ticket §Scope mentions
//! reducing the cat's visibility to threats while frozen. That
//! requires modulating the predator-side detection path, which is
//! a multi-system change (sensing.rs::update_target_existence_markers
//! plus per-species detection profiles). Phase 1 leaves the coupling
//! out: while Hide is dormant the runtime effect is moot, and
//! activating the visibility coupling without first verifying that
//! the lift fires correctly would couple two follow-on changes
//! unnecessarily. Tracked as a separate predicate-refinement
//! ticket.
//!
//! Spec rows:
//!
//! | Axis | Shape | Rationale |
//! |---|---|---|
//! | §2.3 `Hide.safety_deficit` | `Linear { slope: 0.5 }` | Bounded base score so Hide never wins the contest organically — only when 105's modifier lifts it under the cornered-and-overmatched gate. Single-axis CompensatedProduct (n=1) for shape consistency with 094/088 templates. |
//! | §3.1.1 `Hide` | `CompensatedProduct` | n=1; kept CP (not WS) for future axis growth (cover-tile-distance, ally-presence-inverse). |
//! | §L2.10.3 Intention | `Goal(false)` | Same shape as Flee — committed Blind until safety restored or counter expires. |
//! | Maslow tier | 2 | Safety-layer response — sibling to Flee. |

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;
use crate::resources::sim_constants::ScoringConstants;

/// Scalar input name — same as Flee's, by design. The cat's "feels
/// unsafe" perception drives both valences; the choice between them
/// is owned by the modifier layer (105 gates Freeze on
/// `escape_viability < threshold AND combat_winnability < threshold`,
/// otherwise 047's Flee branch fires).
pub const SAFETY_DEFICIT_INPUT: &str = "safety_deficit";

/// Ticket 268 — `Affordance(Freeze, self, NearestThreat)` axis. Reads
/// the 261 substrate's per-(perceiver, target) Freeze affordance
/// scalar. Conditional 2nd axis: pushed onto the consideration list
/// only when `hide_affordance_freeze_weight > 0.0`. CP semantics
/// `c · 0 = 0` would zero the whole product if a weight-0 axis were
/// included, so dormant ⇒ axis absent ⇒ score bit-identical to the
/// single-axis form.
pub const AFFORDANCE_FREEZE_INPUT: &str =
    crate::resources::action_affordances::AFFORDANCE_FREEZE_INPUT;

/// Ticket 268 — recency-of-threat-cue axis. Reads the cat's
/// MentalModel facet at the nearest-threat entity OR at
/// `ContextBeliefs[HereNow]` (max). Conditional 3rd axis behind
/// `hide_recency_of_threat_cue_weight`.
pub const RECENCY_OF_THREAT_CUE_INPUT: &str = "hide_recency_of_threat_cue";

/// Ticket 268 — perceived-intent-clarity axis. Reads
/// `PredatorBeliefs[nearest_threat].perceived_intent_clarity`. The
/// inverse direction is load-bearing: Hide wins under *unclear*
/// intent (predator's commitment ambiguous), Flee wins under clear
/// hostile intent. The activation follow-on encodes the inversion in
/// the curve shape; the scalar surfaced here is raw clarity.
/// Conditional 4th axis behind
/// `hide_perceived_intent_clarity_weight`.
pub const PERCEIVED_INTENT_CLARITY_INPUT: &str = "hide_perceived_intent_clarity";

pub struct HideDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HideDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        // Linear bounded curve — Hide's organic score caps at 0.5
        // even at full safety_deficit. That's intentional: Hide should
        // never beat Flee (which uses flee_or_fight Logistic with
        // saturated peak ~0.88) under normal conditions. Only the
        // 105 modifier's additive lift (proposed +0.70 when activated)
        // makes Hide competitive — and only when the cornered-and-
        // overmatched gate trips, splitting it from the Flee valence.
        let safety_curve = Curve::Linear {
            slope: 0.5,
            intercept: 0.0,
        };

        let mut considerations: Vec<Consideration> = vec![Consideration::Scalar(
            ScalarConsideration::new(SAFETY_DEFICIT_INPUT, safety_curve),
        )];
        let mut weights = vec![1.0_f32];

        // 268: conditional Affordance(Freeze) axis. Ships at weight 0.0
        // (axis absent — CP-safe). Activation follow-on tunes the weight.
        let aff_freeze_weight = scoring.hide_affordance_freeze_weight.clamp(0.0, 1.0);
        if aff_freeze_weight > 0.0 {
            considerations.push(Consideration::Scalar(ScalarConsideration::new(
                AFFORDANCE_FREEZE_INPUT,
                Curve::Linear {
                    slope: 1.0,
                    intercept: 0.0,
                },
            )));
            weights.push(aff_freeze_weight);
        }

        // 268: conditional recency-of-threat-cue axis. Reads
        // PredatorBeliefs at nearest_threat OR ContextBeliefs[HereNow]
        // (max). Linear identity over the already-[0,1] facet value.
        let recency_weight = scoring.hide_recency_of_threat_cue_weight.clamp(0.0, 1.0);
        if recency_weight > 0.0 {
            considerations.push(Consideration::Scalar(ScalarConsideration::new(
                RECENCY_OF_THREAT_CUE_INPUT,
                Curve::Linear {
                    slope: 1.0,
                    intercept: 0.0,
                },
            )));
            weights.push(recency_weight);
        }

        // 268: conditional perceived-intent-clarity axis. The DSE-side
        // shape is identity (Linear slope=1); the activation follow-on
        // chooses inversion direction via the constant's tuned curve
        // params or via a sibling curve. Shipped as identity here so
        // the activation thread isn't pre-foreclosed.
        let intent_weight = scoring.hide_perceived_intent_clarity_weight.clamp(0.0, 1.0);
        if intent_weight > 0.0 {
            considerations.push(Consideration::Scalar(ScalarConsideration::new(
                PERCEIVED_INTENT_CLARITY_INPUT,
                Curve::Linear {
                    slope: 1.0,
                    intercept: 0.0,
                },
            )));
            weights.push(intent_weight);
        }

        Self {
            id: DseId("hide"),
            considerations,
            composition: Composition::compensated_product(weights),
            // Phase 1 dormancy gate (104): `HideEligible` was never
            // authored until ticket 170, which lifts the dormancy
            // contract. The filter still gates against the marker —
            // unchanged from Phase 1 semantics.
            eligibility: EligibilityFilter::new().require(markers::HideEligible::KEY),
        }
    }
}

impl Default for HideDse {
    fn default() -> Self {
        Self::new(&ScoringConstants::default())
    }
}

impl Dse for HideDse {
    fn id(&self) -> DseId {
        self.id
    }

    fn considerations(&self) -> &[Consideration] {
        &self.considerations
    }

    fn composition(&self) -> &Composition {
        &self.composition
    }

    fn eligibility(&self) -> &EligibilityFilter {
        &self.eligibility
    }

    fn default_strategy(&self) -> CommitmentStrategy {
        // §7.5: like Flee, Hide is an event-driven anxiety-interrupt
        // response. Blind-committed once installed so it cannot be
        // preempted by normal scoring until the achievement condition
        // (safety restored, freeze counter exhausted) fires.
        CommitmentStrategy::Blind
    }

    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "freeze_concluded",
                // Mirror's Flee's `|_, _| false` shape — the freeze
                // counter ticks down via `resolve_hide`'s witnessed
                // step output rather than a world-state predicate.
                // Phase 2/3 wires the actual achievement check
                // (e.g. `safety > threshold` after threat departs).
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::Blind,
        }
    }

    fn maslow_tier(&self) -> u8 {
        2
    }
}
impl crate::ai::dse::CatDse for HideDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::Hide
    }

    fn life_stages(&self) -> crate::ai::dse::LifeStageSet {
        crate::ai::dse::LifeStageSet::ALL
    }
}

/// Build the Hide DSE for registration. Called once at plugin load.
pub fn hide_dse(scoring: &ScoringConstants) -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(HideDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::eval::{evaluate_single, ModifierPipeline};
    use crate::components::physical::Position;

    #[test]
    fn hide_dse_id_is_stable() {
        assert_eq!(HideDse::new(&ScoringConstants::default()).id().0, "hide");
    }

    #[test]
    fn hide_dse_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            HideDse::new(&ScoringConstants::default())
                .composition()
                .mode,
            CompositionMode::CompensatedProduct
        );
    }

    #[test]
    fn hide_dse_maslow_tier_is_two() {
        assert_eq!(HideDse::new(&ScoringConstants::default()).maslow_tier(), 2);
    }

    #[test]
    fn hide_dse_requires_hide_eligible_marker() {
        // Phase 1 dormancy contract: the eligibility filter MUST gate
        // on `HideEligible`, which is not authored anywhere in the
        // codebase. This test pins the contract — if the filter ever
        // drops the requirement, Hide becomes organically reachable
        // and the bit-identical-baseline invariant breaks.
        let dse = HideDse::new(&ScoringConstants::default());
        assert_eq!(dse.eligibility().required, vec![markers::HideEligible::KEY]);
    }

    #[test]
    fn hide_dse_dormant_without_eligible_marker() {
        // The substrate-bit-identity check: with the dormancy marker
        // absent (the Phase-1 baseline state), Hide MUST be ineligible
        // regardless of `safety_deficit`. evaluate_single returns None.
        use crate::ai::considerations::LandmarkAnchor;
        let dse = HideDse::new(&ScoringConstants::default());
        let entity = Entity::from_raw_u32(1).unwrap();
        let has_marker = |_: &str, _: Entity| false;
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
            field_cost: None,
        };
        let maslow = |_: u8| 1.0;
        let modifiers = ModifierPipeline::new();
        let fetch = |_: &str, _: Entity| 0.9_f32;

        let out = evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch);
        assert!(
            out.is_none(),
            "Phase 1 dormancy: Hide must be ineligible without HideEligible authoring"
        );
    }

    #[test]
    fn hide_dse_score_capped_when_eligible() {
        // Even if the eligibility marker were somehow authored, the
        // bounded Linear { slope: 0.5 } curve caps Hide's organic
        // score at 0.5 — well below Flee's saturated peak (~0.88).
        // This pins the substrate-vs-modifier separation: Hide can
        // only win the contest via 105's additive lift, not on its
        // own.
        use crate::ai::considerations::LandmarkAnchor;
        let dse = HideDse::new(&ScoringConstants::default());
        let entity = Entity::from_raw_u32(1).unwrap();
        let has_marker = |key: &str, _: Entity| key == markers::HideEligible::KEY;
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
            field_cost: None,
        };
        let maslow = |_: u8| 1.0;
        let modifiers = ModifierPipeline::new();
        let fetch = |name: &str, _: Entity| match name {
            SAFETY_DEFICIT_INPUT => 1.0,
            _ => 0.0,
        };
        let scored = evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch)
            .expect("eligible with HideEligible authored");
        assert!(
            scored.raw_score <= 0.5 + 1e-5,
            "Hide organic score must cap at 0.5; got {}",
            scored.raw_score
        );
    }

    #[test]
    fn hide_dse_boxed_registers() {
        let registry_entry = hide_dse(&ScoringConstants::default());
        assert_eq!(registry_entry.id().0, "hide");
    }

    // -----------------------------------------------------------------------
    // Ticket 268 — conditional-axis pattern verification
    // -----------------------------------------------------------------------

    #[test]
    fn hide_dse_ships_with_single_axis_at_default_weights() {
        // Ticket 268 — the three new consideration axes
        // (Affordance(Freeze), recency_of_threat_cue,
        // perceived_intent_clarity) ship at weight 0.0 (dormant).
        // CompensatedProduct semantics `c · 0 = 0` would zero the
        // whole product if a zero-weight axis were present, so the
        // axes MUST be absent from the considerations list at
        // dormant weights.
        let s = ScoringConstants::default();
        assert_eq!(s.hide_affordance_freeze_weight, 0.0);
        assert_eq!(s.hide_recency_of_threat_cue_weight, 0.0);
        assert_eq!(s.hide_perceived_intent_clarity_weight, 0.0);
        let dse = HideDse::new(&s);
        assert_eq!(
            dse.considerations().len(),
            1,
            "dormant weights ⇒ only the safety_deficit axis is present"
        );
    }

    #[test]
    fn hide_dse_axes_appear_when_weights_lifted() {
        // Ticket 268 — when the activation follow-on lifts any of the
        // three weights, the matching axis appears in the
        // considerations list with a Linear identity curve.
        let mut s = ScoringConstants::default();
        s.hide_affordance_freeze_weight = 0.5;
        s.hide_recency_of_threat_cue_weight = 0.3;
        s.hide_perceived_intent_clarity_weight = 0.2;
        let dse = HideDse::new(&s);
        assert_eq!(dse.considerations().len(), 4);
        let names: Vec<&str> = dse
            .considerations()
            .iter()
            .filter_map(|c| match c {
                Consideration::Scalar(sc) => Some(sc.name),
                _ => None,
            })
            .collect();
        assert!(names.contains(&SAFETY_DEFICIT_INPUT));
        assert!(names.contains(&AFFORDANCE_FREEZE_INPUT));
        assert!(names.contains(&RECENCY_OF_THREAT_CUE_INPUT));
        assert!(names.contains(&PERCEIVED_INTENT_CLARITY_INPUT));
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static HIDE_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 1300,
        construct: |s| hide_dse(s),
    };
