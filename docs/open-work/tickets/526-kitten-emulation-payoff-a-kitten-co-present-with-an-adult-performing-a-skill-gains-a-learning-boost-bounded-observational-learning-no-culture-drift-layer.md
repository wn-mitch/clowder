---
id: 526
title: Kitten emulation payoff — a kitten co-present with an adult performing a skill gains a learning boost (bounded observational learning, no culture-drift layer)
status: ready
cluster: life-cycle
orchestration: substrate-sensitive
initiative: [generational-continuity, smarter-cats]
added: 2026-07-09
parked: null
blocked-by: []
supersedes: []
related-systems: [collective-memory]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Generational knowledge today flows only through the deliberate mentoring path —
an elder must *intend* to teach. But kittens learn a great deal simply by
watching adults do things, with no one intending to teach. Emulation payoff wires
that: a kitten co-present with an adult performing a skilled action gains a small
learning boost toward that skill. It makes skill acquisition robust to mentor-
absence (the same robustness 521 gives play), rewards the natural behavior of
kittens hanging around working adults, and is the emergent, no-director learning
the thesis calls for — a kitten who shadows the best hunter grows up a better
hunter, and nobody had to run a lesson.

## Scope

- An observation trigger: while a kitten is co-present with (and attending to) an
  adult executing a skilled action, the kitten accrues a small skill-xp increment
  toward that skill, through the **existing** skill substrate.
- Attention gating so it's emulation, not osmosis: the kitten must be idle/near/
  oriented (reuse whatever proximity + attention primitive mentoring or grooming
  uses), and the boost is a fraction of what a deliberate mentoring bout grants —
  watching is weaker than being taught.
- A modest scoring nudge (optional, low weight) so kittens mildly prefer being
  near skilled adults — the "shadow the hunter" behavior. Keep this small; the
  payoff is the xp, not a new drive.
- Tunables (emulation xp fraction, attention radius, per-skill eligibility) in
  `src/resources/sim_constants.rs`.

## Out of scope — the reframe that made this a cheap win

- **No colony-culture layer, no cultural drift, no copy-error propagation.** The
  ambitious "observational learning + cultural drift" idea this was reframed from
  scored only 160 (earn-the-slot, shadowfox-risk) precisely because of a
  culture→behavior→culture feedback loop and a drift-tracking metric. This ticket
  deliberately keeps ONLY the bounded per-kitten xp side-write and drops the loop.
  The full cultural-drift system remains a deferred research spike (see Log), NOT
  this ticket.
- No new skill-tree or progression system — reuse the existing skill substrate.
- No feedback into any adult's behavior — the adult is unaware it is being
  emulated; the effect is one-way onto the kitten.

## Current state

New. Not started. Provenance: surfaced by the `ideonomy` skill (2026-07-09 tuple
`negation + substitution / cycle / symmetry·intentionality·materiality`) as the
intentionality-substitution of §5 axis #6 — swapping "knowledge transfer is
DELIBERATELY taught" for "acquired by emergent imitation." User elevated it and
reframed toward "kitten emulation paying off," which is what dropped the
shadowfox-risk feedback loop. Re-priced with `rank-sim-idea` under the bounded
framing.

Rank-sim-idea triage (2026-07-09):

| Framing | V | F | R | C | H | Score | Bucket        |
|---------|---|---|---|---|---|-------|---------------|
| Full cultural-drift (original) | 4 | 5 | 2 | 2 | 2 | 160 | earn the slot |
| **Bounded emulation payoff (this ticket)** | 4 | 5 | 4 | 4 | 4 | **1280** | **cheap win** |

Bounded-framing justifications:
- V=4: makes skill acquisition robust to mentor-absence + rewards natural kitten
  behavior; generational + smarter-cats.
- F=5: emergent learning by imitation, no director — pure thesis fit.
- R=4: isolated proximity-triggered xp side-write on an existing substrate; no
  feedback loop, no drift tracking; observable.
- C=4: one observation system + skill-xp hook; ~300–500 LOC, one file.
- H=4: isolated axis, no culture→behavior→culture loop (that was the H=2 tell in
  the original); at most one measured metric, no bespoke canary. (H-source:
  structural tells — the reframe removed the two tells that fired on the original.)

The reframe is the point: same V and F, but dropping the feedback loop and the
drift metric moved R/C/H from 2/2/2 to 4/4/4 — an 8× score jump. This is
rank-sim-idea's "reframe to raise a low axis" working as intended.

## Approach

Model on mentoring's existing observation/attention primitive: when a kitten is
attending to an adult mid-skill-action, write a fractional xp increment to the
kitten's matching skill. No message to the adult, no colony aggregate, no drift.
The optional "shadow the skilled adult" scoring nudge is a small consideration on
an existing kitten idle/follow behavior — keep its weight low so it doesn't
distort kitten movement or preempt play (521) / begging. All tunables in
`src/resources/sim_constants.rs`. Pairs with 525 (lost lore): emulation is a
gain path on the knowledge known-set, lost lore is the drain path.

## Verification

- Deterministic scenario: place a kitten near a skilled adult repeatedly
  performing a skill on a fixed seed; assert the kitten's skill xp rises faster
  than a control kitten kept away, and slower than a deliberately-mentored kitten
  (watching < being taught).
- `just verdict` on a full soak: generational-continuity canary unaffected or
  improved; play/mentoring canaries unaffected; hard survival gates unchanged; no
  new balance canary required — if one appears, the culture-drift loop leaked back
  in (re-triage against the reframe boundary above).
- Focal-cat trace: the kitten's skill increment appears on co-presence; the adult
  shows no emulation-induced score change (one-way effect confirmed).

## Log
- 2026-07-09: opened from the `ideonomy` (negation/cycle) → `rank-sim-idea`
  dark-mirror pass. Original full cultural-drift framing scored 160 (shadowfox-
  risk). User reframed to "kitten emulation paying off"; re-priced at 1280 (cheap
  win) after dropping the culture→behavior→culture feedback loop and drift metric.
  **The full cultural-drift / colony-culture system remains a deferred research
  spike** — do NOT expand this ticket into it; open a separate spike if that
  ambition is revived, and prove substrate reachability (the original R=2) first.
