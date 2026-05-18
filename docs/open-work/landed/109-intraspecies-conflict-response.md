---
id: 109
title: IntraspeciesConflictResponse — full four-valence (fight/flight/freeze/fawn) social response
status: done
cluster: ai-substrate
initiative: []
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 5561bb04
landed-on: 2026-05-07
---

## Why

The social analog of `AcuteHealthAdrenaline` (047). Predators don't accept appeasement; cats *do*. Intraspecies conflict (subordinate-vs-dominant context, mate competition, territorial dispute) has a full four-valence response repertoire — including **fawn** (the appeasement valence missing from predator response).

Reads a separate `social_status_distress` scalar (subordinate cat in dominant cat's space; mate competition with a stronger rival; territorial intrusion by a high-status cat). Distinct from physical body distress; distinct from predator threat.

## Scope

**Phase A** (this ticket): substrate scaffolding + Flee valence (subordinate retreat). Other valences are sub-tickets.

- New `social_status_distress` scalar in `interoception.rs`. Composition TBD — likely combines status differential vs nearest cat with proximity/intrusion factors.
- Publish via `ctx_scalars`.
- New `IntraspeciesConflictResponseFlight` modifier — lifts movement-away-from-dominant action (Flee or new "Withdraw" subaction; pick during impl).

**Phase B sub-tickets** (open during this work):
- `IntraspeciesConflictResponseFight` — territorial combat valence; lifts Fight against same-species rival.
- `IntraspeciesConflictResponseFreeze` — hold-position low-body-posture; reuses Hide/Freeze DSE from ticket 104.
- `IntraspeciesConflictResponseFawn` — belly-up, slow blink, appeasement gesture. **Requires new `Submit` gesture DSE** (or repurposes existing socialize-gesture machinery) — likely its own infrastructure ticket.

## Verification

- Phase A: focal-trace soak with subordinate cat near dominant cat shows withdrawal behavior over staying-put.
- Phase B: each sub-ticket gets its own focal-trace + hypothesize cycle.

## Out of scope

- Cross-species fawn (e.g. cat appeasing a fox) — ecologically incoherent; predator-response branches do not include fawn for that reason.
- Submit DSE infrastructure — likely opens as its own ticket alongside 109-Phase B.

## Log

- 2026-05-01: Opened as the social analog to ticket 047's AcuteHealthAdrenaline framework. Blocked by 104 (Hide/Freeze DSE) for the Freeze sub-valence; Phase A (Flee) can ship without it.
- 2026-05-02: 104 landed (2a68f595). **Phase A scaffolding landed** at ca140e5d — `IntraspeciesConflictResponseFlight` modifier registered (pipeline +1), 2 ScoringConstants, 6 unit tests. The `social_status_distress` scalar is published as a 0.0 stub from `ctx_scalars`; v1 composition `(status_diff_to_nearest_cat × proximity_factor)` requires a defensible status-differential signal + per-cat nearest-cat resolution, which lands alongside lift activation. Phase B sub-tickets opened: 142 (Freeze), 143 (Fight), 144 (Fawn), 145 (Submit gesture DSE infrastructure).
- 2026-05-07: **Phase A activated.** v1 signal selected per user prompt: composite `(respect_diff + age_diff + bond_asymmetry) × proximity_factor` over single-proxy options. Each arm is a single orthogonal social signal (`feedback_single_axis_perception_scalars` discipline); composing arms inside the perception layer means Phase B sub-tickets (142/143/144) inherit a richer scalar without re-touching the perception surface. **Composition** (in `src/systems/interoception.rs::social_status_distress`):  `respect_diff = clamp(other.respect - focal.respect, 0, 1)` · `age_diff = clamp((other.age_ticks - focal.age_ticks) / age_normalization_ticks, 0, 1)` · `bond_asymmetry = clamp(colony_avg_bond_to_other - focal.bond_to_other, 0, 1)` (averages `Relationship.fondness` over `relationships.all_for(other)` excluding focal) · multiplied by `proximity_factor = clamp(1 - distance / radius, 0, 1)`. Five new `ScoringConstants` knobs (`social_perception_radius` default 8 tiles · `social_status_distress_respect_weight` / `_age_weight` / `_bond_weight` defaults 1/3 each · `social_status_distress_age_normalization_ticks` default 1 sim-year). **Plumbing**: new `social_status_distress: f32` field on `ScoringContext` populated at both production builders (`evaluate_and_plan` in goap.rs:1613 + `evaluate_dispositions` in disposition.rs:921). `Option<&Age>` added to per-cat queries; `age_query` + `needs_query` SystemParam fields added to `WorldStateQueries` and `EvalDispositionSideEffects` for cross-cat lookups (read-only aliases of the per-cat iteration borrows; Bevy allows aliased read-only queries). **Lift**: `default_intraspecies_conflict_flight_lift` 0.0 → 0.30 (pressure-shape magnitude mirroring 106's hunger-urgency). `intraspecies_conflict_flight_default_inert` test renamed `*_default_active_lift` and updated to assert 0.50-base + 0.30 lift under saturated distress. 5 new unit tests for `social_status_distress` covering: alone case, far-away case, respect-arm lift, age-arm lift, range-clamping. 1920 lib tests pass; `just check` clean. Verification approach (per ticket §Verification): focal-trace soak with subordinate cat near dominant cat; `just scenario` not appropriate here because composite-distress is a steady-state signal that fires every tick under the right conditions (unlike 108's rising-derivative which requires multi-tick state changes). **Phase B sub-tickets (142/143/144/145) unblocked** — they consume the same `social_status_distress` scalar with different lift targets and viability gates.
- 2026-05-07: **Phase A verified — PASS.** Soak `logs/tuned-42` at HEAD `94b99910`/`5561bb04`, baseline `logs/tuned-42-pre-108-109` at `ee0dfb05` (post-119, pre-108/109). `just verdict` exit code 1 (`concern`); the concern bands are duration-confounded drift, not survival/canary regressions. **Hard gates pass:** `survival: pass`, `Starvation == 0` (down from 2 — surprising improvement on the inherited 204 cluster), `ShadowFoxAmbush == 1` (≤ 10), `never_fired_expected_positives == []`. **Continuity canaries** all ≥ 1 except `burial == 0` (inherited from 119, owned by ticket 204). **Macro deltas** (post / pre): `kittens_born` 2 / 3, `bonds_formed` 34 / 34, `peak_population` 9 / 9, `seasons_survived` 5 / 7, `colony_score.aggregate` 1960.69 / 1974.69 (-0.7%). **Per-rate (duration-normalized):** `negative_events_total` -36% per-rate, `acute_health_adrenaline_flee` interrupts 71594 → 20600 (-71% raw — 109A's preemptive Flee lift reduces how often 047's health-adrenaline interrupt fires; legitimate compositional effect). Continuity per-rate: `courtship` -8%, `grooming` +22%, `mentoring` +33%, `mythic-texture` ~0%. **Two follow-up flags surfaced** (neither blocks Phase A close-out): (1) `duration_drift_pct: 25.0%` — the binary processes ~25% fewer ticks per wallclock minute. Likely root cause: `social_status_distress` does an O(N) nearest-cat scan per cat per tick. Phase B (142/143/144) won't add perception cost — they're just additional readers on the same scalar — so the marginal cost of follow-on lifts is small, but the existing 25% baseline is worth optimizing. Opens as ticket 205. (2) The `Starvation: 2 → 0` improvement is unexpected and worth feeding back to ticket 204's investigation: 109A's Flee lift may be shifting bonding/courtship distribution enough to free adult-feeding bandwidth, OR the 25% perf-slowdown means kittens didn't have enough wallclock to enter the starvation decay window. The per-rate evidence (kittens_born 2 in 108127 ticks vs 3 in 144258 ticks ≈ same per-tick rate) suggests the population dynamics are similar, so it's likely a real shift, not a perf artifact. Phase B sub-tickets unblocked: 142 (Freeze) ready, 143 (Fight) ready, 144 (Fawn) still blocked by 145 (Submit DSE infra, which is ready but unlanded). 109 graduates to `done`, file moves to `landed/`.
