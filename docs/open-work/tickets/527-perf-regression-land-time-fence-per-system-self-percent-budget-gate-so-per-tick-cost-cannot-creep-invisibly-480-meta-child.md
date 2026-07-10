---
id: 527
title: Perf-regression land-time fence — per-system self-percent budget gate so per-tick cost cannot creep invisibly (480 meta-child)
status: ready
cluster: tooling-diagnostics-ui
orchestration: substrate-sensitive
initiative: []
added: 2026-07-09
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
[480](480-sim-per-tick-throughput-regression-63-p90-decline-197-72-tickssec-over-five-weeks-flamegraph-bisect-reclaim.md)
exists because per-tick cost crept **63% over five weeks (p90 197 → 72
ticks/sec) with nobody noticing** until a throughput chart was built after the
fact. The regression is "death by knives" — each feature landing added a per-tick
frame (`track_sustained_copresence`, `integrate_beliefs` Pass B, crafting
aspirations, the belief integrator) that individually looked cheap and collectively
halved throughput. 480 is the *curative* epic: flamegraph-bisect and reclaim, one
child per knife (485/486/500/504/505/506). This ticket is the *preventive* half —
the reason 480 "likely never closes" is that there is no gate stopping the next
knife from landing. Ticket [431](../landed/431-hot-frame-catalog-per-tick-vs-event-driven-audit-of-top-10-cpu-consumers.md)
put a "bench harness / CI perf gate" explicitly **out of scope** ("different
ticket; this is local-diagnosis-driven"). Now that 480 has a whole tail of
children proving the creep is structural and not a one-off, the cost/benefit has
flipped: a land-time fence is the highest-leverage, lowest-risk perf move available
because it touches measurement only, never sim behavior. Framed against the perf
state-machine: entering `RECOMPUTE-PER-TICK` is trivial (it's the default Bevy
system shape) while leaving it costs a whole ticket + determinism gate — a sticky
entry with an expensive exit, so the state fills up silently. This fence prices the
entry.

## Scope
- A repeatable **per-system self-% profile** captured from a short fixed-seed
  headless run (samply against the `profiling` profile — the `just flamegraph`
  path from [430](../landed/430-add-just-flamegraph-recipe-macos-dtrace-setup-doc.md)
  already produces the JSON + symbol sidecar this reads).
- A committed **budget file** (`docs/diagnostics/perf-budgets.json` or similar)
  recording an allowed self-% / inclusive-% ceiling per top-N hot system, seeded
  from the current post-506 HEAD profile.
- A **`just perf-fence`** recipe that runs the short profile and exits non-zero when
  any budgeted system exceeds its ceiling by more than a tolerance band, or when a
  *new* system appears in the top-N without a budget entry (the "unbudgeted knife"
  case — this is the one that caught us in 480).
- A **narrator/report line** naming which system breached and by how much, so the
  failure is actionable at land-time rather than a bare exit code.
- Wire it into the discipline docs that name when to run it (per memory
  `feedback_diagnostic_tools_need_discipline_wiring` — the workflow doc that says
  "run the fence before landing a per-tick system" ships in the same PR as the
  recipe).

## Out of scope
- **Fixing** any breach the fence surfaces — that's a 480-child, not this ticket.
  This ticket only *detects*.
- **Wall-clock ticks/sec gating** as the primary signal — too noisy under
  parallel-session CPU contention (480 uses p90 precisely to control for it).
  Self-% of a fixed-sample profile is contention-invariant and is the gate; a
  ticks/sec trend can be a secondary advisory line.
- **A hosted CI runner.** macOS-local `just perf-fence` first; a GitHub-Actions
  wiring can follow once the local gate is trusted (macOS profiling needs full
  Xcode / dtrace entitlements — see 430's recipe notes).
- The **budget-market scheduler** (priority tiers, deferral) — that is the
  *allocation* idea from the atlas Economic page, tracked separately; this ticket
  is monitoring only.

## Current state
Opened 2026-07-09 from an `/ideate` pass over "our historical perf approaches."
Both the state-machine and atlas draws independently flagged that every landed fix
is *curative* and the `REGRESSION-FENCED` state was never entered — this ticket
enters it. Precedent instruments already exist: `just flamegraph` (430), the
`soak-throughput-over-time` logdb chart (480's measurement child), and baseline
profiles under `docs/diagnostics/baseline-profiles/`. Nothing built yet.

## Approach
1. Reuse `just flamegraph 42 30` (30 s is enough for a stable self-% ranking on
   the hot systems; perf soaks are short per memory `feedback_perf_soaks_short_runs`)
   to produce `profile.json.gz` + `.syms.json`.
2. `scripts/profiling/samply_top.py` already ranks frames — extend it (or add a
   sibling) to emit a machine-readable top-N self-% / inclusive-% table.
3. Seed `perf-budgets.json` from the current HEAD (post-506) profile with a
   tolerance band (start generous, e.g. +25% relative, tighten later).
4. `just perf-fence` diffs a fresh profile against the budget file: breach → exit 2
   with the offending rows; unbudgeted new top-N system → exit 2 with a "new knife,
   add a budget entry (and consider whether it should be event-driven — see
   `project_per_tick_discipline_default_event_driven`)" message.
5. Determinism note: the profile is a *sampling* artifact — self-% will jitter run
   to run. The tolerance band absorbs sampling noise; do NOT gate on exact
   percentages. Budgets are updated deliberately (a PR that intentionally moves a
   ceiling is a reviewable event — that visibility is the point).

## Verification
- `just perf-fence` passes at HEAD (budgets seeded from HEAD).
- Synthetic breach: temporarily inflate a budgeted system's ceiling downward (or
  add a `std::hint::black_box` busy-loop behind a debug flag) and confirm the fence
  exits non-zero naming that system.
- Delete a system's budget entry and confirm the "unbudgeted top-N system" path
  fires.
- `just check && just test`.
- Discipline-wiring check: the workflow doc naming when to run the fence is updated
  in the same PR (grep the doc for `perf-fence`).

## Log
- 2026-07-09: opened from `/ideate` (state-machine + atlas draws). Preventive
  counterpart to the curative 480 epic; revives the fence 431 deferred. Priced #1
  in both draws — highest leverage / lowest determinism risk (measurement only,
  never touches sim behavior).
