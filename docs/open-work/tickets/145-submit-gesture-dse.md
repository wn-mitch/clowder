---
id: 145
title: Submit gesture DSE — appeasement infrastructure for IntraspeciesConflict.Fawn
status: ready
cluster: ai-substrate
added: 2026-05-02
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

DSE infrastructure prerequisite for ticket 144 (Fawn valence of 109's
N-valence intraspecies conflict framework). Real cats appease via
specific behaviors — belly-up, slow blink, head turn-away,
scent-marking — that are observably distinct from withdrawal (Flight
109a) and stillness (Freeze 142). A Modifier alone cannot lift this
valence because no existing DSE expresses it; this ticket adds the
DSE and step.

Sibling pattern to ticket 104 (Hide/Freeze DSE) — both are gesture
DSEs that ship the *capability* before any modifier lifts them.

## Scope

- New `Action::Submit` variant.
- New `SubmitDse` in `src/ai/dses/submit.rs` — eligibility gates on
  presence of a higher-status cat in line-of-sight; considerations
  shape the magnitude based on status differential and proximity.
- New `resolve_submit` step in `src/steps/disposition/submit.rs` with
  the standard 5-heading rustdoc preamble. Real-world effect: cat
  performs the appeasement gesture at its current position, no
  movement. Optional secondary effect on the dominant cat (de-escalate
  their fight-readiness state) — TBD during impl, can be a follow-up
  if scope creeps.
- `Feature::SubmitGestured` classified per
  `Feature::expected_to_fire_per_soak()` — returns false initially
  (rare event, exempt from per-seed canary until colony hits a
  scenario producing it regularly).
- Could repurpose existing socialize-gesture machinery (per ticket
  109 §Scope) — investigate during impl whether `SocializeDse`'s
  target-taking infrastructure can be reused with a different
  intention.

## Verification

- Unit test: DSE scores zero when no higher-status cat visible;
  non-zero when subordinate-vs-dominant pair present.
- Step contract test: `resolve_submit` mutates only `CurrentAction`
  and (optionally) the target cat's de-escalation state — no
  resource consumption, no movement.

## Out of scope

- The Modifier that lifts Submit (ticket 144 — separate ticket).
- Status-differential composition (owned by 109's Phase-3 perception
  coupling work).
- Cross-species appeasement (ecologically incoherent — Submit fires
  only on same-species dominants).

## Log

- 2026-05-02: Opened as DSE-infrastructure prerequisite for ticket
  144 (Fawn valence) alongside 109 Phase A landing.
