---
id: 090
title: L4/L5 self-perception — mastery-confidence, purpose-clarity, esteem-distress
status: ready
cluster: ai-substrate
added: 2026-04-30
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

### The epistemic gap

Surfaced during 087's planning audit. The `populate_dse_registry` catalog has DSE coverage for Maslow tiers 1 (physiological) and 2 (safety) and partial coverage for 3 (belonging via socials). Tiers 4 (esteem — respect, mastery) and 5 (self-actualization — purpose) have *zero* DSE coverage, yet those needs decay (`src/resources/sim_constants.rs` defines `respect_base_drain`, `mastery_base_drain`, `purpose_base_drain`). They drain to zero with no DSE pulling them.

The drain isn't the root problem. The root problem is that a cat with collapsing esteem has no *understanding* of that state — no internal representation that the IAUS can act on. Without perception, there can be no response. The data model tracks the decay; the cat doesn't know it's happening.

### The apophenic layer

Ticker 087 covered the physiological tier: pain, fight-or-flight, body distress. Players watch a wounded cat flee and read adrenaline. That's the oldest emotional layer — survival hardware, universally legible.

This ticket moves up to the *social animal's inner life*. Mastery, purpose, esteem — the emotional narratives that separate a creature with an inner world from one that merely eats and sleeps. When these are wired correctly through the IAUS into behavior:

- A cat with depleted `mastery_confidence` *seeks situations where it can feel competent*. Players read: an uncertain apprentice, looking for someone to learn from.
- A cat with `purpose_clarity = 0.0` is *directionless*. Players read: existential drift — technically alive, going through motions, waiting for something to matter.
- A cat with high `esteem_distress` *acts out*. Players read: wounded pride, craving recognition, picking the wrong fights.

These projections are apophenia — players impose human emotional narrative onto emergent behavior. The substrate makes it honest: the cat really is experiencing what the player is reading. The perception layer is the mechanism that connects raw need-decay to agentic self-knowledge to observable behavior.

This ticket builds the perception half. The downstream DSEs (separate tickets) supply the behavioral response. Neither works without the other; perception comes first.

### Scope boundary

Closing the catalog gap requires both perception (knowing the cat's L4/L5 state) and DSEs (acting on it). This ticket is *only* about perception — the substrate that publishes `mastery_confidence` / `purpose_clarity` / `esteem_distress` scalars and the `LowMastery` / `LackingPurpose` / `EsteemDistressed` ZST markers. The downstream DSE catalog work is a separate ticket (or tickets) under the §L2.10 enumeration, and depends on this one.

This is *not* "balance tuning on refactor-affected metrics" (deferred per CLAUDE.md). It's substrate plumbing — perception coverage of needs that already exist in the data model.

## Substrate-over-override pattern

Part of the substrate-over-override thread (see [093](093-substrate-over-override-epic.md)) — extends 087's substrate to higher Maslow tiers, on-thread but not directly retiring an override.

**Hack shape**: similar to [089](089-interoceptive-self-anchors.md), this is substrate-expansion not hack-retirement. The current state — needs decay (respect/mastery/purpose) with no DSE pulling them — is *implicitly* a hack: silently dropping signals because no perception layer publishes them. The cat has no agentic awareness of L4/L5 distress.

**IAUS lever**: `mastery_confidence`, `purpose_clarity`, `esteem_distress` scalars + `LowMastery`, `LackingPurpose`, `EsteemDistressed` ZST markers — perception coverage of Maslow tiers that already exist in the data model. Future L4/L5 DSEs (separate tickets) consume these without overrides.

**Sequencing**: blocked-by 087 (landed at fc4e1ab). Perception-only ticket; downstream DSE catalog work depends on it but is out-of-scope here.

**Canonical exemplar**: 087 (CriticalHealth interrupt → `pain_level` + `body_distress_composite` axes, landed at fc4e1ab).

## Scope

- Extend `src/systems/interoception.rs` (087) with three pure derivation functions: `mastery_confidence`, `purpose_clarity`, `esteem_distress`.
- Add three ZST markers to `src/components/markers.rs` after `BodyDistressed`: `LowMastery`, `LackingPurpose`, `EsteemDistressed`.
- Extend `author_self_markers` in `interoception.rs` to insert/remove the three new markers transitionally. The query gains `&Skills` and `Option<&Aspirations>`.
- Add three threshold constants to `DispositionConstants` in `src/resources/sim_constants.rs`.
- Add three fields to `ScoringContext` in `src/ai/scoring.rs`; add three `m.insert(...)` calls to `ctx_scalars()`.
- Update both `ScoringContext` construction sites (`goap.rs`, `disposition.rs`) to populate the new fields.
- *No DSE work in this ticket.*

## Implementation Plan

Execute in this order to keep the repo green at every commit:

1. **`sim_constants.rs`** — add three fields + default fns to `DispositionConstants`.
2. **`markers.rs`** — add three ZST structs + impl blocks after `BodyDistressed`.
3. **`interoception.rs`** — add three pure derivation fns; update imports; extend `author_self_markers` query + loop; update existing test spawn calls to include `Skills::default()` (required — `&Skills` becomes a query member); add new unit tests.
4. **`scoring.rs`** — add three fields to `ScoringContext`; add three `m.insert()` calls to `ctx_scalars()`.
5. **`goap.rs`** — add three fields to the `ScoringContext { … }` literal.
6. **`disposition.rs`** — same.
7. `just check` to verify step-resolver and time-unit linters pass; `just test` to run unit tests.

## Derivation Formulas

### `mastery_confidence(skills: &Skills) -> f32`

```clowder/src/systems/interoception.rs#L1-1
(skills.hunting + skills.foraging + skills.herbcraft + skills.building + skills.combat + skills.magic)
    / 6.0
```

Equivalently `skills.total() / 6.0` — `Skills::total()` already sums all six fields. Divide by 6 (the number of skill axes) to get the arithmetic mean. `.clamp(0.0, 1.0)` is applied at return because `Skills` fields have no hard upper bound; in practice diminishing returns keep them near [0, 1] but the clamp is a correctness guard.

**IAUS consumption** — a future DSE like "Pursue-mastery" uses a `Logistic { midpoint: 0.4, steepness: 8 }` consideration on this axis: low mastery_confidence → high urgency to practice. A `Consideration::Scalar { key: "mastery_confidence" }` with an `Invert + Logistic` composite drives "the less competent I feel, the more I want to improve."

**Narrative rationale** — felt-competence is what separates the experienced hunter from the fumbling novice. Both may be alive; only one *knows what it's doing*. A cat at `mastery_confidence ≈ 0.07` (freshly spawned, default skills) is the apprentice who hasn't found its footing yet. A cat at `≈ 0.5` has range — it can hunt, build, forage, and knows it. The IAUS converts this self-knowledge into preference: low confidence → seek skill-building situations → behavior players read as learning, striving, finding one's domain. Without this scalar, the cat has no agentic relationship to its own competence; it just decays.

---

### `purpose_clarity(aspirations: Option<&Aspirations>) -> f32`

```clowder/src/systems/interoception.rs#L1-1
if aspirations.map_or(false, |a| !a.active.is_empty()) { 1.0 } else { 0.0 }
```

Binary {0.0, 1.0} — the cat either has at least one active aspiration chain or it doesn't. `None` (no `Aspirations` component) maps to 0.0; `Some` with an empty `active` vec also maps to 0.0.

**IAUS consumption** — used as a gate-axis: a `Consideration::Scalar { key: "purpose_clarity", curve: Linear { slope: 1.0, intercept: 0.0 } }` multiplies the score of any "pursue-aspiration" DSE to zero when the cat has no active aspiration. The binary nature is intentional — gradient intermediate states (e.g., "aspiration half-complete") are encoded in `ActiveAspiration::progress`, not in this scalar. This scalar answers *whether* the cat has direction, not *how far along* it is.

**Narrative rationale** — the difference between a cat with purpose and one without is not degree; it is kind. This is why the scalar is binary. A cat that is 40% through its hunting chain still *has* direction — it knows what the next step is. A cat with no active aspiration is existentially adrift: technically functional, going through the colony's motions, but not working toward anything of its own. `LackingPurpose` makes that state visible to the IAUS. Future DSEs can then produce observable restlessness — wandering further, sleeping more, socializing with an edge — that players read as "that cat seems lost." The perception layer makes the cat's inner life legible.

---

### `esteem_distress(needs: &Needs) -> f32`

```clowder/src/systems/interoception.rs#L1-1
(1.0 - needs.respect).max(1.0 - needs.mastery).clamp(0.0, 1.0)
```

Max of the two L4 deficit axes. Parallels `body_distress_composite`'s max-of-deficits semantics: any one L4 axis going critical is enough to signal esteem distress, regardless of the other. Both `needs.respect` and `needs.mastery` are satisfaction scalars in [0, 1] (high = need met, low = deficient).

**IAUS consumption** — a future "Seek-validation" DSE uses `Logistic { midpoint: 0.5, steepness: 6 }` on this axis: above ~0.5 distress, the cat prioritizes esteem-restoring behaviors. A `ScoreModifier` (analog of the `BodyDistressPromotion` modifier from 088) can apply a lift to all esteem-relevant DSEs when `esteem_distress` is high.

**Narrative rationale** — esteem distress is the wounded social animal. A cat whose respect has collapsed feels unseen; a cat whose mastery has collapsed feels incompetent. Either is enough to mark crisis — the max-of-deficits formula preserves that OR semantics. When `EsteemDistressed` fires, future DSEs can produce behavior players read as wounded pride: picking fights that aren't necessary, withdrawing from the colony, seeking conspicuous contributions to be witnessed. The max-of-deficits design also means the distress resolves asymmetrically — restoring respect doesn't fix the mastery wound. Both axes need recovery, which drives richer behavioral variety than a mean would.

---

## SimConstants Additions

All three constants belong in `DispositionConstants` in `src/resources/sim_constants.rs`, inserted **after the `pain_normalization_max` block** (after line ~L1984 in the struct, after the `default_pain_normalization_max` function in the default-fn section).

### Struct fields (after `pain_normalization_max`)

```clowder/src/resources/sim_constants.rs#L1984-1984
/// Ticket 090 — `LowMastery` ZST marker gate. Fires when
/// `mastery_confidence` (mean of all six `Skills` fields) is strictly below
/// this threshold. Default 0.35: a cat averaging 35% skill across all six
/// axes has meaningfully low felt-competence; above 35% the cat is coping.
/// Freshly spawned cats start near 0.07 (default skills), well below this —
/// `LowMastery` fires for all novice cats and clears as they practise.
#[serde(default = "default_low_mastery_threshold")]
pub low_mastery_threshold: f32,
/// Ticket 090 — `LackingPurpose` ZST marker gate. Fires when
/// `purpose_clarity` is strictly below this threshold. Default 0.5: since
/// `purpose_clarity` is binary {0.0, 1.0}, the 0.5 midpoint is the
/// conventional binary-signal threshold — fires exactly when the cat has no
/// active aspiration. Raise above 1.0 to permanently suppress the marker;
/// lower to 0.0 to suppress permanently in the other direction. Both are
/// nonsensical; 0.5 is the only meaningful value for this binary signal.
#[serde(default = "default_lacking_purpose_threshold")]
pub lacking_purpose_threshold: f32,
/// Ticket 090 — `EsteemDistressed` ZST marker gate. Fires when
/// `esteem_distress` (max of L4 deficits) strictly exceeds this threshold.
/// Default 0.55: intentionally lower than `body_distress_threshold` (0.6)
/// because L4 distress is chronic / slow-onset, not acute. A cat whose
/// respect or mastery need is below 45% satisfied is meaningfully
/// esteem-distressed; physiological distress sets in later because urgent
/// survival is a more immediate signal.
#[serde(default = "default_esteem_distressed_threshold")]
pub esteem_distressed_threshold: f32,
```

### Default functions (after `default_pain_normalization_max`)

```clowder/src/resources/sim_constants.rs#L2182-2184
fn default_low_mastery_threshold() -> f32 {
    0.35
}

fn default_lacking_purpose_threshold() -> f32 {
    0.5
}

fn default_esteem_distressed_threshold() -> f32 {
    0.55
}
```

### `impl Default for DispositionConstants` (after the `pain_normalization_max: default_pain_normalization_max()` line)

```clowder/src/resources/sim_constants.rs#L2510-2511
low_mastery_threshold: default_low_mastery_threshold(),
lacking_purpose_threshold: default_lacking_purpose_threshold(),
esteem_distressed_threshold: default_esteem_distressed_threshold(),
```

---

## Markers

Add the three structs to `src/components/markers.rs` **directly after `BodyDistressed`** (after line ~L153). Follow the exact pattern of the 087 markers: `#[derive(Component, Debug, Clone, Copy)]`, rustdoc header, impl block with `pub const KEY: &str`.

```clowder/src/components/markers.rs#L150-153
/// Mean skill level across all six `Skills` fields below
/// `DispositionConstants::low_mastery_threshold`. The cat's felt-competence
/// is meaningfully low — drives future "seek-mastery" / "pursue-practice"
/// DSEs. Note: fires for all freshly spawned cats (default mean ~0.07) and
/// clears as skills grow past the threshold. Authoring:
/// `interoception::author_self_markers`. Ticket 090.
#[derive(Component, Debug, Clone, Copy)]
pub struct LowMastery;
impl LowMastery {
    pub const KEY: &str = "LowMastery";
}

/// No active aspiration (`Aspirations::active.is_empty()` or no `Aspirations`
/// component). The cat has no directed striving — drives future
/// "adopt-aspiration" / "pursue-purpose" DSEs. Authoring:
/// `interoception::author_self_markers`. Ticket 090.
#[derive(Component, Debug, Clone, Copy)]
pub struct LackingPurpose;
impl LackingPurpose {
    pub const KEY: &str = "LackingPurpose";
}

/// Max of L4 deficits — `max(1 - respect, 1 - mastery)` exceeds
/// `DispositionConstants::esteem_distressed_threshold`. Parallels
/// `BodyDistressed` for the esteem tier: the unified "I feel undervalued
/// or incompetent" signal. Authoring: `interoception::author_self_markers`.
/// Ticket 090.
#[derive(Component, Debug, Clone, Copy)]
pub struct EsteemDistressed;
impl EsteemDistressed {
    pub const KEY: &str = "EsteemDistressed";
}
```

**`markers.rs` test extensions** — add the three new markers to the existing `state_markers_queryable` test (spawn an entity with all three, assert each is queryable). Add them to `faction_overlay_marker_keys_unique` (or a new `l4_l5_marker_keys_unique` test) to assert the three KEYs are distinct from all existing keys.

---

## Pure Function Signatures

Add to `src/systems/interoception.rs`, after `body_distress_composite`:

```clowder/src/systems/interoception.rs#L1-1
/// Mean of all six `Skills` field values, normalized into `[0, 1]`. High
/// skill → high felt-competence. Uses `Skills::total()` / 6.0; clamped
/// because `Skills` fields have no hard upper bound.
///
/// Default cats (total ≈ 0.4) → ~0.07. Practised cat (all fields ~0.6) → ~0.6.
pub fn mastery_confidence(skills: &Skills) -> f32 {
    (skills.total() / 6.0).clamp(0.0, 1.0)
}

/// `1.0` if the cat has at least one active aspiration, `0.0` if none or if
/// the `Aspirations` component is absent. Binary signal — presence of
/// directed striving, not degree of progress.
pub fn purpose_clarity(aspirations: Option<&Aspirations>) -> f32 {
    if aspirations.map_or(false, |a| !a.active.is_empty()) {
        1.0
    } else {
        0.0
    }
}

/// Higher of the two L4 (esteem) need deficits: `max(1 - respect, 1 - mastery)`.
/// Parallels `body_distress_composite`'s max-of-deficits semantics. Range [0, 1].
pub fn esteem_distress(needs: &Needs) -> f32 {
    (1.0 - needs.respect)
        .max(1.0 - needs.mastery)
        .clamp(0.0, 1.0)
}
```

Imports to add at the top of `interoception.rs`:

```clowder/src/systems/interoception.rs#L1-1
use crate::components::aspirations::Aspirations;
use crate::components::markers::{
    BodyDistressed, EsteemDistressed, LackingPurpose, LowHealth, LowMastery, SevereInjury,
};
use crate::components::skills::Skills;
```

---

## `author_self_markers` System Changes

### Query

The current 6-element tuple becomes 11 elements — within the safe Bevy range (no `SystemParam` bundling needed yet):

```clowder/src/systems/interoception.rs#L1-1
cats: Query<
    (
        Entity,
        &Health,
        &Needs,
        &Skills,
        Option<&Aspirations>,
        Has<LowHealth>,
        Has<SevereInjury>,
        Has<BodyDistressed>,
        Has<LowMastery>,
        Has<LackingPurpose>,
        Has<EsteemDistressed>,
    ),
    Without<Dead>,
>,
```

**Important**: `&Skills` is required (not `Option<&Skills>`). All cats that have `Health` and `Needs` must also have `Skills` — this is the canonical spawn shape. No cats in the production spawner omit `Skills`.

### Constants

After the existing threshold reads, add:

```clowder/src/systems/interoception.rs#L1-1
let low_mastery_threshold = constants.disposition.low_mastery_threshold;
let lacking_purpose_threshold = constants.disposition.lacking_purpose_threshold;
let esteem_distressed_threshold = constants.disposition.esteem_distressed_threshold;
```

### Loop destructuring

```clowder/src/systems/interoception.rs#L1-1
for (
    entity,
    health,
    needs,
    skills,
    aspirations,
    has_low_health,
    has_severe_injury,
    has_body_distressed,
    has_low_mastery,
    has_lacking_purpose,
    has_esteem_distressed,
) in cats.iter()
```

### Gating predicates (after existing `want_body_distressed`)

```clowder/src/systems/interoception.rs#L1-1
let want_low_mastery = mastery_confidence(skills) < low_mastery_threshold;
let want_lacking_purpose = purpose_clarity(aspirations) < lacking_purpose_threshold;
let want_esteem_distressed = esteem_distress(needs) > esteem_distressed_threshold;
```

### Match blocks (after existing three match blocks)

```clowder/src/systems/interoception.rs#L1-1
match (want_low_mastery, has_low_mastery) {
    (true, false) => {
        commands.entity(entity).insert(LowMastery);
    }
    (false, true) => {
        commands.entity(entity).remove::<LowMastery>();
    }
    _ => {}
}
match (want_lacking_purpose, has_lacking_purpose) {
    (true, false) => {
        commands.entity(entity).insert(LackingPurpose);
    }
    (false, true) => {
        commands.entity(entity).remove::<LackingPurpose>();
    }
    _ => {}
}
match (want_esteem_distressed, has_esteem_distressed) {
    (true, false) => {
        commands.entity(entity).insert(EsteemDistressed);
    }
    (false, true) => {
        commands.entity(entity).remove::<EsteemDistressed>();
    }
    _ => {}
}
```

---

## `ScoringContext` Changes

### New fields in `pub struct ScoringContext<'a>` (`src/ai/scoring.rs`)

Add after `body_distress_composite` (the last 087 field, around line ~L226):

```clowder/src/ai/scoring.rs#L226-226
/// Ticket 090 — interoceptive perception. Mean of all six `Skills` field
/// values normalized into `[0, 1]`; `skills.total() / 6.0`. High skill →
/// high felt-competence. Freshly spawned cats ≈ 0.07. Computed via
/// `crate::systems::interoception::mastery_confidence`.
pub mastery_confidence: f32,
/// Ticket 090 — interoceptive perception. `1.0` if the cat has at least one
/// active `ActiveAspiration`, `0.0` if none or if the `Aspirations` component
/// is absent. Binary presence signal — not a gradient. Computed via
/// `crate::systems::interoception::purpose_clarity`.
pub purpose_clarity: f32,
/// Ticket 090 — interoceptive perception. Max of the two L4 (esteem) need
/// deficits: `max(1 - respect, 1 - mastery)`. Parallels
/// `body_distress_composite` for the esteem tier. Range `[0, 1]`. Computed
/// via `crate::systems::interoception::esteem_distress`.
pub esteem_distress: f32,
```

### New inserts in `fn ctx_scalars()` (`src/ai/scoring.rs`)

Add after the `body_distress_composite` insert block (around line ~L435):

```clowder/src/ai/scoring.rs#L435-435
// Ticket 090 — interoceptive perception. L4/L5 Maslow scalars.
// `mastery_confidence` and `esteem_distress` are continuous [0, 1];
// `purpose_clarity` is binary {0.0, 1.0}. All three are pre-computed at
// `ScoringContext` construction by `crate::systems::interoception` helpers.
m.insert(
    "mastery_confidence",
    ctx.mastery_confidence.clamp(0.0, 1.0),
);
m.insert("purpose_clarity", ctx.purpose_clarity.clamp(0.0, 1.0));
m.insert("esteem_distress", ctx.esteem_distress.clamp(0.0, 1.0));
```

---

## `ScoringContext` Construction Site Changes

Both sites (`src/systems/goap.rs` and `src/systems/disposition.rs`) already have `skills` (from `&Skills` in the cat query) and `aspirations` (from `Option<&Aspirations>` in the cat query). No new query bindings are needed. Add the three fields immediately after the `body_distress_composite` assignment at both sites:

```clowder/src/systems/goap.rs#L1302-1302
// Ticket 090 — interoceptive perception. `skills` and `aspirations`
// are already in the cat query; no new binding required.
mastery_confidence: crate::systems::interoception::mastery_confidence(skills),
purpose_clarity: crate::systems::interoception::purpose_clarity(aspirations),
esteem_distress: crate::systems::interoception::esteem_distress(needs),
```

Same block verbatim in `disposition.rs` after the `body_distress_composite` assignment (around line ~L821).

---

## Test Matrix

All tests live in `src/systems/interoception.rs` under `#[cfg(test)]`. Follow the exact structure of the existing test module.

### Breaking change: existing spawn calls

`&Skills` is now a required query member. Every existing test that spawns `(Health { … }, comfortable_needs())` must be updated to `(Health { … }, comfortable_needs(), Skills::default())`. Affected spawn sites:
- `low_health_marker_inserted_below_threshold`
- `low_health_marker_clears_when_healed`
- `severe_injury_marker_only_for_unhealed_severe`
- `body_distressed_marker_responds_to_any_axis`
- `dead_cats_skipped`

Also extend `dead_cats_skipped` to assert that `LowMastery`, `LackingPurpose`, and `EsteemDistressed` are absent on the dead entity.

### Pure function tests

| Test name | Input | Expected |
|---|---|---|
| `mastery_confidence_zero_skills` | All fields 0.0 | 0.0 |
| `mastery_confidence_full_skills` | All fields 1.0 | 1.0 |
| `mastery_confidence_default_skills` | `Skills::default()` | `(0.4 / 6.0).clamp(0.0, 1.0)` ≈ 0.0667 |
| `mastery_confidence_clamped_above_one` | All fields 2.0 | 1.0 (clamped) |
| `mastery_confidence_partial_mean` | hunting=0.6, others 0.0 | 0.1 |
| `purpose_clarity_none` | `None` | 0.0 |
| `purpose_clarity_empty_active` | `Some(Aspirations { active: vec![], … })` | 0.0 |
| `purpose_clarity_nonempty_active` | `Some(Aspirations { active: vec![<one aspiration>], … })` | 1.0 |
| `esteem_distress_full_needs` | respect=1.0, mastery=1.0 | 0.0 |
| `esteem_distress_both_zero` | respect=0.0, mastery=0.0 | 1.0 |
| `esteem_distress_takes_max` | respect=0.3, mastery=0.8 | 0.7 (respect axis dominates) |
| `esteem_distress_takes_max_other_way` | respect=0.9, mastery=0.2 | 0.8 (mastery axis dominates) |
| `esteem_distress_clamps` | respect=-0.1, mastery=1.1 | 1.0 (clamped) |

For `purpose_clarity_nonempty_active`, construct `ActiveAspiration` minimally:
```clowder/src/systems/interoception.rs#L1-1
ActiveAspiration {
    chain_name: "TestChain".to_string(),
    domain: crate::components::aspirations::AspirationDomain::Hunting,
    current_milestone: 0,
    progress: 0,
    adopted_tick: 0,
    last_progress_tick: 0,
}
```

### Marker lifecycle tests (World + Schedule)

Each test uses the existing `setup_world()` helper and spawns with `(Health, Needs, Skills, [optional Aspirations])`.

| Test name | Setup | Tick | Assert |
|---|---|---|---|
| `low_mastery_fires_for_default_skills` | `Skills::default()` (mean ≈ 0.067 < 0.35) | 1 | `LowMastery` present |
| `low_mastery_clears_when_skilled` | Start with default; then set all fields to 0.8 | 2nd tick | `LowMastery` absent |
| `low_mastery_boundary_at_threshold` | All skills = 0.35 (mean = 0.35, NOT < threshold) | 1 | `LowMastery` absent (strict `<`) |
| `lacking_purpose_fires_without_aspirations_component` | No `Aspirations` on entity | 1 | `LackingPurpose` present |
| `lacking_purpose_fires_with_empty_active` | `Aspirations { active: vec![], completed: vec![] }` | 1 | `LackingPurpose` present |
| `lacking_purpose_clears_when_aspiration_adopted` | Start without `Aspirations`; then insert with one active aspiration | 2nd tick | `LackingPurpose` absent |
| `esteem_distressed_fires_when_respect_low` | `needs.respect = 0.3`, `needs.mastery = 0.9` → distress = 0.7 > 0.55 | 1 | `EsteemDistressed` present |
| `esteem_distressed_fires_when_mastery_low` | `needs.respect = 0.9`, `needs.mastery = 0.2` → distress = 0.8 > 0.55 | 1 | `EsteemDistressed` present |
| `esteem_distressed_absent_when_needs_satisfied` | `needs.respect = 0.6`, `needs.mastery = 0.6` → distress = 0.4 < 0.55 | 1 | `EsteemDistressed` absent |
| `esteem_distressed_clears_on_recovery` | Start distressed (respect=0.3); then raise to 0.7 | 2nd tick | `EsteemDistressed` absent |
| `dead_cats_skipped` *(extend existing)* | Existing dead cat spawn + `Skills::default()` | 1 | `LowMastery`, `LackingPurpose`, `EsteemDistressed` all absent (in addition to existing assertions) |

**Transitional-only check**: for `low_mastery_fires_for_default_skills`, run the schedule *twice* without changing state. The second run must not attempt to re-insert (the `match` arm `(true, true) => {}` covers this — assert no panic/duplicate component error). Bevy insert is idempotent at the ECS level but the transition guard prevents the `commands.entity(entity).insert()` call from firing on steady state, which is the architectural invariant to preserve.

---

## Verification

Since no DSEs consume the new scalars or markers yet, there is **no behavior change**. The verification target is: the system runs without panic, the scalars land in `ctx_scalars()`, and the markers are authored on cats with appropriate state.

### Soak gate

```clowder/docs/diagnostics/log-queries.md#L1-1
just soak 42
just verdict logs/tuned-42/
```

Expected: exit 0. `never_fired_expected_positives == 0` trivially passes — no new positive `Feature::*` variants are added in this ticket (perception, not behavior). No balance drift expected.

### Scalar presence check (focal trace)

Run a focal-cat trace for a freshly spawned cat (low skills, no aspirations):

```clowder/docs/diagnostics/log-queries.md#L1-1
just soak-trace 42 <name-of-any-young-cat>
```

In the resulting `trace.jsonl`, filter for entries that carry the scoring-context scalar dump. The three new keys must be present:

```clowder/docs/diagnostics/log-queries.md#L1-1
jq 'select(.scalars != null and (.scalars | has("mastery_confidence")))
    | {tick, cat: .cat, mc: .scalars.mastery_confidence, pc: .scalars.purpose_clarity, ed: .scalars.esteem_distress}' \
  logs/tuned-42/trace.jsonl \
  | head -5
```

Expected: at least one row, with `mc` ≈ 0.07 and `pc` = 0.0 for a freshly spawned cat with no aspirations.

### Marker authoring check

Markers are ECS components, not events — there is no `events.jsonl` line for a ZST insert. Proof-of-authoring is structural: the system is registered in Chain 2a alongside 087's markers, so it runs every tick on every non-dead cat. A `just soak` completing with footer written and no panics is sufficient. For manual inspection, `just inspect <name>` can show the current component set if the inspect subtool has been extended to dump markers (out-of-scope for this ticket).

If you want an approximate signal from the soak log, this recipe counts trace events where `esteem_distress` is non-trivial (> 0.1), confirming the scalar is computed and non-zero for at least one cat:

```clowder/docs/diagnostics/log-queries.md#L1-1
jq 'select(.scalars.esteem_distress != null and .scalars.esteem_distress > 0.1) | .cat' \
  logs/tuned-42/trace.jsonl \
  | sort -u | wc -l
```

Expected: nonzero — most cats will have some L4 deficit by mid-run.

---

## Out of Scope

- Any L4/L5 DSE — separate catalog tickets. The three scalars and three markers have no consumers until those tickets land.
- Maslow gate semantics (L5 already gates on L1–L4 satisfied per the substrate refactor §3.4). This ticket adds perception of L4/L5 state; it does not change the gating logic.
- Tuning the drain rates on `respect` / `mastery` / `purpose` — balance work, deferred until the substrate stabilizes.
- A `ScoreModifier` for esteem-distress promotion (analog of the §L2.10 `BodyDistressPromotion` modifier from ticket 088) — that is DSE/modifier catalog work, not perception work, and depends on this ticket.
- `just inspect` extension to render L4/L5 markers — tooling ticket, separate concern.

## Log

- 2026-04-30: Opened alongside 087. Blocked-by 087 (perception substrate) until that lands. Catalog gap surfaced during inventory: `populate_dse_registry` has zero DSE coverage for Maslow L4/L5 yet those needs decay.
- 2026-04-30: Expanded to full implementation-ready spec. Blocked-by resolved (087 landed fc4e1ab). Status: ready.
