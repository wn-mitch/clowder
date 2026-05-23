---
id: 453
title: Close inter-cat Mates-bond exclusivity gap
status: done
cluster: social-coordination
initiative: []
added: 2026-05-23
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 4630cf9aa266
landed-on: 2026-05-23
orchestration: substrate-sensitive
---

## Why

Running the sim in visual mode surfaced cats holding multiple concurrent `BondType::Mates` bonds with different partners — a "polyamorous" appearance. Investigation confirms substrate accident, not intended design: `JointIntention` is a per-cat singular `Component` (the intent of "one active courtship at a time" is structurally in the type), and `BondType::Mates` is documented in `docs/systems/warmth-split.md` as the romantic-pair tier (grief duration scales by bond tier). But no inter-cat exclusivity gate exists at the bond writer: `check_bonds` (`src/systems/social.rs:335-475`) iterates pair-by-pair and independently promotes any pair crossing the Mates thresholds, so cat A can be Mates with cat B and cat C simultaneously. The user framed monogamous pair-bonding as the **current substrate shape** — not a permanent design rule — pending future "romantic depth" work (infidelity, polyamory, jealousy, multi-sire pregnancy). The fix enforces singleness in a way that future relaxation can flip rather than redesign.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/ai/markers/marker_eligibility/has_eligible_mate.rs` | `HasEligibleMate` checks orientation, range, fertility, and candidate's bond tier — never the actor's existing bond count | `[verified-correct]` (not enforcement layer) |
| L2 DSE eligibility | `src/ai/dses/mate.rs:68` | `MateDse` `.require(HasEligibleMate::KEY)` — correct shape, downstream of the gap | `[verified-correct]` |
| L2 target candidate filter | `src/ai/dses/mate_target.rs:187-201` | Filters `other`'s bond tier to `Partners|Mates`, but never reads `other`'s other relationships — a Partners-bonded candidate who is Mates-bonded elsewhere still passes | `[verified-gap]` |
| L2 emission predicate | `src/ai/joint_intention.rs:381-441` | `pick_courtship_partner` scans candidates for `bond_tier_score > 0`; no self-side check that `self_entity` is already Mates-bonded, no candidate-side check for third-party Mates bonds | `[verified-gap]` |
| L3 softmax | `src/ai/scoring.rs` | n/a — the gap is upstream of softmax | `[verified-correct]` |
| Action→Disposition mapping | `src/components/disposition.rs::from_action` | n/a | `[verified-correct]` |
| Plan template | `src/ai/planner/build_mating_chain` | Assumes upstream eligibility; downstream of the gap | `[verified-correct]` |
| Completion proxy | `src/components/commitment.rs` | n/a | `[verified-correct]` |
| Resolver | `src/steps/disposition/mate_with.rs` | Does not write `Relationship.bond`; not the enforcement point | `[verified-correct]` |
| **Bond writer** | `src/systems/social.rs:424-446` (`check_bonds`) | Per-pair iteration via `Relationships::pairs_iter_mut`. Computes `new_bond` from thresholds (`mates_*_threshold`) and assigns `rel.bond = new_bond` when `new_bond > old_bond`. **No per-cat scan for existing Mates bonds before promotion.** | **`[verified-defect]`** |

## Fix candidates

**Parameter-level options** (rejected — no parameter exists; this is a structural gap):
- R1 — *(none applicable; the defect is an absent invariant, not a wrong threshold)*

**Structural options:**
- R2 (**split**) — n/a. Not splitting `BondType::Mates` or any DSE variant.
- R3 (**extend**) ← **chosen** — extend `check_bonds` with a per-actor "already holds a Mates bond?" precondition before promotion; extend `pick_courtship_partner` and `mate_target_dse` candidate filters with the same gate at the perception layer.
- R4 (**rebind**) — n/a.
- R5 (**retire**) — n/a.

## Recommended direction

R3 (extend). Implementation:

1. **`check_bonds` (load-bearing structural enforcer).** Before the `pairs_iter_mut` loop, build a `BTreeMap<Entity, Vec<Entity>>` of existing Mates partners per cat. For any cat with >1 partners, keep the canonical (lowest-Entity-bits) partner at Mates and demote the rest to Partners in a fixup pass — this migrates existing polyamorous state deterministically. Then during the main loop, before promoting a pair to `Mates`, consult a `BTreeSet<Entity>` of "cats currently holding a Mates bond" (kept in sync with the fixup + promotion path); if either side is already in the set, cap the proposed `new_bond` at `Partners`. End-of-system `debug_assert!` that the invariant holds.

2. **`pick_courtship_partner` (perception gate).** Add an early-return `None` when `self_entity` has any Mates bond via `relationships.iter_for(self_entity)`. In the candidate loop, additionally skip `other` if `other` has a Mates bond with a third party.

3. **`mate_target_dse` candidate filter.** Same shape as (2) on the candidate side: skip cats Mates-bonded to a third party. The actor-side check is unnecessary here — the existing `Partners|Mates` candidate filter already restricts to the bonded partner once the actor is locked.

**Why extend, not redefine `BondType::Mates`:** keeping exclusivity as a *promotion-time invariant in `check_bonds`* (not a semantic property of the enum) preserves the relaxation path for future romantic-depth work. Adding a `Promiscuous` personality trait or a colony-level threshold later only has to flip the gate; the bond type stays stable.

## Out of scope

Future "romantic depth" — explicitly parked, not deferred:

- **Multi-sire pregnancies.** `Pregnant.partner: Option<Entity>` stays single-valued.
- **Infidelity / jealousy substrate.** No `Jealousy` axis, no romantic-axis decay on partner's third-party courtship, no `Mates` *breakage* on observed infidelity.
- **Polyamory traits / colony-level configurability.** No `Promiscuous` marker, no `MAX_MATES_PER_CAT` constant. The natural extension point when this work happens is the `check_bonds` gate added here.
- **Tom mate-guarding vs queen matrilineal affiliation.** The broader ethological-grounding work (matrilineal kin groups, allomothering, transient mate-guarding) is a separate substrate question.

No follow-on tickets opened: the user has named these as future areas of interest, not parked work with a known shape (per [[feedback_close_the_clade]]).

## Verification

1. `just check && just test` — covers:
   - **Unit tests** for the migration helpers (`mates_partners_by_cat`, `collect_excess_mates_to_demote`): V-shape demote, triangle collapse, no-op when invariant holds.
   - **Unit tests** for the perception gates (`pick_courtship_partner` returns `None` when self already Mates-bonded; skips candidates Mates-bonded elsewhere; `resolve_mate_target` skips candidate when Mates-bonded elsewhere; keeps own mate when actor is bonded).
   - **Integration tests** through `check_bonds` via Schedule: refuses second Mates promotion, demotes pre-existing polyamory deterministically. A standalone scenario file was considered but folded into these integration tests — same surface, less setup boilerplate (close-the-clade discipline).
2. `just soak-trace 42 Simba` — canonical 15-min release deep-soak.
3. `just verdict logs/tuned-42` against current baseline — survival canaries (grooming / play / mentoring / courtship) must each hold ≥1; `MatingOccurred` must still fire (the fix should not suppress mating, only refuse second-bond promotion).
4. `just frame-diff logs/baseline-<current>/trace-Simba.jsonl logs/tuned-42/trace-Simba.jsonl` — MateDse / Courtship-related rows should be ~unchanged for monogamous cats and slightly suppressed for cats whose prior behavior was polyamorous.

## Log

- 2026-05-23: opened.
- 2026-05-23: 2026-05-23: landed — check_bonds migration + exclusivity gate; perception filters in pick_courtship_partner and mate_target; 10 new tests; just check && just test green.
