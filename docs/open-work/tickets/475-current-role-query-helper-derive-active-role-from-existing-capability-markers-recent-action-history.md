---
id: 475
title: Current-role query helper (derive active role from existing capability markers + recent action history)
status: ready
cluster: social-coordination
orchestration: substrate-sensitive
initiative: [welfare-fidelity]
added: 2026-05-26
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

[[474]]'s warder-succession detector needs to ask "is this cat *currently* fulfilling the warder role?" — distinct from "is this cat *capable* of warding?" The capability markers `IsHerbalist` / `IsSpiritualist` (landed [[173]]) + `CanWardFromSupply` / `CanCook` / `CanCleanse` answer the second question; nothing answers the first. A cat with `CanWardFromSupply` who has never warded shouldn't trigger succession-on-death; a cat with `CanWardFromSupply` who has warded at the same site 5 times in the last 10 seasons IS the active warder whose absence creates a vacancy.

Per user reframe 2026-05-26: *"none of these are explicit but all easily slot into the substrate."* No new component, no new marker. The active-role query is a derived helper over the existing capability markers + the event log (`WardPlaced`, `Cook`, `MagicCleanse`, etc. action events queryable by `cat` field). Once authored, [[474]]'s succession detector + narrative-tier templates ("the warder Heron fell to corruption") + future role-shaped queries all share the helper.

## Hot context

In `logs/tuned-42-01eb555d`, Heron placed durable wards at (35,60) on ticks 1228797 / 1228806 — that's his second + third successful WardPlaced events (he'd warded earlier in the run too). Bramble has magic skill 1.06 but never cast MagicCleanse on a colony tile during the run — she's capable but not active in the shaman role. Without `current_role`, the succession detector at [[474]] can't tell "the warder died" apart from "any CanWardFromSupply cat died" — and the second triggers false-positive succession on cats who never warded.

This is the **helper-layer** of the kin-care cluster: [[472]] (foundation), [[473]] (perception), [[474]] (consumer of this helper), [[475]] (this ticket).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Capability markers | landed [[173]] `IsHerbalist / IsSpiritualist / HasCorruptionNearby capability markers`; landed 084 `CanWardFromSupply`; landed 014 batch `CanCook`, `CanCleanse`, etc. | Eligibility markers exist for every role; they answer "can this cat" not "is this cat currently." | `[verified-correct]` |
| Action event log | `src/resources/event_log.rs` action events (`WardPlaced`, `Cook`, `MagicCleanse`, etc., all keyed by `cat`) | The event stream carries action-history per cat; queryable with tick-range. | `[verified-correct]` |
| Cat aspirations | `src/components/aspiration.rs` (or similar — `Provider of the Colony`, `Master of the Hunt`, `Shadow Fighter`, etc.) | Self-identified domain commitments; complementary signal to action-history. A cat with `Master of the Hunt` aspiration self-identifies as a hunter even if they haven't hunted recently. | `[verified-correct]` |
| Existing "recent X" patterns | landed [[219]] `shared recent-ambush event marker` | Precedent for "recent event from log" queryable substrate. Same shape applies to "recent WardPlaced" / "recent MagicCleanse." | `[verified-correct]` |
| Missing: current_role helper | (new) | No existing function answers "is this cat currently the warder / shaman / forager?" The closest is per-DSE eligibility filters, which gate on capability not activity. | `[verified-defect-shape]` |

## Fix candidates

**Parameter-level options**:
- R1 — [[474]] reads markers directly: `count(CanWardFromSupply ∩ ¬is_festering ∩ ¬Incapacitated)`. Loses the "actually performed the role" distinction; triggers succession on cats who never warded.

**Structural options**:

- R2 (**split**) — **Recommended.** Author one query helper `current_role(cat: Entity, events: &EventLog, markers: &MarkerSnapshot, aspirations: &Aspirations, tick: u64) -> Option<RoleKind>` returning the cat's effectively-current role based on (a) which `Can*` / `Is*` markers they carry, (b) which actions of that role they've performed in the last N seasons (configurable `current_role_window_seasons`), (c) which aspiration domain they self-identify with. `RoleKind` is a small enum: `Warder`, `Shaman`, `Forager`, `Cook`, `Hunter`, `Coordinator`. Helper module probably under `src/ai/roles.rs` or `src/resources/roles.rs`. No new component, no new marker, no new resource — just a function operating on existing state. [[474]]'s succession detector reads it; narrative-tier templates read it.
- R3 (**extend**) — Add a `CurrentRole { kind, since_tick }` component authored by a per-tick system. Heavier-weight; introduces new state that can drift from the underlying markers + events. Violates the user's "slots into existing substrate" principle.
- R4 (**rebind**) — Roles are entirely inferred from aspirations: `CurrentRole = aspiration_domain(cat)`. Loses the action-history evidence; a cat with `Provider of the Colony` aspiration but who never hunts is still labeled "hunter."
- R5 (**retire**) — Skip the helper, accept [[474]]'s false positives. Loses the narrative texture (the colony recognizes specific cats as specific roles).

## Recommended direction

**R2 (split)** — pure query helper, zero new state. Combines three existing signals (markers + action history + aspirations) into one queryable function. The combinator weighting is configurable per role.

Landing approach:
1. Author the `RoleKind` enum + the `current_role` function.
2. Add a `current_role_window_seasons` config knob (default ~3-5 seasons; tunable).
3. Add unit tests covering: capability-but-no-action (returns None for that role); recent-action-plus-marker (returns the role); stale-action-only (returns None after window); aspiration-only-no-action (returns None for that role, since aspiration without practice doesn't count as "currently filling").
4. Refactor any existing role-shaped queries to use the helper.

## Out of scope

- The colony demand signals that consume this helper ([[474]]).
- Role-specific narrative templates — opens as a follow-on when [[474]]'s succession events start firing.
- Persistent `RoleKind` on a cat's biography / lineage record — separate substrate; this ticket is the query helper, not biography.
- Multi-role cats (cat is both Cook and Shaman) — the helper returns the primary role; multi-role handling can compose at the caller.

## Verification

- `just check && just test` clean. Unit tests cover the role-derivation cases above.
- Composition: [[474]]'s succession detector uses this helper for its warder-count query. Verify via scenario test in [[474]]'s scope.
- Behavior-neutral at land: no caller reads the helper yet; the function exists for [[474]] to consume.

## Related work

<!-- linkages:start -->
- · **472** (ready, combat-threat) — Festering wound substrate (BLOCKER — sets the cluster precondition for needing this helper).
- · **474** (blocked, social-coordination) — Warder succession + shaman dispatch (PRIMARY CONSUMER of this helper).
- ✓ landed **173** (done, ai-substrate) — IsHerbalist / IsSpiritualist capability markers (the role markers this helper reads).
- ✓ landed **219** (done, ai-substrate) — Shared recent-ambush event marker (precedent for "recent event from log" pattern).
- ✓ landed **049** (done, ai-substrate) — §9.2 faction overlay markers (sibling marker substrate).
- · **289** (ready, combat-threat) — EngageThreat retry cooldown (sibling "recent-event-driven gate" pattern).
- · **470** (ready, belief-perception) — Ward-siege fear influence map (cluster sibling — perception-before).
- · **471** (ready, combat-threat) — Damage events to log (cluster sibling — telemetry-during).
- · **473** (blocked, belief-perception) — Corrupted-kin signal map (cluster sibling).
<!-- linkages:end -->

## Log

- 2026-05-26: opened from seed-42 soak `logs/tuned-42-01eb555d` kin-care cluster. Scope reduced from "new CurrentRole component" to "pure query helper" after `just similar` surfaced landed [[173]] (role-capability markers already exist) — the substrate is there, just needs the active-role derivation. Blocked-by [[472]] for the cluster activation precondition. Primary consumer is [[474]].
