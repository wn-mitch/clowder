---
id: 249
title: Extend DispositionFailureCooldown coverage to Resting/Guarding/PickingUp et al. (R3 from 247)
status: ready
cluster: null
added: 2026-05-08
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
247 promoted H7 to `[verified-defect]`: `DispositionFailureCooldown::signal_key`
in `src/ai/modifier.rs:2673-2686` covers Hunt, Forage, Cook,
HerbcraftGather, HerbcraftPrepare, HerbcraftWard, MagicScry,
MagicDurableWard, MagicCleanse, MagicColonyCleanse, MagicHarvest,
MagicCommune, Caretake, Build, Mate, Mentor — but NOT Resting,
Guarding, PickingUp, Discarding, Trashing, Handing, Socializing,
Exploring, Mating, Burying, Grooming, Coordinating. After a
planning failure on an uncovered disposition, the cat re-elects
the same disposition immediately, slamming the planner. 247's
collapsed run footer concentrated `Resting:GoalUnreachable=1172`
and `Guarding:GoalUnreachable=526` in the uncovered set; the cliff
was driven by a no-Stores cascade but the cooldown gap let the
cascade slam the planner without back-pressure.

R4 (247) resolved the trigger-3 churn that triggered the
cascade, so the cooldown gap is no longer load-bearing in the
seed-42 soak. But the gap is real — any future cascade that
elevates an uncovered disposition's planning failures (no-Stores,
no-RestingSpot, no-recipient handoffs, etc.) will exhibit the
same planner-slam pattern. This ticket closes the gap.

## Scope
- Extend `signal_key` match arms at `src/ai/modifier.rs:2673-2686`
  to cover Resting, Guarding, PickingUp, Discarding, Trashing,
  Handing, Socializing, Exploring, Mating, Burying, Grooming,
  Coordinating.
- Add the corresponding signal-authoring entries in
  `ScoringContext` (the `disposition_failure_signal_*` family
  populated at `src/systems/goap.rs:~2010-2053`) so each newly-
  covered disposition has its own normalized recent-failure age.
- Verify the existing seed-42 soak doesn't regress (gap closure
  is additive — only changes behavior when planning failures
  actually occur on the newly-covered dispositions).

## Out of scope
- The trigger-3 capture-timing fix (H3 from 247) — see ticket 248.
- Tuning the `disposition_failure_cooldown_ticks` constant —
  separate balance concern.
- Adding new dispositions to the system (this is coverage
  expansion for existing dispositions only).

## Current state
- 247 landed at sha `7cd1b00b` resolving the trigger-3 churn
  cliff that exposed this gap on seed-42.
- The H7 `[verified-defect]` audit row in 247 documents the
  current vs missing coverage.
- 123 (`docs/open-work/landed/`) introduced
  `RecentDispositionFailures` substrate; 112
  (`docs/open-work/landed/`) retired per-disposition exemption
  lists. This ticket continues that line of work.
- Each newly-covered disposition needs a
  `disposition_failure_signal_*` ScoringContext scalar AND a
  consumer in some DSE / modifier (otherwise the signal is dead).
  Audit which DSEs / modifiers should read each new signal.

## Approach
1. Walk every `DispositionKind` variant; for each currently
   uncovered, decide whether failure-cooldown semantics make
   sense:
   - **Yes** — extend `signal_key`, add ScoringContext scalar,
     add consumer in the disposition's primary DSE (or in a
     cross-cutting modifier).
   - **No** — explicit `_ => None` arm with rationale comment.
     (E.g., Burying/Mating may complete in 1 tick and have no
     "failure" mode worth back-pressuring.)
2. Land the extension. Re-soak `just soak-trace 42 Mallow` to
   confirm no regression.
3. Update 247's H7 audit row to `[fixed by 249]`.

## Verification
1. **No-regression soak** — `just soak-trace 42 Mallow` plus
   `just verdict logs/tuned-42 --baseline <pre-249>` shows
   noise-band drift on aggregate / welfare / continuity tallies.
   Coverage extension is additive; adding signals shouldn't
   change behavior on a healthy run.
2. **Cliff-replay scenario** — manual override:
   `intention_preempt_strength_regime_boundary = 0.0` (replicates
   247's collapsed run). With 249 in place, the cliff still
   manifests (because 248 owns the actual fix) BUT the planner-
   slam pattern is back-pressured: Resting/Guarding planning-
   failure counts should be bounded by the cooldown window. Use
   this to verify the cooldown gates actually fire on the
   newly-covered dispositions.
3. **Read-site audit** — for each newly-added scalar, grep
   confirms at least one consumer (DSE or modifier) reads it.
   Otherwise the signal is dead.

## Log
- 2026-05-08: opened from 247's §Out of scope. H7 row promoted
  in 247 via code-side queries. R4 (247) resolved the seed-42
  cliff; this ticket closes the cooldown coverage gap so the
  planner has back-pressure for any future cascade on
  Resting / Guarding / PickingUp / et al.
