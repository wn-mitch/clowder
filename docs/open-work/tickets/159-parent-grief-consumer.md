---
id: 159
title: Parent grief consumer for kitten / dependent death
status: ready
cluster: ai-substrate
added: 2026-05-04
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

`src/systems/death.rs:170-195` provides bonded grief only via
`BondType::Mates | Partners | Friends`. There is no parent-specific
grief pathway. `src/systems/growth.rs:219-228` carries an explicit TODO
flagging that the `markers::Parent` ZST is "staged for future grief
consumers" — those consumers have never landed. As a result, when a
kitten dies (starvation, mauling, sickness) the surviving mother /
father feels nothing more than a generic `Friends`-grief if they
happened to befriend the kitten via cry-broadcast (ticket 156). The
mother of two starved kittens (Robinkit-33 / Maplekit-98 in the
seed-42 deep-soak that motivated ticket 158) experienced no
emotional response distinguishable from any random colony cat's.

This is a substrate gap: the bond from parent to kitten is real
substrate (`KittenDependency` references the parent entity, and
`update_parent_markers` authors the `Parent` ZST), but the death
pipeline's grief author at `death.rs:170-195` has no consumer for it.

## Out of scope for 158

Ticket 158 narrowly fixes the *survival* gate (kittens not starving in
the first place). This ticket is the orthogonal *emotional response*
gap that ticket 158 surfaced when the user asked about emotional
impacts of kitten deaths.

## Scope

Author a parent-grief consumer in `src/systems/death.rs` that mirrors
the existing bonded-grief shape (lines 170-195) keyed on
`KittenDependency` references rather than `BondType::*`. Bond-type
design questions to resolve in this ticket:

1. **Decay shape.** Does kittenship-grief decay over ticks (like
   Friends-grief, 500 ticks) or persist (like fated love)? Default
   suggestion: **persist** — losing a child is a load-bearing
   biographical fact for the parent, not a transient mood blip.
2. **Magnitude.** Should it be larger than Mates-grief (-0.7 ×
   fondness, 3000 ticks)? Default suggestion: same magnitude
   as Mates, longer duration.
3. **Personality modulation.** Is the magnitude scaled by `compassion`
   or `warmth`? Default suggestion: yes, by `compassion` —
   high-compassion parents feel kitten loss harder. This pairs with
   the existing `caretake_compassion_bond_scale` discipline that
   already differentiates parents on the same axis.
4. **Coverage.** Does the grief fire only on `Starvation` death, or
   any cause? Default suggestion: any cause — a parent grieves a
   kitten taken by shadow-fox the same as one taken by hunger.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Marker | `src/components/markers.rs` (`Parent`) | Authored each tick by `update_parent_markers` (`growth.rs:235`). Drops within the same tick that the kitten enters `Dead` (per §4.3 ordering hazard, the surviving parent's marker is ALREADY gone by the time death.rs runs). | `[verified-correct]` |
| Death event | `src/systems/death.rs:101-137` | `Feature::DeathStarvation` etc. records the death; narrative pushed; `EventLog::Death` emitted with cause + location. | `[verified-correct]` |
| Witness grief | `src/systems/death.rs:141-168` | Cats within `grief_detection_range` (default 5) get `-0.3 mood × 50 ticks`. Doesn't distinguish parent. | `[verified-correct]` |
| Bonded grief | `src/systems/death.rs:170-195` | Loops over `Mates / Partners / Friends` bonds; applies `-(intensity × fondness)` mood with bond-type-specific duration. | `[verified-correct]` |
| Parent grief | (none — N/A) | No consumer exists. | `[verified-defect]` |
| Channel for "parent at time of death" | `CatDied.survivors_by_relationship` event payload (per §4.3 prose at `markers.rs:454-455`) | **Future** — not yet implemented. Today, post-death the `Parent` marker is already gone from a parent who lost their last kitten, so we cannot infer parent-status from `With<Parent>` in the death-handling pass. | `[suspect — design call]` |

The `[suspect]` row in the audit table is the channel question: does the
parent-grief consumer query `KittenDependency` directly *before* the
death cleanup runs, or does it consume a future
`CatDied.survivors_by_relationship` payload? Resolving this is part of
the ticket scope.

## Fix candidates

**Parameter-level**:

- R1 — Compassion-scaled mood-modifier on `Parent`-marked survivors at
  death time. Read `KittenDependency.mother == dead || .father == dead`
  before cleanup. Apply `MoodSource::ParentGrief, intensity = -0.7 ×
  compassion, duration = 5000 ticks`. Add a `Feature::ParentGrief`
  variant to `Feature::*` and classify in
  `Feature::expected_to_fire_per_soak()` per the GOAP step contract.

**Structural** (per CLAUDE.md "Bugfix discipline" — every fix-shape
must include one):

- R2 (**split**) — give kittenship-grief its own `BondType::Offspring`
  variant in `src/components/relationship.rs`, populated at birth in
  `pregnancy.rs` (mother-kitten, father-kitten, sibling-sibling).
  Lifts the parent-child bond into the same axis as Mates / Friends,
  unifying the grief author at `death.rs:170-195` instead of
  authoring a parallel pathway.

The structural option (R2) is the substrate-over-override choice —
parent-child becomes a first-class `BondType` rather than an inferred
relationship. If R2 lands, the existing `bonded_grief` author already
handles the rest.

## Verification

1. **Unit test**: parent + kitten + mate, kill the kitten, assert the
   parent has a `MoodSource::ParentGrief` modifier applied with
   personality-scaled intensity.
2. **Soak narrative inspection**: in a fresh seed-42 soak (post-158),
   when a kitten does die, the narrative log SHOULD emit a
   parent-specific line (e.g., "Mocha stands long at the place where
   Robinkit-33 fell"). Hand-eyeball at first, then add a
   `Feature::ParentGriefNarrative` for activation tracking.
3. **No regression on Friends/Mates grief**: existing 1833 lib tests
   continue passing, in particular all `death.rs` tests.

## Out of scope

- Generalized "loss" emotional response for non-kin (e.g., losing a
  beloved patrol partner). That's bond-type design for the existing
  `Mates / Partners / Friends` pathway.
- Multi-tick narrative arc for grief (e.g., parents visiting the
  death tile repeatedly). Defer to a separate ticket once the basic
  mood-modifier path is in place.

## Log

- 2026-05-04: opened in the same commit that lands ticket 158's
  structural fix. Surfaced by user question "what emotional impacts
  does the kitten death have" — investigation found the existing
  death pipeline has no parent-specific channel.
