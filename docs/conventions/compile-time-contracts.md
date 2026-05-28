# Prefer compile-time contracts to runtime checks

When the same invariant can be expressed as a trait method, exhaustive `match`, sub-trait, distributed slice, or const fn signature, choose the type-level form.

## The cost hierarchy

| Enforcement mechanism | Ongoing review cost | Failure mode |
|---|---|---|
| Compile-time (trait, exhaustive match, distributed slice, const fn) | **Zero.** Adding a violating variant is a compile error. | Caught before any code runs. |
| CI script (substrate-stub etc.) | Low. Script must keep up with new patterns. | Caught in CI, before merge. |
| Runtime divergence | **Highest.** Silent failure, weeks of investigation. | Surfaces as drift in production data — tickets 436 / 437 pattern. |

## Worked example — ticket 438

The `CatDse` sub-trait requires `fn action() -> Action`. Registering a cat DSE without naming its `Action` variant is a compile error. Round-trip enforcement comes from a reverse-direction const fn `dse_id_for_action`.

The `cat_dses` registry is populated from a `linkme::distributed_slice` so the parallel hand-maintained list that previously diverged from registration is structurally gone — adding a DSE means writing one constructor + one registration entry in the same file, both required by construction.

Compare to the prior shape: a registry call in one file + a `score_dse_by_id` dispatcher branch in another, where forgetting either silently inerted the DSE. The compile-time refactor eliminated the failure mode entirely.

## When the trait shape can't carry it

CI scripts (`check_substrate_stubs.sh` / `check_marker_snapshot_wiring.sh` / `check_influence_map_registry.sh`) extend the discipline to CI when the invariant crosses module boundaries the type system can't see. See [`substrate-stubs.md`](substrate-stubs.md).

## Precedents

- **217 / 319** — CI-script enforcement (substrate stub + method registry).
- **367 / 437 / 438** — trait + distributed-slice enforcement of dispatch-by-construction.
- **436 / 437** — the dispatcher-missing silent-failure class that motivated 438's compile-time fix.
