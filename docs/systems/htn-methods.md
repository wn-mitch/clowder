# HTN method composition (design note)

**Status: design note, not implemented.** Companion to the 128
epic (`docs/open-work/tickets/128-htn-method-composition.md`),
which tracks the implementation children. This document specifies
the substrate; the epic dashboards the work.

## Problem

Today's L2 substrate (126 `HeldIntention`) commits a cat to a
single goal-shaped Intention with a `CommitmentStrategy` and a
momentum lift. That's correct for one-step intentions
("groom_partner_smooth_fur"), but breaks down for multi-step arcs
that span minutes-to-weeks of sim-time:

- **`PairingActivity` / `JointIntention` (127)** carries Courtship
  through four hand-coded stages (Approach / Courting / Mating /
  Bonded) — substrate that's effectively a single hand-rolled
  *method* with no general vocabulary.
- **`Aspirations` chains (`src/components/aspirations.rs`)** track
  milestone progress with no per-tick emission of *what to do
  next* toward the milestone — the cat scores DSEs from the
  bottom up; the aspiration sits above as a passive observer.
- **Mating's L3 four-step chain (`MoveTo → Socialize → GroomOther
  → MateWith`)** lives in `disposition.rs:1873-1919` as a hand-
  coded plan template. Each new multi-step Intention re-invents
  this shape.
- **No unified inspection surface.** `JointIntention`,
  `Aspirations.active`, `Pregnant.stage`, `HeldIntention`,
  `KittenDependency`, `FatedLove` each carry a slice of "what is
  this cat aspirationally doing?" — readers reconstruct it by
  hand.

Hierarchical Task Network (HTN) method composition addresses all
four uniformly. Methods *decompose* an `Intention::Goal` into an
ordered sub-goal sequence; the goal-stack carries the cat's
current cursor; a single inspection surface renders the full
aspirational landscape.

## Master-spec alignment

HTN methods are **not a new layer** — they're the decomposition
mechanism for an existing one. The master spec already commits a
three-layer commitment architecture
(`docs/systems/ai-substrate-refactor.md`):

### §7.M three-layer mating (canonical worked example)

Mating uses Rao & Georgeff AI4 `INTEND(INTEND(φ))` to nest
commitment at three timescales:

| Layer | Horizon | Strategy | Persistence | Example |
|---|---|---|---|---|
| L1 `ReproduceAspiration` | multi-year | `OpenMinded` | High | "I want to be a parent" |
| L2 `PairingActivity` | multi-season | `OpenMinded` | Medium | "I am courting Hazel" |
| L3 `MateWithGoal` | single event | `SingleMinded` | High (Finish-Him) | "I am mating now" |

HTN methods don't change this. They make L3 (the multi-step Goal
that today hand-codes its chain) registry-driven. The same
three layers stay; the *decomposition* of L3's chain becomes a
catalogue.

### §7.7 aspiration-level commitment

§7.7 commits the L1 contract:
- Aspirations are *long-horizon Intentions that emit short-horizon
  Intentions*.
- Default strategy at the aspiration layer: `OpenMinded`.
- Reconsideration is event-driven over five classes (§7.7.a-e:
  life-stage, grief, fate, mood drift, plateau).
- `AspirationSet` holds 0..N concurrent aspirations with a
  consistency-check via four conflict classes (§7.7.1).

`src/systems/aspirations.rs` already exists with domain-affinity
chain selection + a partial `OpenMinded` stagnation-abandon path.
What's missing is the **positive emission logic** — given an
active aspiration at tick T, *which* short-horizon Intention does
it emit? See §H below.

### §L2.10.4 Intention = Goal | Activity

`Intention::Goal(GoalState) | Activity(ActivityKind, Termination)`.
Strategy rides on the Intention, not the DSE. **HTN methods
decompose `Goal` Intentions only.** `Activity` Intentions are by
definition sustained, not multi-step; method composition doesn't
apply.

### §4.7 substrate vs search-state

§4.7.2 mechanical classifier: a field is substrate iff (a) no
`StateEffect::Set*` mutates it during A* expansion AND (b) an
external authorship path exists. The proposed `HeldGoalStack`
passes cleanly:

1. No `StateEffect::Set*` reaches it (A* runs on `PlannerState`
   per sub-goal; the stack is invisible to the planner).
2. The L2 evaluator authors it from observable world state
   (winning DSE's emitted Intention + applicable method).

→ **Substrate**. No hybrid case (§4.7.3) needed.

### §11.5 trace records walk registries

> "The trace emitter must never hardcode a channel / map / DSE /
> consideration list. It walks the registries at runtime."

HTN's trace surface must walk `MethodRegistry`, not hardcode per-
method emission. Adding a new method to the registry
automatically appears in `trace-*.jsonl` and `CatSnapshot.goal_stack`
via a passive registry walk.

## Literature alignment

Methods compose against canonical HTN patterns
(`docs/reading-list.md` §C4):

### SHOP2 (Nau et al. 2003)

SHOP2 distinguishes three vocabularies. Clowder mapping:

| SHOP2 | Clowder analogue | Today |
|---|---|---|
| Operator | Step resolver (`resolve_*` under `src/steps/`) | Exists; CLAUDE.md contract |
| Method | `Method { goal_label, preconditions, sub_goals, failure_strategy }` | **128 introduces** |
| Axiom | Substrate marker (§4 doctrine) | Exists |
| Compound task | `Intention::Goal { state: { label } }` with registered method | Exists per §L2.10.4 |
| Primitive task | `Intention::Goal` with no registered method (adopted directly) | Exists per 126 |
| State | `MarkerSnapshot` + `PlannerState` split per §4.7 | Exists |

SHOP2 supports both ordered task lists (default) and `:unordered`
(parallel-order). **Clowder commits to ordered-only in Phase 1**;
`:unordered` is a deferred enrichment.

### F.E.A.R.-style total-order HTN (Humphreys, Game AI Pro vol. 1)

Game-AI HTN simplifies SHOP2 for runtime ergonomics:
- Methods register at compile time (Rust: `&'static [Method]`).
- Total-order task lists.
- First-applicable-precondition method selection (no softmax over
  methods — softmax already lives at L2 Intention scoring).
- Short-horizon plans; replan on explicit world-state change.

Clowder mirrors F.E.A.R.-style for Phase 1:
- Methods register at app build via `populate_method_registry`,
  parallel to `populate_dse_registry` and
  `populate_influence_map_registry` (ticket 207 precedent).
- Ordered sub-goal lists only.
- Method selection: first-applicable-precondition match.
- Replan trigger: per-leaf `replan_count >= max_replans`
  (existing GOAP signal, §7.2) → method-failure cascade.

## Three-layer composition

| Layer | Horizon | Component / system | Strategy | Drop gate | New under 128? |
|---|---|---|---|---|---|
| **L1 Aspiration** | multi-year | `Aspirations` Component + `aspirations.rs` (existing) | `OpenMinded` (§7.7) | §7.7.a-e event-driven | No — but emits[] table added per §H |
| **L2 Method frame** | per-Goal-Intention | NEW `HeldGoalStack` Component + `MethodRegistry` resource | inherits from method; backtrack on leaf abandon | leaf abandon → method failure strategy | **Yes — 128** |
| **L2 Intention** (leaf) | multi-tick | `HeldIntention` (126) | per `Intention.strategy` (§7.1) | §7.1-§7.6 drop gate | No |
| **L3 Plan** | per-A*-step | `GoapPlan` + `src/ai/planner/` | `Blind` (replans on belief change) | `replan_count >= max_replans` (§7.2) | No |

**Critical relationship.** The top frame of `HeldGoalStack` is the
parent of `HeldIntention`. They're kept consistent by a single
authorship site (the L2 evaluator at
`src/systems/goap.rs:568-635::evaluate_and_plan`). The top frame
names *which method* and *which sub-goal index*; `HeldIntention`
names *which leaf intention*. Together they form the cat's
current actor-private commitment vector.

## Architecture

### `MethodRegistry` resource

```rust
// src/ai/methods/mod.rs (future file)
pub struct Method {
    pub id: MethodId,                              // canonical slug
    pub goal_label: &'static str,                  // matches GoalState.label
    pub applicable_when: ApplicableWhen,           // §G dormancy-typed
    pub sub_goals: &'static [SubGoal],             // ordered
    pub failure_strategy: MethodFailure,           // Backtrack | Abandon | Retry
}

pub enum SubGoal {
    Goal(GoalState),                               // recursive — has its own method
    Primitive {
        label: &'static str,
        action: Action,                            // primitive DSE leaf
        target_hint: TargetHint,                   // per §6.3 target-taking
    },
}

pub enum MethodFailure {
    Backtrack,                                     // try next applicable method
    Abandon,                                       // propagate failure up
    Retry { max_attempts: u8 },                    // reset sub_goal_index = 0
}
```

Methods register at app build via `populate_method_registry`
(precedent: `populate_dse_registry`, `populate_influence_map_registry`).

### `HeldGoalStack` Component

```rust
// src/components/held_goal_stack.rs (future file)
#[derive(Component, Debug, Clone, serde::Serialize)]
pub struct HeldGoalStack {
    pub frames: Vec<GoalFrame>,    // top = active leaf; len <= MAX_DEPTH
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GoalFrame {
    pub method: MethodId,
    pub goal_label: &'static str,
    pub sub_goal_index: usize,     // cursor into method.sub_goals
    pub adopted_tick: u64,
    #[serde(skip)]
    pub target: Option<Entity>,
    pub source: IntentionSource,   // extended with AspirationEmitted variant
}
```

- Sibling Component to `HeldIntention`, not a replacement.
- §4.7.2 classifies as **substrate** (no `StateEffect::Set*`
  reaches it; L2 evaluator authors it).
- `MAX_DEPTH` cap (8 — measured-and-revised in implementation)
  prevents authoring loops; cap-hit emits
  `Feature::MethodDepthExceeded`.
- `target` for target-taking methods (per §6.3).
- `source` extends 126's `IntentionSource` with
  `AspirationEmitted(AspirationId)` for the L1→L2 emission path.
- Serialize-only (Entity skip) per 126 + 127 precedent. Runtime
  state, rebuilt on load.

### L2 evaluator integration (single author site)

Author site: `src/systems/goap.rs:568-635::evaluate_and_plan`.
Per-tick sequence:

1. Score DSEs (existing).
2. Softmax-select winning DSE (existing).
3. Read winning DSE's emitted `Intention`.
4. If `Intention::Goal { state, .. }` AND
   `MethodRegistry.lookup(state.label, world, cat)` returns
   `Some(method)`:
   - Push `GoalFrame { method, sub_goal_index: 0, .. }` onto
     `HeldGoalStack`.
   - Recursively resolve sub_goals[0]: if it's another
     `SubGoal::Goal`, look up its method, push another frame.
     Repeat until a primitive leaf is reached or `MAX_DEPTH`
     hits.
   - Adopt the leaf as `HeldIntention` (existing 126 path).
5. Else (no method or `Activity` Intention): adopt directly as
   `HeldIntention` (existing 126 behavior — preserved as the
   no-method fallback).

The 126-existing `HeldIntention` author site is preserved as the
fallback; the new HTN logic is an *enrichment* gate before it.

### Lifecycle Features

Three new `Feature::*` in `src/resources/system_activation.rs`,
classified per `expected_to_fire_per_soak()`:

| Feature | Polarity | Expected | Fires when |
|---|---|---|---|
| `MethodAdopted` | Positive | `true` | A method is selected at L2 (frame pushed) |
| `SubGoalAdvanced` | Positive | `true` | A leaf fulfilled and the next sub-goal is adopted |
| `MethodBacktracked` | Neutral | `false` | A method failed; alternative tried or stack popped |
| `MethodDepthExceeded` | Neutral | `false` | Stack hit `MAX_DEPTH`; authoring-loop canary |

### Trace + inspection surface

Per §11.5 *registry-walk discipline*, every surface that renders
goal-stack state walks the registry — no hardcoded per-method
emission.

**L3 trace sidecar (`src/resources/trace_log.rs::L3Commitment`)**
gains a `method_stack` field. Per focal-cat tick:

```json
{"layer": "L3", "cat": "Whiskers", ...,
 "momentum": {...},
 "method_stack": [
   {"method": "acquire_stealth_via_self_craft",
    "goal": "stealth_gear_acquired",
    "sub_goal_index": 0, "of": 4, "target": null,
    "source": "aspiration:hunting-mastery"},
   {"method": "gather_stealth_materials",
    "goal": "stealth_materials_in_inventory",
    "sub_goal_index": 2, "of": 3, "target": null}
 ],
 "leaf_intention": {"kind": "Goal",
                    "label": "tile_resource_acquired",
                    "target": "tile(34,18)"}}
```

**L1Aspiration trace record (new)** — per active aspiration per
tick, the emit-walk that produced this tick's emission:

```json
{"layer": "L1Aspiration", "cat": "Whiskers", "tick": 1610042,
 "aspiration": "hunting-mastery", "milestone": 2,
 "emit_walk": [
   {"label": "hunt_high_value_prey", "applicable": true,
    "method_live": true, "emitted": true}
 ],
 "fallback_used": false}
```

**`events.jsonl` `CatSnapshot`** gains `goal_stack` and
`active_aspirations` fields:

```rust
pub struct GoalFrameSnapshot {
    pub method: String,           // slug
    pub goal_label: String,       // slug
    pub sub_goal_index: usize,
    pub sub_goal_count: usize,
    pub target: Option<String>,   // stable slug, not Entity
    pub source: String,           // "self" | "coordinator" | "aspiration:<id>"
}
```

**`examples/inspect_cat.rs`** gains a `print_aspirations` section
rendering: aspiration set, current goal stack, recent method
history (`MethodAdopted` / `SubGoalAdvanced` / `MethodBacktracked`
events extracted from the snapshot history).

All three surfaces read the *same* `HeldGoalStack` +
`Aspirations` substrate. Single source of truth.

## Lifecycle — adopt / advance / backtrack / abandon / complete

**Adopt.** L2 evaluator pushes a frame (above). Emits
`Feature::MethodAdopted`. Recurses into the first sub-goal,
pushing additional frames for nested `SubGoal::Goal` entries.

**Advance.** Existing `IntentionFulfilled` Feature point in
`resolve_goap_plans`. When a leaf fulfills:
- Increment top frame's `sub_goal_index`.
- If `sub_goal_index < method.sub_goals.len()`: adopt the next
  sub-goal (recurse for `SubGoal::Goal`). Emit
  `Feature::SubGoalAdvanced`.
- Else: pop the frame; propagate advance to the parent (its
  `sub_goal_index` increments). Recurse upward.
- If stack empty: emit `Feature::IntentionFulfilled` for the
  root goal.

**Backtrack.** Existing `IntentionAbandonReason::BecameImpossible`
or `TargetInvalid` on the leaf. Consult top frame's
`method.failure_strategy`:
- `Backtrack`: try the next-applicable method for the same
  `goal_label`. Push new frame, adopt sub-goal 0. Emit
  `Feature::MethodBacktracked`.
- `Abandon`: pop the frame; propagate abandonment to parent.
- `Retry { max_attempts }`: reset `sub_goal_index = 0`,
  increment retry counter; abandon if counter exhausted.

**Abandon.** Stack empty. Emit `Feature::IntentionAbandoned`
(existing 126 Feature) for the root goal.

**Complete.** Stack empty. Emit `Feature::IntentionFulfilled`
(existing 126 Feature) for the root goal.

## Substrate classification (§4.7.2 trace)

Run the mechanical classifier on `HeldGoalStack`:

1. Does any `StateEffect::Set*` mutate it during A* expansion?
   `grep "StateEffect::Set" src/ai/planner/` returns hits for
   `SetPreyFound`, `SetInteractionDone`, `SetConstructionDone`,
   etc. None mention `HeldGoalStack`, `GoalFrame`, or
   `sub_goal_index`. **No.**
2. External authorship path? Yes — the L2 evaluator at
   `goap.rs:568-635` authors it from observable world state
   (winning DSE's emitted Intention + applicable method's
   precondition predicate).

→ **Substrate.** Cleanly. No hybrid case (§4.7.3). Read by:

- Actor's own scoring + drop-trigger pipelines (single-actor
  readership per §4.7.5's perceivability constraint, mirroring
  126's `HeldIntention` discipline).
- Actor's own trace emitter (L3Commitment + CatSnapshot).
- Never read across cats — methods are actor-private
  decomposition, like `HeldIntention`. Cross-cat practice state
  lives in 127's `JointIntention` (mutually-public substrate),
  which methods *mirror* but don't replace.

## L1→L2 emission picker (§H)

§7.7 names what makes an aspiration **drop** but not what makes
it **emit** a specific short-horizon Intention at any given tick.
HTN method composition catches emitted Goals by `state.label`;
something has to emit those labels. The picker design closes
this gap.

### Picker contract

Per cat per tick, per active `ActiveAspiration`:

1. **Already-in-flight check.** If
   `HeldGoalStack.frames[0].source == AspirationEmitted(this.id)`,
   skip emission — commitment momentum (§7.4) carries the
   existing commitment. The picker is a re-emission policy, not
   a thrash policy.
2. **Walk the milestone's authored `emits` list.** Find the
   first `Emit` row where:
   - `MethodRegistry.lookup(row.label, world, cat).is_some()` —
     a method exists AND is applicable (dormant methods are
     skipped naturally because `lookup` returns `None` for
     `PendingSubstrate` variants).
   - `row.applicable_when(world, cat)` — per-cat precondition.
   Emit `Intention::Goal { state: { label: row.label, .. },
   strategy: row.strategy }` into the L2 scoring pool.
3. **Domain-affinity fallback.** If no `emits` row applies,
   walk the registry for any method whose `goal_label` is tagged
   with the aspiration's domain AND whose precondition holds.
   Pick the highest-affinity method's `goal_label`, emit.
4. **Silent quiet.** If neither path produces a candidate, the
   aspiration emits nothing this tick. Multiple quiet ticks
   escalate to §7.7.e stagnation-abandon (existing).

### `Milestone` shape extension

```rust
pub struct Milestone {
    pub name: &'static str,
    pub gate: fn(&World, Entity) -> bool,         // existing
    pub emits: &'static [Emit],                   // NEW
    pub progress_tracker: ProgressTracker,        // existing
}

pub struct Emit {
    pub label: &'static str,
    pub applicable_when: fn(&World, Entity) -> bool,
    pub strategy: CommitmentStrategy,
    pub priority: Priority,                       // enum, not f32
}

pub enum Priority {
    Primary,    // first-line emission
    Secondary,  // tried after Primaries
    Tertiary,   // safety net
}
```

The picker walks `emits` in `Priority` order, then by
registration order within a tier; first match wins. **No softmax
over emissions inside one aspiration** — softmax happens at the
L2 DSE pool where this aspiration's emission competes against
every other emission (other aspirations, per-tick DSE scoring,
coordinator directives).

### Composition

**With §7.W axis-capture.** Cats with multiple concurrent
aspirations (§7.7.1) emit one Goal per aspiration per tick. All
emissions enter the L2 pool simultaneously; softmax picks one to
adopt. Losing emissions are **not dropped** — they stay as
"active but losing" axes per §7.W.2, accumulating fulfillment-
deficit and feeding mood-valence. The picker doesn't enforce
mutual exclusion; the L2 softmax does selection; §7.W handles
the warring-self dynamic.

**With coordinator directives (057).** Directives are a sibling
emission path. The recipient cat's L2 evaluator sees both the
aspiration's emitted Intention AND the directive's emitted
Intention (with `source: CoordinatorDirective(coord)`). Both
compete in the same softmax pool. 057 doesn't go through the
picker; it has its own author path.

### Reactive-emit yield rule (composition rule, ticket 395)

§L2.10.6's softmax-over-Intentions across `{DSE-Activity-default}
∪ {emitted-Goals}` is the eventual contract — emitted Goals
compete in the same pool as DSE Activity-default wraps, and the
L2 winner reflects their relative scores. Phase-1 implementation
(at 320 land) deferred the formal softmax and uses a **priority
override** at the L2 wrap site (`goap.rs:2733-2790`): when any
emission exists, the highest-`Priority` row replaces the
softmax-winning Activity wrap with `Intention::Goal { label,
strategy }` unconditionally. The 364 frame-pin
(`goap.rs:2410-2449`) then overrides `chosen_action` to the
frame's leaf primitive. Together these two overrides discard the
L2 softmax winner whenever a Live method's reactive emit fires —
even when softmax would have picked a high-urgency rescue DSE
like Caretake for a starving kitten.

Until §L2.10.6's formal softmax lands, **reactive emits must
declare their yield conditions explicitly in `applicable_when`**:
a substrate marker whose presence suppresses the emit for the
tick. The rule:

> A reactive emit's `applicable_when` predicate MUST consult an
> already-authored marker that signals "an acute urgency within
> this method's own substrate domain is in flight." When the
> marker is set, the predicate returns false → no emission → no
> Goal-wrap → no frame push → the L2 softmax winner (Caretake,
> Flee, whatever the per-tick DSEs scored) executes as authored.
> The frame from prior ticks survives via `resolve_goap_plans`'s
> `PreserveStackOnly` path; once the marker clears, the arc
> resumes mid-stride on the next tick.

**Why this is substrate, not a hack.** The yield marker
(`IsParentOfHungryKitten` for `rear_kitten`) was already
authored for an unrelated consumer (Caretake's own-kitten-anywhere
targeting fallback, ticket 161). Consuming it here adds no new
substrate — the reactive emit reads the same `0/1` signal that
Caretake's DSE consumes. The rule composes naturally with §4.7.2's
substrate-vs-search-state classifier: yield markers are L1
substrate authored from observable world state; they're not new
search-state introduced by the rule.

**Why per-emit and not at the pin.** The pin operates on already-
pushed frames. Yielding at the pin layer requires either (a)
authoring an interrupt list per method (substrate proliferation,
slippery slope) or (b) re-deriving softmax winner vs. pin's leaf
each tick to decide whether to honor the pin (replaying scoring
state). Yielding at the emit layer is one substrate touchpoint
per method, consumes existing markers, and short-circuits both
overrides simultaneously.

**Why per-method and not global.** Each method's "acute urgency"
is domain-specific. `rear_kitten` yields to hungry-kitten
substrate (`IsParentOfHungryKitten`); `mourn_at_grave` (when its
`Mourning` insertion path lands) will yield to e.g.
`HasAcuteSafetyNeed` (immediate predator threat) or
`HasAcuteHungerNeed` (starvation imminent), authored at whichever
system already computes those urgency thresholds. A global yield
predicate would either be too permissive (yields when no
domain-acute condition exists) or too coarse (suppresses Live
arcs that shouldn't yield). Each emit row owns its yield contract
and points to the marker's existing author.

**Anti-patterns this rule prevents:**

- *Authoring a new marker just for the yield.* If no existing
  substrate signals the acute urgency, the missing substrate is
  the actual ticket — author it for the domain consumer first
  (the DSE that would benefit from the signal), then consume it
  from the reactive-emit predicate as a downstream effect.
- *Yielding in the pin (`goap.rs:2410`) instead of the emit.*
  This is one layer too low. By the time the pin runs, the L2
  wrap site has already replaced the held intention with the
  emitted Goal and the frame has been pushed; un-pinning produces
  inconsistent state (held intention says "kitten_reared", but
  chosen_action says Caretake — what does `IntentionFulfilled`
  hook do?).
- *Using `applicable_when` as a generic per-tick veto.* The
  yield is for *acute domain urgency* — events that the method's
  own substrate flags as "this arc shouldn't be in flight right
  now." Non-urgent competing priorities (the cat is also tired,
  the cat is also hungry-on-self) are L2 softmax's job; they
  should be expressed as DSE scores, not yield markers.

**Precedent table.** Maintained as Live methods adopt the
contract:

| Method | Yield marker | Author | Reason |
|---|---|---|---|
| `rear_kitten` | `IsParentOfHungryKitten` | `update_kitten_cry_map` (ticket 161) | A dependent kitten's hunger is below the cry threshold — Caretake's softmax score is high; arc must yield so the rescue runs. |
| `mourn_at_grave` (pending) | TBD (likely `HasAcuteSafetyNeed`) | TBD | Predator-near-self / starvation-imminent — survival overrides grief arc. |

§L2.10.6's formal softmax retires this rule by making it
implicit: a high-scoring Caretake DSE would simply outscore the
emitted Goal in the unified pool, and the L2 wrap would adopt
Caretake without needing an `applicable_when` gate at all.
Until then, the per-emit yield is the substrate-clean way to
express "this Live arc shouldn't be pushed right now."

## Dormant-method discipline (§G) — `ApplicableWhen::PendingSubstrate`

Many methods the universal-aspiration framing requires (stealth-
cloak crafting, mourning vigils, ward apprenticeships) depend on
substrate that doesn't yet exist (slot inventory, wearable-effect
hooks, crafting recipes, grief-vigil actions). 128 still ships
these method *definitions* — they're load-bearing design
artifacts — but they must remain dormant until prerequisite
substrate lands.

### Wrong primitives

- **`todo!()` / `unimplemented!()` / `unreachable!()`** — all
  panic at runtime. They break Clowder's no-silent-failure
  step-resolver discipline, encode no blocker metadata, and rely
  on ad-hoc grep for audit.

### Right primitive — typed dormancy

```rust
pub enum ApplicableWhen {
    Live(fn(&World, Entity) -> bool),
    PendingSubstrate {
        blocker: &'static str,                   // ticket slug
        eventual: fn(&World, Entity) -> bool,    // compiles; never called
    },
}
```

`MethodRegistry::lookup` returns `None` for `PendingSubstrate`
methods. They appear in `just methods --pending` with their
blocker; they participate in design-time grep / similar-search;
they exercise the type system (the `eventual` predicate must
typecheck against current substrate even when its truth value is
always false today).

Four properties `todo!()` lacks:

1. **Compiles end-to-end.** The eventual predicate references
   real markers, components, helper fns — design intent is
   type-checked.
2. **No runtime panic risk.** Registry returns None; the L2
   author site's no-method-applies fallback handles selection.
3. **Audit surface.** `scripts/check_method_registry.sh`
   verifies each `blocker` names an **open** ticket in
   `docs/open-work/tickets/` AND that the ticket's frontmatter
   `wires-method` field references this method's id.
4. **Glue-or-bust discipline.** Every `PendingSubstrate { blocker }`
   method MUST have a corresponding ticket that explicitly wires
   it. Methods without glue tickets fail CI. This is the
   load-bearing claim that keeps "natural trees" sprouting:
   design intent without an open ticket rots; the script makes
   rotting a CI failure.

### Action-enum stubs for dormant methods

Where a dormant method references an `Action::*` variant that
doesn't exist yet (`Action::WearItem`, `Action::Craft`,
`Action::PetitionCoordinator`), add the variant via the existing
substrate-stub-allowlist discipline (precedent §4.7.7, ticket
252):

1. Variant declaration in `Action::*` (one line in the enum).
2. Placeholder step resolver with the five required rustdoc
   headings; body returns `StepOutcome::Failed { reason:
   "<blocker-ticket> not yet landed" }`. Contract-compliant,
   non-witnessing; never reached in production because no live
   method emits the Action.
3. `scripts/substrate_stubs.allowlist` entry naming the wiring
   ticket and resolver symbol.

`StepOutcome::Failed { reason }` is the canonical "intentionally
inert" exit — distinct from a silent advance (forbidden), a
panic (loses the run), and a witnessed advance (lies about
effect).

### Tier 1 / Tier 2 split

**Tier 1 — Live methods (`ApplicableWhen::Live`) at 128's land.**
Methods whose entire sub-goal chain references existing Actions
and substrate. Land batch is deliberately narrow:

- `courtship_method` — mirrors 127's `JointIntention.stage`
  advance. Sub-goals: approach-partner → allogroom-partner →
  mate-with-partner (existing 4-step chain).
- `gestation_method` — mirrors `Pregnant.stage` (Early → Mid →
  Late). Method narrates the progression; pregnancy.rs still
  authors stage transitions.
- `aspiration_milestone_wrapper.hunting` — wraps the Hunting
  chain's milestones with `emits[]` tables.
- `aspiration_milestone_wrapper.social` — wraps the Social
  chain's milestones.

**Tier 2 — Demonstration methods (`ApplicableWhen::PendingSubstrate`).**
Methods that document design intent for the substrate roadmap.
Compile, live in registry, appear in `just methods --pending`,
flip to `Live` when blockers land:

- `acquire_stealth_via_self_craft` — blocker:
  wearable-slot + crafting-recipe substrate.
- `acquire_stealth_via_commission` — same blocker.
- `mourn_at_grave` — blocker: grief-vigil action vocabulary.
- `rear_kitten` — blocker: kitten-rearing action vocabulary.
- `aspiration_milestone_wrapper.<chain>` for Combat / Herbcraft
  / Exploration / Building / Leadership — each has its own glue
  ticket that authors the chain's `emits[]` tables and flips
  the wrapper to Live.

The Tier-1/Tier-2 distinction is **registry-data**, not folder-
structure. Methods coexist in `populate_method_registry`; the
`ApplicableWhen` variant is the discriminator.

## Migration catalogue — how existing substrate composes

Six existing aspiration-shaped substrates. Disposition under HTN:

| Substrate | File | Disposition | Glue |
|---|---|---|---|
| `Aspirations` chains | `components/aspirations.rs` | **Stays as L1 substrate; gains per-milestone `emits` table.** Aspirations are the engine that picks which Goal label to emit per tick (§H above); the picker walks each milestone's authored `emits` list. Methods catch the emitted Goal labels and decompose them. No "migration" — the Component stays; what changes is the milestone-definition shape. | Per-chain wrapper ticket (one per `AspirationDomain`). |
| `JointIntention` (127) | `joint_intention.rs` | **Mirrored.** `courtship_method` has 4 sub-goals matching the 4 stages (Approach → Courting → Mating → Bonded). Method advance keeps `JointIntention.stage` in sync. JointIntention stays as mutually-public projection per 127. | Tier 1 — courtship_method at 128's land. |
| `Pregnant` stages | `pregnancy.rs` | **Mirrored.** Gestation method (Early → Mid → Late) per §7.M.7. Pregnant stays as DSE activation gate per §7.M.7.6. | Tier 1 — gestation_method at 128's land. |
| `KittenDependency` | `kitten.rs` | **New method on the mother.** `rear_kitten(target_kitten)`: nurse → wean → teach → release. KittenDependency stays on the kitten as the maturity tracker. | Tier 2 — blocker: kitten-rearing action vocabulary. |
| `FatedLove` / `FatedRival` | `fate.rs` | **Mythic seeds.** Awakening flips markers that bias method-applicability gates (Courtship-with-Fated-partner method has higher applicability score). No new method; existing methods gain Fated-preconditions. | No glue — composes via existing method preconditions. |
| Coordinator `Directive` | `coordination.rs` | **Method seeds (057).** Each `DirectiveKind` maps to a method id. Recipient cat's L2 evaluator reads the directive, looks up the method, adopts it with `source: CoordinatorDirective(coord)`. | Pairs with 057 (`coordinator-directive-intention-strategy-row`). |

Per-tick DSEs (Mentor, Patrol, Scry, GroomOther, Caretake, Bury,
Cook) stay as primitives. Methods that *use* them list them as
`SubGoal::Primitive` entries.

## Inspection invariant — single source of truth

Three surfaces render goal-stack state; all three read the same
substrate:

1. **`just inspect <cat>` (`examples/inspect_cat.rs`)** — new
   `print_aspirations` section. Renders aspiration set + goal
   stack + recent method history.
2. **L3 trace sidecar (`L3Commitment.method_stack`)** — per-tick
   focal-cat record.
3. **`events.jsonl CatSnapshot.goal_stack`** — minimum-schema
   rendering for logdb queries.

All three are registry-walked per §11.5. Adding a new method to
`populate_method_registry` automatically appears in all three
surfaces.

## Strategist-coordinator alignment

`docs/systems/strategist-coordinator.md` is a design note for a
**colony-level** strategic-goal selector (Civ-style two-layer
priority queue). HTN method composition is the **per-cat**
tactical decomposition. They compose:

- The strategist-coordinator (when implemented) selects strategic
  goals for the colony.
- Coordinators issue per-cat directives (057's strategy row) that
  emit `Intention::Goal` with `source: CoordinatorDirective(coord)`.
- HTN methods catch those Intentions and decompose them just
  like aspiration-emitted ones.
- The cat experiences directives as method-driven multi-step
  arcs, not as inscrutable per-tick score bumps.

128 doesn't implement the strategist; 128 makes per-cat HTN
ready for it. The dovetail with 057 is named (the
PendingSubstrate methods include directive-decomposition
methods).

## Future / out of scope

Phase-2 enrichments, deferred to follow-on tickets:

- **`:unordered` method bodies** (SHOP2-style partial-order
  decomposition). Authoring complexity is real; defer until a
  use case demands parallel sub-goals.
- **Softmax over methods.** First-match-precondition is the
  Phase-1 commit; spec recommends first-match permanently
  because softmax already lives at L2.
- **Cross-cat method composition.** Banned by 126's actor-
  private discipline. Joint practices (127) provide the
  mutually-public substrate where cross-cat coordination
  belongs.
- **Method depth cap value.** Phase-1 commits 8 as a guess.
  Implementation ticket measures actual depths from a soak.
- **Auto-derived `Goal.state.achieved` predicate.** Today's
  `GoalState.achieved` is fn over world state. Under HTN,
  achievement is "all sub-goals completed." Spec recommends
  auto-derive with override allowed.
- **Per-emit `cooldown_ticks`.** Prevents picker thrash after
  method failure. Probably yes; tracked as an open question
  in the epic.

## Worked example — Whiskers' stealth-cloak arc

Whiskers is an adult cat. Over the run, `Aspirations` has adopted
the `hunting-mastery` chain (domain: Hunting, milestone 2 — hunt
success rate ≥ 0.6; current 0.42). 128's land state: the
infrastructure + `aspiration_milestone_wrapper.hunting` is Live;
`acquire_stealth_via_self_craft` and `_commission` are
PendingSubstrate.

**Tick 1.610M — picker emits**

Whiskers has no `HeldGoalStack` frame from `hunting-mastery`
(check 1). The milestone-2 `emits` walk:

```rust
&[
    Emit { label: "hunt_high_value_prey", priority: Primary,
           applicable_when: high_value_prey_believed_in_range, .. },
    Emit { label: "stealth_gear_acquired", priority: Secondary,
           applicable_when: lacks_stealth_gear, .. },
    Emit { label: "stalking_skill_mentored", priority: Secondary,
           applicable_when: mentor_with_stalking_available, .. },
]
```

Row 1: registry lookup `hunt_high_value_prey` →
method `pursue_high_value_hunt` exists, Live, applicable
(belief says prey in range). **Picker emits**
`Intention::Goal { state: { label: "hunt_high_value_prey" }, .. }`.
Enters L2 scoring pool. Softmax picks it (winning tick).

L2 evaluator: registry lookup → push `GoalFrame
{ method: "pursue_high_value_hunt", sub_goal_index: 0,
  source: AspirationEmitted(hunting-mastery), .. }`. Adopt
sub-goal 0 (a Primitive: `stalk_prey` targeting the mouse).
Existing GOAP plans the step chain. Whiskers hunts.

**Tick 1.612M — alternative path triggered**

Suppose Row 1's `applicable_when` returns false this tick
(belief now reports prey scarcity). Picker continues to Row 2:
`stealth_gear_acquired`. Registry lookup: `acquire_stealth_via_self_craft`
is `PendingSubstrate` → returns `None`. Try
`acquire_stealth_via_commission` — also dormant. **Registry
returns None for the goal label.** Picker continues to Row 3
(`stalking_skill_mentored`).

**Once the wearable-slot substrate ticket lands and
`acquire_stealth_via_self_craft` flips to Live**, the picker's
Row 2 catches. The L2 evaluator pushes a Method frame for the
stealth-cloak arc, and Whiskers begins the multi-step
acquisition arc described in the §H worked-example walk-through
(`gather_stealth_materials` → `reach_workshop` → `craft_stealth_cloak`
→ `don_gear`).

**Tick 1.700M — sub-goal advance**

Whiskers completes `find_thornberry_resin` (the third primitive
of the `gather_stealth_materials` method). Existing
`IntentionFulfilled` Feature point in `resolve_goap_plans`:
- Increment top frame's `sub_goal_index` (frame:
  `gather_stealth_materials`). 2 → 3.
- `sub_goal_index (3) == sub_goals.len() (3)`: pop frame.
- Propagate advance to parent frame (`acquire_stealth_via_self_craft`):
  `sub_goal_index` 0 → 1.
- Adopt new leaf: sub-goal 1 of parent (`reach_workshop`,
  primitive). Whiskers heads to the workshop.

Emit `Feature::SubGoalAdvanced` (once for the inner pop, once
for the outer advance).

**Tick 1.750M — failure cascade**

Suppose the workshop is destroyed before Whiskers reaches it.
The leaf `reach_workshop` abandons with `TargetInvalid`. Top
frame is `acquire_stealth_via_self_craft`,
`failure_strategy: Backtrack`. Registry: is
`acquire_stealth_via_commission` applicable now? Recompute
precondition with current world state. Yes (a coordinator is
online). Push new frame; adopt sub-goal 0
(`petition_for_gear`). Emit `Feature::MethodBacktracked`.

If neither method applies: pop the outer frame, propagate
`BecameImpossible` up to the picker layer. Aspiration's emission
goes to silent-quiet (Row 2 method no longer applicable). Next
tick, picker walks `emits` again and tries Row 3.

The aspiration **doesn't drop** — `hunting-mastery` persists.
Whiskers tries a different lever toward milestone 2.

## Worked example — Mating L3 ported

Today's mating chain in `disposition.rs:1873-1919`:

```rust
build_mating_chain() → [MoveTo, Socialize, GroomOther, MateWith]
```

Ports to:

```rust
Method {
    id: "mate_with_goal",
    goal_label: "mating_event_completed",
    applicable_when: ApplicableWhen::Live(mate_with_goal_applicable),
    sub_goals: &[
        SubGoal::Primitive { label: "approach_partner",
                             action: Action::Navigate,
                             target_hint: TargetHint::Partner },
        SubGoal::Primitive { label: "socialize_with_partner",
                             action: Action::Socialize,
                             target_hint: TargetHint::Partner },
        SubGoal::Primitive { label: "groom_partner",
                             action: Action::GroomOther,
                             target_hint: TargetHint::Partner },
        SubGoal::Primitive { label: "complete_mating",
                             action: Action::Mate,
                             target_hint: TargetHint::Partner },
    ],
    failure_strategy: MethodFailure::Abandon,
}
```

Behavior is preserved 1:1. What changes:

- The chain template moves out of `disposition.rs` into
  `populate_method_registry`.
- Mating's L3 commitment becomes inspectable from `just inspect`
  and trace.
- The chain's `Action::Navigate` resolver doesn't change; only
  the harness that sequences it does.

This is the third Tier-1 method's worked example: the chain
hand-coded today is registry-driven tomorrow with zero behavior
change. The substrate earns its keep by making future multi-step
arcs (stealth-cloak, rear-kitten, mourning-vigil) compose
through the same vocabulary that Mating uses.

## Cross-refs

- 128 epic — `docs/open-work/tickets/128-htn-method-composition.md`
  (dashboard for the 25 children).
- 060 — `docs/open-work/tickets/060-ai-substrate-refactor-epic.md`
  (parent epic; Cluster C row points at 128).
- §7.M / §7.7 / §L2.10.4 / §4.7 / §11.5 — master spec
  (`docs/systems/ai-substrate-refactor.md`).
- 126 — `docs/open-work/landed/126-bdi-intention-substrate.md`
  (BDI substrate the leaf layer rests on).
- 127 — `docs/open-work/landed/127-joint-intention-substrate.md`
  (joint-intention substrate the Courtship method mirrors).
- 207 — `docs/open-work/landed/207-influence-map-registry-walk.md`
  (precedent for `populate_*_registry` + no-stub enforcement).
- `docs/systems/strategist-coordinator.md` (colony-level
  strategist; dovetails with 057 + HTN).
- SHOP2 (Nau 2003) + Game AI Pro vol. 1 ch. 12 (Humphreys) —
  `docs/reading-list.md` §C4.
