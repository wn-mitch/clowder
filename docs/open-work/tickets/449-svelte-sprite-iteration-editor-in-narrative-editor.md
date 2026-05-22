---
id: 449
title: Svelte sprite-iteration editor in narrative-editor
status: blocked
cluster: rendering
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-22
parked: null
blocked-by: [448]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
With the data-driven sprite manifest from 448, we need a visual workspace to actually use it — picking sprites by editing TOML in a text editor is no improvement over editing match arms. The existing Svelte frontend at `tools/narrative-editor/` already hosts four iteration tools (templates / quiz / logs / trace) and is the natural home: hash-routed pages, file-drop pattern, pixel-art-aware tailwind. A 5th `#/sprites` page presents the gallery + repick UX + write-back to `bindings.toml`, while a 6th `#/catalog` page replaces and retires the standalone 1304-line `tools/sprite_catalog.html`.

## Scope
- New page `src/pages/SpriteEditor.svelte` at route `#/sprites`. Three-column gallery (items / buildings / herbs / flavor_plants / prey / wildlife), each cell showing variant name + current sprite cropped from its source atlas, `image-rendering: pixelated`.
- Click cell → side panel with full source atlas, grid overlay, current pick highlighted, hover-to-preview, click-to-repick.
- Save button POSTs the updated TOML to a Vite dev-server middleware endpoint (`vite.config.ts` adds `configureServer` with `/api/sprite-bindings` POST handler that writes to `assets/sprites/bindings.toml`).
- New page `src/pages/SpriteCatalog.svelte` at route `#/catalog` ports the atlas-tab UI from `tools/sprite_catalog.html` — same hover-highlight + named-cell behavior, tab per atlas.
- Shared `src/components/AtlasGrid.svelte` for the per-atlas grid display, used by both pages.
- `src/App.svelte` + `Nav.svelte` updated for the two new routes.
- `vite.config.ts` adds `server.fs.allow` for `../../assets/` so PNGs load relative-path; same middleware handles the POST.
- `tools/sprite_catalog.html` deleted in the same commit that lands `SpriteCatalog.svelte`.
- New `just sprite-editor` recipe pre-opens the `#/sprites` route.

## Out of scope
- Re-picking decisions (which sprite to use for each variant) — that's the *user* of this tool, not a deliverable.
- Sourcing new sprites — separate follow-on if the existing library is insufficient.
- Production build of the editor — it's a dev-only tool; no production server, no auth on the POST endpoint.

## Current state
Blocked by 448. The Svelte app's framework (Svelte 5 + Vite 8 + Tailwind 4 + TypeScript) is in place; adding a 5th and 6th hash route is structurally trivial. The atlas-tab patterns to port live in `tools/sprite_catalog.html` (1304 lines vanilla HTML/CSS/JS; lines 37 for pixel rendering CSS, mid-file for tab logic).

## Approach
Phase the work after 448 lands:
1. **Read-only browser** at `#/sprites`. Fetch `bindings.toml`, parse, render gallery. No write-back yet. Validates the schema + the gallery UX.
2. **Write-back + repick UI.** Vite middleware POST + side-panel atlas picker. Save round-trips to the running game via Bevy hot-reload from 448.
3. **Catalog port.** Reusable `AtlasGrid` extracted; `SpriteCatalog.svelte` rebuilds the standalone HTML's UX; delete the HTML in the same commit.

## Verification
- `just sprite-editor` opens the editor at `#/sprites`; gallery loads every variant matching what the game renders.
- Click-repick-save round-trip <1 second from save-button click to running-game sprite swap.
- `#/catalog` shows every atlas visible in the old HTML with hover-highlight + named-cell behavior preserved.
- `tools/sprite_catalog.html` no longer exists in the working tree.
- `just check && just test` green.

## Log
- 2026-05-22: opened, blocked-by 448. Plan at `/Users/will.mitchell/.claude/plans/i-want-to-do-elegant-cosmos.md`.
- 2026-05-22: Phase 3 (read-only browser at `#/sprites`) landed in same commit as 448 Phase 1+1b+2. `tools/narrative-editor/src/pages/SpriteEditor.svelte` fetches `bindings.toml` via a Vite dev-server middleware exposing the workspace `assets/` directory at `/assets/*`. Renders four categories (items, buildings summer+winter, herbs ×4 stages, flavor plants ×4 stages) with sprites cropped from source atlases at pixel-perfect scaling. `smol-toml` added as a dep. `just sprite-editor` recipe wired.
- 2026-05-22: Phase 4 (write-back + repick UX) landed. Vite middleware now exposes `POST /api/sprite-bindings` (atomic-rename write of body TOML to disk) and `GET /api/sprite-assets/png` (recursive PNG enumeration under `assets/`, 4245 paths). Editor adds a side panel with two repick modes: (a) atlas-cell picker via shared `AtlasGrid` component for items/herbs/flavor_plants — click any cell in the source atlas to repick, currently-bound cells outlined green, active selection thick accent ring; (b) PNG path picker for buildings — filterable list of all atlas PNGs with inline thumbnails, click to swap a variant's texture path. Save button serializes the in-memory `BindingsFile` back to TOML via `smol-toml.stringify` and POSTs; Bevy hot-reload (448 Phase 2) picks the file change up within ~0.5s. POST validated round-trip end-to-end via curl.
- 2026-05-22: Phase 5 (absorb `tools/sprite_catalog.html`) landed. New `tools/narrative-editor/src/pages/SpriteCatalog.svelte` at `#/catalog` ports the 4-atlas browser (items/herbs/trees/chars) with the same hover-highlight, named-cell outlines, and side info panel as the 1304-line standalone HTML. Reuses `AtlasGrid`. `tools/sprite_catalog.html` deleted; `docs/animations.md` reference updated to point at `#/catalog`. New `just sprite-catalog` recipe. Production build clean (297KB JS gzipped to 99KB). All 5 phases of 449 complete; ticket ready to land once 448 Phase 1c (wildlife/prey) ships and the substrate is fully migrated.
- 2026-05-22 polish: building previews unreadable at 80px width (especially WardPost 24×59 sliver). Fix: `buildingPreviewStyle` now fits into a maxSize × maxSize box preserving aspect ratio so every preview occupies similar visual area; bumped gallery preview to 128px and side-panel preview to 192px. Variants now lay out side-by-side instead of vertical stack and carry their texture filename as a sub-label. SpriteCatalog now reads `[atlases.*]` dynamically from `bindings.toml` — declaring a new atlas in TOML adds a tab here automatically, no code edit.
