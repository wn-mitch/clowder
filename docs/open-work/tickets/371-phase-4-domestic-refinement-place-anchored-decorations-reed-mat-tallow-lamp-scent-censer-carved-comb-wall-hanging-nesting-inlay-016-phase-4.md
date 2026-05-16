---
id: 371
title: Phase 4 domestic refinement — place-anchored decorations (Reed Mat, Tallow Lamp, Scent Censer, Carved Comb, Wall-Hanging, Nesting Inlay) (016 Phase 4)
status: blocked
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-16
parked: null
blocked-by: [370]
supersedes: []
related-systems: [crafting.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Land Phase 4 place-anchored decorations — Reed Mat / Woven Rug, Tallow Lamp, Scent Censer, Carved Comb, Wall-Hanging, Nesting Inlay — that shape the environment every colony member shares rather than buffing the cat who placed them. All entries are `CraftedDecoration`s placed at a tile. Targets preservation + generational knowledge + mythic texture §5 axes simultaneously. Parent epic: [016](016-crafting-items-recipes-stations.md).

## Scope
- New `CraftedDecoration` component family (distinct from `CraftedItem`); place-anchored, not carried.
- Six new `Recipe` entries on the Workshop; output → tile placement.
- Tile-level effects: warmth (Reed Mat → sleep quality, kitten-cradle bias), illumination (Tallow Lamp → night-fear reduction in 3-tile radius), scent (Scent Censer → modulates `fox_scent_map` repellent + `prey_scent_map` slight mask, herb-content-driven), grooming-action quality (Carved Comb → action buffed, not cat), colony-memory marker (Wall-Hanging → naming-eligible on Significant events near it), alcove preservation-weight upgrade (Nesting Inlay → permanent alcove upgrade).
- Minimal `TileAmenities` interface for future `environmental-quality.md` A-cluster refactor consumption (ships even if that refactor hasn't landed).
- Tallow Lamp refuel chain (attending-cat tending cycles, similar to Smoking Rack from 367).
- Wall-Hangings naming-eligible via `naming.md` substrate (with neutral-fallback if naming hasn't landed).

## Out of scope
- Phase 5 cumulative artifacts (→ 372).
- `environmental-quality.md` A-cluster refactor itself (Phase 4 reads from it once it lands).

## Approach
See `docs/systems/crafting.md` Phase 4. Hypothesis: on seed-42 `--duration 900`, hearth-tile kitten-sleep count rises ≥1.5× vs. decoration-disabled control; mythic-texture count rises ≥1 additional named landmark per sim-year from decoration-origin events; `Starvation = 0` canary holds (no decoration-effort displacement of food-effort).

## Verification
- `just hypothesize <spec.yaml>` for the kitten-sleep + mythic-texture co-canaries with a decoration-disabled control.
- `just verdict <run-dir>` — all continuity canaries hold.

## Log
- 2026-05-16: opened as 016 epic decomposition (Phase 4; parent 016, blocked-by 370).
