---
id: 322
title: Action-enum stubs for dormant HTN methods
status: done
cluster: items-crafting
initiative: [smarter-cats]
added: 2026-05-14
parked: null
blocked-by: []
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-14
---

## Why

128 epic infrastructure. Tier-2 dormant methods reference Action
enum variants that don't yet exist (`Action::WearItem`,
`Action::Craft`, `Action::PetitionCoordinator`). The substrate-
stub-allowlist discipline (precedent §4.7.7, ticket 252) ships
these variants today with placeholder resolvers, so dormant
methods can typecheck against them in
`populate_method_registry` from day one.

Per
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
§Dormant-method discipline / Action-enum stubs: every new
`Action::*` variant ships with a placeholder `resolve_*` (five
required rustdoc headings, body returns
`StepOutcome::Failed { reason }`) + an entry in
`scripts/substrate_stubs.allowlist`.

## Scope

- Add `Action::WearItem`, `Action::Craft`,
  `Action::PetitionCoordinator` variants (and any additional
  variants the Tier-2 method catalogue requires — refine during
  implementation).
- Placeholder `resolve_*` per variant:
  - Five required rustdoc headings per CLAUDE.md step-resolver
    contract.
  - Body: `StepOutcome::Failed { reason: "<blocker-ticket>
    not yet landed" }`. Contract-compliant; never witnesses.
- Allowlist entries in `scripts/substrate_stubs.allowlist`
  naming each variant's wiring ticket
  (#332 grief-vigil → Vigil/GriefSit; #333 kitten-rearing →
  Wean/Teach/Release; #334 stealth-cloak → WearItem/Craft).
- Initial `PendingSubstrate` method registrations in
  `populate_method_registry` for `mourn_at_grave`, `rear_kitten`,
  `acquire_stealth_via_self_craft`,
  `acquire_stealth_via_commission` — each `blocker` pointing at
  its wiring ticket (#332, #333, #334).

## Out of scope

- Implementing the Action effects (that's #332-#334 per Tier-2
  glue tickets).
- The methods themselves are registered here as PendingSubstrate
  with `eventual` predicate referencing the new variants; the
  flip-to-Live happens in #332/#333/#334.

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #4 of 25; fully parallel with #319 in Batch A
(no inter-dependency). Cluster `items-crafting` per the plan —
natural home for substrate-stub allowlist additions.

## Approach

Per htn-methods.md §G + §4.7.7 precedent. The placeholder
resolver pattern was validated by ticket 252 (the Action::Flee
discovery). `StepOutcome::Failed { reason }` is the canonical
intentionally-inert exit.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes (substrate-stub allowlist accepts the new
  entries).
- `just methods --pending` lists `mourn_at_grave`, `rear_kitten`,
  `acquire_stealth_via_self_craft`, `acquire_stealth_via_commission`
  with their respective blockers.
- Manual test: confirm the placeholder resolvers panic-free under
  contract-lint inspection (the contract is non-witnessing
  failure, not silent advance).

## Log

- 2026-05-14: opened as 128 epic child #4 (Batch A infrastructure,
  parallel with #319).
- 2026-05-14: landed. 8 dormant Action variants + 4 PendingSubstrate methods (mourn_at_grave / rear_kitten / acquire_stealth_via_self_craft / acquire_stealth_via_commission) registered. First real exercise of check_method_registry.sh's bidirectional gate; negative tests verified Pass A (blocker→open ticket) and Pass B (frontmatter wires-method back-ref). Placeholder resolvers under src/steps/disposition/ carry 5-heading contract; allowlist entries in scripts/substrate_stubs.allowlist anchor each resolver to its wiring ticket (#332/#333/#334).
