# A1 IAUS core — kickoff

Session-startup artifact for the AI substrate refactor's A1 trunk. Pairs
with [`ai-substrate-refactor.md`](ai-substrate-refactor.md) (the spec)
and [`docs/open-work.md`](../open-work.md) #14 (live refactor status) +
#5 cluster A (A1 "TOP PRIORITY" framing).

A1 is the gravitational center of the refactor: Considerations + response
curves + multiplicative composition. §1–§3.4 of the spec is paper until
this lands; §11 instrumentation, §13.1 constants cleanup, and §L2.10
unified surface are all gated behind it.

---

## Phase structure

A1 is not a single commit — the trunk is serial design work, the per-DSE
ports that follow are where fan-out pays.

| Phase | Scope | Parallelizable? |
|-------|-------|-----------------|
| **A1.0 — substrate trunk** | §1 trait + §2 curves + §3.1–§3.4 composition + §3.6 granularity. New module, nothing existing touched. | No. One session. |
| **A1.1 — per-DSE ports** | 21 cat + 9 fox DSEs. Each is one commit with seed-42 canary check. | Authoring yes; landing serial. |
| **A1.2 — §11 focal-cat replay** | Sidecar JSONL emitter with per-layer records. New file territory. | Yes, parallel to A1.1. |
| **A1.3 — Hunger Logistic migration** | First measured curve shift. Single axis. Requires A1.0+A1.1. | No. Balance attribution needs isolation. |
| **A1.4 — §13.1 retired constants cleanup** | Delete `incapacitated_*`, emergency bonuses. | Lands after A1.1. |
| **A1.5 — §L2.10 unified evaluation surface** | Single `evaluate()` entry, DSE catalog. | Bundle with A1.1 tail end. |

**Why the trunk can't fan out:** Consideration trait + Curve enum +
composition functions are a single coherent API. Two agents designing
it in parallel produce incompatible APIs; you'd pick one and throw the
other away. Fan-out becomes productive at A1.1, where each DSE port is
a mechanical translation against a published, stable API.

**Why A1.1 lands serially even when fan-out-authored:** CLAUDE.md's
balance methodology requires per-commit seed-42 canary attribution.
Simultaneous merges collapse that. Fan out the authoring; queue the
landing.

---

## A1.0 kickoff prompt

Paste into a new session. Self-contained — briefs cold.

```
You're picking up the AI substrate refactor's A1 trunk — the IAUS core
that's been blocking the rest of the refactor. Read these first, in order:

  1. docs/systems/ai-substrate-refactor.md §1, §2, §3.1–§3.4, §3.6
  2. docs/open-work.md #14 (refactor live status) and #5 cluster-A
     entry A1 (the "TOP PRIORITY" framing)
  3. CLAUDE.md "AI Substrate Refactor (in-flight)" + "GOAP Step Resolver
     Contract" sections
  4. src/ai/scoring.rs (2.8k lines; the thing you are NOT touching this
     commit but must understand)
  5. src/components/physical.rs Needs::level_suppression — the Maslow
     pre-gate that must be preserved bit-for-bit
  6. src/resources/sim_constants.rs ScoringConstants (~57 fields; stays
     put this commit, reshape happens in A1.1)

SCOPE FOR THIS COMMIT — substrate trunk only:

  - §1.1 Consideration trait (input → curve → [0,1] score)
  - §1.2 three flavors: scalar, positional, consideration-of-considerations
  - §2.1 Curve primitive enum: Linear, Polynomial, Logistic, Logit,
    Piecewise, with shape parameters
  - §2.2 function-evaluated backing (NO LUT compilation yet — the spec
    explicitly says "start function-evaluated")
  - §3.1 Composition modes: CompensatedProduct, WeightedSum, Max
  - §3.2 Compensation factor — Mark's (1 - 1/n) * (1 - score) correction
  - §3.3 Weight-rationalization machinery per §3.3.1 (RtM / RtEO /
    absolute-anchor peer groups); the per-DSE assignments are already
    enumerated in the spec — you consume that table, don't redesign it
  - §3.4 Maslow pre-gate: wraps the composed score, preserves
    level_suppression semantics exactly
  - §3.6 granularity: [0,1] pain-scale discipline in the trait surface

Land it as a new module — `src/ai/iaus/` feels right (or
`src/ai/considerations/` if you prefer; pick one and commit).

EXPLICIT NON-GOALS (each is its own later phase; don't scope-creep):

  - No per-DSE ports. scoring.rs action blocks stay exactly as they
    are this commit. The new substrate ships dormant.
  - No curve migrations (Hunger Logistic etc.) — that's A1.3.
  - No §11 instrumentation — that's A1.2, parallelizable with per-DSE
    ports.
  - No §13.1 retired-constants cleanup — gated on per-DSE port landing.
  - No §L2.10 unified evaluate() surface — bundles with A1.1 tail.
  - No ScoringConstants reshape — stays flat this commit; per-DSE
    ports in A1.1 will migrate fields to curve-shape params.

DELIVERABLES:

  - New module with trait + curves + composition + Maslow pre-gate
  - Unit tests: each curve primitive's output at >=5 sampled inputs
    matches analytical values to f32 precision
  - Unit tests: CompensatedProduct with 2, 3, 4 axes matches the
    (1 - (1 - score) * (1 - 1/n)) formula Mark specifies; with one
    axis ~= 0 the product ~= 0
  - Unit tests: Maslow pre-gate on the trunk's composition produces
    identical output to the current level_suppression cascade on
    matching inputs
  - Rustdoc on the trait + composition functions citing the spec
    sections
  - docs/open-work.md entry for Phase A1.0 landed with commit hash

VERIFICATION:

  - just check (cargo check + clippy clean)
  - just test (all existing + new unit tests pass)
  - just soak 42 clears all survival canaries — trivial, since
    scoring.rs is untouched; this is the sanity check that the new
    module is genuinely dormant
  - Record the canary footer in the landing entry for before/after
    comparability with A1.1 commits

CONVENTIONS:

  - Bevy 0.18 (Messages not Events, etc. — see CLAUDE.md ECS Rules)
  - No magic numbers — tunables belong in SimConstants even if unused
    by scoring.rs this commit
  - Additive, not destructive: nothing existing changes
  - Conventional commits: `feat:` no scope
  - Don't wrap public API in early-return defensive checks; trust
    internal callers per CLAUDE.md "Don't add error handling for
    scenarios that can't happen"

OUT OF SCOPE TO RAISE BEFORE IMPLEMENTING:

  - Whether to collapse WeightedSum into CompensatedProduct (spec
    commits all three modes; don't redesign)
  - Whether to LUT-compile curves now (§2.2 says "start
    function-evaluated"; don't preempt)
  - Anything touching scoring.rs this commit

If you find the spec ambiguous on a curve-shape parameter or a
composition edge case, STOP and ask. Don't guess mid-port; the
spec is load-bearing and alternatives will need to be redone.
```

---

## After A1.0 lands — fan-out plan for A1.1+

Once the trunk is in place with a stable published API:

- **Fan out per-DSE ports (A1.1).** Assign groups of DSEs across agents
  — e.g. Maslow-layer-1 actions (Eat, Sleep, Drink) to one session,
  social (Socialize, Mate, Mentor, Caretake, Groom) to another,
  magic/herb (Herbcraft, PracticeMagic, Ward, Cleanse, Harvest,
  Commune) to a third, fox dispositions to a fourth. Each agent writes
  the consideration bundle + unit tests verifying behavior preservation
  at sampled inputs against the pre-refactor `scoring.rs` output.
- **Land serially through one driver session.** That session runs
  `just soak 42` between each commit and records canary footers in the
  landing entry. Balance methodology requires per-commit attribution;
  simultaneous merges collapse that.
- **A1.2 instrumentation runs fully parallel** on a separate branch —
  it only adds emission sites, doesn't change math.
- **Spec gaps come back as a spec-edit commit before A1.1 starts.**
  A1.0 may surface enumeration gaps the trunk reveals. Don't let A1.1
  begin against an under-specified substrate.

---

## Cross-refs

- Spec: [`ai-substrate-refactor.md`](ai-substrate-refactor.md) §1, §2,
  §3.1–§3.4, §3.6, §11 (A1.2), §L2.10 (A1.5)
- Refactor tracker: [`../open-work.md`](../open-work.md) #14
- Priority framing: [`../open-work.md`](../open-work.md) #5 cluster-A
  entry A1
- Follow-on debts: [`../open-work.md`](../open-work.md) #13 (A1.4
  gating on A1.1 landing)
- Landed predecessors: Phase 4a (softmax + §3.5 modifiers), Phase 4b
  (§4 marker foundation + §6.3 TargetTakingDse), Phase 4c (§6.5 slate
  closed), Phase 5a (`StepOutcome<W>` contract).
