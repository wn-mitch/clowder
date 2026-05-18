---
id: 127
title: Joint-intention substrate for two-cat practices
status: done
cluster: C
initiative: []
added: 2026-05-02
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, scoring-layer-second-order.md]
related-balance: []
landed-at: b5455647d48d
landed-on: 2026-05-11
---

## Why

Cluster-C C2 framing (007 §C2): Versu-style social practices are
multi-agent coordinated behaviors with shared state — courtship,
co-mentoring, joint cache-stocking aren't two cats independently
scoring the same DSE, they're *one* multi-stage structure that two
cats co-enter, co-progress, and that drops cascade across when one
party abandons.

126 made per-cat goal-shaped commitment a first-class substrate
(`HeldIntention`), but the *actor-private* discipline rules out
coordinated practices: HeldIntention is by-design unreadable across
cats. 127 fills the gap with a *mutually-public* substrate category
specifically for two-cat practices.

The codebase already has a working two-cat-coordination pattern —
`PairingActivity` in `src/components/pairing.rs` — carved one-off
for mating. 127 generalizes that shape into substrate any practice
can compose against, and uses **Courtship as the first consumer by
porting `PairingActivity` → `JointIntention { practice: Courtship }`
in this ticket** (no follow-on cleanup debt). This is a structural
refactor that is a balance change by CLAUDE.md's definition:
migration parity (canonical seed-42 metrics within ±10% of post-272
baseline) is a hard exit criterion.

Until 258 (C3 mental models) ships, JointIntention also serves as
the **memory proxy**: the cat doesn't have a model of "Hazel and I
have been courting for three weeks" — the substrate holds that
fact for them and partner-reading is just realistic perception of
a publicly-performed practice.

## Semantic category

Three substrate categories on a cat:

| Category | Example | Read by | Authored by |
|---|---|---|---|
| **Actor-private commitment** | `HeldIntention` | Actor's own scoring + drop | L2 evaluator on softmax winner |
| **Public physical state** | `Dead`, `Injured`, `Pregnant`, `HeadDownCurled` (242) | Anyone via markers / `MarkerSnapshot` | Per-system step resolvers |
| **Publicly-performed practice state** | `JointIntention` *(this ticket)* | **Both partners** + cascading drop-gate | Practice author system (matchmaker) |

`JointIntention` is **mutually-public substrate** — not an
exception to 126's actor-private rule, but a distinct semantic
category. Real cats perceive each other's persistent
practice-engagement through scent, posture, mounting tolerance,
repeated proximity. We don't have those perception channels yet,
so the substrate stands in. Reading the partner's `JointIntention`
is reading publicly-performed practice state, not the partner's
internal commitment.

### Codified body language; codified irony

The doctrinal pair:

| | "Codified X" | Substrate proxy for |
|---|---|---|
| JointIntention itself | codified long-term body language | partner-perception (until 258 mental models) |
| Mismatched JointIntention stages | codified irony | dramatic-irony narrative beats |

When two paired cats hold mismatched stages — one cat believes
they're in `CourtshipCourting`; the other still holds
`CourtshipApproach` — the gap **is dramatic irony, mechanically
codified**. One cat is wooing while the other is just being
friendly; the audience (and the diagnostic tooling) can see it;
neither cat does. Codified — meaning produced as a measurable
side-effect of the field discipline, not as authored content. The
field discipline IS the literary device.

### Field discipline

`JointIntention` carries *only observables* — fields that map to
"long-term body language a real cat could perceive": practice,
partner, role, stage, tick markers. Internal heart-state
(`commitment_strength`, expiry, source) stays in the actor's own
`HeldIntention`. If a field can't be expressed as observable
practice-state, it does not belong in `JointIntention`.

## Current state

- `src/components/pairing.rs` ships `PairingActivity` — per-cat
  Component with `partner: Entity`, `adopted_tick`,
  `last_interaction_tick`. The §7.M `PairingDropBranch` enum (5
  variants) is the existing drop vocabulary. Per-cat drop
  predicate `should_drop_pairing(proxies, config)`.
- `src/ai/pairing.rs` ships `author_pairing_intentions` — per-tick
  author/drop system. Symmetric matchmaker: both cats co-emit when
  matched, both cats independently evaluate drop.
- `src/components/pairing.rs::pairing_bias_multiplier` — Commit B
  (ticket 257) helper. Resolvers in
  `src/steps/disposition/{groom_other,mentor_cat,socialize}.rs`
  and `src/ai/dses/socialize_target.rs` query the
  partner-equals-target case and multiply fondness/familiarity
  deltas by `pairing.bias_multiplier`.
- `src/resources/sim_constants.rs::PairingConstants` — 7 knobs
  (`candidate_range`, `emission_threshold`, `bias_multiplier`,
  `quality_*_weight × 3`, `romantic_floor`, `fondness_floor`).
- `Feature::PairingIntentionEmitted`,
  `Feature::PairingDropped`, `Feature::PairingBiasApplied` — three
  activation-counter Features, all canary-validated.
- 16 source files reference `PairingActivity` directly; 142 raw
  call sites.

`PairingActivity` has zero stage vocabulary — it's effectively one
persistent "Active" state until drop. The novel piece in
`JointIntention` IS stage progression + cross-practice generality.

## Proposed architecture

### `JointIntention` Component

```rust
#[derive(Component, Debug, Clone, serde::Serialize)]
pub struct JointIntention {
    /// Which practice this is. Drives matchmaker compatibility,
    /// stage-advancement logic, drop branches, and per-practice
    /// Feature emission.
    pub practice: PracticeKind,
    /// The other participant. `Entity` has no `Default`, so
    /// `#[serde(skip)]` (mirrors `PairingActivity.partner`).
    #[serde(skip)]
    pub partner: Entity,
    /// Role this cat performs in the practice. Courtship uses
    /// `Mutual` to preserve the existing symmetric PairingActivity
    /// shape 1:1; future practices may use asymmetric roles
    /// (Mentor/Apprentice, CacheHost/CacheStocker).
    pub role: PracticeRole,
    /// Current stage this cat believes the practice is in. Stages
    /// MAY mismatch between partners — see §Codified irony.
    pub stage: PracticeStage,
    /// Tick the JointIntention was first authored on this cat.
    pub adopted_tick: u64,
    /// Tick this cat last entered its current stage. Drives the
    /// `StageStalled` drop branch and trace `ticks_in_stage`.
    pub stage_entered_tick: u64,
    /// Most-recent observed partnered interaction (any
    /// practice-biased resolver pick). Refreshed by resolvers that
    /// already call `pairing_bias_multiplier` today.
    pub last_interaction_tick: u64,
}
```

All six fields are observable practice-state per §Field discipline.
Notably absent: `commitment_strength`, `expiry_tick`, `source`
(those live on the actor's `HeldIntention`, which a cat in a joint
practice ALSO holds).

### Practice / role / stage enums

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub enum PracticeKind {
    Courtship,
    // Future: CoMentoring, JointCacheStocking, PlayBout — each is
    // a separate follow-on ticket per §Out of scope.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub enum PracticeRole {
    /// Symmetric — both partners hold the same role. Used by
    /// Courtship initially (preserves PairingActivity's symmetric
    /// shape 1:1).
    Mutual,
    // Future: Initiator, Responder, Mentor, Apprentice, CacheHost,
    // CacheStocker — reserved for follow-on practices.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub enum PracticeStage {
    // Courtship stages — concrete vocabulary for the first practice.
    /// L1 eligibility opened (Friends bond + reproductive-eligible).
    /// PairingActivity's "matched, no bias yet" semantics map here.
    CourtshipApproach,
    /// Bias readers active; resolver picks multiplied;
    /// fondness/familiarity accruing. PairingActivity's "Active"
    /// state maps here.
    CourtshipCourting,
    /// Partners-or-Mates bond + fertile window for the queen-side.
    /// MateWithGoal predominantly fires from this stage.
    CourtshipMating,
    /// Post-conception or post-Mates-bond settled state. Bias
    /// still active; mating-DSE eligibility paused. Today's
    /// "PairingActivity held during pregnancy" maps here.
    CourtshipBonded,
    // Future stages for other practices.
}
```

`#[non_exhaustive]` on all three so future practices can extend
without breaking match exhaustiveness in archived trace
deserialization.

### Stage progression — observable, not synchronized

Each cat's stage advances **independently** based on their own
observable proxies. **Stages may briefly mismatch** between
partners; that's narrative texture (§Codified irony). The
maintenance system reads the cat's own state and bumps stage when
a transition predicate fires:

| Practice | Stage | Advance predicate (self-observable) |
|---|---|---|
| Courtship | Approach → Courting | first paired-resolver tick (`last_interaction_tick != adopted_tick`) |
| Courtship | Courting → Mating | bond ≥ Partners AND (Tom: not Winter; Queen: Estrus) |
| Courtship | Mating → Bonded | bond == Mates OR `Pregnant` on self OR partner |

Stage advance fires `Feature::JointStageAdvanced { practice, from,
to }`. Mismatched-stage windows are observable in the footer via
`Feature::JointStageMismatchTickAccrued { practice }` — see §Codified
irony for the diagnostic surface.

### Compatibility predicate (matchmaker)

The author system asks `is_practice_compatible(self, other,
practice)` symmetrically. For Courtship, this IS the existing
`pick_partner` predicate set verbatim — orientation compatibility +
reproductive eligibility + Friends-or-better bond + within
`candidate_range` + score ≥ `emission_threshold`. The matchmaker is
the only place the symmetric/asymmetric distinction matters
(matched pairs are committed; role is assigned at adopt time).

Future practices declare their own compatibility predicate (a
function `(self_fit, other_fit, rel) -> bool`); dispatch is
`match practice { Courtship => courtship::is_compatible(..), .. }`.

### Drop cascade — `JointDropBranch`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub enum JointDropBranch {
    // Five PairingDropBranch carry-overs (rename for joint context):
    PartnerInvalid,        // Dead/Banished/Incapacitated/despawned
    BondLost,              // bond no longer reaches the practice floor
    AspirationCascade,     // self life-stage / orientation / pregnancy invalidates eligibility
    SeasonOut,             // Tom-in-Winter / Queen-Anestrus/Postpartum
    DesireDrift,           // both axes collapse — evaluated on SELF state, not partner
    // Novel for JointIntention:
    PartnerLeftPractice,   // partner no longer holds compatible JointIntention
    StageStalled,          // self has been in stage > `stage_stall_ticks` ticks
    CompatibilityLost,     // is_practice_compatible(self, partner) flipped false
    Completed,             // terminal stage reached (`CourtshipBonded`)
}
```

`PartnerLeftPractice` is the cascade trigger. The maintenance
system checks
`partner.JointIntention.is_some_and(|p| p.practice == self.practice && p.partner == self_entity)`.
If false, drop self with this branch. This is the *only* read of
partner's substrate-internal state, and per §Semantic category is
allowed because JointIntention IS publicly-performed practice
state.

Branch precedence (first-match wins): `PartnerInvalid →
PartnerLeftPractice → BondLost → AspirationCascade → SeasonOut →
CompatibilityLost → StageStalled → DesireDrift → Completed`.
Mirrors `PairingDropBranch`'s ordering for the carry-over branches;
novel branches inserted to fire before slow-collapse checks
(cascade should always win over slow-collapse).

### Composition with `HeldIntention`

A cat in a JointIntention also holds a `HeldIntention` reflecting
the practice's stage-N held action (e.g., for `CourtshipCourting`,
`held_action = Action::Pair`). When the joint stage advances, the
cat's HeldIntention is re-authored to match.
`HeldIntention.target == JointIntention.partner` for paired holds.
The `IntentionMomentum` modifier reads the HeldIntention as usual;
the JointIntention doesn't supply a separate momentum scalar — its
job is the *structural* commitment (practice membership), not the
scoring lift.

### Substrate-vs-search-state classifier (§4.7)

`JointIntention` is **substrate**:

1. Not mutated by A* — `StateEffect::Set*` operates on
   `PlannerState`, never on cat Components.
2. Externally authored by the practice-author system (a Bevy
   system).

→ Substrate. Consumed by:

- Actor's own scoring pipeline (resolver bias readers read
  self.JointIntention).
- Actor's own drop-gate (reads self.JointIntention proxies).
- **Partner's drop-gate** (reads partner.JointIntention for cascade
  detection). This is the explicit category-3 read — JointIntention
  is mutually-public substrate.

### Bias readers — preserve the 257 Commit B contract

`pairing_bias_multiplier` becomes `joint_bias_multiplier(joint:
Option<&JointIntention>, target: Entity, practice: PracticeKind,
bias_multiplier: f32) -> (f32, bool)`. Fires when
`joint.is_some_and(|j| j.practice == practice && j.partner ==
target)`. The resolver-side call site is unchanged in shape — same
`(multiplier, amplified)` return — but the helper now filters by
practice as well as partner.
`Feature::PairingBiasApplied` becomes
`Feature::JointBiasApplied { practice: Courtship }`.

### Author system

`author_joint_intentions` system (`src/ai/joint_intention.rs`,
replacing `src/ai/pairing.rs`). Per-tick:

1. For every eligible cat with an existing `JointIntention`:
   - Build per-practice proxies → evaluate
     `should_drop_joint(proxies, config)` → on `Some(branch)`,
     remove component + fire
     `Feature::JointIntentionDropped { practice }`.
   - Evaluate stage-advance predicate → on `Some(new_stage)`,
     update stage + `stage_entered_tick` + fire
     `Feature::JointStageAdvanced`.
2. For every eligible cat WITHOUT a `JointIntention`: for each
   registered practice, run the matchmaker
   (`pick_compatible_partner(..)`). On match,
   `commands.entity(self).insert(JointIntention::new(...))` + fire
   `Feature::JointIntentionEmitted { practice }`. Symmetric — both
   cats co-emit.

The system runs every tick, idempotent. Replaces
`author_pairing_intentions` in `SimulationPlugin::build()`'s
schedule edge (same edge — after `update_mate_eligibility_markers`).

## Codified irony

When two paired cats hold mismatched JointIntention stages — one
cat believes they're in `CourtshipCourting`; the other still holds
`CourtshipApproach`, or has slipped back to a friendlier register
— the gap **is dramatic irony, mechanically codified**. One cat
is wooing while the other is just being friendly; the audience
(and the diagnostic tooling) can see it; neither cat does. The
field discipline IS the literary device.

Substrate hooks:

- `Feature::JointStageMismatchTickAccrued { practice }` — per-tick
  increment when `self.stage ≠ partner.stage`. Counted once per
  pair per tick — the lower-Entity-index side reports so the
  canary doesn't double-count.
- Trace-sidecar field on the focal cat's `L3Commitment` record:
  `joint_stage_mismatch_ticks_total: u64`.
- Direction-aware footer breakout (`mismatch_self_ahead` /
  `mismatch_partner_ahead`) so future tooling can distinguish "I
  think we're past where you do" from "you think we're past where
  I do" — the asymmetry of irony.

This is the substrate hook for future "courtship-misread"
narrative beats (Talk-of-the-Town-style gossip about the cat who
didn't realize they were being courted) and for tuning the
matchmaker (high mismatch → matchmaker is over-eager or
compatibility predicate is too loose; low mismatch → matchmaker
is fine and the colony just isn't dramatic enough).

## Touch points

**New files:**

- `src/components/joint_intention.rs` — Component +
  `PracticeKind` / `PracticeRole` / `PracticeStage` /
  `JointDropBranch` enums + per-practice drop / advance /
  compatibility dispatch + unit tests (mirror
  `pairing.rs`'s test depth — drop-branch precedence + bias-
  multiplier invariants).

**Modified files (16, matches the call-site analysis):**

- `src/components/mod.rs` — register new module; remove `pairing`
  after migration.
- `src/components/pairing.rs` — **deleted**. Its semantic content
  moves into `joint_intention.rs`.
- `src/ai/pairing.rs` → renamed `src/ai/joint_intention.rs` —
  generalized author system.
- `src/ai/dses/socialize_target.rs` — replace `PairingActivity`
  reads with `JointIntention { practice: Courtship }`.
- `src/steps/disposition/{groom_other,mentor_cat,socialize}.rs` —
  same.
- `src/ai/commitment.rs` —
  `target_invalidates_intention` checks `JointIntention.partner`.
- `src/components/held_intention.rs` — no field change;
  doc-comment updates on co-existence with `JointIntention`.
- `src/plugins/simulation.rs` — system rename in schedule edge.
- `src/resources/sim_constants.rs` — `PairingConstants` →
  `CourtshipPracticeConstants` inside a new `PracticeConstants`
  block; `#[serde(default)]` carry-over so old archive headers
  still deserialize. Add `stage_stall_ticks` knob (default 10000 ≈
  50 sim-time seconds, generous).
- `src/resources/system_activation.rs` — Features renamed:
  `PairingIntentionEmitted` → `JointIntentionEmitted { practice }`;
  `PairingDropped` → `JointIntentionDropped { practice }`;
  `PairingBiasApplied` → `JointBiasApplied { practice }`. Add
  `JointStageAdvanced`, `JointStageMismatchTickAccrued`. Carry-over
  Features keep their canary classification; `JointStageAdvanced`
  classifies `true` (expected per soak); mismatch ticks classify
  `false` (mismatch is healthy-sometimes-zero).
- `src/resources/trace_log.rs` — extend `L3Commitment` with
  optional `joint: Option<JointIntentionCapture>` block.
- `src/scenarios/mate_chain.rs` — update scenario assertions; this
  scenario is the load-bearing test for the migration.
- `src/systems/{disposition,goap,sensing}.rs` — replace
  `Option<&PairingActivity>` query fields with
  `Option<&JointIntention>` filtered on `practice == Courtship`.

**Not modified:**

- `src/components/held_intention.rs::HeldIntention` shape —
  co-exists; no field changes needed.
- DSE registry — `MateDse` and the courtship/mate DSEs operate on
  the same proxies; they just read `JointIntention` instead of
  `PairingActivity`.

## Dependencies

- `blocked-by: []`. Joint practices are mutually-authored
  (matchmaker scoring at adoption time), not cue-derived, so
  242 + 243 (body-cue + behavior-observation) are NOT prerequisites.
  126 is landed (the actor-private counterpart that establishes
  the §Semantic category framing).
- Pairs with 027 / 027b (mating cadence) — `PairingActivity` is
  the exemplar this ticket subsumes. The post-272 mating cadence
  is what migration parity is judged against.
- Pairs with 257 Commit B — bias-reader contract that must survive
  the rename.

## Out of scope

Each opens as a sibling ticket on the 127-landing commit per the
CLAUDE.md "antipattern migration follow-ups are non-optional" rule
(titles are descriptive — IDs are allocated at land time):

- **Additional concrete practices.** Co-mentoring (with 026
  apprenticeship XP), joint cache-stocking (with cooking/midden),
  play-bouts (the play continuity canary). Each gets its own
  follow-on with `--blocked-by 127`.
- **N>2 joint practices.** Group hunting parties, gossiping rings,
  kitten-tending circles. The `partner: Entity` field is bilateral
  by construction; multi-cat practices need a different shape
  (`participants: HashSet<Entity>` or a shared `PracticeSession`
  resource).
- **Asymmetric Courtship roles.** Today's PairingActivity is
  symmetric (`PracticeRole::Mutual`) and the migration preserves
  that 1:1. Asymmetric tom-Courter / queen-Courtee semantics are a
  follow-on tuning if behavior wants it.
- **Body-cue-driven joint adoption.** A cat observing a partner's
  body-cues (242 / 243) and *opting in* to a joint practice from
  cue-reading rather than matchmaker scoring. Composes once 242 +
  243 land. The matchmaker pattern in 127 is sufficient for
  Courtship parity.
- **Mental model of partner's commitment.** Once 258 (C3 mental
  models) ships, the cat's mental model can carry the *belief*
  about partner's JointIntention; that belief can lag the
  ground-truth substrate, enabling richer narrative. 127 ships
  ground-truth only.
- **Per-practice trust modulation.** Coordinator-directive trust
  (130) composes with `HeldIntention.source`, not JointIntention.

## Exit criterion

Three conditions on the canonical seed-42 deep-soak via
`just verdict`:

1. **Migration parity.** All mating-pipeline metrics within ±10%
   of the post-272 baseline (`bonds_formed`, `kittens_born`,
   `MatingOccurred`; `PairingIntentionEmitted_count` →
   `JointIntentionEmitted_count{Courtship}` mapped 1:1). Drift >
   ±10% triggers the four-artifact hypothesis methodology per
   CLAUDE.md. Hard survival gates pass:
   `Starvation == 0`, `ShadowFoxAmbush ≤ 10`,
   `never_fired_expected_positives == 0`.
2. **New positive Features fire.**
   `JointIntentionEmitted{Courtship}`,
   `JointBiasApplied{Courtship}`, `JointStageAdvanced` all
   non-zero on seed-42. Canary holds for the rename (all three are
   `expected_to_fire_per_soak() => true`).
3. **Drop cascade works.** Targeted scenario test (`just scenario`,
   new fixture under `src/scenarios/`): two paired cats; one drops
   their JointIntention via `DesireDrift`; partner's
   `PartnerLeftPractice` fires within 1 tick.

## Preparation reading

- Evans & Short, "Versu — A Simulationist Storytelling System"
  (IEEE TCIAIG, 2014) — the practices vocabulary 127 is named
  after. Cluster-C C2 framing in 007.
- [`docs/open-work/landed/126-bdi-intention-substrate.md`](../landed/126-bdi-intention-substrate.md)
  §Perceivability + §Substrate vs search-state — the actor-private
  rule 127 is the explicit counterpoint to (mutually-public
  substrate as a distinct category).
- `src/components/pairing.rs` + `src/ai/pairing.rs` — the exemplar
  this ticket subsumes. Read the §7.M drop-branch precedence and
  the symmetric matchmaker carefully; the migration must preserve
  both.
- Ticket 257 Commit B (landed `10f65c47`) — the bias-reader
  contract that must survive the rename.
- Ticket 272 (landed `2e3666e3`) — post-257 mating-cadence
  stabilization; migration parity is judged against this baseline.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-08 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- · **129** (blocked, C, score 0.89) — Care DSEs over perceivable intentions
- · **  1** (in-progress, —, score 0.87 (cross-cluster)) — Explore dominance over targeted leisure
- ✓ landed **146** (done, ai-substrate, score 0.87 (cross-cluster)) — 088 BodyDistressPromotion courtship-coverage investigation

<!-- linkages:end -->
## Log

- 2026-05-02: opened as 126 follow-on per CLAUDE.md
  antipattern-migration rule.
- 2026-05-11: design fleshed out from placeholder. Decisions:
  subsume `PairingActivity` into `JointIntention { practice:
  Courtship }` in this ticket (no follow-on cleanup debt);
  JointIntention is mutually-public substrate by §Semantic
  category (distinct from 126's actor-private rule, not an
  exception to it); codified-body-language proxy until 258's
  mental models land; mismatched stages are **codified irony**
  (measurable via `JointStageMismatchTickAccrued`). `blocked-by:
  []` confirmed — joint practices are mutually-authored, not
  cue-derived, so 242 / 243 are NOT prerequisites.
- 2026-05-11: Landed 2026-05-11 across 3 commits (A: substrate alongside PA, B: switch readers + stage progression + cascade, C: delete PA + open follow-ons 273-281). Verdict fail traced to (a) pre-existing chronic kitten starvation per ticket 273 and (b) baseline drift since SimConstants shape changed; re-baseline tracked by 281.
