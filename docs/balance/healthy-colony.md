# Healthy colony reference (seed 42, 900s deep-soak)

What does a "normal" Clowder run look like? This doc names the **expected
range** of every footer metric and **what each metric tells you** about the
colony. Use it together with `just verdict <run-dir>` and
`just fingerprint <run-dir>` to read a soak's outputs.

> **Source:** initial bands derived from `logs/sweep-baseline-5b/` (15 runs,
> seeds 42 / 99 / 7 / 2025 / 314 × 3 reps). Refresh after any commit that
> moves a survival or continuity canary, or quarterly. To regenerate the
> bands:
>
> ```
> just sweep healthy-colony-refresh
> just sweep-stats logs/sweep-healthy-colony-refresh > /tmp/bands.json
> # update the table below from /tmp/bands.json
> ```
>
> The numbers below are **mean ± stdev** unless noted; "expected" is roughly
> mean ± 2σ truncated at zero. Treat single-soak readings outside the band
> as worth investigating, not necessarily broken.

## Survival metrics

These are hard gates (the survival canaries — `just check-canaries`).

| Field | Expected (n=15) | What it tells you |
|---|---|---|
| `deaths_by_cause.Starvation` | 1.2 ± 1.7 (cap: 0 hard) | Food chain producing faster than consumption. **Failure mode:** Eat DSE not winning when hungry, or food economy broken. |
| `deaths_by_cause.ShadowFoxAmbush` | 4.3 ± 2.5 (cap: ≤10) | Ward-defense pipeline containing shadowfoxes. **Failure mode:** corruption climbing without ward placement; check `wards_placed_total`. |
| `deaths_by_cause.Injury` | 0.2 ± 0.6 | Combat / accident mortality baseline. |
| `deaths_by_cause.WildlifeCombat` | 0.9 ± 0.9 | Cat-vs-fox/hawk/snake fights ending badly. |

## Magic register

Active magic register is non-negotiable per `docs/systems/project-vision.md`.

| Field | Expected (n=15) | What it tells you |
|---|---|---|
| `wards_placed_total` | 141 ± 66 (range 50–293) | Magic register active; priestess is authoring wards. **Failure mode:** zero or near-zero placement → priestess never seeded, or ward DSE unreachable. |
| `wards_despawned_total` | 141 ± 66 | Mirrors placement; ward strength decays at expected rate. |
| `ward_count_final` | 0.1 ± 0.4 | At end-of-soak most wards have decayed/been sieged. Sustained `>0` means cats are placing faster than corruption sieges. |
| `ward_siege_started_total` | 703 ± 416 | Shadow-foxes attempting to break wards. High and rising = corruption pressure climbing. |
| `shadow_fox_spawn_total` | 7.3 ± 5.2 | Spawn cadence from corrupted tiles. **Failure mode:** zero = corruption never crosses spawn threshold (defenses too easy); ≥20 = corruption climbing too fast. |
| `shadow_foxes_avoided_ward_total` | 4686 ± 2834 | Shadow-foxes route around active wards. High = wards working. Zero with non-zero spawns = wards aren't blocking paths. |

## Anxiety + interrupt load

| Field | Expected (n=15) | What it tells you |
|---|---|---|
| `anxiety_interrupt_total` | 23842 ± 24636 | Cats fleeing to safety mid-plan. High variance (CV ~100%) — not actionable below 80k unless climbing across a baseline. |
| `negative_events_total` | 87919 ± 47166 | Sum of all negative-valence Feature firings. Climbing = colony stress rising. |

## Activation

| Field | Expected (n=15) | What it tells you |
|---|---|---|
| `positive_features_active` | 18 ± 1 (of 32 total) | Distinct positive Features that fired ≥1×. **Failure mode:** drop below 15 = subsystems silently dead. Pair with `never_fired_expected_positives` (canary). |
| `neutral_features_active` | 13 ± 2 (of 20 total) | Neutral telemetry breadth. Drops here mean instrumentation regressed or the sim collapsed before reaching some triggers. |

## Continuity (behavioral range)

These are continuity canaries — `just check-continuity`. Each must fire ≥1×; the
expected magnitudes below are floors above which you have headroom.

| Field | Expected (≥1) | What it tells you |
|---|---|---|
| `continuity_tallies.grooming` | ≥5 (typical: ~150–200) | Self-care behavior firing. **Failure mode:** zero = survival lock has crowded out fulfillment. |
| `continuity_tallies.play` | ≥1 | Play behavior firing. |
| `continuity_tallies.mentoring` | ≥1 | Adult cats mentoring juveniles. |
| `continuity_tallies.burial` | ≥1 | Burial / mourning rites for fallen cats. |
| `continuity_tallies.courtship` | ≥1 | Courtship arcs (CourtshipInteraction + CourtshipDrifted + MatingOccurred). |
| `continuity_tallies.mythic-texture` | ≥1 per sim year | Named mythic events (Calling, banishment, named-object craft). |

## Plan failures (informational)

Plan-failure tallies are not gated; they're a window into where step resolvers
fail. Top 3 most common at baseline:

- `EngagePrey: lost prey during approach` ~3675 ± 4990 (huge variance — depends on prey density)
- `EngagePrey: seeking another target` ~664 ± 501
- `EngagePrey: stuck while stalking` ~425 ± 460

Drift here is normal. Watch for **new** failure reasons appearing or **old**
ones disappearing entirely — those signal step-resolver behavior changes.

## How to use this doc

- **`just fingerprint <run-dir>`** evaluates each metric in this table against
  the run's footer and emits a per-field verdict (`in-range / low / high /
  failure`). Drop-in companion to `just verdict`.
- **Individual investigation:** when a band is violated, drill in with
  `just q` (`/logq` skill) targeted at the specific metric — e.g.
  `just q deaths logs/tuned-42 --cause=ShadowFoxAmbush` for an ambush spike.
- **Refreshing the bands:** after a substrate change that legitimately moves
  the colony (post-AI-substrate refactor; post-Phase-7 founder-age fix),
  re-derive from a fresh sweep and append a `## Iteration` section here
  rather than overwriting.
