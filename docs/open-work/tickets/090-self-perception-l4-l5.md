---
id: 090
title: L4/L5 self-perception — mastery-confidence, purpose-clarity, esteem-distress
status: ready
cluster: ai-substrate
added: 2026-04-30
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Surfaced during 087's planning audit. The `populate_dse_registry` catalog has DSE coverage for Maslow tiers 1 (physiological) and 2 (safety) and partial coverage for 3 (belonging via socials). Tiers 4 (esteem — respect, mastery) and 5 (self-actualization — purpose) have *zero* DSE coverage, yet those needs decay (`src/resources/sim_constants.rs` defines `respect_base_drain`, `mastery_base_drain`, `purpose_base_drain`). They drain to zero with no DSE pulling them.

Closing the catalog gap requires both perception (knowing the cat's L4/L5 state) and DSEs (acting on it). This ticket is *only* about perception — the substrate that publishes mastery-confidence / purpose-clarity / esteem-distress scalars and markers. The downstream DSE catalog work is a separate ticket (or tickets) under the §L2.10 enumeration, and depends on this one.

This is *not* "balance tuning on refactor-affected metrics" (deferred per CLAUDE.md). It's substrate plumbing — perception coverage of needs that already exist in the data model.

## Substrate-over-override pattern

Part of the substrate-over-override thread (see [093](093-substrate-over-override-epic.md)) — extends 087's substrate to higher Maslow tiers, on-thread but not directly retiring an override.

**Hack shape**: similar to [089](089-interoceptive-self-anchors.md), this is substrate-expansion not hack-retirement. The current state — needs decay (respect/mastery/purpose) with no DSE pulling them — is *implicitly* a hack: silently dropping signals because no perception layer publishes them. The cat has no agentic awareness of L4/L5 distress.

**IAUS lever**: `mastery_confidence`, `purpose_clarity`, `esteem_distress` scalars + `LowMastery`, `LackingPurpose`, `EsteemDistressed` ZST markers — perception coverage of Maslow tiers that already exist in the data model. Future L4/L5 DSEs (separate tickets) consume these without overrides.

**Sequencing**: blocked-by 087 (landed). Perception-only ticket; downstream DSE catalog work depends on it but is out-of-scope here.

**Canonical exemplar**: 087 (CriticalHealth interrupt → `pain_level` + `body_distress_composite` axes, landed at fc4e1ab).

## Scope

- Extend interoceptive perception module (087) with new scalars: `mastery_confidence` (derived from skill levels — high skills → high confidence), `purpose_clarity` (derived from `Aspiration` state if present, else 0), `esteem_distress` (derived from low respect/mastery).
- New ZST markers (placed alongside 087's): `LowMastery`, `LackingPurpose`, `EsteemDistressed`. Per the pattern, gate thresholds defined in SimConstants.
- *No DSE work in this ticket.* The DSEs that consume these scalars/markers (Mentor-self-as-apprentice? Vision-quest? Pursue-aspiration?) are catalog work, not perception work.
- Update `ctx_scalars()` to include the three new keys.

## Verification

- Unit tests: scalars compute correctly from `Skills` / `Aspiration` / `Needs` state.
- Markers fire/clear at the configured thresholds.
- `just soak` shows the new markers being authored on cats with appropriate state. Without consumers, no behavior change; `never_fired_expected_positives == 0` (no positive features added — these are perception, not behaviors).

## Out of scope

- Any L4/L5 DSE — separate catalog tickets.
- Maslow gate semantics (L5 already gates on L1–L4 satisfied per refactor §3.4).
- Tuning the drain rates on respect/mastery/purpose — balance work, deferred until the substrate stabilizes.

## Log

- 2026-04-30: Opened alongside 087. Blocked-by 087 (perception substrate) until that lands. Catalog gap surfaced during inventory: `populate_dse_registry` has zero DSE coverage for Maslow L4/L5 yet those needs decay.
