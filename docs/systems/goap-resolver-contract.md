# GOAP step resolver contract

Every `pub fn resolve_*` under `src/steps/**` returns `StepOutcome<W>` (`src/steps/outcome.rs`) — module rustdoc carries the witness-shape rationale. The contract makes "silent-advance with no real-world effect" a **type error**: callers MUST route Feature emission through `record_if_witnessed`, never directly on `StepResult::Advance`.

## Five required rustdoc headings

Every `pub fn resolve_*` carries all five. Grepped by `scripts/check_step_contracts.sh` via `just check`.

```text
/// **Real-world effect** — what this mutates when it succeeds.
/// **Plan-level preconditions** — `StatePredicate`s the planner guarantees before this step runs.
/// **Runtime preconditions** — what this checks internally; what happens if the check fails
///                              (MUST NOT return witnessed Advance when the effect didn't happen).
/// **Witness** — the `StepOutcome<W>` shape and what `W` records.
/// **Feature emission** — which `Feature::*` the caller passes to `record_if_witnessed`
///                         (Positive / Neutral / Negative).
```

## Exemplars

- `src/steps/disposition/cook.rs`
- `src/steps/disposition/feed_kitten.rs`
- `src/steps/building/tend.rs`

## Never-fired canary

New positive `Feature::*` must be classified in `Feature::expected_to_fire_per_soak()` (`src/resources/system_activation.rs`). Returning `true` enrolls the feature in the seed-42 canary; rare-legend events (`ShadowFoxBanished`, `FateAwakened`, …) return `false` and are exempt.

**Default new variants to `false`.** Lift to `true` only after a healthy seed-42 soak observes ≥1× firing. Asymmetric cost: false→true is a one-line edit; true→false costs a full re-soak. The 1:1 sibling rule applies — a new Feature inherits its gameplay-event sibling's classification. (Memory: `feedback_new_features_default_expected_false`.)

## Hard survival gates

The canary surfaces on the seed-42 deep-soak; see [`../discipline/verification.md`](../discipline/verification.md) for the gate list (`Starvation == 0`, `ShadowFoxAmbush <= 10`, footer line written, `never_fired_expected_positives == 0`).
