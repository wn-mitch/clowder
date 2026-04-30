---
id: 082
title: 027b L2 PairingActivity reactivation on the hardened substrate
status: parked
cluster: planning-substrate
added: 2026-04-29
parked: 2026-04-29
blocked-by: [083]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [027-l2-pairing-activity.md]
landed-at: null
landed-on: null
---

## Why

Once the substrate is hardened (072–074 minimum; 075–081 desirable for full coverage), 027b's L2 PairingActivity author can ship without exposing the stuck-loop fragility. The original ticket 027b's "topological reshuffle" diagnosis was wrong; the actual mechanism was a long-horizon RNG drift cascading through an unguarded planning substrate. Hardening removes the amplifier; activation now just produces normal IAUS scoring.

Parent: ticket 071. Blocked by the minimum hardening (072 refactor + 073 cooldown + 074 alive guards). Recommended: also wait for 075 / 076 / 078 / 080 / 081 to land before activating, so the substrate is fully girded.

## Scope

- Uncomment `src/plugins/simulation.rs:322` so `crate::ai::pairing::author_pairing_intentions` re-enters chain 2a's marker batch.
- Replace the deferral block comment (lines 295–321) with a single-line activation pointer: `// L2 PairingActivity activated YYYY-MM-DD on substrate hardening 071`.
- Promote `Feature::PairingIntentionEmitted` and `Feature::PairingBiasApplied` to `expected_to_fire_per_soak() => true` in `src/resources/system_activation.rs`. (`PairingDropped` stays at `false` — it's bursty and not load-bearing for canary.)
- Single-seed verification soak: `just soak 42 && just verdict logs/tuned-42-pairing-active`. Hard gates: `Starvation = 0`, `ShadowFoxAmbush ≤ 10`, all six continuity canaries ≥ 1.
- Multi-seed sweep per ticket 027b Commit C: `just baseline-dataset 2026-MM-DD-bug3-l2pairing` + `just sweep-stats … --vs logs/baseline-2026-04-25` to validate predictions P1–P4 from `docs/balance/027-l2-pairing-activity.md`:
  - **P1**: `MatingOccurred > 0` in ≥ 1/12 sweep runs.
  - **P2**: `BondFormed_Partners > 0` in ≥ 4/12 runs.
  - **P3**: `PairingBiasApplied / SocializeTargetResolves > 0.10` in ≥ 50% of runs.
  - **P4**: Survival canaries within ±10% noise band; Cohen's d < 0.5 on `mean_lifespan` and `colony_size_end_of_window`.
- On full pass: `just promote logs/baseline-2026-MM-DD-bug3-l2pairing 027bug3-l2pairing` (refreshes the stale `post-033-time-fix` pointer in `logs/baselines/current.json`); flip 027b status to `done`; close 027 cluster.
- Append observation + concordance to `docs/balance/027-l2-pairing-activity.md`.

## Out of scope

- The `groom_other_target` and `apply_pairing_bonus` bias channels — defer to ticket 027c-bias-channels per 027b's original "out of scope" section. (When they land, they should follow ticket 078's pattern: each new bias channel is a Consideration on the target DSE, not a post-hoc lift.)
- Mate-target Intention pin (if one is added later) — same pattern: Consideration, not pin.
- Tuning `PairingConstants` defaults beyond their current values — post-landing balance work.

## Approach

Files:

- `src/plugins/simulation.rs:322` — uncomment the schedule line; replace the deferral block comment with the one-line activation pointer.
- `src/resources/system_activation.rs::expected_to_fire_per_soak()` — flip `PairingIntentionEmitted` and `PairingBiasApplied` to `true`.
- `docs/balance/027-l2-pairing-activity.md` — append a 2026-MM-DD observation block with the post-hardening soak's footer + concordance verdict against P1–P4.
- `docs/open-work/tickets/027b-l2-pairing-activity.md` — flip `status: done`, fill `landed-at: <sha>`, `landed-on: <date>`; move file to `docs/open-work/landed/2026-MM.md` per CLAUDE.md's "When work lands" protocol.

## Verification

- Single-seed soak verdict passes (`Starvation = 0`, `ShadowFoxAmbush ≤ 10`, all six continuity canaries ≥ 1).
- Multi-seed sweep clears P1–P4.
- `Feature::PairingIntentionEmitted` and `Feature::PairingBiasApplied` fire non-zero counts on the soak (not in `never_fired_expected_positives`).
- Baseline promoted; 027b ticket flipped to `done` and moved to landed; this ticket flipped to `done` after.

## Log

- 2026-04-29: Opened under sub-epic 071.
- 2026-04-29: Re-parked. Sub-epic 071's Wave 1 (072) + Wave 2 (073/074/075/078/079/080) substrate hardening landed at main `43cc38a7`. Activation attempt (uncommenting the L2 author at `src/plugins/simulation.rs:327`) confirms the substrate hardening prevents the originating starvation cascade — `Starvation = 0` in `logs/tuned-42-082-pairing-active-farming-regress/` (vs `Starvation = 3` in `logs/tuned-42-027b-active-failed/`). The L2 author works (`PairingIntentionEmitted = 14651`, `PairingDropped = 14650`). However a NEW regression surfaced: registering `author_pairing_intentions` in chain 2a perturbs Bevy 0.18's topological sort enough to collapse Farming entirely (Farming plans 448 → 0; `CropTended` 5070 → 0; `CropHarvested` 176 → 0). The substrate hardening absorbed the originating cascade but not the scheduler-shift effect on Farming. Now `blocked-by: [083]`; ticket 083 investigates and fixes the schedule-shift root cause. Once 083 lands, this ticket's reactivation soak should clear with Farming intact.
