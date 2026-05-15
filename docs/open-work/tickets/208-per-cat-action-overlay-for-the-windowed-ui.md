---
id: 208
title: Per-cat Action overlay for the windowed UI
status: ready
cluster: tooling-diagnostics-ui
orchestration: swarm-safe
initiative: []
added: 2026-05-07
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

A windowed-sim observation on 2026-05-07 reported "cats stand around the
town center while food stores read 0/50". Diagnostic drill of
`logs/tuned-42` (today's seed-42 soak, same commit) showed the AI was
actually doing the opposite: in the cold-start window (ticks
1,200,000–1,205,000) cats spent ~57% of action time on food work
(Cook 19.8% · PickUp 19.5% · Forage 11.8% · Hunt 6.3%) and ~25% on
GroomOther / Coordinate. Wander was 1.5%, Socialize 0.25%. Simba's
focal trace put PickUp at 81% of L2 wins — he was hauling the
35 ground-spawn food items into stores.

The windowed UI doesn't surface any of that. Every cat renders with the
same idle sprite (atlas index 0 in `entity_sprites.rs:202`); the
`Action` enum is only visible by clicking a cat one-by-one, which opens
the inspector panel (`cat_inspect.rs:257`). There is no per-cat label,
no debug overlay, no colony-level activity tally, no F-key toggle for
AI state. The result: a player observing the colony cannot tell
Cook from Wander from Idle without click-walking every cat.

This is a *legibility* gap, not an AI defect. It's also a diagnostics
gap — devs verifying behaviour have to either click cats individually or
drop into headless logs.

## Scope

- **F-key debug overlay** (default off) that, when enabled, draws a small
  text label above each cat sprite showing the current `Action` enum
  variant — e.g. `Cook`, `PickUp`, `GroomOther`, `Wander`. Toggleable
  per-session; off-by-default to keep the default screen clean.
- **Bind to a free F-key.** F4/F5/F6/F7/F8 are taken (grid, camera
  follow, terrain overlays). Default to **F9** unless an existing
  binding conflicts.
- **Inspector parity** — the overlay reads the same `Action` data
  `cat_inspect.rs:257` already renders, so there is one source of truth.
- **Toggle hint** — extend the bottom status-bar keyboard hints
  (`status_bar.rs:74`) to mention the new key.

## Out of scope

The following adjacent improvements are **not** part of this ticket but
are durably named here so they don't rot into conversation memory. Each
should be opened as a follow-on ticket if and when prioritised:

- **Per-Action sprite states / animations** — Cook pose, Sleep pose,
  GroomOther pose, etc. Closes the gap properly long-term but is real
  pixel-art + animation work; lives downstream of art budget. Open as
  a separate ticket blocked-by art-direction decisions.
- **Colony-level activity tally in the HUD** — "5 cooking · 3 hunting".
  Adds an aggregate widget to the bottom status bar or right panel.
  Open as a separate ticket if/when player-legibility scope expands.
- **Speech-bubble / emote system** — in-fiction signal of intent
  (different from a debug overlay). Adjacent to ticket 126 (BDI
  intention substrate, in-fiction perceivable commitment).
- **DSE-score / plan-target / influence-map overlays** — the
  diagnostician's wishlist. Useful for substrate-refactor work but a
  full feature in their own right; open as a tooling ticket.

## Current state

- AI behaviour is healthy in cold-start (verified via
  `logs/tuned-42` seed=42 commit=9573dc8d, action distribution
  + Simba focal trace).
- Inspector panel already renders `Action` via `Debug` formatting at
  `cat_inspect.rs:257`. The data is present, just not surfaced
  passively.
- Sprite atlas index is hardcoded to 0 at `entity_sprites.rs:202`; cats
  carry no `AnimationTimer`. Per-Action sprite differentiation is a
  separate, larger problem.
- No existing ticket addresses player-facing AI legibility. Closest
  neighbours (126 BDI intention substrate, 135 continuous-position
  migration) are different concerns.

## Approach

Add a Bevy system that, when the overlay resource is enabled, spawns
or updates a `Text2d` (or equivalent) child entity above each cat
showing `format!("{:?}", action)`. Mirrors the inspector path
(`cat_inspect.rs:257`) so there is one source of truth on what string
to render.

- **Resource:** new `ActionOverlayEnabled(bool)` resource, default
  `false`, toggled by an input system on the chosen F-key.
- **Render system:** queries `(&CurrentAction, &Transform)` and either
  spawns or updates a child label entity. Despawn labels when the
  resource flips to `false`.
- **Z-ordering:** label at sprite z + 1 to draw above sprite, below
  any UI overlay layer.
- **Performance:** O(N) per tick over cats; with current colony sizes
  this is negligible. Use change detection (`Changed<CurrentAction>`)
  to avoid re-allocating the text on every tick when the action is
  unchanged.

Files likely touched (read-only references — do not pre-edit):

- `src/rendering/ui/cat_inspect.rs:257` — source of truth for action
  formatting.
- `src/rendering/entity_sprites.rs` — sprite construction; the overlay
  child can be attached here or in a dedicated module.
- `src/rendering/ui/status_bar.rs:74` — extend keyboard-hint string.
- `src/rendering/camera.rs` — F-key binding precedent (F5).
- `src/rendering/debug_grid.rs:105` — F4-key binding precedent.

## Verification

- **Manual.** Launch `just run` (or whatever windowed entry-point).
  Press the overlay F-key. Confirm every cat now shows their
  `Action` text label, and the labels update as cats switch actions.
  Confirm pressing the key again removes the labels cleanly.
- **Cross-check.** Click an arbitrary cat to open the inspector. The
  inspector's `Action: ...` line MUST match the overlay text for that
  cat at the same instant — proving the two paths read the same data.
- **Cold-start sanity.** Walk the first ~30 seconds of windowed
  gameplay with the overlay on. The visible action mix should
  approximate the headless cold-start distribution from
  `logs/tuned-42`: a lot of `PickUp` and `Cook`, some `Forage` /
  `Hunt`, occasional `GroomOther` / `Coordinate`, almost no
  `Wander`. If the visible mix is dominated by `Wander` or no-action,
  there is an additional bug (open a separate ticket).
- **No regression.** `just check` + `just test`. Headless soak
  unchanged (overlay is render-only; no sim-side mutation).

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-08 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **121** (done, substrate-over-override, score 0.88 (cross-cluster)) — Cats stand around for ~1500 ticks at game start until first kitchen lands
- · **  1** (in-progress, —, score 0.87) — Explore dominance over targeted leisure
- ✓ landed ** 49** (done, —, score 0.87) — §9.2 faction overlay markers

<!-- linkages:end -->
## Log
- 2026-05-07: opened. Triggered by windowed observation that surfaced a
  legibility gap, not an AI defect — diagnosed against
  `logs/tuned-42`. Out-of-scope items recorded here so they don't rot
  out of session memory; open follow-ons if/when prioritised.
