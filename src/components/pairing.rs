//! L2 PairingActivity Intention component — §7.M of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! L2 sits between L1 ReproduceAspiration (life-arc, OpenMinded) and
//! L3 MateWithGoal (per-attempt SingleMinded goal carried by
//! `DispositionKind::Mating`). Its purpose is **structural commitment
//! to a single partner** across ticks: a cat that holds
//! `PairingActivity { partner }` will preferentially direct
//! grooming/socializing toward that one cat across many ticks, which is
//! what closes the structural fondness/familiarity gap from a Friends
//! bond to a Partners bond. Without it, the only escalation path is the
//! passive courtship-drift loop in `social.rs::check_bonds`, which
//! mathematically caps `romantic` near the threshold and accumulates
//! fondness/familiarity diffusely across all peers.
//!
//! **Substrate placement.** Per `commitment.rs:175–183`'s doc-comment,
//! L1/L2 strategies live inline on the emitting layer, *not* as a new
//! `DispositionKind`. So L2 is a per-cat ECS component (this file) that
//! is read at evaluation time by existing target-pickers
//! (`socialize_target.rs`, `groom_other_target.rs`) and by
//! `score_actions` for additive Socialize/Groom/Wander bias. The
//! component carries `partner: Entity` so the bias can compare a
//! candidate against *this specific cat*, not just "any partner".
//!
//! **Mutual exclusivity.** A cat holds at most one `PairingActivity`.
//! The author system in `crate::ai::pairing` enforces this by checking
//! `Without<PairingActivity>` on the emission branch and on drop only
//! removing the existing component.
//!
//! **Drop semantics.** `should_drop_pairing` returns `Some(DropBranch)`
//! whenever any of the §7.M-compatible OpenMinded gates fires. The
//! conjunction-floor on romantic+fondness collapse means a Mates-bonded
//! pair with high fondness and zero-romantic (post-conception cooldown)
//! does *not* drop on the romantic axis alone — both must collapse.

use bevy_ecs::prelude::*;

use crate::components::fertility::FertilityPhase;
use crate::components::identity::{Gender, LifeStage, Orientation};
use crate::resources::relationships::BondType;
use crate::resources::time::Season;

/// Branch identifying which §7.M drop gate fired. Useful for trace
/// observability (Commit C records the branch on the focal-cat
/// `PairingCapture`) and for the `Feature::PairingDropped` activation
/// counter without losing per-branch granularity in narrative output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PairingDropBranch {
    /// Partner is `Dead`, `Banished`, `Incapacitated`, or despawned.
    PartnerInvalid,
    /// Bond no longer reaches `Friends` or higher (or relationship row
    /// no longer exists). Defensive — `social::check_bonds` upgrades
    /// only today, but a future ticket may add downgrade.
    BondLost,
    /// Both `romantic < pairing_romantic_floor` AND
    /// `fondness < pairing_fondness_floor`. The §7.M.1 OpenMinded
    /// `still_goal == false` arm.
    DesireDrift,
    /// Tom-side cats: `season == Winter`. Queens / Nonbinaries:
    /// `Fertility { phase ∈ {Anestrus, Postpartum} }`. The
    /// photoperiodic / cycle-out drop §7.M.4 names.
    SeasonOut,
    /// Cat no longer meets the L1-equivalent reproductive eligibility
    /// gate (life-stage transitioned past Adult/Elder, became Asexual,
    /// or Pregnant). The L1 cascade-drop §7.M.1 names — modeled on
    /// the existing `MatingFitness` snapshot since the L1
    /// ReproduceAspiration aspiration-catalog entry is not yet
    /// authored (verified 2026-04-28: no Reproduce chain in
    /// `assets/narrative/aspirations/*.ron`).
    AspirationCascade,
}

/// L2 PairingActivity Intention persisted on the cat. §7.M.1.
///
/// Inserted by `crate::ai::pairing::author_pairing_intentions` on a
/// matched candidate; removed by the same system on any
/// `PairingDropBranch` trigger. The component is the source of truth
/// — there is no parallel ZST marker. Bias readers query
/// `Option<&PairingActivity>` directly.
///
/// Only `Serialize` is derived (not `Deserialize`) because `partner:
/// Entity` has no `Default` and the component is pure runtime state —
/// no save/load path round-trips it. The trace pipeline at
/// `trace_log.rs` (Commit C) reads it via `Serialize` only.
#[derive(Component, Debug, Clone, serde::Serialize)]
pub struct PairingActivity {
    /// The committed partner.
    #[serde(skip)]
    pub partner: Entity,
    /// Tick the Pairing was first emitted. Drives the focal-trace
    /// `ticks_held` field landing in Commit C.
    pub adopted_tick: u64,
    /// Tick of the most recent observed interaction with `partner`
    /// (any `Feature::CourtshipInteraction` or pairing-biased
    /// resolver pick). Refreshed by Commit B's bias readers; in
    /// Commit A this stays at `adopted_tick` (no readers yet).
    pub last_interaction_tick: u64,
}

impl PairingActivity {
    /// Convenience constructor used by the author system.
    pub fn new(partner: Entity, tick: u64) -> Self {
        Self {
            partner,
            adopted_tick: tick,
            last_interaction_tick: tick,
        }
    }
}

/// Per-cat snapshot of every field `should_drop_pairing` reads.
///
/// Built once in the author system's per-cat loop (a small struct
/// rather than a fresh world query per check). Mirrors the
/// `MatingFitness` snapshot pattern in `crate::ai::mating`.
#[derive(Debug, Clone, Copy)]
pub struct PairingProxies {
    /// Self life-stage. Adult/Elder pass; Kitten/Young trigger
    /// `AspirationCascade`.
    pub self_stage: LifeStage,
    /// Self orientation. Asexual triggers `AspirationCascade`.
    pub self_orientation: Orientation,
    /// Self gender. Tom-side seasonality differs from
    /// Queen/Nonbinary.
    pub self_gender: Gender,
    /// `true` when `Pregnant` is on self → `AspirationCascade`.
    pub self_is_pregnant: bool,
    /// Self fertility phase if applicable. `None` for Toms /
    /// Kitten / Young / Elder — those branches are caught by
    /// life-stage / gender, not by phase.
    pub self_fertility_phase: Option<FertilityPhase>,
    /// `true` when partner is `Dead`, `Banished`, `Incapacitated`,
    /// or despawned. Any flips → `PartnerInvalid`.
    pub partner_invalid: bool,
    /// Current bond between self and partner. `None` and any
    /// non-Friends/Partners/Mates value triggers `BondLost`.
    pub bond: Option<BondType>,
    /// Self ↔ partner romantic axis.
    pub romantic: f32,
    /// Self ↔ partner fondness axis.
    pub fondness: f32,
    /// Current sim season — Winter triggers `SeasonOut` for Toms.
    pub season: Season,
}

/// Constants the drop gate consults. Lifted into the function
/// signature so the unit tests can construct a deterministic floor
/// without standing up a `Res<SimConstants>`.
#[derive(Debug, Clone, Copy)]
pub struct PairingDropConfig {
    pub romantic_floor: f32,
    pub fondness_floor: f32,
}

/// Pure §7.M drop gate. Returns `Some(branch)` iff any drop trigger
/// fires; `None` means hold the Intention for another tick.
///
/// Branch precedence (first-match wins): partner-invalid → bond-lost
/// → aspiration-cascade → season-out → desire-drift. Picked so that
/// "the partner is Dead" reports `PartnerInvalid` rather than the
/// downstream `DesireDrift` that follows from a Dead-partner
/// relationship row.
pub fn should_drop_pairing(
    proxies: &PairingProxies,
    config: &PairingDropConfig,
) -> Option<PairingDropBranch> {
    if proxies.partner_invalid {
        return Some(PairingDropBranch::PartnerInvalid);
    }
    match proxies.bond {
        Some(BondType::Friends | BondType::Partners | BondType::Mates) => {}
        None => return Some(PairingDropBranch::BondLost),
    }
    let aspiration_cascade = !matches!(proxies.self_stage, LifeStage::Adult | LifeStage::Elder)
        || matches!(proxies.self_orientation, Orientation::Asexual)
        || proxies.self_is_pregnant;
    if aspiration_cascade {
        return Some(PairingDropBranch::AspirationCascade);
    }
    let season_out = matches!(proxies.self_gender, Gender::Tom)
        && matches!(proxies.season, Season::Winter)
        || matches!(
            proxies.self_fertility_phase,
            Some(FertilityPhase::Anestrus) | Some(FertilityPhase::Postpartum)
        );
    if season_out {
        return Some(PairingDropBranch::SeasonOut);
    }
    if proxies.romantic < config.romantic_floor && proxies.fondness < config.fondness_floor {
        return Some(PairingDropBranch::DesireDrift);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn happy_proxies() -> PairingProxies {
        PairingProxies {
            self_stage: LifeStage::Adult,
            self_orientation: Orientation::Straight,
            self_gender: Gender::Queen,
            self_is_pregnant: false,
            self_fertility_phase: Some(FertilityPhase::Estrus),
            partner_invalid: false,
            bond: Some(BondType::Friends),
            romantic: 0.4,
            fondness: 0.5,
            season: Season::Spring,
        }
    }

    fn config() -> PairingDropConfig {
        PairingDropConfig {
            romantic_floor: 0.05,
            fondness_floor: 0.30,
        }
    }

    #[test]
    fn happy_path_holds_intention() {
        assert_eq!(should_drop_pairing(&happy_proxies(), &config()), None);
    }

    #[test]
    fn drops_when_partner_invalid() {
        let mut p = happy_proxies();
        p.partner_invalid = true;
        assert_eq!(
            should_drop_pairing(&p, &config()),
            Some(PairingDropBranch::PartnerInvalid)
        );
    }

    #[test]
    fn partner_invalid_outranks_other_drop_branches() {
        // A degenerate state where every drop branch fires must report
        // PartnerInvalid since first-match precedence orders that branch first.
        let p = PairingProxies {
            partner_invalid: true,
            bond: None,
            self_stage: LifeStage::Kitten,
            self_orientation: Orientation::Asexual,
            self_is_pregnant: true,
            self_fertility_phase: Some(FertilityPhase::Anestrus),
            self_gender: Gender::Tom,
            season: Season::Winter,
            romantic: 0.0,
            fondness: 0.0,
        };
        assert_eq!(
            should_drop_pairing(&p, &config()),
            Some(PairingDropBranch::PartnerInvalid)
        );
    }

    #[test]
    fn drops_when_bond_lost() {
        let mut p = happy_proxies();
        p.bond = None;
        assert_eq!(
            should_drop_pairing(&p, &config()),
            Some(PairingDropBranch::BondLost)
        );
    }

    #[test]
    fn holds_for_partners_and_mates_bonds() {
        for bond in [BondType::Partners, BondType::Mates] {
            let mut p = happy_proxies();
            p.bond = Some(bond);
            assert_eq!(should_drop_pairing(&p, &config()), None);
        }
    }

    #[test]
    fn drops_when_kitten() {
        let mut p = happy_proxies();
        p.self_stage = LifeStage::Kitten;
        assert_eq!(
            should_drop_pairing(&p, &config()),
            Some(PairingDropBranch::AspirationCascade)
        );
    }

    #[test]
    fn drops_when_asexual() {
        let mut p = happy_proxies();
        p.self_orientation = Orientation::Asexual;
        assert_eq!(
            should_drop_pairing(&p, &config()),
            Some(PairingDropBranch::AspirationCascade)
        );
    }

    #[test]
    fn drops_when_pregnant() {
        let mut p = happy_proxies();
        p.self_is_pregnant = true;
        assert_eq!(
            should_drop_pairing(&p, &config()),
            Some(PairingDropBranch::AspirationCascade)
        );
    }

    #[test]
    fn drops_tom_in_winter() {
        let mut p = happy_proxies();
        p.self_gender = Gender::Tom;
        p.self_fertility_phase = None;
        p.season = Season::Winter;
        assert_eq!(
            should_drop_pairing(&p, &config()),
            Some(PairingDropBranch::SeasonOut)
        );
    }

    #[test]
    fn holds_tom_outside_winter() {
        let mut p = happy_proxies();
        p.self_gender = Gender::Tom;
        p.self_fertility_phase = None;
        p.season = Season::Summer;
        assert_eq!(should_drop_pairing(&p, &config()), None);
    }

    #[test]
    fn drops_queen_in_anestrus() {
        let mut p = happy_proxies();
        p.self_fertility_phase = Some(FertilityPhase::Anestrus);
        assert_eq!(
            should_drop_pairing(&p, &config()),
            Some(PairingDropBranch::SeasonOut)
        );
    }

    #[test]
    fn drops_queen_in_postpartum() {
        let mut p = happy_proxies();
        p.self_fertility_phase = Some(FertilityPhase::Postpartum);
        assert_eq!(
            should_drop_pairing(&p, &config()),
            Some(PairingDropBranch::SeasonOut)
        );
    }

    #[test]
    fn drops_when_both_axes_collapse() {
        let mut p = happy_proxies();
        p.romantic = 0.01;
        p.fondness = 0.05;
        assert_eq!(
            should_drop_pairing(&p, &config()),
            Some(PairingDropBranch::DesireDrift)
        );
    }

    #[test]
    fn holds_when_only_romantic_collapses() {
        // Mates-bonded post-conception cooldown shape — fondness stays
        // strong, romantic dips. Should not drop.
        let mut p = happy_proxies();
        p.bond = Some(BondType::Mates);
        p.romantic = 0.0;
        p.fondness = 0.85;
        assert_eq!(should_drop_pairing(&p, &config()), None);
    }

    #[test]
    fn holds_when_only_fondness_collapses() {
        // Defensive — fondness shouldn't slide alone in normal play, but
        // the floor logic should still hold the Intention since the conjunction
        // hasn't fired.
        let mut p = happy_proxies();
        p.fondness = 0.0;
        p.romantic = 0.6;
        assert_eq!(should_drop_pairing(&p, &config()), None);
    }
}
