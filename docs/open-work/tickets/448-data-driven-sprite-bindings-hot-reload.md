---
id: 448
title: Data-driven sprite bindings + hot reload
status: ready
cluster: rendering
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-22
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
Sprite bindings for items (43 `ItemKind` variants), buildings (11 `StructureType` variants × per-type variant rolls × winter variants), herbs (8 kinds × 4 growth stages), flavor plants, prey, and wildlife are all bound via hand-written `match` statements in `src/rendering/entity_sprites.rs`. There is no visual workspace for "what does each variant look like, side by side, with its source atlas visible." Iteration is currently: edit a match arm, `cargo run --release`, eyeball, repeat. Three "previously wrong" comments at `entity_sprites.rs:948,953,958` (Moonpetal / Calmroot / Dreamroot atlas indices) are the visible scar tissue of this loop. This ticket lands the substrate — a data-driven `bindings.toml` + loader + hot reload — so 449 can build the Svelte editor on top.

## Scope
- New module `src/rendering/sprite_bindings.rs` with `SpriteBindings` resource — one `HashMap<String, ItemBinding>` per category, keyed by `{:?}`-formatted enum variant.
- New file `assets/sprites/bindings.toml` extracted from the current match statements (semantics-preserving — phase 1 produces zero visual delta).
- Three lookup functions (`item_sprite_index`, `building_sprite`, `herb_sprite_index`) become 3-line wrappers reading from the resource. Missing variant at runtime is a hard panic with the variant name — no fallback.
- Bevy `AssetLoader` for the TOML so the asset participates in the existing hot-reload pipeline; a new system listens for `AssetEvent::Modified` and re-runs the attach pass on `Item` / `Structure` / `Herb` / `FlavorPlant` entities.
- Exhaustiveness unit test iterates every variant of each enum and asserts presence in the manifest (CLAUDE.md "Prefer compile-time contracts" applied to string-keyed TOML).

## Out of scope
- Svelte editor UI — tracked in 449.
- Sprite re-picking decisions (which sprite each variant should use) — separate aesthetic-iteration follow-on, scope-TBD.
- Sourcing new sprites (brushpile dens, fire-pit hearths, cairn watchtowers) if the current library doesn't contain better-fitting candidates — flagged as a creative gap; the substrate ships regardless.

## Current state
Nothing landed toward this. Asset paths in `src/rendering/sprite_assets.rs` are stable (loaded via `asset_server.load(path)`); the loader extension fits cleanly alongside them. The narrative-editor `.ron` template loader is the working precedent for a Bevy custom asset with hot reload.

## Approach
1. Define `SpriteBindings` resource + `BindingEntry` enum (atlas-index variant + standalone-PNG variant + multi-texture variant for buildings with per-type render size).
2. Implement `AssetLoader` for `*.toml` (Bevy 0.18 trait shape; serde-de via `toml` crate).
3. Extract the manifest content from the current three match statements verbatim — every atlas index and texture path. Mechanical; the exhaustiveness test catches drift.
4. Wrap the three lookup functions; the system signatures stay the same so callers in `attach_entity_sprites` etc. don't change.
5. Add `AssetEvent::Modified<SpriteBindings>` listener system; on event, re-run the attach pass for affected entity kinds.
6. Add round-trip unit test using `strum::IntoEnumIterator` (already a dependency) over each enum.

## Verification
- `just check && just test && just ci` green, including the new exhaustiveness test.
- `just run` renders the spawn colony visually identical to the pre-change baseline (capture screenshot pre- and post-, byte-diff-friendly via `BEVY_ASSET_ROOT=...`).
- Edit `bindings.toml` by hand while game runs (`echo` an alternate atlas index for a single variant), confirm the sprite swaps without restart within ~1 second.
- `just verdict logs/tuned-42/` after a fresh `just soak 42` — no continuity-canary or footer drift; rendering doesn't touch sim state but verification is cheap.

## Log
- 2026-05-22: opened. Plan at `/Users/will.mitchell/.claude/plans/i-want-to-do-elegant-cosmos.md`.
- 2026-05-22: Phase 1 (items + herbs + flavor plants) + Phase 1b (buildings + winter variants) + Phase 2 (hot reload via 0.5s mtime polling + EntitySpriteMarker stripping) landed in single commit alongside 449 Phases 3 + 4 + 5. `RenderingData` SystemParam introduced to keep `attach_entity_sprites` under the Bevy 16-param limit. Six exhaustiveness tests cover ItemKind / HerbKind / FlavorKind / StructureType (all variants) + winter-only-for-documented-structures. **Phase 1c (wildlife + prey) deferred** — animated-atlas schema needs richer per-species fields (atlas grid + frame_count + render_scale + optional tint + multi-variant for Fish/Bird). Picked up after the editor lands so the editor's atlas-cell UX is proven on the simpler schema first. Toml dep added; legacy `assets.den_textures` / `hearth_texture` / etc. fields in SpriteAssets are now orphaned (`pub` so dead_code lint doesn't fire) and get pruned in Phase 1c's commit alongside the wildlife/prey migration.
- 2026-05-22 polish: multi-atlas substrate landed. Manifest now has a top-level `[atlases.*]` table where every atlas is registered with its grid (cols × rows × tile). 10 atlases declared at land (items, items_dairy, items_all, herbs, furniture, chest, basic_plants, basic_grass_biom, trees, chars). `SpriteBindings` gains an `atlases: HashMap<String, AtlasHandles>` field; lookup methods now return `AtlasSprite { texture, layout, index }` instead of bare `usize`, so items / herbs / flavor plants can each reference any registered atlas — not just the hardcoded `items_texture` / `herbs_texture` pair. `every_referenced_atlas_is_declared` exhaustiveness test added (7 sprite_bindings tests total now). `SpriteAssets.items_texture` + `items_layout` + `herbs_texture` + `herbs_layout` removed (the bindings registry owns them). Editor side panel gains an atlas dropdown so users can repick an item's atlas in one click; index auto-resets to 0 on atlas swap. This unblocks the 8× expansion of available item sprites the user asked for (the existing items-atlas had ~77 unused cells AND there are now 7 alternative atlases — milk-items has 44 dairy sprites, basic-pack furniture/chest have additional prop options).
