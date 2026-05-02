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

/// Scalar input name — same as Flee's, by design. The cat's "feels
/// unsafe" perception drives both valences; the choice between them
/// is owned by the modifier layer (105 gates Freeze on
/// `escape_viability < threshold AND combat_winnability < threshold`,
/// otherwise 047's Flee branch fires).
pub const SAFETY_DEFICIT_INPUT: &str = "safety_deficit";

pub struct HideDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HideDse {
    pub fn new() -> Self {
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

        Self {
            id: DseId("hide"),
            considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                SAFETY_DEFICIT_INPUT,
                safety_curve,
            ))],
            composition: Composition::compensated_product(vec![1.0]),
            // Phase 1 dormancy gate: `HideEligible` is never authored,
            // so this filter rejects every candidate. Phase 2/3 lands
            // the authoring system alongside lift activation.
            eligibility: EligibilityFilter::new().require(markers::HideEligible::KEY),
        }
    }
}

impl Default for HideDse {
    fn default() -> Self {
        Self::new()
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

/// Build the Hide DSE for registration. Called once at plugin load.
pub fn hide_dse() -> Box<dyn Dse> {
    Box::new(HideDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::eval::{evaluate_single, ModifierPipeline};
    use crate::components::physical::Position;

    #[test]
    fn hide_dse_id_is_stable() {
        assert_eq!(HideDse::new().id().0, "hide");
    }

    #[test]
    fn hide_dse_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            HideDse::new().composition().mode,
            CompositionMode::CompensatedProduct
        );
    }

    #[test]
    fn hide_dse_maslow_tier_is_two() {
        assert_eq!(HideDse::new().maslow_tier(), 2);
    }

    #[test]
    fn hide_dse_requires_hide_eligible_marker() {
        // Phase 1 dormancy contract: the eligibility filter MUST gate
        // on `HideEligible`, which is not authored anywhere in the
        // codebase. This test pins the contract — if the filter ever
        // drops the requirement, Hide becomes organically reachable
        // and the bit-identical-baseline invariant breaks.
        let dse = HideDse::new();
        assert_eq!(dse.eligibility().required, vec![markers::HideEligible::KEY]);
    }

    #[test]
    fn hide_dse_dormant_without_eligible_marker() {
        // The substrate-bit-identity check: with the dormancy marker
        // absent (the Phase-1 baseline state), Hide MUST be ineligible
        // regardless of `safety_deficit`. evaluate_single returns None.
        use crate::ai::considerations::LandmarkAnchor;
        let dse = HideDse::new();
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
        let dse = HideDse::new();
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
        let registry_entry = hide_dse();
        assert_eq!(registry_entry.id().0, "hide");
    }
}
