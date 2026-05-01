---
id: 089
title: Interoceptive self-anchors — spatial self-perception (OwnInjurySite, OwnSafeRestSpot)
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

087 published *scalar* interoception only — `pain_level` and
`body_distress_composite` exposed via `ScoringContext` /
`ctx_scalars`, plus three ZST markers (`LowHealth`, `SevereInjury`,
`BodyDistressed`). The architectural symmetry with external
perception isn't complete: `sensing.rs` publishes both scalars *and*
typed spatial anchors (`LandmarkAnchor::NearestThreat`,
`NearestKitchen`, `OwnSleepingSpot`, …), so DSEs can compose
distance-to-X considerations through `LandmarkSource::Anchor`.
Interoceptive perception has the scalars but no spatial anchors —
"where on the map should this cat go *given its body state*" has
no first-class substrate.

Two consumer shapes this unblocks:

- **`OwnSafeRestSpot`** — body-state-appropriate location to rest.
  `Sleep`'s existing `OwnSleepingSpot` SpatialConsideration is
  degenerate today (both authoring sites fall back to `Some(*pos)`
  — `disposition.rs#L890`, `goap.rs#L1370` — distance = 0,
  Power-Invert axis pinned at 1.0 every cat every tick). There is
  no real "where would I want to rest right now" signal feeding
  the IAUS contest.
- **`OwnInjurySite`** — first-class spatial encapsulation of
  *where* the cat was wounded. The future `TendInjury` DSE
  consumes it; in this ticket we land authoring + resolver wiring
  + a scoring-path integration test, *without* the DSE.

**Real-encapsulation discipline.** The original stub punted both
locations (memory-stub + landing-without-consumer). Both punts are
substrate-over-override antipatterns: authoring stubbed substrate
collapses real signal into noise, and authoring a variant nothing
resolves through the scoring path leaves the wiring untested. Both
are pulled into scope here. `TendInjury` itself stays out — that's
a behavior, not the spatial encapsulation it depends on.

## Substrate-over-override pattern

Substrate expansion on the substrate-over-override thread (093),
not a hack-retirement. Without typed self-anchors, a future
`TendInjury` DSE or a refined `Sleep`-near-safety would have to
reach into per-cat queries directly inside the scoring path
(override shape) instead of declaring `LandmarkSource::Anchor(…)`
and letting the §L2.10 substrate resolve it (composable shape).

**IAUS lever:** `LandmarkAnchor::OwnSafeRestSpot` consumed via
`SpatialConsideration` in `Sleep`. Mirrors external perception's
typed-anchor convention.

**Canonical exemplar:** 087 (`fc4e1ab`,
`landed/087-interoceptive-perception-substrate.md`) — same layered
shape (perception module → ScoringContext field → DSE
consideration → unit + soak-trace verification).

## Stub corrections discovered during investigation

The stub was written before 087 landed. One correction stands; two
were reversed by the user's "encapsulate properly, don't stub"
direction — both reversals are listed here so the intent is on the
record.

1. **There is no `src/ai/dses/rest.rs`.** 087's "Implementation
   deviation" and 088's "Self-care class is 6 DSEs, not 7"
   document the same finding: the catalog has only `Sleep`, not a
   distinct `Rest`. `Sleep` produces the `Resting` disposition
   that the critical-health interrupt special-cases. The consumer
   in this ticket is therefore `Sleep` (`src/ai/dses/sleep.rs`).
   Stands.
2. **`OwnInjurySite` *is* authorable — but `Injury` needs a
   `where` field.** `src/components/physical.rs#L71-77` —
   `Injury { kind, tick_received, healed, source }` carries no
   position. All four `apply_injury` call sites
   (`systems/combat.rs#L295-310`, `systems/wildlife.rs#L964-974` +
   `#L2241-2251`, `systems/magic.rs#L1098-1108`) already have the
   cat's `Position` in scope. Adding `at: Position` to `Injury`
   threads that data through; the resolver picks the most-recent
   unhealed injury's `at`. Authoring becomes pure-fn, unit-
   testable. (Reverses the original stub-correction's punt to a
   follow-on.)
3. **`OwnSafeRestSpot` resolves through real persistent memory.**
   `Memory` (`components/mental.rs#L120-160`) already provides the
   ring-buffer-with-decay shape the user's correction calls for —
   capacity 20, weakest-eviction, `decay_memories`
   (`systems/memory.rs#L13-21`) running 0.001/tick firsthand,
   `MemoryEntry { event_type, location, tick, strength,
   firsthand }`. Adding `MemoryType::Sleep` to the existing enum
   reuses the entire decay/eviction substrate and the
   `persistence.rs` plumbing. Authored by `resolve_sleep` on
   chain advance; resolver scans for the strongest unsuppressed-
   by-nearby-Threat-memory entry. (Reverses the original stub-
   correction's `ColonyCenter` fallback.)

After these corrections this ticket adds **two new anchors**
(`OwnSafeRestSpot`, `OwnInjurySite`), a new memory variant, a new
`Injury.at` field, with `Sleep` as the only DSE consumer this
ticket. `OwnTerritoryCenter` is explicitly **out of scope** — see
§Out of scope below.

## Implementation plan

Seven commits, each green at `just check`. Ordering: data-shape
extensions first (memory variant + injury field), substrate
extensions next (enum + resolver), authoring before consumption,
DSE wiring last.

### 1. `MemoryType::Sleep` variant + safe-rest authoring

**Files:**

- `src/components/mental.rs#L88-101` — add `Sleep` to
  `pub enum MemoryType`. The enum is `serde::Serialize +
  Deserialize`; new variant lands at the end so existing JSONL
  fixtures decode unchanged.
- `src/resources/colony_knowledge.rs#L81-89` —
  `knowledge_description` exhaustive match: add
  `MemoryType::Sleep => format!("the rested ground near {location_desc}")`.
  The colony-knowledge layer doesn't propagate Sleep memories
  (see step 4 — they're personal, not gossip), but the match must
  stay exhaustive.
- `src/systems/aspirations.rs#L520-530` — `track_milestones`
  string-match: leave Sleep without a milestone arm. The default
  `_ => None` already swallows it.
- `src/steps/disposition/sleep.rs#L25-37` — `resolve_sleep` gains
  a `&mut Memory` argument and writes a `MemoryEntry` on the
  `StepResult::Advance` branch:

  ```text
  if ticks >= duration {
      memory.remember(MemoryEntry {
          event_type: MemoryType::Sleep,
          location: Some(*self_position),
          involved: vec![],
          tick,
          strength: c.safe_rest_memory_strength_initial, // SimConstants
          firsthand: true,
      });
      StepOutcome::bare(StepResult::Advance)
  }
  ```

  Five-heading rustdoc gets a sixth-line update naming
  `MemoryType::Sleep` as a side-effect on Advance. **Real-world
  effect** widens; **Witness** stays `StepOutcome<()>` (Sleep is
  ubiquitous, not a Positive Feature — see 087's same call).
- `src/systems/disposition.rs#L3774-3778` — `dispatch_chain_step`
  `StepKind::Sleep` arm: thread the cat's `Position` and `&mut
  Memory` through. Both are already in the dispatch closure's
  query (Memory borrowed at the resolve-disposition-chains query;
  re-check the System param shape with `cargo check` after this
  edit).

**SimConstants** (`src/resources/sim_constants.rs`,
`DispositionConstants` block alongside `pain_normalization_max`):

- `safe_rest_memory_strength_initial: f32` (default `0.6` — under
  the `memory_strength_severe` of an Injury-grade memory; rests
  fade out faster than wounds).

**Tests** (in `src/steps/disposition/sleep.rs` `mod tests` —
file currently has none; create the module):

- `resolve_sleep_advance_writes_safe_rest_memory` — call once
  with `ticks == duration`, assert `memory.events.last()` is a
  `MemoryType::Sleep` entry at `self_position` with strength
  `safe_rest_memory_strength_initial`.
- `resolve_sleep_continue_does_not_write_memory` — call with
  `ticks < duration`, assert memory is unchanged.

Decay system (`systems/memory.rs::decay_memories`) is already
registered in `SimulationPlugin::build()` (`plugins/simulation.rs`
in the Chain 2a block) and processes every `MemoryEntry`
uniformly — no new schedule wiring.

### 2. `Injury.at` field + thread through `apply_injury`

**File:** `src/components/physical.rs#L71-77` — add
`pub at: Position,` to `Injury`. The struct is `Serialize +
Deserialize`; either add `#[serde(default = "default_position")]`
or accept the breaking-deserialization migration (the project's
`persistence.rs` save format will replay test fixtures — confirm
none of them snapshot pre-089 injury records; if they do, the
serde-default is the safe path).

**Callers** (all four pass `cat_pos` they already hold):

- `src/systems/combat.rs#L661-678` — `apply_injury` signature
  gains `at: Position`; `damage_to_injury` (`#L640-655`) propagates
  it into the constructed `Injury`.
- `src/systems/combat.rs#L295-310` (wildlife combat) — passes
  `*cat_pos` (in scope as the cat's `Position` at L271).
- `src/systems/wildlife.rs#L964-974` — passes `*wl_pos` (the
  cat's position; the `cat_pos`-shadowing variable in the outer
  loop).
- `src/systems/wildlife.rs#L2241-2251` — passes the cat's
  position from the `cats.get_mut` query result.
- `src/systems/magic.rs#L1098-1108` (misfire wound transfer) —
  passes the misfire victim's position; the surrounding scope
  already has it for the narrative.

Also: `src/systems/combat.rs#L312-322` — the
`MemoryType::Injury` `memory.remember` write currently passes
`location: None`. Change to `location: Some(*cat_pos)` while
already touching the file. (Free correctness improvement; no
consumer relies on the `None` shape — verified by grepping
`MemoryType::Injury` consumers.)

**Tests** (existing tests in `combat.rs#L920-940` already
construct `Injury` literals — fan-out adds `at: Position::new(0,
0)`):

- `apply_injury_records_inflicted_position` (new) — call
  `apply_injury(&mut h, 0.05, 10, InjurySource::Unknown, c, at:
  Position::new(7, 3))` and assert `h.injuries[0].at ==
  Position::new(7, 3)`.

### 3. `LandmarkAnchor::OwnSafeRestSpot` + `OwnInjurySite` variants

**File:** `src/ai/considerations.rs#L149-187`.

Insertion in the "Cat-side dynamic landmarks" section after
`OwnSleepingSpot`:

```text
    /// Body-state-appropriate safe rest spot — strongest recent
    /// `MemoryType::Sleep` entry not suppressed by nearby
    /// `MemoryType::ThreatSeen` / `Death` memories. Sleep. Ticket 089.
    OwnSafeRestSpot,
    /// Position where the cat was most recently wounded — most-
    /// recent unhealed `Injury.at`. Reserved for the future
    /// `TendInjury` DSE; in 089 the resolver is exercised by an
    /// integration test only. Ticket 089.
    OwnInjurySite,
```

Both are `Copy`-clean unit variants (preserves the design-note
docblock at `#L143-148`).

**`CatAnchorPositions`** (`src/ai/scoring.rs#L168-189`):

```text
    /// `LandmarkAnchor::OwnSleepingSpot` — Sleep (B2).
    pub own_sleeping_spot: Option<Position>,
    /// `LandmarkAnchor::OwnSafeRestSpot` — Sleep, body-state safe-
    /// rest axis. Memory-derived; `None` if the cat has no Sleep
    /// memories yet (newly-spawned cats). Ticket 089.
    pub own_safe_rest_spot: Option<Position>,
    /// `LandmarkAnchor::OwnInjurySite` — future TendInjury DSE.
    /// `None` if the cat has no unhealed injuries. Ticket 089.
    pub own_injury_site: Option<Position>,
```

`#[derive(Default)]` at `#L165` gives `None` automatically — but
every existing `CatAnchorPositions { … }` struct-literal in
`scoring.rs` tests must be field-extended. Grep for
`CatAnchorPositions {` shows 8 sites at L2068, L2216, L2387,
L2648, L2725, L2821, L3134, L3212. Mechanical fan-out — same
pattern 087 followed for `pain_level` / `body_distress_composite`.

**Resolver** (`src/ai/scoring.rs#L735-758`, `anchor_position`
closure inside `score_dse_by_id`):

```text
            LandmarkAnchor::OwnSleepingSpot => ctx.cat_anchors.own_sleeping_spot,
            LandmarkAnchor::OwnSafeRestSpot => ctx.cat_anchors.own_safe_rest_spot,
            LandmarkAnchor::OwnInjurySite => ctx.cat_anchors.own_injury_site,
```

Compilation gate: the closing `_ => None` catch-all (`#L755`)
will silently swallow new variants. Either explicit-list ahead of
it, or remove the catch-all entirely (preferred — there are no
fox-side variants reachable from cat scoring; the catch-all
exists to swallow them and could be replaced with explicit
`LandmarkAnchor::OwnDen | … => None`).

### 4. Author both anchors from interoceptive perception

**File:** `src/systems/interoception.rs` — two new pure helpers
alongside `pain_level` / `health_deficit` / `body_distress_composite`.

```text
/// Body-state-appropriate safe-rest tile. Scans the cat's
/// `Memory` for `MemoryType::Sleep` entries; returns the
/// strongest entry's position whose location is not suppressed
/// by any nearby (`<= safe_rest_threat_suppression_radius`)
/// `MemoryType::ThreatSeen` or `Death` memory. Returns `None`
/// when no qualifying memory exists.
///
/// Suppression rule: a Sleep memory at L is rejected if any
/// ThreatSeen/Death memory exists at L' with
/// `L.manhattan_distance(L') <= safe_rest_threat_suppression_radius`.
/// This is the "I remember resting here, but I also remember a
/// hawk here last week" gate.
pub fn own_safe_rest_spot(
    memory: &Memory,
    suppression_radius: i32,
) -> Option<Position> { … }

/// Most-recent unhealed-injury site. Scans `health.injuries`
/// for `!healed` entries, returns the one with the highest
/// `tick_received`'s `at`. `None` when no unhealed injuries.
pub fn own_injury_site(health: &Health) -> Option<Position> { … }
```

`safe_rest_threat_suppression_radius` lives in
`SimConstants::disposition` (default `5` tiles — a one-room
ward; calibrated against `threat_awareness_range` already at
`disposition.threat_awareness_range`).

**Construction sites** (both reuse the same helpers; both already
borrow `Memory` and `Health`):

- `src/systems/disposition.rs#L878-922` — `evaluate_dispositions`
  literal. `memory` and `health` are in scope (used at
  `pain_level` call L815). Add:

  ```text
  own_safe_rest_spot: crate::systems::interoception::own_safe_rest_spot(
      memory, d.safe_rest_threat_suppression_radius,
  ),
  own_injury_site: crate::systems::interoception::own_injury_site(health),
  ```
- `src/systems/goap.rs#L1352-1413` — parallel literal in
  `evaluate_and_plan`. Same two adds.

**Tests** (`src/systems/interoception.rs` `mod tests`):

- `own_safe_rest_spot_none_with_empty_memory` — `Memory::default()`
  → returns `None`.
- `own_safe_rest_spot_picks_strongest_sleep_memory` — three Sleep
  entries at strengths 0.3 / 0.5 / 0.4; assert returned position
  matches the 0.5-strength entry.
- `own_safe_rest_spot_suppressed_by_nearby_threat` — Sleep at
  (10,10) strength 0.6; ThreatSeen at (12,10) (within radius 5).
  Assert `None` (or the next-best Sleep entry if one exists).
- `own_safe_rest_spot_unsuppressed_by_distant_threat` — Sleep at
  (10,10) strength 0.6; ThreatSeen at (50,50). Assert `(10,10)`.
- `own_safe_rest_spot_stable_across_ticks` — call twice on the
  same `Memory`; assert equal return. Future-proofing canary
  against Memory iteration-order non-determinism.
- `own_injury_site_none_with_no_injuries` — `Health::default()`.
- `own_injury_site_picks_most_recent_unhealed` — three injuries
  at ticks 100 / 200 / 150 with positions A / B / C; tick-200
  unhealed; assert position B.
- `own_injury_site_ignores_healed` — most-recent injury healed,
  earlier one unhealed; assert the earlier's position.

### 5. `Sleep` consumes `OwnSafeRestSpot` via SpatialConsideration

**File:** `src/ai/dses/sleep.rs`.

`SAFE_REST_RANGE` constant (top of file, alongside
`SLEEP_SPOT_RANGE`):

```text
/// §L2.10.7 SafeRest range — Manhattan tiles. Tighter than
/// `SLEEP_SPOT_RANGE` (15.0) because the safe-rest signal is
/// "I'd rest here right now if I happened to be near," not "I
/// should travel across the colony to get here." 10 tiles
/// matches the home-range scale where memory-based
/// associations stay vivid. Ticket 089.
pub const SAFE_REST_RANGE: f32 = 10.0;
```

Sixth `Consideration::Spatial`, same Power-Invert curve shape as
`OwnSleepingSpot`'s `spot_distance`:

```text
let safe_rest_distance = Curve::Composite {
    inner: Box::new(Curve::Polynomial { exponent: 2, divisor: 1.0 }),
    post: PostOp::Invert,
};

Consideration::Spatial(SpatialConsideration::new(
    "safe_rest_distance",
    LandmarkSource::Anchor(LandmarkAnchor::OwnSafeRestSpot),
    SAFE_REST_RANGE,
    safe_rest_distance,
)),
```

**Weight rebalance.** Current five weights (087's already-rescaled
shape) `[0.40·0.90, 0.24·0.90, 0.16·0.90, 0.10, 0.20·0.90]` sum
to 1.0. Same trick: scale every existing weight by 0.95, add new
axis at 0.05.

```text
composition: Composition::weighted_sum(vec![
    0.40 * 0.90 * 0.95, // energy_deficit
    0.24 * 0.90 * 0.95, // day_phase
    0.16 * 0.90 * 0.95, // health_deficit
    0.10 * 0.95,        // pain_level
    0.20 * 0.90 * 0.95, // sleep_spot_distance
    0.05,               // safe_rest_distance (089)
]),
```

The 0.05 weight matches 088's "small additive nudge from a self-
perception axis" precedent and keeps cats with empty Sleep memory
(the `None` resolver path scores the spatial axis at 0.0)
behaviorally identical to pre-089.

**Tests** (extend `src/ai/dses/sleep.rs` `mod tests`):

- `sleep_has_six_axes` — bumps existing `sleep_has_five_axes`
  (`#L185-190`) from 5 to 6.
- `sleep_uses_own_safe_rest_spot_anchor` — mirrors
  `sleep_uses_own_sleeping_spot_anchor` (`#L201-211`) for the new
  axis name.
- `sleep_weights_sum_to_one` — already exists at `#L240-244`;
  exercises the rebalance correctness automatically.
- `safe_rest_axis_decays_with_distance` — build `EvalCtx` with
  `cat_anchors.own_safe_rest_spot = Some(Position::new(0,0))`,
  evaluate spatial axis at `self_position = (0,0)` then `(10,0)`.
  Assert `score_near > score_far`. Pattern: copy
  `evaluate_cook_with_markers` shape from
  `src/ai/dses/cook.rs#L204-225`.

### 6. `OwnInjurySite` integration test (no DSE consumer)

The user's correction explicitly forbids landing the variant
"without a consumer." `TendInjury` itself is out of scope, but a
unit-test-level *integration consumer* exercises the full
authoring → ScoringContext → resolver path and proves the
encapsulation. **File:** `tests/own_injury_site_resolver.rs`
(new integration test, following `tests/` convention per project
rules) — or `src/ai/scoring.rs` `mod tests` (in-source) if the
existing `score_dse_by_id` test fixtures are cheaper to extend.

Test shape (new module, ~40 lines):

```text
#[test]
fn own_injury_site_resolves_to_most_recent_unhealed_injury_position() {
    // 1. Construct Health with two unhealed injuries at distinct
    //    `at` positions, tick_received differing.
    // 2. Build CatAnchorPositions via the same helper used by
    //    disposition.rs / goap.rs (interoception::own_injury_site).
    // 3. Build ScoringContext with that cat_anchors.
    // 4. Construct a synthetic SpatialConsideration in a test-
    //    only DSE that requests LandmarkAnchor::OwnInjurySite.
    // 5. Call score_dse_by_id (or evaluate_single directly).
    // 6. Assert the resolved anchor_position matches
    //    most-recent-unhealed.at (the 'I am wounded *here*'
    //    contract).
}
```

The synthetic DSE is constructed inline in the test — does not
land in `populate_dse_registry`. This proves the
`LandmarkAnchor::OwnInjurySite` variant resolves correctly
through the same scoring path that production DSEs will use,
without prematurely committing to TendInjury's curve / weight /
Maslow-tier choices. The resolver path being green here is the
substrate-over-override discipline — the future TendInjury commit
adds *only* the DSE definition and the `populate_dse_registry`
line, with no scoring-internals churn.

A second, lighter-weight test stays in
`src/systems/interoception.rs` `mod tests`
(`own_injury_site_picks_most_recent_unhealed` etc., from step 4)
— the integration test in this step adds the scoring-path proof
on top of those pure-fn assertions.

### 7. Wiki + open-work index regen

`SimulationPlugin::build()` doesn't change in this ticket (no new
systems), so `docs/wiki/systems.md` does not regenerate. **The
ticket flip *does* require `just open-work-index`** in the lands-
day commit per CLAUDE.md "Long-horizon coordination".

If `OwnTerritoryCenter` follow-on (see §Out of scope) opens in
the same commit per the antipattern-migration discipline, that's
two ticket-file additions — index regeneration covers both.

## Verification

**Unit tests** (added in steps 1–6, run via `cargo nextest run
--features all`):

- `resolve_sleep_advance_writes_safe_rest_memory`
- `resolve_sleep_continue_does_not_write_memory`
- `apply_injury_records_inflicted_position`
- `own_safe_rest_spot_none_with_empty_memory`
- `own_safe_rest_spot_picks_strongest_sleep_memory`
- `own_safe_rest_spot_suppressed_by_nearby_threat`
- `own_safe_rest_spot_unsuppressed_by_distant_threat`
- `own_safe_rest_spot_stable_across_ticks`
- `own_injury_site_none_with_no_injuries`
- `own_injury_site_picks_most_recent_unhealed`
- `own_injury_site_ignores_healed`
- `sleep_has_six_axes`
- `sleep_uses_own_safe_rest_spot_anchor`
- `sleep_weights_sum_to_one` (existing — exercises the rebalance)
- `safe_rest_axis_decays_with_distance`
- `own_injury_site_resolves_to_most_recent_unhealed_injury_position`

**Focal-cat trace.** `just soak-trace 42 <wounded-cat-with-sleep-history>`:

- A wounded cat that has Sleep memories at recent positions
  unsuppressed by nearby Threat memories should record
  `safe_rest_distance` raw input < 0.3 (cat is near a remembered
  rest spot), curve output > 0.85 — adds roughly `0.05 × 0.85 =
  0.0425` to the Sleep total over the same cat's pre-089 score.
- Same cat moved 10+ tiles from any remembered rest spot records
  curve ≤ 0.0 — net contribution `≈ 0`, indistinguishable from
  pre-089. The "memory-driven gradient" claim is falsifiable in
  this trace pair.
- Newly-spawned cat (empty Memory) records `own_safe_rest_spot:
  None` → spatial axis `score = 0.0` per the
  `LandmarkSource::Anchor` substrate convention (087 documented
  this floor). Behaviorally identical to pre-089 for fresh cats
  until they sleep once.

**Continuity canaries** (CLAUDE.md "Hard survival gates" + the
seven continuity signals): purely additive substrate with a 0.05
weight + 0.95 rebalance of the others should not move
`deaths_by_cause.Starvation` (must stay 0),
`deaths_by_cause.ShadowFoxAmbush` (≤10), or footer presence;
the seven canaries (`grooming` / `play` / `mentoring` / `burial`
/ `courtship` / `mythic-texture` + `KittenMatured`) should each
register ≥ 1.

**`just soak 42 && just verdict logs/tuned-42`** is the gate.
Exit 0 expected. Lands-day soak comparison is straightforward —
two new spatial axes that floor at 0.0 for un-memoried / un-
injured cats means the *behavioral* baseline shifts only for
cats that have actually slept and only when they're near where
they slept; the colony-aggregate metrics should sit well inside
the ±10% no-hypothesis band.

**Drift watch.** A 5% reweight of every existing Sleep axis is
small enough that drift on Sleep-related metrics
(`sleep_action_count`, `energy_recovered_per_sim_year`) should
land inside ±10%. If anything moves more than that, `just
soak-trace` against the focal cat above produces the four-
artifact prediction-vs-observation evidence side.

## Risks / open questions

- **`Memory.capacity = 20` shared across all `MemoryType`s.**
  Adding a Sleep variant introduces capacity contention with
  ThreatSeen / Injury / ResourceFound / Triumph entries. With a
  weakest-evicted policy and decay, frequently-slept-at locations
  will accumulate strength and survive eviction; one-off rest
  spots will fade. The behavioral risk is a cat with several
  high-strength Triumph memories crowding out Sleep entries —
  unlikely in practice (Triumph fires on banishment, rare). If it
  *is* a problem, the follow-on is bumping Memory.capacity or
  introducing a per-MemoryType sub-cap; flag here so the soak
  trace surfaces it if real.
- **`Injury.at` serde compatibility.** Pre-089 save fixtures
  encode injuries without `at`. Two paths: serde-default to
  `Position::new(0, 0)` (silent loss of authoring fidelity for
  legacy injuries — they'll resolve to map origin) or a one-time
  migration script. Default-to-origin is acceptable because
  `OwnInjurySite` has no consumer this ticket; fix during the
  TendInjury follow-on if it matters there.
- **`safe_rest_threat_suppression_radius` calibration.** Default
  5 is a guess at "ward-room scale." If the soak trace shows the
  suppression rule firing on ~every Sleep memory (because cats
  also see threats almost everywhere), the radius is too large
  and `OwnSafeRestSpot` collapses to mostly-`None`. The trace
  invocation above includes this directly: the wounded-cat
  Sleep score should record a non-zero `safe_rest_distance`
  axis on at least one focal cat for the substrate to be doing
  any work.
- **Authoring on `resolve_sleep` Advance.** Sleep durations
  vary (50–200+ ticks per call from the deficit-multiplier
  formula at `disposition.rs#L1746-1755`). Authoring on Advance
  rather than per-tick avoids 100× over-counting; the
  per-tick-strength baseline lands at 0.6 once per completed
  Sleep, which decays to ~0.0 over 600 ticks (firsthand 0.001
  rate). That timescale matches "I rested here a few sim-hours
  ago" — verified against the day-cycle constants.
- **Memory iteration determinism.** `Memory.events` is a
  `VecDeque` — insertion-order iteration. Tie-breaking among
  equal-strength Sleep entries goes to the earliest-inserted.
  Same-seed replay holds. The
  `own_safe_rest_spot_stable_across_ticks` test guards the
  invariant.

## Out of scope

- **`TendInjury` DSE.** The behavior — a wounded cat moving to
  its `OwnInjurySite` to recover — is a separate ticket. This
  ticket lands the *spatial encapsulation* it depends on
  (variant + resolver + authoring + integration test) so the
  TendInjury commit reduces to "add DSE, register in
  `populate_dse_registry`."
- **`LandmarkAnchor::OwnTerritoryCenter`.** No real consumer in
  this ticket, and substrate-over-override discipline says don't
  author unused substrate (the same reasoning the original stub
  applied to `OwnInjurySite` — overridden there because the
  real spatial encapsulation has unit-test exercise; here it
  doesn't). **Per CLAUDE.md "Antipattern migration follow-ups
  are non-optional," a follow-on ticket
  `tickets/NNN-own-territory-center-anchor.md` opens in the same
  commit that lands this one** if no consumer (Patrol-with-
  territory-bias, e.g.) is already on the index. Status `ready`,
  `## Why` referencing this section.
- **Memory-capacity sub-caps.** §Risks names the contention
  question; the follow-on lives there if soak shows it.
- **`Injury.at` serde migration script.** §Risks names it; the
  serde-default-to-origin path is the in-this-ticket choice.
- **L4/L5 self-perception scalars (090).** Separate ticket per
  087's published roadmap.

## Log

- 2026-04-30 — Stub captured during 087 landing
  (`landed/087-interoceptive-perception-substrate.md` §7).
- 2026-05-01 — Plan expanded. Memory-based safe-rest authoring
  pulled into scope per user correction (reverses original
  ColonyCenter-fallback punt; reuses existing `Memory` ring-
  buffer + `decay_memories` substrate via new `MemoryType::Sleep`
  variant). `OwnInjurySite` encapsulation made real:
  `Injury.at: Position` field threaded through all four
  `apply_injury` callers, authoring via
  `interoception::own_injury_site`, scoring-path integration
  test as the unit-test-only consumer (TendInjury DSE remains
  out of scope). `OwnTerritoryCenter` confirmed out of scope per
  substrate-over-override discipline; follow-on ticket to open
  in lands-day commit.
