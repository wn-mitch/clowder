---
id: 028
title: observer registration regression + unified headless/windowed pipeline
status: done
cluster: null
also-landed: [30]
landed-at: null
landed-on: 2026-04-25
---

# observer registration regression + unified headless/windowed pipeline

**Landed:** 2026-04-25 | **Tickets:** 028 (Headless build silently drops 4 personality-event observer cascades), 030 (Unify headless and windowed build pipeline — kill the manual mirror).

Two tickets landed together as a six-phase refactor that fixes the
silent `play` continuity canary and retires the
`build_schedule`/`setup_world` manual-mirror architecture flagged in
CLAUDE.md as a recurring failure mode.

**Hypothesis (028):** Registering the four `register_observers`
cascades in headless and pushing `EventKind::PlayFired` from
`on_play_initiated` would take `continuity_tallies.play` from 0 to a
reliable non-zero value across every soak.

**Hypothesis (030):** Replacing the manual `setup_world` /
`build_schedule` mirror with a single `App + SimulationPlugin +
HeadlessIoPlugin` pipeline would eliminate the dormant-mirror
regression class entirely, with no behavior change beyond the 028 fix.

**Concordance:**
- Phase A soak (`logs/tuned-42-phase-A/`):
  `continuity_tallies.play` = 19 (was 0). `never_fired_expected`
  10 → 8 (`BondFormed`, `Socialized` now fire because the play
  observer cascade runs). All four hard survival canaries pass.
- Phase D soak (`logs/tuned-42-phase-D/`, App + plugins pipeline):
  starvation 0, shadowfox 3, footer written, play 721, courtship
  2955, mythic-texture 35. Survival canaries hold; continuity
  tallies improved (4/6 pass vs. baseline 1/6).

**Architecture (post-030):**
- `SimulationPlugin::build()` is the sole authoritative registration
  site for messages, observers, FixedUpdate systems, and world setup
  (`setup_world_exclusive` Startup system). DSE catalog is
  `populate_dse_registry()` called by `register_dses_at_startup`
  Startup system reading live `SimConstants`.
- `HeadlessIoPlugin` (new, `src/plugins/headless_io.rs`) owns
  `HeadlessConfig`, JSONL writer resources, header writes,
  per-tick flush, tick-budget exit, and end-of-run footer emission.
- `run_headless` is now ~110 lines: build App with
  `MinimalPlugins + SimulationPlugin + HeadlessIoPlugin`, set
  `TimeUpdateStrategy::ManualDuration` for deterministic ticks, run
  `app.update()` until `should_exit()`, emit footer, autosave.
- Deleted from `src/main.rs`: `build_schedule`, legacy `setup_world`,
  legacy `build_new_world`, `flush_new_entries` /
  `flush_event_entries` / `flush_trace_entries`,
  `build_headless_footer`, `load_log_file`,
  `sensory_env_multipliers_snapshot`, duplicate `load_templates` /
  `load_zodiac_data` / `load_aspiration_data`. ~1000 lines removed.
- CLAUDE.md "Headless Mode", "Simulation Verification", and "AI
  Substrate Refactor / DSE registration sites" sections rewritten to
  drop manual-mirror caveats and update the DSE-registration-sites
  count from three to one.

**Audit (028 Fix 3, no code change):** The other three observer
cascades (`on_temper_flared`, `on_directive_refused`,
`on_pride_crisis`) emit narrative + relationship/mood mutations only.
None target a continuity-canary class today. Leaving them as-is is
intentional; if a future canary class wants to track temper outbursts
or directive refusals, those events would be added at that time.

**028 Fix 4 (regression-guard test) intentionally not added** —
ticket 030's structural fix supersedes the need (per 028's
"Note on durability"). The interim `register_observers_world(&mut World)`
sibling fn added in Phase A was deleted in Phase D once the App
pipeline subsumed it.

**Phase landing order:**
- Phase A: `register_observers_world` + `EventKind::PlayFired` push.
- Phase B: `setup_world_exclusive` moves into `SimulationPlugin`.
- Phase C: `HeadlessIoPlugin` scaffolding (additive).
- Phase D: `run_headless` rewritten to use App pipeline; legacy
  `setup_world` / `build_new_world` / message-registry block / DSE
  block deleted; `populate_dse_registry` + `register_dses_at_startup`
  introduced as the single DSE-registration site.
- Phase E: `build_schedule` deleted; CLAUDE.md updated.
- Phase F: tickets 028 + 030 landed; this entry.
