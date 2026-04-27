---
name: rank-sim-idea
description: Triage a proposed Clowder simulation idea with the V×F×R×C×H rubric before writing code. Use whenever the user proposes adding a new system, feature, mechanic, creature, item, or behavior to the sim. Produces a score, bucket, and recommendation anchored against the shadowfox calibration — prevents narratively-cool, ops-expensive ideas from slipping in unpriced. Trigger phrases - "add X to the sim", "what if cats could Y", "new system for Z", "I want a feature that...", "we should have a <creature/item/mechanic>". Do NOT fire on balance tweaks to existing systems, bug fixes, or refactors.
---

# Rank a Sim Idea

Clowder's backlog gets hit constantly with new-feature proposals. Ideas
are not equal-cost (Tynan Sylvester, *Designing Games*). A rubric that
prices only implementation cost lets shadowfox-class ideas — narratively
brilliant, operationally expensive forever — slip in unpriced. This
skill runs the triage *before* code gets written, so the user sees the
true cost while they still own the decision.

## When to fire

**Fire when** the user proposes something new for the simulation:

- "Add a crow species that steals shiny items"
- "What if cats could dig burrows"
- "I want a weather event where…"
- "We should have a [new creature / item / mechanic / behavior]"
- Any `docs/systems/*.md` stub that doesn't yet exist

**Do NOT fire when:**

- The user is tuning an existing system ("bump
  `mentor_warmth_diligence_scale`") — that's balance work, covered by
  `CLAUDE.md` §"Balance Methodology".
- The user is fixing a bug or refactoring.
- The idea is already in `docs/systems/*.md` or `docs/open-work.md` —
  that work is priced by the existing indexes. Redirect them there.
- The idea is about the AI substrate (cluster A1–A4). That has its own
  planning surface in `docs/open-work.md`.

## The rubric

Five axes, each scored 1–5. **Multiplied**, not summed — a low score
on any axis sinks the idea, because one-dimensional wins don't ship.

Implementation cost (**C**) and simulation-health tax (**H**) are
separated deliberately: cheap to build and expensive to live with are
independent failure modes, and conflating them hides the second one
until it's too late. Shadowfoxes are the case study.

### V — Value

How much this moves the needle on the three continuity canaries
(`project-vision.md` §"Continuity canaries": generational, ecological
variety, mythic texture) and §5 sideways-broadening (grooming, play,
courtship, burial, preservation, generational knowledge).

- **5** — Directly lights a currently-zero canary, or unblocks
  multiple §5 axes.
- **4** — Strong §5 alignment; lights a partially-passing canary.
- **3** — Supports the thesis without moving a canary.
- **2** — Orthogonal polish; helps one subsystem but doesn't change
  what the world looks like.
- **1** — Tooling or diagnostic only; no in-world effect.

### F — Fit

Concordance with `project-vision.md`: honest world, no director,
ecology-with-metaphysical-weight, emergent complexity, survival-lock as
a bug.

- **5** — The idea *is* the thesis (e.g. The Calling, generational
  knowledge). A DF-style beer-cats-puke-depression chain reaction.
- **4** — Clear thesis fit, no tension.
- **3** — Compatible but could be misbuilt into a director-ish system.
- **2** — In tension with the thesis if not tuned carefully.
- **1** — Fights the thesis (would need to be reframed to ship).

### R — Risk *(higher = safer)*

Probability it works as predicted without regressing canaries on the
first implementation.

- **5** — Isolated; no regression surface; observable outcome.
- **4** — Extends an existing subsystem; regression scope bounded.
- **3** — Touches scoring or GOAP; requires A/B verification per
  `CLAUDE.md` §"Balance Methodology".
- **2** — Architectural; could produce flipper or second-order
  effects.
- **1** — Hypothesis-level; unclear whether the desired behavior is
  reachable with the current substrate.

### C — Implementation Cost *(higher = cheaper)*

One-time engineering effort, inverted.

- **5** — ≲300 LOC, one file, no new messages/components.
- **4** — 300–700 LOC, extends existing systems.
- **3** — 700–1.2k LOC, new ECS subsystem but no GOAP touch.
- **2** — 1.2k+ LOC, GOAP or coordination rework.
- **1** — Multi-cluster; gated on A1 IAUS refactor or requires new
  architecture above GOAP.

### H — Simulation-Health Tax *(higher = lower tax)*

**The ongoing cost of living with the feature.** Tuning cycles,
canaries it forces us to maintain, interaction surface with existing
scoring, destabilization of unrelated canaries. The shadowfox axis.

- **5** — Zero ongoing tax. No new canary, no scoring interaction, no
  tuning slot in balance soaks.
- **4** — Local tax. One or two constants to keep tuned; no new
  canary.
- **3** — Moderate. A new measured metric but not a hard gate;
  touches one existing balance axis.
- **2** — High. Forces a new canary or regularly destabilizes an
  existing one; shows up in every balance diff.
- **1** — Shadowfox-class. Dedicated canary + defense pipeline +
  permanent presence in every tuning session.

## Shadowfox calibration anchor

Every score is read *relative to shadowfoxes*, which shipped and are
expensive forever. Scoring shadowfoxes retrospectively:

| Axis | Score | Reason |
|------|-------|--------|
| V | 5 | Mythic-texture canary pillar; fog-bound corruption-born predator is the thesis in one creature. |
| F | 5 | Ecology-with-metaphysical-weight in one line. |
| R | 2 | Fear/ward/flee interaction with scoring destabilized mortality; required a bespoke canary to even *detect* misbehavior. |
| C | 3 | Built, but significant — the ambush + corruption-spawn pipeline contributed meaningfully to `wildlife.rs` (2.5kloc). |
| H | 1 | Dedicated canary (`ShadowFoxAmbush ≤ 5` in `CLAUDE.md`), defense pipeline stub (`docs/systems/shadowfox_wards.md`), perpetual tuning slot. Maximum ongoing tax. |

**Score: 5 × 5 × 2 × 3 × 1 = 150** → "expensive but valuable" bucket.
Matches lived experience: shipped, narratively load-bearing,
permanently expensive to keep tuned.

**Any proposal scoring V=5/F=5 but H=1/R=2 is asking to become the next
shadowfox.** Surface that explicitly in the output.

## How to estimate H at proposal time

H is the hardest axis to score and the whole reason this skill exists.
Triangulate three sources — state which drove the score in the output.

### (a) Structural tells

The proposal is at high risk of H=1–2 if it exhibits **two or more**
of:

- Introduces a new entity type that *reads and writes* existing
  scoring axes (fear, wards, flee, hunger, bond) rather than living in
  its own isolated axis.
- Probabilistic rare-event existence with cascade consequences
  ("spawns occasionally, but when it does, the colony reshapes").
- Creates a feedback loop coupling two or more existing subsystems
  (shadowfox: corruption → spawn → ward-building → corruption pushback
  → spawn rate).
- Forces a bespoke canary because the failure mode isn't legible to
  existing canaries (shadowfox's `ShadowFoxAmbush ≤ 5` exists because
  generic `Starvation = 0` wouldn't have caught it).
- Tunables cross-coupled with other systems' tunables — its constants
  will show up as variables in future balance threads for unrelated
  work.

Conversely, an isolated new axis with its own canary-free scoring
contribution is H=4–5 (e.g. a new leisure action that scores on an
existing mood axis and doesn't feed back).

### (b) Memory of priors

Query `mcp-memory-service` with `retrieve_memory`, searching for tag
`clowder` + `ongoing-tax` plus the proposal's structural features.
Each shipped system should leave a `pattern` memory so the next
triage is sharper:

```
name: ongoing-tax-<system>
tags: [clowder, ongoing-tax, pattern]
type: pattern
content:
  System: <name>
  Predicted H at proposal: <n>
  Observed H after N sim-years of tuning: <n>
  Structural tells that fired: [list]
  Tuning iterations consumed: <n> (count from docs/balance/)
  Bespoke canary required: yes/no
```

Cold start today — the rubric gets sharper with each system priced.
After running this skill, if the user proceeds with the idea, commit
the memory as part of the idea's landing commit so the prior is
captured.

### (c) Retrospective grep — concrete evidence beats priors

Before committing H, grep `docs/balance/*.md` for the nearest-relative
shipped system and count iteration threads:

```
Grep docs/balance/*.md for terms matching the proposal's analogue
(e.g. "fox", "corruption", "ward", "scent", "detection").
Count distinct iteration headers ("Iteration 1", "Iteration 2", ...).
```

A proposal analogous to a system that burned 3+ balance iterations is
probably H≤2. Name the specific balance thread(s) in the output.

## Score buckets

- **>1000** — cheap win. Pick up next session.
- **300–1000** — worthwhile; plan carefully.
- **80–300** — expensive but valuable; earn the slot. Requires
  hypothesis + prediction per `CLAUDE.md` §"Balance Methodology".
- **<80** — defer unless a dependency forces the hand, or reframe to
  raise a low axis before reconsidering.

## Output format

Produce exactly this block — the user will paste it into
`docs/open-work.md` if the idea lands:

```
## Rank: <idea name>

| V | F | R | C | H | Score | Bucket |
|---|---|---|---|---|-------|--------|
| n | n | n | n | n | NNN   | bucket |

**Justifications**
- V: ...
- F: ...
- R: ...
- C: ...
- H: ... (H-source: structural tells / memory / balance-grep; cite specifics)

**Dependencies:** gated on A1 IAUS refactor? other stubs?

**Canary hit:** which currently-zero continuity canary this lights
(generational, ecological variety, mythic texture), or "none".

**Shadowfox comparison:** one sentence — would this become the next
permanent-tax item? Point to the structural tells that fired or didn't.

**Recommendation:** pick up / plan carefully / earn the slot / defer.
If "earn the slot", name the hypothesis + prediction the user will
need to write per balance methodology.
```

## What this skill does NOT do

- **Score ideas already stubbed in `docs/systems/` or `docs/open-work.md`.**
  Those are priced by the existing backlog indexes. Redirect the user
  to those files. If the user explicitly requests a retroactive score
  (audit), that's fine — note it as retrospective in the output.
- **Score refactors of the AI substrate (A1–A4).** Those have their
  own planning surface (`docs/systems/ai-substrate-refactor.md`,
  `docs/open-work.md` §§5).
- **Make the decision.** Produce the triage artifact; the user
  decides whether to proceed, reframe, or defer.

## References for the skill (kept here so the rubric doesn't rot)

- `docs/systems/project-vision.md` — §5 sideways-broadening, the
  continuity canaries, the ecological-magical-realist thesis. V and
  F axes quote from here.
- `CLAUDE.md` §"Balance Methodology" — the hypothesis / prediction /
  observation / concordance format required for ideas in the
  80–300 bucket.
- `CLAUDE.md` §"Long-horizon coordination" — where to file the
  output if the idea lands.
- `docs/wiki/systems.md` — current stub inventory; cross-check before
  scoring to confirm the idea isn't already priced.
- `docs/open-work.md` — in-flight thread index; same cross-check.
