---
id: 458
title: PracticeMagic soft affinity / skill considerations
status: ready
cluster: magic-mythic
orchestration: substrate-sensitive
initiative: [mythic-texture]
added: 2026-05-23
parked: null
blocked-by: []
supersedes: []
related-systems: [project-vision.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 004 retired the `magic_affinity / magic_skill > threshold` binary
gate at `src/ai/scoring.rs`, restoring L2 magic-DSE visibility for the
~60% of cats with low affinity. But the per-DSE `CompensatedProduct`
shape still zeros the score for kittens (skill=0): each magic DSE
(except Harvest) includes `magic_skill` as a multiplicative axis with
a `linear()` curve passing 0 through. And `magic_affinity` is no longer
read by any scorer — only by `check_misfire` at action time.

`docs/systems/project-vision.md` frames magic as ecological: a kitten
wandering into a FairyRing should feel the pull. Today, that kitten
sees the magic DSEs in their L2 trace at score 0 and never picks them.
This ticket fills the remaining substrate gap so the perception layer
encodes the pull, not just the eligibility.

## Scope

- Replace `Consideration::Scalar(ScalarConsideration::new("magic_skill", linear()))` in `src/ai/dses/practice_magic.rs` with a curve that has a positive intercept (e.g., `Linear { slope, intercept }` where intercept > 0). Pick a value that keeps high-skill cats winning over kittens but doesn't fully zero kittens.
- Add `Consideration::Scalar(ScalarConsideration::new("magic_affinity", <curve>))` to each of Scry / DurableWard / Cleanse / ColonyCleanse / Commune. (Harvest uses `herbcraft_skill` instead — decide whether to add affinity as a soft pull there too.)
- Update CP weight vectors to reflect the new axis count.
- Confirm `ctx.magic_affinity` plumbing in `ScoringContext` is intact (post-004 it's a write-only field with no readers — this ticket gives it a reader again).
- Balance hypothesis: predict the effect on MagicScry / MagicCleanse counts, kitten-led magic events, MisfireOccurred aftermath. Run `just hypothesize` end-to-end.

## Out of scope

- Adding a kitten-specific magic DSE family.
- Reworking the misfire system.
- Adding new magic actions / spells.

## Current state

Ticket 004 landed on 2026-05-23 (this session). The legacy binary gate
is gone; magic DSEs are visible in L2 for all eligible cats but the CP
shape leaves kittens at score=0 because `magic_skill * 0 = 0`. Module
rustdoc in `src/ai/dses/practice_magic.rs` cites this ticket as the
substrate-completion follow-on.

## Approach

Read `src/ai/dses/practice_magic.rs` post-004 — six DSE constructors,
each currently composing 3–4 considerations. Adjust:

1. Audit each DSE's `magic_skill` axis. Replace `linear()` with `Linear { slope: 1.0 - intercept, intercept: 0.1 }` (or a domain-appropriate intercept) so skill=0 → 0.1, skill=1 → 1.0. This makes high-skill cats still dominate the score but kittens can register.
2. Add `magic_affinity` as a Scalar consideration on Scry / DurableWard / Cleanse / ColonyCleanse / Commune (Harvest is a judgment call — see Scope). Use a `linear()` curve so high-affinity cats score higher.
3. Update CP weight vectors (currently `vec![1.0, 1.0, 1.0]` etc.) to reflect the new axis count.

Validate the substrate-honesty: the L2 trace should now show a non-zero score for high-affinity-low-skill kittens near a corruption hotspot or FairyRing — the ecological pull the project-vision describes.

## Verification

1. `just check` — type / linter / substrate-stub gates.
2. `just test` — DSE unit tests; new test asserting kitten with affinity=0.7 skill=0 scores non-zero on Scry.
3. `just scenario <name>` — define or use a scenario that places a kitten with high affinity / no skill near a corrupted tile; confirm L2 trace shows non-zero MagicCleanse and / or MagicDurableWard score.
4. `just hypothesize <spec.yaml>` — four-artifact methodology to bound balance impact. Expect: MisfireOccurred-aftermath rises (kittens attempting magic, channeling more than they can control); ScryCompleted / CleanseCompleted counts rise modestly; whole-colony characteristic metrics drift <10%.
5. `just verdict <run-dir>` — survival canaries hold (Starvation == 0, ShadowFox ambush ≤ 10, continuity canaries ≥ 1).
6. `just frame-diff <baseline> <new>` — per-DSE drift attribution; expect the six magic DSEs to show |Δ mean| consistent with the prediction.

## Log

- 2026-05-23: opened as the substrate-completion follow-on to ticket 004 per its plan's "extend" structural option.
