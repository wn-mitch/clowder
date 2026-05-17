---
id: 399
title: L2 ParentingActivity — express non-primary-caretaker parent roles per §7.M.2 (design)
status: ready
cluster: social-coordination
initiative: [smarter-cats, htn-method-composition]
added: 2026-05-17
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, htn-methods.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

398's L1-only landing scopes `RaiseOffspringAspiration` adoption to
**mothers**: the chain's `AspirationLift` (+0.2 on Caretake) wins
softmax for compassionate primary caretakers, but applying it to
fathers caused a 41× `HandoffItem`-no-recipient plan-failure spike
(both parents over-attempting Caretake while the mother already
handled it). Mother-only is a structurally honest L1 minimum, but it
leaves the spec's other half unexpressed: §7.M.2 explicitly names
"*the partner's aspiration shifts toward a provisioner role via
personality-weighted pick — diligent → Hunt-biased; compassionate →
Caretake-biased*." A father with no Kinship-adjacent substrate signal
is dispositionally silent during his kittens' dependency window —
which is *also* wrong per spec.

**This isn't a bug, it's an expression-design question.** What does
each personality archetype's parenthood *look like* in substrate?
Concrete character anchors (user-provided, 2026-05-17):

- **"Hard working union man"** — high diligence + high loyalty + maybe
  low warmth/compassion. Doesn't cuddle the kittens but provides
  reliably via Hunt → shared Stores. Pride in providing. "I bring
  home the bacon" is the expression of paternal care; absence from
  the nest is not absence of investment. Substrate should bias Hunt
  with target-preference toward partner's food-need / household
  Stores, not Caretake.
- **"Left to go get cigarettes"** — low loyalty + low compassion + low
  tradition + high independence. Physically drifts away, doesn't
  return, no investment in offspring. Substrate should *withhold*
  parenting-role bias entirely; the kittens are biographical
  background, not behavioral influence. Welfare canaries should
  surface this as a legitimate observable arc (not a bug).
- **The standard mother** — high compassion + high warmth. Primary
  caretaker; 398's L1 mother-only path already expresses this.
- **The protective parent** (any sex) — high boldness + high temper
  + moderate compassion. Patrol-bias scoped to dependent's tiles,
  Fight-readiness elevated for threats near nest. Caretake-secondary.
- **The low-compassion mother** (corner case) — has the Parent
  marker (gave birth) but her personality says she's not a natural
  primary caretaker. Currently 398's mother-only adoption still gives
  her Kinship → Caretake lift. Probably wrong: she should express
  the role her personality fits (provisioner, protector, even
  partial absence), not be forced into the compassionate-mother
  template.
- **The auntie / uncle / kin-adjacent alloparent** — not a parent
  (no Parent marker) but high-compassion + high-loyalty cat in the
  colony, helping with the kittens. Currently silent in substrate;
  the spec's L2 ParentingActivity framing leaves space for this but
  doesn't enumerate it.

§7.M.2's spec text is the design seed; the implementation shape is
open. The question this ticket exists to resolve: **how do we express
each archetype in substrate without re-introducing the override layer
398 explicitly retired?**

## Spec anchors

- `docs/systems/ai-substrate-refactor.md` §7.M.2 (post-consequence
  cascade — *"the partner's aspiration shifts toward a provisioner
  role via personality-weighted pick — diligent → Hunt-biased;
  compassionate → Caretake-biased"*).
- §7.M.1 Layer 2 `PairingActivity` — the canonical analog. Spec text:
  *"a multi-season, ambient activity biasing existing DSE weights
  without prescribing actions ... Playful cat initiates play-bouts ...
  Diligent cat provisions partner ... Bold cat defends partner
  territory ... Affectionate cat allogrooms constantly. No new
  mechanics are needed for any of these — they fall out of Layer 2 as
  a weight modifier across the existing DSE set."* This is the
  precedent shape for ParentingActivity.
- §7.4 persistence-tier table: `Mating L2 PairingActivity` = Medium
  tier; `Caretaking` = Medium. ParentingActivity should mirror.
- CLAUDE.md design pillar #3 (richer perception, better strategy):
  "compose personality / phobias / ambient context at the modifier
  layer, never inside the underlying perception scalar." The bias
  belongs on the modifier layer reading personality × ParentingActivity,
  not folded into Caretake's COMPASSION_INPUT axis.
- Existing analog in code: `JointIntention` substrate
  (`src/components/joint_intention.rs`, ticket 127 — "subsumes §7.M L2
  PairingActivity per the L2 framing"). Mirrors the per-cat-Component
  "I am currently engaged in this practice with these other cats"
  shape that ParentingActivity probably wants.

## Design open questions

These are the questions that need answering BEFORE implementation
candidates make sense. Each is a real branch in the design space, not
a parameter tuning.

1. **One aspiration with personality-weighted L2, or multiple
   aspirations with role-typed adoption?**
   - (a) Single `RaiseOffspringAspiration` adopted by ALL parents;
     `ParentingActivity` L2 expresses the role variance
     (diligent-father → Hunt-bias, compassionate-mother → Caretake-
     bias).
   - (b) Multiple chains: `RaiseOffspringAspiration` (caretaker
     role, mother-adopted) + `ProvideForOffspringAspiration`
     (provisioner role, partner-adopted-when-diligent) + ???. Each
     chain's `AspirationLift` directly lifts the role's primary
     action.
   - Tradeoff: (a) is closer to spec's "one L1 with L2 bias" framing;
     (b) leverages existing `AspirationLift` machinery without new
     L2 substrate but multiplies chain count.
2. **Where does `ParentingActivity` live in substrate?**
   - Per-cat Component (analog: `JointIntention`)?
   - Field on `Aspirations.active[Kinship].state`?
   - Inferred per-tick from `Personality` × `Parent` marker (no
     stored state)?
3. **How does personality choose role within "I am a parent"?**
   - Hard-threshold: `compassion > 0.5` → caretaker; else
     provisioner-or-other (binary cleavage, but loses the gradient).
   - Soft-weight: each role gets a score from personality; highest
     wins (continuous, but adds "which role am I in" softmax to L2).
   - Hybrid: compassion + loyalty + diligence axes compose into a
     `parental_role: f32` scalar that biases modifier weights.
4. **What's the "left to go get cigarettes" expression?**
   - Active migration away (Wander toward map edge, Patrol far from
     nest)?
   - Silent dispositional drift (no parenting bias; behaves as a
     non-parent would)?
   - Both, with a `paternal_abandonment_threshold` that promotes
     drift → migration over time?
   - This is welfare-canary-adjacent: the colony should be able to
     observe it as a narrative arc, not a substrate bug.
5. **Where does alloparenting fit?**
   - Separate aspiration (`AlloparentingAspiration` adopted by high-
     compassion non-parents when a litter exists in the colony)?
   - L2 modifier that lifts Caretake for high-compassion cats who
     witness `IsParentOfHungryKitten` markers on other cats?
   - Defer to a later ticket (this ticket scopes to parents only)?
6. **How does the role drop?**
   - Same lifecycle as `RaiseOffspringAspiration` (Parent marker
     clears)?
   - Independent (provisioner role persists past kitten maturity if
     the partner is still alive — provider habits stick)?
   - Event-driven (partner death drops provisioner role; grief
     cascade per §7.7.b)?

## Scope (after design questions resolve)

Choose one shape from §"Design open questions" and implement:

- The substrate Component / Aspiration / Marker that holds role state.
- The personality → role mapping (pure-function or system).
- The modifier(s) in `src/ai/modifier.rs` that bias DSE weights by
  role × personality (mirroring `JointIntention`'s bias pattern).
- Lifecycle adoption + drop (where appropriate, alongside or in place
  of `adopt_kinship_aspiration`).
- L2 trace records so the role is visible in focal-cat traces.

## Out of scope

- 398's L1 layer (already landed; this ticket builds on it).
- Full §L2.10.6 unified softmax + per-tier persistence-bonus (398
  Phase 1c/1d follow-on; orthogonal to this ticket's role-expression
  question).
- Grief cascade (§7.7.b) — bereavement-driven role shifts. Open as
  separate ticket once §7.7.b ships.
- Visitor / outsider parenting arcs (a wandering tom fathers a
  litter then leaves — different lifecycle than colony-member
  parenthood).

## Current state

398's mother-only `adopt_kinship_aspiration` has fathers as
dispositionally silent during kitten-dependency windows. The
"left-to-go-get-cigarettes" expression is currently the *default*
for fathers (no signal, no bias), but only by accident — there's no
substrate naming this as a chosen role vs. a substrate gap.

## Approach

Design ticket — discuss-then-implement. Bring §"Design open questions"
to user, resolve the design space, then open one or more
implementation tickets blocked-by this one (and/or fold the resolved
shape into this ticket's `## Scope`).

## Verification

Once implementation lands:

- Focal-trace a hard-working-union-man father (high diligence + high
  loyalty + low compassion + has dependent kittens). Verify Hunt
  target preference biases toward partner's food-need / household
  Stores during the dependency window.
- Focal-trace a left-to-go-get-cigarettes father (low loyalty + low
  compassion + high independence). Verify NO Kinship bias applied;
  cat drifts naturally.
- Focal-trace the standard mother (398's existing path). Verify
  Caretake bias holds (no regression).
- `plan_failure_canary` does NOT regress on
  `HandoffItem: no recipient` (the 41× spike 398's mother-only fix
  resolved must not return).
- Pebblekit-67-equivalent kittens still survive (398's survival gate
  intact under the new substrate).

## Log

- 2026-05-17: opened as a 398 follow-on after the mother-only L1
  landing surfaced the spec's other half (fathers as provisioners,
  alloparenting, low-compassion mothers). User-provided character
  anchors ("hard working union man," "left to go get cigarettes")
  ground the design space. Framing: this is an expression-design
  question, not a bug.
