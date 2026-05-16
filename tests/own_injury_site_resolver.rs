//! Ticket 089 / 095 — integration test for the
//! `LandmarkAnchor::OwnInjurySite` substrate path.
//!
//! Ticket 095 Phase 1 Stage B retired `Health.injuries` in favor of the
//! anatomical `CatBodyModel` substrate. The pure-fn
//! `interoception::own_injury_site` is unit-tested in-place; this test
//! proves the resolved `Position` survives the round-trip through
//! `CatAnchorPositions`, which is what the
//! `LandmarkSource::Anchor(LandmarkAnchor::OwnInjurySite)` resolver in
//! `scoring.rs::score_dse_by_id` reads. Once `TendInjury` lands it adds
//! the DSE definition + registry line; this test guarantees the
//! substrate it depends on already resolves correctly.

use clowder::ai::considerations::{LandmarkAnchor, LandmarkSource, SpatialConsideration};
use clowder::ai::curves::{Curve, PostOp};
use clowder::ai::scoring::CatAnchorPositions;
use clowder::components::body_zones::{BodyPart, CatBodyModel};
use clowder::components::physical::Position;
use clowder::resources::sim_constants::SimConstants;
use clowder::systems::interoception::own_injury_site;

#[test]
fn own_injury_site_resolves_to_cat_position_when_wounded() {
    let c = SimConstants::default();
    let mut model = CatBodyModel::default();
    // Tissue damage 0.3 → Wounded tier (≥ 0.26 threshold).
    model.apply_damage(
        BodyPart::Throat,
        0.3,
        &c.combat.body_zone_condition_thresholds,
        &c.combat.body_zone_permanent_at_destroyed,
    );

    let cat_pos = Position::new(7, 3);
    let resolved = own_injury_site(&model, cat_pos);
    assert_eq!(
        resolved,
        Some(cat_pos),
        "wounded body should anchor on the cat's current position"
    );

    // Substrate-over-override discipline: the same Position the helper
    // produces is the Position the scoring resolver reads back through
    // `CatAnchorPositions::own_injury_site`. Field round-trip proves
    // the wire-up.
    let anchors = CatAnchorPositions {
        own_injury_site: resolved,
        ..Default::default()
    };
    assert_eq!(anchors.own_injury_site, Some(cat_pos));

    // The synthetic SpatialConsideration that the future TendInjury
    // DSE will own — verifies the new `LandmarkAnchor` variant is
    // declarable as a `LandmarkSource::Anchor(...)` from outside the
    // crate. Compilation alone is the assertion; an empty body would
    // be just as load-bearing as a runtime check, since the variant
    // is a unit enum.
    let sc = SpatialConsideration::new(
        "tend_injury_distance",
        LandmarkSource::Anchor(LandmarkAnchor::OwnInjurySite),
        10.0,
        Curve::Composite {
            inner: Box::new(Curve::Polynomial {
                exponent: 2,
                divisor: 1.0,
            }),
            post: PostOp::Invert,
        },
    );
    assert!(matches!(
        sc.landmark,
        LandmarkSource::Anchor(LandmarkAnchor::OwnInjurySite)
    ));
}

#[test]
fn own_injury_site_none_when_only_bruised() {
    let c = SimConstants::default();
    let mut model = CatBodyModel::default();
    // Bruised tier (tissue 0.1) is below the Wounded gate (0.26).
    model.apply_damage(
        BodyPart::Tail,
        0.1,
        &c.combat.body_zone_condition_thresholds,
        &c.combat.body_zone_permanent_at_destroyed,
    );
    assert_eq!(
        own_injury_site(&model, Position::new(5, 5)),
        None,
        "Bruised-only body must not yield an injury anchor"
    );
}
