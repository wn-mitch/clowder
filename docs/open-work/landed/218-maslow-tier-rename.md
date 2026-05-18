---
id: 218
title: Rename Maslow `Level N` → `Tier N` to disambiguate from substrate `L1/L2/L3`
status: done
cluster: ai-substrate
initiative: []
added: 2026-05-07
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: e8dd505d
landed-on: 2026-05-07
---

## Why

Two `L#` numbering systems collide in the codebase:

- **AI substrate** (`L1 markers / L2 DSE scores / L3 softmax`) — defined
  in `docs/systems/ai-substrate-refactor.md:551-577`. ~1,055 mentions
  plus ~139 `§L2.10.x` anchor IDs cross-referenced from many tickets.
  Renaming the substrate vocabulary is prohibitive.
- **Maslow needs** (`Level 1..5`) — ~416 mentions, almost entirely code
  comments and a few public symbols. Renaming this is local and cheap.

Sub-agents and fresh sessions routinely conflate the two. Ticket 194
§F9 traces a 189-cluster diagnostic delay back to two Explore agents
that inherited the wrong premise about which `L#` was meant. The user
picked Option B from a three-way choice (A: glossary-only; B: rename
Maslow; C: rename substrate — rejected on anchor cost) so the collision
is structurally eliminated, not papered over with a note.

## Changes

**Symbol renames** (compiler-checked across all callers):

- `Needs::level_suppression(&self, level: u8)` →
  `tier_suppression(&self, tier: u8)` — `src/components/physical.rs`.
- Parallel impls on `FoxNeeds` (`src/components/fox_personality.rs`),
  hawk needs (`src/ai/hawk_scoring.rs`), snake needs
  (`src/ai/snake_scoring.rs`) renamed in lockstep.
- `DispositionKind::maslow_level()` → `maslow_tier()` —
  `src/components/disposition.rs`. Sibling impls on the three creature
  planners (`src/ai/{fox,hawk,snake}_planner/mod.rs`) renamed.
- `UrgentNeed::maslow_level: u8` → `maslow_tier: u8` —
  `src/components/goap_plan.rs`, plus all struct-literal sites in
  `src/systems/goap.rs` (urgency-emission and preempt-gate paths).

**Comment / banner rewrites** in: `src/components/physical.rs`,
`src/systems/needs.rs` (Tier 1..5 banners), `src/components/fox_personality.rs`,
`src/ai/{fox,snake}_scoring.rs`, `src/ai/snake_planner/mod.rs`,
`src/systems/fox_goap.rs`, `src/ai/scoring.rs`, `src/ai/eval.rs`,
`src/ai/dses/{flee,socialize,mentor,cook}.rs`,
`src/components/disposition.rs` rustdoc, `src/systems/colony_score.rs`,
`src/resources/sim_constants.rs`.

**Docs**:

- `CLAUDE.md` Architecture/Maslow bullet — added a glossary clause
  distinguishing Maslow tiers from substrate `L1/L2/L3`.
- `docs/systems/ai-substrate-refactor.md` — Maslow-context "Level N"
  references at the §3.4 pre-gate definition, the Fox-disposition
  H4/H5 tables, and the warmth-split bullet rewritten to use "tier".
  All `§L2.10.x` substrate anchor IDs preserved (139 unchanged).
- `docs/systems/{a1-iaus-core-kickoff,refactor-plan,sleep-that-makes-sense}.md`,
  `docs/balance/{guarding-exit-recipe,acceptance-restoration,substrate-phase-3}.md`,
  `docs/systems/recreation.md` — Maslow-context "level" prose updated.
- `scripts/generate_wiki.py` — regex parser updated to match the new
  `// Tier N` source comments; output strings use "Tier" vocabulary.
  `docs/wiki/needs.md` regenerated via `just wiki`.

**Out of scope** (verified preserved):

- `NeedLevel` enum (`Critical`/`Low`/`Moderate`/`Satisfied`) — severity
  bucket, different concept. 13 references in
  `src/resources/narrative_templates.rs` unchanged.
- `LifeStage` (`Kitten`/`Young`/`Adult`/`Elder`).
- Substrate `L1/L2/L3` vocabulary except the one cross-vocabulary
  outlier at the `Maslow L1 (§3.4)` warmth-split bullet.
- All `§L2.10.x` substrate anchor IDs.
- Historical archives in `docs/open-work/landed/` and the frozen
  `docs/balance/eat-inventory-threshold.predictions.json` — point-in-
  time records, intentionally not retroactively edited.

## Verification

- `just check` clean — substrate-stub lint, step-resolver contract
  lint, time-units lint, IAUS coherence, item-transfers contract,
  InfluenceMap registry check all pass.
- `just test` — 1965 tests passed, 0 failed. The rename is purely
  token-level; assertion bodies inside renamed tests compute the same
  suppression values.
- `cargo check` clean.
- Anchor-preservation grep: `§L2.` count in
  `docs/systems/ai-substrate-refactor.md` unchanged at 139.
- `NeedLevel` count in `src/resources/narrative_templates.rs`
  unchanged at 13.

## Log

- 2026-05-07: Landed as trivial work via "Coverage gap (c)" path
  (no `tickets/NNN.md` precedes; written directly to `landed/`).
  Single commit covers symbol renames + comments + docs + script
  regen. Behavior change: zero. SHA backfilled in follow-up commit.
