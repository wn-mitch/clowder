---
id: 126
title: BDI intention substrate — perceivable per-cat commitment with momentum
status: ready
cluster: C
added: 2026-05-02
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, scoring-layer-second-order.md, strategist-coordinator.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

`docs/balance/scoring-layer-second-order.md` framing #1 names the
problem in Clowder terms; Rao & Georgeff (1991) and Wooldridge ch. 4
name it in BDI terms. The per-tick scoring layer has no model of the
economy it drives — bonds, skills, mentoring, mating, cooking are
slow-state supply chains that only accumulate when a *specific*
disposition repeats on a *specific* (cat × target) pair across many
ticks. Per-tick re-scoring with no goal-level commitment produces
two visible failures:

- **Flipper behavior near equal scores.** Two dispositions within the
  Tenure-modifier's ~0.10 lift oscillate; neither's plan completes;
  no Feature fires.
- **Supply-chain collapse.** Mating cadence (027), mentoring,
  cooking, and grooming-other have all hit zero or near-zero in
  soaks because the upstream commitment never holds long enough to
  reach the action that fires the trunk Feature.

Existing scaffolding goes part of the way:

- `Intention` enum + `CommitmentStrategy` (Blind / SingleMinded /
  OpenMinded) live in `src/ai/dse.rs` and ride on every DSE's `emit()`
  output (Phase 3a metadata, Phase 6a consumer wired in
  `src/ai/commitment.rs::should_drop_intention`).
- `PairingActivity` (`src/components/pairing.rs`) is a per-cat
  Component carrying a committed partner Entity, with full §7.M
  drop-gate vocabulary (`PairingDropBranch`), Adopted/Dropped
  Features, and a ticket-027b lineage proving the per-component
  per-DSE pattern works.
- `CommitmentTenure` (`src/ai/modifier.rs:448`) is a flat
  anti-oscillation modifier on the held disposition, governed by
  `disposition.oscillation_score_lift` (default 0.10).

What's missing is the substrate generalization: an Intention is held
*per cat per goal* (not just per matched-mate-partner), is *visible
to other cats' DSEs* (which is what unlocks helping/coordination
without an out-of-fiction director), and is the unit on which
momentum, lifecycle Features, and HTN handoff hang. Today the
scoring layer can only see "this cat had GoapPlan last tick" — not
"this cat intends X toward Y, since tick T, with strength S, sourced
from self vs from coordinator C."

## Current state

- `src/ai/scoring.rs` re-scores all DSEs every tick from
  `Needs`/`Personality`/markers; `CommitmentTenure` adds a flat
  per-disposition lift but knows nothing about goal identity or
  target stability.
- `src/systems/goap.rs::resolve_goap_plans` calls
  `commitment::proxies_for_plan` + `should_drop_intention` per cat
  per tick; the strategy table dispatches on `DispositionKind` only,
  not on the held Intention's target or age.
- `Intention` is a value type produced by `Dse::emit()` and consumed
  inline; it is **not** persisted as an ECS Component on the cat.
  The only persisted Intention today is `PairingActivity`, an L2
  one-off carved for mating.
- `src/ai/commitment.rs` exists and is correctly factored. This
  ticket extends, does not replace, that module.
- No cat can perceive another cat's Intention. Care delegation
  ("Hazel intends to rest because injured → I form an intention to
  make her soup") has nowhere to read from. Redundant-help and
  redundant-target conflicts are prevented today only by spatial
  accident plus 080's `Reserved` token (per-action, not per-goal).

## Proposed architecture

### `HeldIntention` Component

A new Component on every cat, optional (absent on freshly-spawned
cats and between adoption cycles). Generalizes `PairingActivity`'s
shape from "the partner I am pairing with" to "the goal-shaped
commitment I currently hold." Authored by the L2 evaluator
(`evaluate_and_plan` in `src/systems/disposition.rs` /
`src/systems/goap.rs`) when a winning DSE's emitted `Intention`
crosses the adoption threshold; cleared by `should_drop_intention`'s
gate.

```rust
#[derive(Component, Debug, Clone, serde::Serialize)]
pub struct HeldIntention {
    /// What the cat is committed to. Reuses the existing
    /// `Intention` enum from `src/ai/dse.rs` (Goal | Activity).
    /// Both variants already carry a `CommitmentStrategy`.
    pub intention: Intention,
    /// Optional target — the cat / building / tile / wildlife the
    /// intention is *toward*. Mirrors §6 target-taking DSEs.
    /// `None` for self-state intentions (Rest, Idle, Wander).
    /// `Serialize`-skip per `PairingActivity`'s precedent (Entity
    /// has no Default and the field is pure runtime state).
    #[serde(skip)]
    pub target: Option<Entity>,
    /// Tick of adoption. Drives momentum age curve and trace
    /// `ticks_held`.
    pub adopted_tick: u64,
    /// Per-Intention strength, in `[0, 1]`. Derived at adoption
    /// time from the winning DSE's score margin over the runner-up
    /// (high margin → high strength → resists preemption).
    /// Constant for the Intention's lifetime; not refreshed per
    /// tick (refreshing per tick re-creates the per-tick churn
    /// problem).
    pub commitment_strength: f32,
    /// Optional hard expiry tick. `None` for `Goal` Intentions
    /// (which end on `achievement_believed`); `Some(t)` for
    /// `Activity::Termination::Ticks(n)`.
    pub expiry_tick: Option<u64>,
    /// Where this intention came from. Self-formed dispositions
    /// are `SelfMotivated`; intentions adopted in response to a
    /// coordinator's directive carry the coordinator's `Entity` so
    /// downstream consumers can measure compliance and (future)
    /// weight the momentum bonus by trust for that coordinator.
    /// See ticket 130 for the trust axis; this ticket commits only
    /// the provenance field + read-site.
    pub source: IntentionSource,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub enum IntentionSource {
    SelfMotivated,
    CoordinatorDirective {
        #[serde(skip)]
        coordinator: Entity,
    },
}
```

`PairingActivity` stays as a specialization (it carries pairing-
specific semantic state — `last_interaction_tick`, the §7.M drop
branches). The two coexist; a cat can hold a `HeldIntention` for
`Hunt(target=Mouse#42)` *and* a `PairingActivity` for a partner.
`HeldIntention` is the generic substrate; `PairingActivity` is the
exemplar that informs its API.

### Goal-shape compatible with future HTN

The `Intention::Goal::state` field is already a `GoalState` carrying
a `&'static str` label and an `achieved` predicate. A future HTN
ticket (C4) needs to pattern-match goals to decompose them into
methods. To keep that hook open without authoring HTN substrate
here, this ticket commits that goal labels follow the existing
`ActivityKind` enum vocabulary plus a small enumerable set of
goal archetypes (`hunger_below_threshold`, `food_in_inventory`,
`partner_groomed`, `kitten_fed`, etc.) — i.e., goal labels are *not*
free-form strings beyond the §L2.10.5 `ActivityKind` list. Future
HTN authoring can extend `ActivityKind` (already
`#[non_exhaustive]`) without breaking persistence. C4 owns the
decomposition logic; this ticket owns only that the goal label is
machine-pattern-matchable.

### Source provenance for coordinator directives

Tickets 057 (`coordinator-directive-intention-strategy-row`) and 081
(`coordination-directive-failure-demotion`) author and consume
coordinator directives respectively. `HeldIntention.source` tags
each held intention as `SelfMotivated` or
`CoordinatorDirective(coordinator)`, making both composable with
this substrate: 057's strategy row, when it lands, writes
`HeldIntention { source: CoordinatorDirective(coord), .. }` instead
of an inline Intention. 081's demotion logic reads `source` plus
`IntentionFulfilled`/`IntentionAbandoned` Features to compute
per-coordinator compliance.

The `IntentionMomentum` modifier *reads* `source` so a future
trust-weighted lift can multiply the bonus by recipient-coordinator
trust without modifying the substrate. **This ticket commits only
the field and the read-site**; the trust axis itself + good-vs-bad
coordinator emergent effects spin out as ticket 130 (see Out of
scope).

Per the "no director" doctrine: directives are *perceivable
substrate that recipients score and may refuse*, not a thumb on the
scale. Once 130 lands, low-margin directives from low-trust
coordinators are expected to fail to adopt the majority of the
time; high-trust coordinators' directives override marginal scores.
Refusal is the default, not the exception.

### Momentum axis on the held intention

`CommitmentTenure` (`src/ai/modifier.rs:448`) lifts the *currently-
winning* disposition's score; this ticket adds a complementary lift
on the *held intention*'s underlying DSE, scaled by
`commitment_strength`:

- New `ScoreModifier`: `IntentionMomentum`. Applied in
  `default_modifier_pipeline` after `CommitmentTenure` (so the two
  stack: oscillation guard + commitment bonus).
- Magnitude: `commitment_strength × intention_momentum_lift`, where
  `intention_momentum_lift` is a new `DispositionConstants` knob
  defaulting to `0.10` (tuned alongside `oscillation_score_lift`'s
  `0.10` so the combined ceiling stays under `softmax_temperature ×
  4` — the threshold above which softmax-over-Intentions becomes
  effectively argmax).
- Decay: linear ramp-down from full lift at `adopted_tick` to zero
  at `expiry_tick` (Activity intentions) or after
  `intention_momentum_decay_ticks` for Goal intentions (new
  `DispositionConstants` knob, default 600 ticks ≈ five minutes
  sim-time at 1389 Hz).

This is what mechanically enforces commitment: the held DSE doesn't
hard-lock the planner; it gets a margin-weighted lift large enough
to require a meaningful alternative to preempt. Preempt threshold is
`commitment_strength × intention_momentum_lift` — tunable per cat
(via `commitment_strength`) and per system (via the constant).

### Reconsideration policy

Replace the per-tick re-evaluation cadence with a four-trigger
reconsideration set, evaluated cheaply each tick in
`resolve_goap_plans`'s existing per-cat loop:

1. **Plan failure / hard-fail.** Already handled by
   `should_drop_intention`'s `achievable_believed == false` arm
   (§7.2 hard-fail channel: `replan_count >= max_replans`). No
   change.
2. **Plan completion.** Already handled by `achievement_believed`.
   No change.
3. **High-urgency disposition crossing a preempt threshold.** New.
   The L1 Maslow pre-gate (`anxiety_interrupts`) already bypasses
   the commitment gate for starvation/threat (`AnxietyInterrupt`
   Feature). This ticket adds a second, softer threshold: any
   non-held DSE whose score exceeds
   `held_score + (commitment_strength × intention_momentum_lift)
   + intention_preempt_margin` triggers a reconsider. New
   `DispositionConstants` knob: `intention_preempt_margin`
   (default 0.05). This is the "single-minded but not stupid"
   knob.
4. **Belief change invalidating the goal.** Goal-specific. For
   target-bearing intentions, the target becoming
   Dead/Banished/Incapacitated/despawned drops the intention via
   the existing `partner_invalid` pattern from
   `should_drop_pairing`. Generalize that check into
   `commitment::target_invalidates_intention(target, world)`.

The four triggers replace nothing today — they extend
`should_drop_intention`'s call site to also fire on (3) and (4).

### Perceivability — the no-director coordination primitive

`HeldIntention` is a `Component`, queryable by any other cat's DSE
via standard Bevy queries. This is the load-bearing substrate-side
property per §4.7: an intention is *not* per-A*-node search state,
it is a fact about the world (this cat is committed to X) authored
by exactly one system (the L2 evaluator) and read by many.

Two consumer patterns this enables, both authored as future tickets
not in this scope:

- **Care DSEs.** A `Caretake_target` candidate query can include
  cats whose `HeldIntention` is `Goal { state: rest, target:
  None }` and who have `Injured`/`LowHealth` markers. The helper's
  intention is an independent commitment authored from observable
  state; no message-passing.
- **Claim tokens.** `Reserved` (`src/components/reserved.rs`,
  ticket 080) already prevents two cats targeting the same
  *resource*. Extend the same pattern: when a cat adopts a
  `HeldIntention` whose target is another cat (e.g.,
  `Mentor(target=apprentice)`), insert a per-target soft-claim
  surface that the runner-up's target picker can read. This stays
  in 080's vocabulary; this ticket only commits the perceivability
  contract that makes it possible.

### Substrate vs search-state classification (§4.7)

`HeldIntention` is **substrate**. Mechanical test per §4.7.2:

1. Does any `StateEffect::Set*` mutate it during A* expansion? No
   — A* operates on `PlannerState`, not on the cat's components.
2. Is there an external authorship path? Yes — the L2 evaluator
   (`evaluate_and_plan`) writes it from observable world state
   (winning DSE's score margin + emitted Intention).

→ Substrate. Consumed via standard Bevy queries; never threaded
through `MarkerSnapshot` (the snapshot is for ZST markers, not
Components carrying payload). Goal/target identity is pattern-read
directly by interested DSEs.

### Lifecycle Features

Three new `Feature::*` variants in `src/resources/system_activation.rs`:

- `IntentionAdopted` (Positive). Fires when L2 inserts a
  `HeldIntention`. `expected_to_fire_per_soak() => true`.
- `IntentionFulfilled` (Positive). Fires when the gate drops on
  `achievement_believed`. `expected_to_fire_per_soak() => true`.
- `IntentionAbandoned { reason: AbandonReason }` (Neutral). Fires
  on every other drop branch. `AbandonReason` enum mirrors the
  trigger that fired: `Preempted`, `BecameImpossible`,
  `TargetInvalid`, `Expired`, `DesireDrift`. The `Feature::*`
  itself is unparameterized (the canary counts the variant); the
  reason is recorded in the trace sidecar.
  `expected_to_fire_per_soak() => false` (drops are bursty;
  matches `PairingDropped`'s precedent).

The four existing `CommitmentDrop*` Features
(`CommitmentDropTriggered`, `…Blind`, `…SingleMinded`,
`…OpenMinded`, `…ReplanCap`) stay as-is — they're per-strategy
counters at the `should_drop_intention` granularity. The new
`IntentionAbandoned` aggregates at the goal granularity. Both fire
on the same drop event; they're complementary observability layers.

### Save/load + events.jsonl surface

- `events.jsonl` header bumps `constants` (three new
  `DispositionConstants` knobs: `intention_momentum_lift`,
  `intention_momentum_decay_ticks`, `intention_preempt_margin`).
  Comparability invariant per CLAUDE.md applies — runs from before
  this ticket are not header-comparable to runs after.
- `events.jsonl` per-tick line: no change. Intention state surfaces
  via the activation footer (`IntentionAdopted`/`Fulfilled`/
  `Abandoned` counts, broken out by `IntentionSource` variant so
  081's compliance metric and 130's trust calibration have the
  substrate they need) and via the focal-cat trace sidecar (§11.3
  L3Commitment record gains an `intention` block — already
  sketched in §11.3's example with `momentum.active_intention` /
  `commitment_strength` fields).
- No save/load round-trip: per `PairingActivity`'s precedent, the
  Component is `Serialize`-only with `Entity`-targets `serde(skip)`.
  Runtime state, rebuilt on load by the next L2 evaluation pass.

## Touch points

New files:

- `src/components/held_intention.rs` — `HeldIntention` Component +
  `IntentionSource` enum + `AbandonReason` enum + Component-level
  helpers. Mirrors `src/components/pairing.rs`'s shape and depth.

Modified files:

- `src/components/mod.rs` — register the new module + re-export.
- `src/ai/commitment.rs` — extend `should_drop_intention` to
  consume preempt-threshold trigger (3) and target-invalidation
  trigger (4); add `target_invalidates_intention`. Keep
  `BeliefProxies` shape; add `held_score` and `best_competitor_score`
  fields (or thread them as a separate `PreemptProxies` struct, TBD
  at implementation).
- `src/ai/modifier.rs` — add `IntentionMomentum` `ScoreModifier`
  alongside `CommitmentTenure`; register in
  `default_modifier_pipeline`. Modifier reads
  `HeldIntention.source` so 130's trust-weighted lift can hook in
  without re-touching the modifier surface.
- `src/ai/scoring.rs` (or wherever `CommitmentTenure` is read) —
  thread `Option<&HeldIntention>` into the modifier ctx so
  `IntentionMomentum` can lift the held DSE.
- `src/systems/goap.rs::resolve_goap_plans` — at the same call
  site that already invokes `proxies_for_plan` +
  `should_drop_intention`, also build `PreemptProxies` and
  `target_invalidates_intention`. On a winning DSE's adoption,
  insert `HeldIntention`; on drop, remove it and record the
  `AbandonReason` against `IntentionAbandoned`.
- `src/systems/disposition.rs::evaluate_and_plan` — same dual
  authorship as `goap.rs` for non-GOAP-driven dispositions.
- `src/resources/sim_constants.rs` — three new
  `DispositionConstants` knobs with rustdoc + `#[serde(default)]`
  fallbacks (so old `events.jsonl` can be deserialized for diff
  tooling).
- `src/resources/system_activation.rs` — `Feature::IntentionAdopted`
  / `IntentionFulfilled` / `IntentionAbandoned` enum variants;
  classify in `expected_to_fire_per_soak()` (first two `true`,
  third `false`); add to `Feature::ALL`. Footer aggregates each
  count broken out by `IntentionSource` variant.
- `src/resources/trace_log.rs` — extend the L3Commitment record per
  §11.3's existing schema sketch.

Not modified: `src/ai/dse.rs`. The existing `Intention` enum is
sufficient; this ticket persists it in a Component, not extends the
type. `src/components/pairing.rs` stays — the per-cat per-Intention
generalization sits alongside, not replacing.

## Out of scope

- **HTN method composition over committed intentions.** That's
  cluster C4 and lands as ticket **128 — HTN method composition
  over `HeldIntention.goal`**, blocked-by 126, `## Why`: "126
  commits the goal-label vocabulary; HTN decomposition over those
  labels is a strategist layer above BDI and an order of magnitude
  larger than 126."
- **Versu-style joint intentions / co-commitment.** A
  `HeldIntention` is single-cat. Two-cat practices (courtship as a
  co-committed multi-stage structure) is C2 and lands as ticket
  **127 — Joint-intention substrate for two-cat practices**,
  blocked-by 126, `## Why`: "126 lands per-cat perceivable
  intentions; joint-commit semantics — both cats must hold
  compatible intentions, drops cascade — needs the perceivability
  primitive but is a separate vocabulary."
- **Care DSE that reads other cats' `HeldIntention`.** The
  perceivability primitive lands here; the consumer DSEs (helper
  cooks soup for resting injured cat) land as ticket **129 — Care
  DSEs over perceivable intentions**, blocked-by 126, `## Why`:
  "126 makes intentions visible; the helper's own intention
  adoption from observed need is the next layer up."
- **Trust-weighted directive momentum + coordinator-quality
  effects.** This ticket lands the `IntentionSource` provenance
  field and a source-aware read-site on `IntentionMomentum`. The
  actual trust axis (per-cat per-coordinator respect Component,
  weighted multiplier on momentum for `CoordinatorDirective`
  sources, compliance-rate aggregate footer line) spins out as
  ticket **130 — Trust-weighted coordinator directive momentum**,
  blocked-by 126 + 057 + 081, `## Why`: "126 commits provenance;
  057 emits directive intentions; 081 demotes failing
  coordinators. 130 is the axis that closes the loop — directive
  lift scales with recipient trust, so high-trust coordinators'
  orders override marginal scores while low-trust coordinators'
  orders mostly fail to adopt. Enables observable good-vs-bad
  coordinator effects without an out-of-fiction director."
- **Subjective belief / mental models.** That's C3; the
  reconsideration trigger (4) "belief change invalidating the
  goal" uses ground-truth proxies (target dead/banished/
  despawned), not subjective belief. C3's mental-model substrate
  would later replace the proxy with a per-cat belief lookup; not
  in this ticket.
- **Removing `PairingActivity`.** Stays as the L2 mating
  specialization. A future cleanup ticket may collapse it into
  `HeldIntention` once the API stabilizes; not in scope here.

## Preparation reading

- Rao & Georgeff (1991), "Modeling Rational Agents within a
  BDI-Architecture" — KR 1991. The canonical commitment-strategy
  vocabulary (`Blind` / `SingleMinded` / `OpenMinded`) that
  `src/ai/dse.rs::CommitmentStrategy` already mirrors.
- Wooldridge, *An Introduction to MultiAgent Systems* (2nd ed.,
  Wiley 2009) ch. 4 "Practical Reasoning Agents" — the
  reconsideration-policy treatment (when does an agent re-evaluate?)
  this ticket's four-trigger set is built from.
- `docs/balance/scoring-layer-second-order.md` — framing #1 is the
  per-tick-churn problem this ticket addresses.
- `docs/systems/ai-substrate-refactor.md` §4.7 — the substrate-vs-
  search-state classifier `HeldIntention` is run through above; the
  doctrine that protects the perceivability contract.
- `docs/systems/ai-substrate-refactor.md` §7 (commitment) +
  `src/components/pairing.rs` — the existing exemplar pattern this
  ticket generalizes.

## Exit criterion

Three conditions, all measured on the canonical seed-42 deep-soak
via `just verdict`:

1. **Plan-preemption rate down ≥ 30% vs baseline.** Diagnostic:
   `IntentionAbandoned { reason: Preempted }` count over total
   `IntentionAdopted` count, footer-aggregated. Baseline: today's
   plan-removal rate from `resolve_goap_plans`'s `plans_to_remove`
   in a current `tuned-42` soak. Treat baseline as the
   `CommitmentDropTriggered` rate; new metric replaces it as the
   primary churn signal.
2. **Survival canaries un-degraded.** Hard gates from CLAUDE.md
   pass: `Starvation == 0`, `ShadowFoxAmbush <= 10`, footer
   written, `never_fired_expected_positives == 0`. Continuity
   canaries: grooming / play / mentoring / burial / courtship /
   mythic-texture each ≥ 1. The intuition this guards: too-strong
   commitment (`intention_momentum_lift` too high) starves cats
   stuck in non-Hunt intentions; the canary catches it.
3. **New positive Features fire.** `IntentionAdopted` and
   `IntentionFulfilled` produce non-zero counts on seed-42; both
   appear in the activation footer; the canary verifies (per
   `expected_to_fire_per_soak() => true`). `IntentionAbandoned`
   produces non-zero counts but is exempt from the canary (drops
   are bursty, see `PairingDropped` precedent).

If (1) regresses survival canaries (2 fails), reduce
`intention_momentum_lift` toward `oscillation_score_lift`'s 0.10.
If (3) under-fires, the adoption threshold is too high — likely
the L2 evaluator is gating adoption on a margin that current
softmax-over-Intentions never produces. Tune
`intention_preempt_margin` first.

## Dependencies / blocked-by

- `blocked-by: []`. The substrate refactor's structural prerequisites
  (Phase 3a Intention vocabulary, Phase 4 target-taking DSEs,
  Phase 6a commitment gate) are all landed (per 060's coverage map:
  Phase 3a/3b/3c/3d ✅, Phase 4 ✅, Phase 6a ✅).
- Pairs cleanly with 057 (coordinator-directive intention strategy
  row) and 081 (coordination-directive failure demotion) — both
  currently `blocked-by: [007]`; on land of 126, retarget both to
  `blocked-by: [126]` since `HeldIntention` + `IntentionSource` is
  the actual prerequisite. 057 will write `HeldIntention { source:
  CoordinatorDirective(coord), .. }`; 081 will read source-tagged
  lifecycle Features for compliance demotion.
- Pairs with 027 (mating cadence) — `PairingActivity` is the
  exemplar; this ticket does not modify it.

## Log

- 2026-05-02: opened from 007 cluster-C C1 expansion. Includes
  `IntentionSource` provenance to enable 057/081/130
  coordinator-directive composition.
