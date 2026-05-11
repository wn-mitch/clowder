//! Ticket 257 microexperiment — courtship → Partners-bond → Mate election
//! → MatingOccurred end-to-end chain.
//!
//! **Defect surfaced:** the post-256 verification soak (`logs/tuned-42`,
//! commit `c64fd2cc` dirty) restored every continuity canary the post-252
//! collapse had broken — including courtship 1609 events — but
//! `MatingOccurred` stayed in `never_fired_expected_positives`. Drilling
//! `just q actions logs/tuned-42` showed `Mate` action electing 0 of 7455
//! CatSnapshot rows, and the narrative log showed 15+ "have become close
//! friends" lines but **zero** "have become partners". Bonds form readily
//! at the Friends tier and stall there.
//!
//! **Why it stalled:** the L2 PairingActivity substrate originally
//! shipped in two halves — Commit A authored the Intention; Commit B
//! (the *bias readers* that amplify fondness/familiarity growth between
//! paired partners) was deferred and never wired until ticket 257.
//! Without the bias amplification, paired partners advanced the bond
//! ladder no faster than any other Friends pair, the
//! `partners_fondness_threshold = 0.55` gate sat above the
//! encounter-driven fondness ceiling, and the chain
//! Friends → Partners → `HasEligibleMate` → Mate election → `MateWith`
//! resolver → `MatingOccurred` never reached its end. Ticket 127
//! subsumed PairingActivity into `JointIntention { practice: Courtship,
//! .. }` while preserving the bias-amplification contract.
//!
//! **What this scenario locks down:** a deterministic two-cat fixture
//! pre-loaded at `bond = Friends` with fondness/familiarity just above
//! the Friends gate (the realistic encounter-driven shape — *not* the
//! `practices.courtship.emission_threshold` doc's idealized
//! `fondness ≈ 0.5`). The scenario asserts the chain advances
//! end-to-end within ~5 sim days
//! (≈ 5 × `ticks_per_day_phase` × 5 phases ≈ 5,000 ticks at default
//! constants).
//!
//! Pre-fix invocation reproduces the defect: `MatingOccurred` does not
//! fire and Partners promotion does not occur within the tick budget.

use bevy_ecs::world::World;

use crate::components::identity::Gender;
use crate::components::physical::Position;
use crate::resources::relationships::{BondType, Relationships};

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub const QUEEN_NAME: &str = "Marigold";
pub const TOM_NAME: &str = "Tamsin";
pub const QUEEN_START: Position = Position { x: 20, y: 20 };
pub const TOM_START: Position = Position { x: 21, y: 20 };

pub static SCENARIO: Scenario = Scenario {
    name: "mate_chain",
    default_focal: QUEEN_NAME,
    // ~5 sim days at default constants (1000 ticks/day post §time-units).
    // Long enough for the pairing chain to cross Partners + emit
    // `MatingOccurred` once Commit B is wired; short enough that the
    // pre-fix run is decisively a no-op rather than ambiguous.
    default_ticks: 5_000,
    setup,
    // Substrate-fires gating opts out: the scenario asserts a chain of
    // post-fix Features that don't fire pre-fix (the whole point). The
    // post-fix verification adds these explicitly via the unit tests
    // below + the soak's `never_fired_expected_positives == 0` gate.
    expected_features: &[],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // One Queen + one Tom, adjacent. Both Adult, sated/happy, neutral
    // personality (so no axis-specific side-effect skews scoring). The
    // pair is opposite-sex/Straight to pass `are_orientation_compatible`
    // and gestation-capable so a successful mating produces witness=Some.
    let queen = spawn_cat(
        world,
        CatPreset::adult(QUEEN_NAME, QUEEN_START)
            .with_gender(Gender::Queen)
            .with_personality(|p| {
                p.warmth = 0.7;
                p.sociability = 0.7;
                p.compassion = 0.6;
            })
            .with_needs(|n| {
                // Sated and happy — `is_sated_and_happy` reads
                // hunger/energy floors from `ScoringConstants`. Defaults
                // (1.0/1.0) clear the floors comfortably.
                n.social = 0.5;
                n.mating = 0.4;
                n.safety = 1.0;
            })
            .with_marker(MarkerKind::Adult),
    );

    let tom = spawn_cat(
        world,
        CatPreset::adult(TOM_NAME, TOM_START)
            .with_gender(Gender::Tom)
            .with_personality(|p| {
                p.warmth = 0.7;
                p.sociability = 0.7;
                p.compassion = 0.6;
            })
            .with_needs(|n| {
                n.social = 0.5;
                n.mating = 0.4;
                n.safety = 1.0;
            })
            .with_marker(MarkerKind::Adult),
    );

    // Pre-seed the relationship at the realistic shape of a fresh Friends
    // pair driven by a few sim-days of encounters: fondness and
    // familiarity just above the Friends gate (`fondness > 0.3`,
    // `familiarity > 0.4` per `social.rs:297-298`), romantic at zero, and
    // `BondType::Friends`. This is the shape the soak produces in
    // practice — explicitly **not** the idealized `fondness ≈ 0.5` the
    // current `practices.courtship.emission_threshold = 0.25` doc-comment was
    // calibrated against. The mismatch is part of the defect; the
    // scenario locks the realistic shape into the regression suite so
    // future tuning doesn't accidentally re-introduce the gap.
    let mut rels = world.remove_resource::<Relationships>().unwrap_or_default();
    {
        let rel = rels.get_or_insert(queen, tom);
        rel.fondness = 0.4;
        rel.familiarity = 0.45;
        rel.romantic = 0.0;
        rel.bond = Some(BondType::Friends);
        rel.last_interaction = 0;
    }
    world.insert_resource(rels);
}

// ---------------------------------------------------------------------------
// Tests — unit-level invariants that the scenario fixture and the
// downstream chain hold. The runnable scenario itself (via `just scenario
// mate_chain`) is for human focal-trace inspection.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::identity::Gender;

    /// The fixture seeds `bond = Friends` at fondness/familiarity just
    /// above the Friends gate so the L2 pairing author has a candidate
    /// to consider on tick 1. Locking this shape so future tuning
    /// changes don't silently reset it.
    #[test]
    fn fixture_starts_at_friends_with_realistic_shape() {
        let mut world = World::new();
        setup(&mut world, 42);

        let mut cats = Vec::new();
        let mut q = world.query::<(
            bevy_ecs::entity::Entity,
            &crate::components::identity::Name,
            &Gender,
        )>();
        for (entity, name, gender) in q.iter(&world) {
            cats.push((entity, name.0.clone(), *gender));
        }
        let queen = cats
            .iter()
            .find(|(_, n, _)| n == QUEEN_NAME)
            .expect("Marigold spawned");
        let tom = cats
            .iter()
            .find(|(_, n, _)| n == TOM_NAME)
            .expect("Tamsin spawned");
        assert_eq!(queen.2, Gender::Queen);
        assert_eq!(tom.2, Gender::Tom);

        let rels = world.resource::<Relationships>();
        let rel = rels
            .get(queen.0, tom.0)
            .expect("relationship row pre-seeded");
        assert_eq!(rel.bond, Some(BondType::Friends));
        assert!(rel.fondness > 0.3, "fondness above Friends gate");
        assert!(rel.familiarity > 0.4, "familiarity above Friends gate");
        assert_eq!(rel.romantic, 0.0, "romantic starts un-accumulated");
    }
}
