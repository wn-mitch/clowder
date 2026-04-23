# A1.4 §13.1 retired-constants cleanup — kickoff

Session-startup artifact for the A1.4 sub-phase of the AI substrate
refactor. Pairs with [`ai-substrate-refactor.md`](ai-substrate-refactor.md)
§2.3 "Retired constants" and [`docs/open-work.md`](../open-work.md) #13
item 13.1.

A1.4 is a deletion-heavy commit, not a design commit. It retires
the workaround constants + the inline `is_incapacitated` branch +
the three corruption-emergency `ScoreModifier` impls whose
contribution the axis-level Logistic absorbs by construction.

**Gate status (2026-04-23 audit): CLOSED.** The per-DSE Logistic
migrations and the `Incapacitated` marker author system are both
prerequisites that have not shipped. A prior iteration of this
kickoff's preamble said "the Logistic curves already landed with
Phase 3c's per-DSE ports" — that was wrong; Phase 3c landed DSE
*shapes*, not every §2.3-assigned axis curve. See
[`docs/open-work.md`](../open-work.md) #13 item 13.1 for the
row-by-row gate table + prerequisite track breakdown. Do not
execute this kickoff until each gate row is open. The prompt below
includes a gate-verification step that will catch this; it exists
precisely so a misread of the status line doesn't cascade into a
broken commit.

---

## Kickoff prompt

Paste into a new session. Self-contained — briefs cold.

````
You're picking up §13.1 of the AI substrate refactor: retire the
workaround constants the curve refactor made obsolete. This is a
deletion-heavy commit, not a design commit. Read these first, in order:

  1. docs/systems/ai-substrate-refactor.md §2.3 "Retired constants by the
     curve refactor" subsection (~line 1193) + the row-by-row retirement
     table immediately above it — this is the authoritative list of
     what's retiring and what replaces it.
  2. docs/open-work.md #13 item 13.1 — the gating rule ("lands in the
     same PR that introduces the Logistic curves that replace them")
     and why premature retirement is dangerous.
  3. docs/open-work.md #5 cluster-A status line (2026-04-23) — confirms
     A1 is landed, which is the gate condition for 13.1.
  4. CLAUDE.md "Balance Methodology" — §13.1 is a behavior-preserving
     cleanup by construction, but the seed-42 soak still has to pass
     the survival canaries before the commit lands.

GATE VERIFICATION — DO THIS FIRST, BEFORE DELETING ANYTHING.

Per #13.1, the constants can only retire once the Logistic curves that
replace them are in place. Verify each row of §2.3's retired-constants
table has a matching Logistic/Piecewise curve in the corresponding DSE
definition under `src/ai/dses/`. Specifically check:

  - `Eat.hunger` — `Logistic(8, 0.75)` (hangry anchor) ⇒ the
    `incapacitated_eat_urgency_scale/offset` workaround retires.
    Grep: `src/ai/dses/eat.rs`.
  - `Sleep.energy` — `Logistic(10, 0.7)` (sleep-dep anchor) ⇒ the
    `incapacitated_sleep_urgency_scale/offset` workaround retires.
    Grep: `src/ai/dses/sleep.rs`.
  - `Herbcraft.ward` territory_max_corruption axis — `Logistic(8, 0.1)`
    ⇒ `ward_corruption_emergency_bonus` retires. Grep:
    `src/ai/dses/herbcraft_ward.rs`.
  - `PracticeMagic.durable_ward` nearby_corruption_level axis —
    `Logistic(8, 0.1)` ⇒ `corruption_sensed_response_bonus` retires.
    Grep: `src/ai/dses/practice_magic.rs` (or its sibling split).
  - `PracticeMagic.cleanse` tile_corruption axis — `Logistic(8,
    magic_cleanse_corruption_threshold)` ⇒
    `cleanse_corruption_emergency_bonus` retires on the cleanse side.
    Grep: same file or sibling split.
  - `PracticeMagic.colony_cleanse` territory_max_corruption axis —
    `Logistic(6, 0.3)` ⇒ the same `cleanse_corruption_emergency_bonus`
    retires on the colony side. Grep: same file or sibling split.
  - `Incapacitated` ECS marker (§4.3) — confirm author system populates
    it correctly and at least one DSE uses it via
    `.require("Incapacitated")` or `.forbid("Incapacitated")`. If the
    marker is present but no DSE forbids it, the
    `if ctx.is_incapacitated` branch cannot retire yet.

If any gate is not satisfied, STOP and report the gap list — don't
delete the constant whose replacement hasn't shipped. The spec is
explicit: "Behavior-preserving once the curves are in; dangerous
before."

SCOPE FOR THIS COMMIT — deletion only, once gates are verified:

  - Delete from `src/resources/sim_constants.rs::ScoringConstants`:
      * `incapacitated_eat_urgency_scale`
      * `incapacitated_eat_urgency_offset`
      * `incapacitated_sleep_urgency_scale`
      * `incapacitated_sleep_urgency_offset`
      * `incapacitated_idle_score`
      * `ward_corruption_emergency_bonus`
      * `cleanse_corruption_emergency_bonus`
      * `corruption_sensed_response_bonus`
    (Eight fields. Delete both the struct definition entry and the
    default-value entry in `Default` impl.)

  - Delete the `if ctx.is_incapacitated` early-return branch in
    `src/ai/scoring.rs::score_actions` (currently around
    `scoring.rs:574–598`). Per §2.3, the `Incapacitated` marker filters
    ineligible DSEs and the canonical Logistic curves on the surviving
    ones (Eat, Sleep, Idle) spike hard enough to dominate without
    bespoke multipliers.

  - Delete the three modifier impls in `src/ai/modifier.rs` that
    consumed the retired corruption-emergency bonuses:
      * `WardCorruptionEmergency`
      * `CleanseEmergency`
      * `SensedRotBoost`
    Plus their registration in `default_modifier_pipeline`. The
    logistic curves on the DSE axes absorb these modifiers'
    contributions directly (the workaround shape retires by
    construction).

  - Delete any test fixtures / mock closures that existed solely to
    set values for the retired constants (grep the test modules in
    `src/ai/modifier.rs`, `src/resources/sim_constants.rs` tests, and
    `src/ai/scoring.rs` tests).

  - Grep for any remaining references to the retired field names
    across `src/`, `scripts/`, and `docs/balance/`. Update or delete
    each. Narrative / telemetry references that describe historical
    sim runs stay; live references to field-reads do not.

  - Update `docs/open-work.md` #13 item 13.1 — move to the Landed
    section with commit hash + a soak-result footnote.

EXPLICIT NON-GOALS:

  - No new curves. Each retired constant is paired with a curve that
    already exists; if a curve doesn't exist, the gate is closed and
    this commit doesn't land (see Gate Verification).
  - No balance tuning. §13.1 is behavior-preserving by construction;
    soak canaries must pass but magnitudes on non-canary metrics are
    expected to drift within the noise envelope because the modifier
    layer is leaving.
  - No re-shaping of `ScoringContext`. The `is_incapacitated` boolean
    field stays (other consumers may still read it for non-scoring
    reasons); only the early-return branch retires.
  - No §3.5 remaining-modifiers port (Pride, Independence, Patience,
    Tradition, Fox-suppression, Corruption-suppression). Those are a
    separate scope; see open-work next-scopes list.

DELIVERABLES:

  - Clean deletions across the files above.
  - All tests pass (`just test`).
  - Landing entry in `docs/open-work.md` Landed section with commit
    hash, soak footer diff, and survival-canary confirmation.

VERIFICATION:

  - `just check` (cargo check + clippy clean).
  - `just test` (all existing tests pass — some may need adjustment
    as retired fields disappear).
  - `just soak 42` (seed-42 15-min release soak). Record the footer
    before the commit (HEAD at b6fac99 or current) and after. All
    four survival canaries must hold (`Starvation == 0`,
    `ShadowFoxAmbush ≤ 5`, `footer_written ≥ 1`,
    `never_fired_expected_positives == 0`). Metric drift within the
    documented seed-42 noise envelope is acceptable; drift above
    ±10% on a characteristic metric (KittenFed, MatingOccurred,
    WardPlaced, ScryCompleted, BondFormed, continuity_tallies)
    requires a hypothesis write-up per CLAUDE.md's Balance
    Methodology, not a quiet acceptance.

CONVENTIONS:

  - Bevy 0.18 (Messages not Events — see CLAUDE.md ECS Rules).
  - Conventional commit: `refactor:` no scope.
  - Additive-not-destructive principle still applies to *behavior*;
    this commit deliberately *is* destructive to the constants
    themselves. Behavior preservation is enforced by the Logistic
    curves already being in place, not by keeping dead fields.
  - No `--no-verify`, no `--amend`; see CLAUDE.md git-safety.

OUT OF SCOPE TO RAISE BEFORE IMPLEMENTING:

  - Whether to delete the `ScoringContext.is_incapacitated` field
    itself — other consumers read it; leave the field, delete only
    the scoring-branch read.
  - Whether to inline or generalize the Logistic curves currently
    hardcoded in the DSE factories — out of scope; balance tuning
    against the stable substrate is deferred per #14.
  - Whether the `*_corruption_emergency_bonus` default values (2.0,
    0.8) should carry forward as a Logistic steepness/midpoint pair
    — no; §2.3 specifies `Logistic(8, 0.1)` for both and the axis
    curves are already tuned to absorb the bonuses' prior
    contribution. Don't re-derive.

If any gate verification fails, STOP and report the gap. Don't
synthesize a workaround ("I'll add the curve too") — that's scope
creep and the A1 substrate work is tracked separately.
````

---

## Cross-refs

- Spec: [`ai-substrate-refactor.md`](ai-substrate-refactor.md) §2.3
  (Retired constants table + subsection)
- Open-work: [`../open-work.md`](../open-work.md) #13 item 13.1
  (gating rule) + #5 cluster-A status line (gate confirmation)
- Parent kickoff: [`a1-iaus-core-kickoff.md`](a1-iaus-core-kickoff.md)
  phase table row A1.4
