---
id: 136
title: WoundedAlly marker + positional dependent-proximity for escape_viability
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

Ticket 103 (`escape_viability` scalar) v1 ships with **marker-only** dependent presence:

```text
has_nearby_dependent = markers.has(Parent, entity) || has_pair_bond
```

This is a coarse approximation of the original ticket spec, which called for "kittens / mate / wounded ally **within threat-radius** pulls viability down." Two tightenings were parked at landing:

1. **Positional dependent proximity.** A parent's penalty currently fires whenever they are a parent, regardless of whether the kitten is anywhere near the threat. Refining to "kitten/mate within `wildlife_threat_range` of the nearest threat" matches the original cost-of-abandonment intuition and makes the scalar sharper.

2. **`WoundedAlly` concept.** No marker today. Cats track `Injured` / `Incapacitated` markers on themselves, but there is no surface for "is there a hurt comrade nearby that I'd be abandoning by fleeing?"

## Scope

1. **`WoundedAlly` author** — colony or per-cat ZST authored when `Injured` || `Incapacitated` is present on a non-self cat within threat radius. Likely lives in `src/systems/sensing.rs` next to existing target-existence markers (`HasThreatNearby`, `HasSocialTarget`), or in a small new perception system.

2. **Positional refinement of dependent term.** In `src/systems/disposition.rs` and `src/systems/goap.rs` populators (the two `escape_viability` call sites), when the cat is `Parent` or pair-bonded:
   - Look up kitten positions via `KittenDependency` reverse-query.
   - Look up mate position via `PairingActivity.partner` + that entity's `Position`.
   - Compute `min_dependent_to_threat = min(kitten_threat_distance, mate_threat_distance)`.
   - Set `has_nearby_dependent` only when that minimum is `<= wildlife_threat_range`.

3. **Optional: graded penalty.** Today the penalty is bool-style. A continuous penalty `(1.0 - dependent_distance_to_threat / wildlife_threat_range).clamp(0, 1)` matches "the closer the kitten is to the threat, the less viable escape is." Decide at land time based on whether the bool-style v1 produces visible artifacts.

## Verification

- Unit tests: parent with kitten *far* from threat → no penalty; parent with kitten *near* threat → penalty. Same for mate. WoundedAlly flag fires correctly.
- `just soak 42 && just verdict` — expect a small dampening on Flee for parents whose kittens are out of harm's way (matches design intent).

## Out of scope

- "Wounded ally" detection of *cooperating* wounded — e.g. fellow guards mid-combat. The marker is just "any injured non-self cat within threat radius." Tactical alliances are a separate scope.
- Caretaking-style behavior for wounded allies (carry-them-to-safety). Would be a new DSE; this ticket is perception only.

## Log

- 2026-05-02: Opened as a follow-on of work 103. v1 of `escape_viability` ships with marker-only dependent presence (Parent || PairingActivity); positional refinement and WoundedAlly axis bundled here for a future tightening pass.
