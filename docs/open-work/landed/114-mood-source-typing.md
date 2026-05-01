---
id: 114
title: MoodSource enum — typed emotional category for MoodModifier
status: done
cluster: emotional-fidelity
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 60c7c6ac
landed-on: 2026-05-01
---

## Why

`MoodModifier.source` is a debug `String`. Fear-based and grief-based modifiers amplify
identically via `anxiety_amplification`. You can't distinguish a fear-shaken cat from a
grief-stricken one at runtime or in log queries. `MemoryType` already has a typed enum
at `src/components/mental.rs:46` — `MoodSource` mirrors that pattern for the mood layer.

Enabling infrastructure for:
- Ticket 116 (grief modifier with `MoodSource::Grief`)
- Future narrative queries ("how many cats are grief-stricken?")
- Per-kind decay rates and anxiety-amplification weights

## Scope

- `MoodSource` enum in `src/components/mental.rs` (Physical / Social / Fear / Grief /
  Triumph / Pride / Magic / Misc).
- `kind: MoodSource` field added to `MoodModifier` with `#[serde(default)]`.
- `MoodModifier::new()` and `MoodModifier::with_kind()` constructors; all 37 push sites
  updated to use constructor form.
- Per-kind constants in `MoodConstants`: `fear_decay_rate: u64` (default 2),
  `fear_anxiety_amp_weight: f32` (default 1.5), `grief_anxiety_amp_weight: f32` (default 0.3).
- `update_mood` uses per-kind step for decay and per-kind weight for anxiety amplification.
- 14 priority push sites classified; remaining 23 default to `Misc`.

## Verification

- `just check` / `just test` — type-safety enforced by exhaustive match on new enum.
- `just soak 42` + `just verdict` — mood-valence distribution should be statistically unchanged
  (Misc amp_weight=1.0 preserves old behavior; Fear/Grief tuned constants ship active but small).

## Log

- 2026-05-01: Opened as emotional-fidelity infrastructure for ticket 116 and future
  narrative queries. Emerged from audit of DSE/internal-state flattening patterns.
