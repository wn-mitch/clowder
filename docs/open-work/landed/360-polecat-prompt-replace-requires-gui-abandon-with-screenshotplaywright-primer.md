---
id: 360
title: Polecat prompt — replace requires-gui abandon with screenshot/Playwright primer
status: done
cluster: process-discipline
orchestration: swarm-safe
initiative: []
added: 2026-05-15
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: pending
landed-on: 2026-05-15
---

## Why

`scripts/foreman.sh:178-196` instructs polecats to self-abandon at prompt-read
time on any ticket whose verification "needs a windowed GUI, an overlay toggle,
a screenshot diff, an egui-based viewer, or a visualization widget" — printing
`polecat-abandoned: <slug> requires-gui` and exiting before any code is touched.
On 2026-05-15 this caused polecats on tickets 208 (per-cat Action overlay) and
259 (L1→L3 activation viz) to throw away both their workspace and the headless
plumbing work that was within their capability.

The framing is wrong on two levels:

1. **Most UI tickets split cleanly.** The plumbing portion (event payload,
   query plumbing, hit-test math, anchor constants, color tokens) is squarely
   headless-verifiable via `just check && just test`. Only the render
   correctness needs visual inspection.
2. **The render correctness is itself automatable.** Bevy already wires
   `Screenshot::primary_window()` + `save_to_disk` in
   [`src/rendering/camera.rs:344`](../../../src/rendering/camera.rs) and exposes
   an `AutoScreenshot` resource (line 414) that captures at fixed ticks. The
   log viewer at `tools/narrative-editor/` is a vite/npm web app — Playwright
   is the natural verification tool.

The framing conflates "renders pixels" with "needs human eyes." Pixels can be
asserted programmatically. The operator's role on UI work should be reviewing
the saved screenshot in the PR — not doing the verification the polecat
abdicated.

## Scope

- Edit `scripts/foreman.sh:178-196` "Verifiability triage" section:
  - Remove the unconditional `requires-gui` abandon for "windowed sim / overlay
    toggle / screenshot diff / egui viewer / visualization widget" tickets.
  - Add a verification-toolchain primer naming:
    - Bevy windowed work → `Screenshot::primary_window()` +
      `save_to_disk("/tmp/clowder_screenshot_<tick>.png")` + pixel/OCR
      assertion. Spawn via headed binary, scrub to known tick, capture, exit.
    - `tools/narrative-editor` work → start `npm run dev` on a fixed port,
      drive with Playwright (`@playwright/test` — add as dev-dep if missing),
      assert against DOM and canvas pixel.
  - Reserve `requires-gui` abandons for cases where verification genuinely
    requires subjective aesthetic judgment (e.g., "the color feels right"),
    not for any ticket whose title mentions a UI surface.

## Out of scope

- Rewriting the `requires-long-soak` / `requires-substrate-judgment` abandon
  paths. Same scrutiny likely applies but the user named GUI specifically.
- Adding Playwright as a permanent dependency to `tools/narrative-editor/`.
  The polecat prompt can `npm install --save-dev @playwright/test` ad-hoc.
- Writing reference screenshot fixtures for 208 / 259. That's the polecat's
  job once this ticket lands.

## Current state

- 2026-05-15: doctrine bug surfaced when foreman ran 3 polecats; 2/3
  abandoned at prompt-read on `requires-gui`. User flagged the abandons as
  premature. Memory entry: `feedback_polecat_no_gui_abandon`.

## Approach

Single-file edit to `scripts/foreman.sh`. The polecat prompt is a heredoc
inside the script; rewrite the "Verifiability triage" section. No code
changes outside that script.

## Verification

- `just check` passes (no script-lint issues).
- Re-spawn polecats on 208 and 259 — they should now attempt the work and
  push real commits rather than abandon at prompt-read.

## Log
- 2026-05-15: opened — see precedent in foreman dispatch that same day.
