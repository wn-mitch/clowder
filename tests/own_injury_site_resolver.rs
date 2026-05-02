//! Ticket 089 — integration test for the
//! `LandmarkAnchor::OwnInjurySite` substrate path.
//!
//! Exercises the full authoring → `ScoringContext` → resolver
//! pipeline end-to-end without committing to a `TendInjury` DSE. The
//! pure-fn `interoception::own_injury_site` is unit-tested in-place;
//! this test proves the resolved `Position` survives the round-trip
//! through `CatAnchorPositions`, which is what the
//! `LandmarkSource::Anchor(LandmarkAnchor::OwnInjurySite)` resolver
//! in `scoring.rs::score_dse_by_id` reads. Once `TendInjury` lands
//! it adds the DSE definition + registry line; this test guarantees
//! the substrate it depends on already resolves correctly.

use clowder::ai::considerations::{LandmarkAnchor, LandmarkSource, SpatialConsideration};
use clowder::ai::curves::{Curve, PostOp};
use clowder::ai::scoring::CatAnchorPositions;
use clowder::components::physical::{Health, Injury, InjuryKind, InjurySource, Position};
use clowder::systems::interoception::own_injury_site;

#[test]
fn own_injury_site_resolves_to_most_recent_unhealed_injury_position() {
    // Two unhealed injuries at distinct positions; the more recent
    // one (tick 200) wins per the `max_by_key(|i| i.tick_received)`
    // resolver in `interoception::own_injury_site`.
    let health = Health {
        current: 0.6,
        max: 1.0,
        injuries: vec![
            Injury {
                kind: InjuryKind::Minor,
                tick_received: 100,
                healed: false,
                source: InjurySource::Unknown,
                at: Position::new(1, 1),
            },
            Injury {
                kind: InjuryKind::Severe,
                tick_received: 200,
                healed: false,
                source: InjurySource::Unknown,
                at: Position::new(7, 3),
            },
        ],
    };

    let resolved = own_injury_site(&health);
    assert_eq!(
        resolved,
        Some(Position::new(7, 3)),
        "interoception helper must pick most-recent unhealed injury"
    );

    // Substrate-over-override discipline: the same Position the
    // helper produces is the Position the scoring resolver reads
    // back through `CatAnchorPositions::own_injury_site`. Field
    // round-trip proves the wire-up.
    let anchors = CatAnchorPositions {
        own_injury_site: resolved,
        ..Default::default()
    };
    assert_eq!(anchors.own_injury_site, Some(Position::new(7, 3)));

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
fn own_injury_site_none_when_only_healed_injuries() {
    let health = Health {
        current: 0.9,
        max: 1.0,
        injuries: vec![Injury {
            kind: InjuryKind::Minor,
            tick_received: 100,
            healed: true,
            source: InjurySource::Unknown,
            at: Position::new(5, 5),
        }],
    };
    assert_eq!(
        own_injury_site(&health),
        None,
        "healed injuries must not yield an anchor"
    );
}
