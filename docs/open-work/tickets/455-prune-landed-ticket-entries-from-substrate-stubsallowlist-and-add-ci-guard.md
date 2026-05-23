---
id: 455
title: Prune landed-ticket entries from substrate_stubs.allowlist and add CI guard
status: ready
cluster: process-discipline
initiative: []
orchestration: swarm-safe
added: 2026-05-23
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

`scripts/substrate_stubs.allowlist` carries entries pointing at tickets that have already landed. The convention at lines 11–13 of the allowlist is explicit: "When the work lands, drop the entry; the lint will then enforce that the substrate stays wired." But the convention is enforced by author discipline at land-time, not by CI — and the convention has drifted at least three times. A CI guard closes the regression vector, matching the bidirectional shape that `scripts/check_method_registry.sh` already enforces for dormant HTN methods (ticket 319 precedent).

## Scope

Two pieces — one corrective edit, one structural CI add:

1. **Prune the verified-stale entries** from `scripts/substrate_stubs.allowlist`:
   - Line 39: `resolve_vigil 332` — ticket 332 (`grief-vigil-action-vocabulary`) is landed; its scope explicitly named `resolve_vigil` step-resolver wiring as deliverable.
   - Line 40: `resolve_grief_sit 332` — same; landed with 332.
   - Line 23: `HasMaterialsInInventory 235-follow-on` — ticket 235 (`smart-deposit-routing-for-clutter-clearance`) landed; the real follow-on is ticket 421 (`central-material-pile-smart-material-deposit-routing`). Either update the cite to `421` (if the marker is still dormant pending 421) or remove the entry (if the marker is now wired). Implementer verifies marker status against `src/components/markers.rs` and the writer in `src/systems/items.rs::update_inventory_markers` before deciding which path applies.

2. **Add CI guard** — `scripts/check_substrate_stubs.sh` (or a small companion script) verifies that every ticket-id cited in `scripts/substrate_stubs.allowlist` resolves to a file in `docs/open-work/tickets/<id>-*.md` and **not** in `docs/open-work/landed/`. Wired into `just check`. Same bidirectional-resolution shape as `check_method_registry.sh` lines 14–17.

## Out of scope

- Auditing the substrate stubs themselves. The allowlist is data; this ticket fixes the data and the guard around it. Whether each stub is still load-bearing is a separate audit (catalogue lives at `docs/open-work/pre-existing/substrate-stub-catalogue.md`).
- Extending the same staleness check to `scripts/influence_map_registry.allowlist` or other allowlist files. If they have the same drift risk, that's a follow-on ticket — open one per allowlist so the guards stay readable.
- Refactoring the parser convention. Format stays as-is (entry · ticket-id · comment).

## Current state

The allowlist ships with these ticket references (verified against `docs/open-work/{tickets,landed}/` 2026-05-23):

| Allowlist line | Entry | Ticket cited | Status | Action |
|---|---|---|---|---|
| 18 | `STUB:src/ai/planner/actions.rs:592:trashing` | 200 | open | keep |
| 23 | `HasMaterialsInInventory` | 235-follow-on | 235 landed; 421 is the real follow-on | retag → 421 OR remove (verify marker writer) |
| 24 | `HasCuriosInInventory` | 16 | open | keep |
| 36 | `resolve_wear_item` | 334 | open | keep |
| 37 | `resolve_craft` | 334 | open | keep |
| 38 | `resolve_petition_coordinator` | 334 | open | keep |
| 39 | `resolve_vigil` | 332 | **landed** | **remove** |
| 40 | `resolve_grief_sit` | 332 | **landed** | **remove** |

Comment lines 41–43 already document that 364's allowlist entries were correctly removed at land-time, demonstrating the convention is workable when followed — but also that there's no enforcement when it's missed.

## Approach

Single-commit shape:

1. Edit `scripts/substrate_stubs.allowlist`: remove lines 39–40; either retag line 23 to `421` or remove (per the verification step above).
2. Add the staleness check to `scripts/check_substrate_stubs.sh` (preferred — keeps allowlist enforcement in one script). Iterates the allowlist, extracts the ticket-id token, asserts each resolves to `docs/open-work/tickets/<id>-*.md` and does not resolve to `docs/open-work/landed/<id>-*.md`. Reuses the same shell idiom as `check_method_registry.sh` Pass A.
3. `just check` should fail on a synthetic mutation (temporarily add a landed-ticket entry) and pass on the real allowlist post-prune. Implementer demonstrates both in the commit message.

### Structural-option menu

- **edit-in-place (chosen)** — extend `check_substrate_stubs.sh` with a third audit (allowlist staleness). One script, one allowlist, one check.
- **separate script (rejected)** — adding `check_allowlist_freshness.sh` (or similar) is over-decomposed for ~20 lines of bash; the parallel risk for `influence_map_registry.allowlist` belongs in its own ticket if it surfaces.
- **frontmatter-driven** (rejected) — having each ticket carry `wires-allowlist-entry: [<name>...]` frontmatter is the bidirectional shape `check_method_registry.sh` uses, but is overkill for allowlists where most entries don't have a one-to-one wiring story. Defer unless the drift recurs.

## Verification

- `just check && just test` green before commit.
- Synthetic mutation: temporarily append `_FAKE_MARKER 332` (or any landed ticket id) to the allowlist; `just check` MUST fail with a specific error naming the landed ticket. Revert.
- `just check` green on the real, pruned allowlist.
- No new lines added to any other allowlist; no other CI script touched.

## Log

- 2026-05-23: opened from session audit (the "is this project vibe-coded" health pass). Audit subagent originally cited four landed tickets (364/367/443/450) as stale entries; verification proved those four were correctly retired at land-time per the comment at lines 41–43, but found three actually-stale references (lines 23/39/40) the agent missed. The structural fix (CI guard) is the load-bearing piece; the prune is housekeeping.
