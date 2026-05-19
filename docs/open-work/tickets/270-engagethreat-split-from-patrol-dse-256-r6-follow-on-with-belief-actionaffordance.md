---
id: 270
title: EngageThreat split from Patrol DSE (256 R6 follow-on with Belief + ActionAffordance)
status: ready
cluster: combat-threat
orchestration: substrate-sensitive
initiative: []
added: 2026-05-10
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

256's structural-options menu lists R6 as a deferred follow-on: "Split `DispositionKind::Guarding` into `Guarding` (proactive perimeter walk) vs `EngageThreat` (reactive combat). Optional; defer until R3+R4+R5 land." This ticket is that follow-on, *plus* the substrate enrichment that the C3 / Affordance cluster makes possible: when EngageThreat splits out, it lands as a Belief+Affordance consumer from day one (reads `MentalModel<Predator>(target).perceived_violence_capability` and `Affordance(Fight, self, target)`).

Splitting EngageThreat from Patrol does two things: (a) clears the resolver-internal phase logic that conflates perimeter-walking with reactive-combat in 256's substrate; (b) gives the substrate a clean DSE to compose Belief+Affordance reads into for combat affordance scoring (sister to 143 IntraspeciesConflictResponseFight Modifier).

Pre-existing related work: 143 (IntraspeciesConflictResponseFight) — combat valence Modifier, intraspecies-only. This ticket lands the cross-species combat DSE (cat fights fox / shadowfox / etc.); 143 then composes as an intraspecies-only Modifier on top.

## Scope

- **`EngageThreat` DSE** (new, `src/ai/dses/engage_threat.rs`): scored from Belief+Affordance reads; resolver implements close-and-strike against the threat target.
- **Patrol DSE refactor**: remove the reactive-combat path from Patrol's resolver chain. Patrol becomes proactive-perimeter-walking only; EngageThreat handles all combat election.
- **`DispositionKind` enum**: split `Guarding` into `Guarding` (Patrol) and `EngageThreat` (combat). Update Action→Disposition mapping in `src/components/disposition.rs::from_action`.
- **Belief axis read**: `MentalModel<Predator>(target).perceived_violence_capability` — gates EngageThreat scoring (don't engage what you can't beat).
- **Affordance axis read**: `Affordance(Fight, self, target)` from substrate 261 — the canonical combat-success scalar.
- **Plan template**: new GOAP plan template under `src/ai/planner/` for the EngageThreat sequence (approach → strike → resolve).
- **Completion proxy** in `src/components/commitment.rs` (per CLAUDE.md bugfix discipline — completion proxies are part of the layer-walk).

## Out of scope

- The Belief substrate (258).
- The ActionAffordances substrate (261).
- Patrol DSE substrate enrichment (ticket 263 — 256-cluster consumers; this ticket pulls combat OUT of Patrol but doesn't otherwise modify Patrol's scoring).
- 143 IntraspeciesConflictResponseFight Modifier (intraspecies-only Modifier on this DSE; complementary).
- Combat damage / health-mutation logic (existing combat resolver semantics preserved; this ticket just rebinds what DSE owns combat election).

## Current state

- Blocked-by 256 (R3+R4+R5 must land first; 256 is currently in-progress per `git status` 2026-05-10) and 261 (ActionAffordances substrate).
- 258 (Belief substrate) is technically a soft blocker for the Belief axis read; if 258 hasn't landed, this ticket can land with just Affordance reads and add Belief reads as a follow-up commit.
- 143 (IntraspeciesConflictResponseFight Modifier) is `ready`, blocked on 109's substrate work. Complementary, not blocking.
- 256's R6 originally framed as resolver-internal split; this ticket extends to also wire the substrate consumers as part of the same change.

## Approach

1. Land 256 first (blocker).
2. Audit 256's resolver chain for the reactive-combat path; identify what gets pulled out.
3. New `EngageThreat` DSE struct + plan template + completion proxy.
4. Update `DispositionKind` enum + Action mapping + Disposition::from_action.
5. Wire Belief + Affordance reads.
6. Verify per-DSE scenario microexperiments + frame-diff against pre-256 baseline (combat election should stay similar; just sourced from EngageThreat instead of Patrol).

## Verification

### Per-DSE scenario microexperiments

- `engage_threat_fires_on_high_fight_affordance` — cat with high HP/combat profile + low `perceived_violence_capability(target)`; verify EngageThreat elevates and Patrol does not.
- `engage_threat_skips_low_fight_affordance` — cat with low HP + high `perceived_violence_capability(target)`; verify EngageThreat does NOT elevate (Flee should win instead).
- `patrol_no_longer_combats` — cat patrolling, threat appears; verify Patrol drops + EngageThreat takes over (combat is no longer routed through Patrol's resolver).

### Soak gates

- ShadowFoxAmbush canary holds (≤ 10) — splitting EngageThreat out shouldn't change combat outcomes, just re-source them.
- Combat mortality canary stable.
- Per `just q anomalies`, neither Patrol nor EngageThreat absorbs >40% of elections (the cascade signature).

### Frame-diff

`just frame-diff <pre-256-baseline> <post-256-and-this-ticket>` — combat-related action elections should stay similar in count and outcome but be sourced from EngageThreat instead of Patrol.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- · **268** (ready, combat-threat, score 0.90) — Hide DSE — wire C3 Belief + ActionAffordance for general threat-response (door-…
- · **269** (blocked, combat-threat, score 0.89) — Submit DSE — wire C3 Belief + ActionAffordance + revisit cross-species extension
- · **315** (blocked, ai-substrate, score 0.88 (cross-cluster)) — activate 263 axes with four-artifact methodology (Flee/Patrol/Hunt + resolver b…

<!-- linkages:end -->
## Log

- 2026-05-10: opened sibling-to-258 as the substrate-enriched version of 256 R6's deferred Patrol/EngageThreat split. Blocks-on 256; soft-blocks-on 258. Session plan: `~/.claude/plans/after-working-256-i-dreamy-fiddle.md`.
- 2026-05-19: accuracy audit pass — no blockers recorded in frontmatter (256/261 soft-blocked as noted in body); patrol.rs exists; aspirational engage_threat.rs correctly deferred to implementation; related infrastructure (143, 256, 263, 261) status verified.
