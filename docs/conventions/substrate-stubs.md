# Substrate stubs are forbidden

The umbrella rule that catches a class of silent-failure bugs: a marker (or InfluenceMap, or DSE-required marker) is *declared* in one file but never *populated* in the place a reader actually looks. Eligibility silently rejects every cat for the entire soak.

Three CI scripts enforce three flavors of the same rule. `just check` runs all three.

## Rule 1 — Markers ship with reader + writer

Every marker in `src/components/markers.rs` ships with **both** a reader (`Has<>` / `With<>` / `X::KEY`) **and** a writer (`.insert(X)` / `.remove::<X>()` / `MarkerSnapshot::set_*`) in the same commit, or with an entry in `scripts/substrate_stubs.allowlist` naming the wiring ticket.

Enforced by `scripts/check_substrate_stubs.sh`.

## Rule 2 — DSE-required markers ship with snapshot population

Every marker referenced by a DSE eligibility filter (`.require(M::KEY)` under `src/ai/dses/`) ships with a `set_entity(M::KEY, …)` or `set_colony(M::KEY, …)` call in `src/systems/goap.rs::evaluate_and_plan` — the populator the eligibility filter actually reads via `MarkerSnapshot.has(...)`.

This is the third clause that closes the 209 / 084 silent-fail class: marker authored + reader-via-`Has<>` + DSE `.require`, but never copied into the snapshot.

Enforced by `scripts/check_marker_snapshot_wiring.sh` (ticket 217).

## Rule 3 — InfluenceMaps ship with registry population

Every `impl InfluenceMap for <Type>` in `src/` ships with a `populate_influence_map_registry` call in `src/plugins/simulation.rs`, or with an allowlist entry naming the wiring ticket.

Enforced by `scripts/check_influence_map_registry.sh`. Precedent: ticket 207.

> **Flaky note:** `check_influence_map_registry.sh` is non-deterministic and reports false offenders. Retry up to ~10× before treating a failure as real. See memory `learning_check_influence_map_flaky.md`.

## Catalogue

Allowlisted stubs and their wiring tickets are catalogued in [`docs/open-work/landed/160-substrate-stub-catalogue.md`](../open-work/landed/160-substrate-stub-catalogue.md).

## Precedents

- **158** — original substrate-stub class identified.
- **209 / 084** — marker authored + reader-via-`Has<>` + DSE `.require`, never copied into snapshot. The motivating case for Rule 2.
- **207** — InfluenceMap registry stub class. The motivating case for Rule 3.
- **217** — added the marker-snapshot-wiring script.
