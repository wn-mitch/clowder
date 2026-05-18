---
id: 415
title: PlayFired gate orthogonal to Explore — play metric stuck at 14 across 066 four-fix
status: ready
cluster: ai-substrate
initiative: []
added: 2026-05-18
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [healthy-colony.md]
landed-at: null
landed-on: null
---

## Why

Surfaced as the "play didn't recover" residual on ticket 066. The 2026-04-24
four-fix landing (passive-exploration stamping, decay 4× slower, decoupled
perception radius, `still_goal` wired to familiarity) addressed Explore
dominance over targeted leisure, but on the `post-127-joint-intention`
canonical baseline (commit `4bcae2de`, seed 42) the footer continued to show
`play: 14` (Phase 0 had 348; Phase 1: 109). The four-fix changed Explore's
scoring landscape; it did NOT recover the play continuity-canary contribution.

Either the dispersion loop is not fully closed in the way 066's framing
assumed, or — and this is the leading hypothesis per 066's 2026-05-11 log —
`PlayFired` (emitted from `src/systems/personality_events.rs:320`, not via a
DSE) is gated by something orthogonal to Explore's L2 scoring. The PlayFired
emit may depend on cat-state preconditions, target-availability, or
constituent-action sequencing that the four-fix didn't touch.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/components/markers.rs` | what gates PlayFired emit — cat must be playing, target must exist, ... | `[suspect]` |
| L2 DSE scores | `src/ai/dses/play_target.rs` (if it exists) or scoring composition for play | does play even have a DSE post-substrate-refactor? | `[suspect]` |
| L3 softmax | `src/ai/scoring.rs` | play's relative weight against Explore + Wander floors | `[suspect]` |
| Action→Disposition mapping | `src/components/disposition.rs::from_action` | which Disposition maps to PlayFired? | `[suspect]` |
| Plan template | `src/ai/planner/...` | does any plan template terminate in PlayFired? | `[suspect]` |
| Completion proxy | `src/components/commitment.rs` | how does play "complete" — what marks the canary fire? | `[suspect]` |
| Resolver | `src/steps/...` | resolver path that emits PlayFired | `[suspect]` |
| Emit site | `src/systems/personality_events.rs:320` | direct emit (not DSE-driven) — what guards the call site? | `[verified-correct]` (per 066 log) |

Every row except the direct emit-site reference is `[suspect]` — the
investigation needs to layer-walk before listing candidates. The emit happens
in `personality_events.rs:320`, not via a DSE, which means PlayFired is not
gated by the same scoring/softmax/commitment pipeline as Explore. That's the
"orthogonal" claim from 066's log.

## Fix candidates

**Parameter-level options:**
- R1 — Lower whatever threshold PlayFired's emit-site checks (if it reads
  cat-state needs / mood / energy and gates on a numeric threshold).
- R2 — Widen the personality / arc preconditions that decide whether a cat
  is eligible to fire PlayFired.

**Structural options** (at least one MUST be drafted):
- R3 (**rebind**) — route PlayFired through the standard DSE/Disposition
  pipeline rather than a direct emit. Move the call site from
  `personality_events.rs` into a normal resolver, gated by a Play DSE that
  competes in L2.
- R4 (**extend**) — keep the direct emit but branch its preconditions on
  cat life-stage or arc so juveniles and elders get different play
  cadences than adults.
- R5 (**retire**) — if play turns out to fire elsewhere via a different
  proxy (e.g., JointIntention practice from ticket 127 / 276), retire
  `PlayFired` and rely on the JointIntention canary path. (Cross-reference
  ticket 276 — "Play-bout practice on JointIntention substrate (play
  continuity canary)".)

## Recommended direction

Decide after layer-walk. The 276 cross-reference is load-bearing: if
JointIntention-based play is the canonical canary path, R5 (retire) may
already be partially in motion and this ticket might collapse into 276.

## Out of scope

- Restoring play to Phase-0 levels (348 events). The realistic target is the
  continuity-canary threshold (≥1 named event per sim year), not the
  pre-refactor magnitude. If the fix lands and play sits at 14 but the
  canary holds via JointIntention, the canary contract is satisfied.
- Re-tuning Explore. 066 closed; Explore's four-fix shipped.

## Verification

Hard-gate: `continuity_tallies.play >= 1` on the canonical seed-42 deep-soak
post-fix. Compare against `post-127-joint-intention` baseline footer.
Focal-cat replay if PlayFired fires for one cat but not others.

## Log

- 2026-05-18: Opened as a follow-on of 066. 066 sub-2's four-fix shipped
  2026-04-24 and verifiably changed Explore scoring; play residual stayed
  at 14 across `post-127-joint-intention` (commit 4bcae2de, seed 42).
  Working hypothesis: PlayFired emit at personality_events.rs:320 is gated
  by preconditions orthogonal to Explore scoring. Cross-reference 276 in
  case JointIntention-based play is the canonical canary path now.
