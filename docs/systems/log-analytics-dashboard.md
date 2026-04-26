# Log Analytics Dashboard

Web-based tool for browsing simulation logs interactively. Inspired by Dwarf Fortress's Legends Viewer.

## Motivation

The balance pass analysis required bespoke Python scripts to extract hunger trajectories, warmth decay rates, action distributions, and per-cat timelines from `logs/events.jsonl` and `logs/narrative.jsonl`. A persistent web dashboard would make this exploratory analysis repeatable and accessible without writing throwaway code each session.

## Inputs

- `logs/events.jsonl` — CatSnapshot, ColonyScore, FoodLevel, PopulationSnapshot, WildlifePopulation, WildlifePositions, PreyPositions, DenSnapshot, HuntingBeliefSnapshot, Death, SystemActivation, Ambush, Ward{Placed,Despawned}, ShadowFox{Spawn,Banished}, PreyKilled, KittenBorn, BuildingConstructed events.
- `logs/narrative.jsonl` — timestamped narrative text with tier classification.

### Event schema notes

- **Log headers carry a commit fingerprint.** Line 1 of both `events.jsonl` and `narrative.jsonl` is a `_header` object with `seed`, `duration_secs`, `commit_hash`, `commit_hash_short`, `commit_dirty`, and `commit_time` (ISO-8601 from `git show -s --format=%cI HEAD`). The `events.jsonl` header additionally carries `sim_config` (tick→day/season scaling), `map_width`/`map_height` (for the map overlay canvas — absent on pre-overlay logs), and the full `constants` dump. `sim_config` is what lets the dashboard map raw ticks to seasons; if you change `ticks_per_season` via tuning, the dashboard picks it up automatically. Values are emitted at compile time by `build.rs`; `commit_dirty: true` means the tree had uncommitted changes when the binary was built and the run is therefore not reproducible from the commit alone. The dashboard must surface dirty runs prominently and refuse to silently compare runs across differing `commit_hash` values without warning.
- **`SystemActivation` events carry three grouped hashmaps: `positive`, `negative`, `neutral`.** Each bucket includes *all* features in its category (count = 0 for features that haven't fired), so a single event is enough to tell "dead system" from "not yet observed." Feature classification lives in `src/resources/system_activation.rs` (`FeatureCategory` + `Feature::category()`) — 32 positive, 20 negative, 20 neutral as of schema v2.
- **`ColonyScore` events expose the split directly:** `positive_activation_score`, `positive_features_active`, `positive_features_total`, `negative_events_total`, `neutral_features_active`, `neutral_features_total`. The old flat `features_active/features_total` pair was removed — it mixed valences and gave a diagnostic that *rose* when the colony was suffering. Do not resurrect it in the dashboard.
- **`WildlifePopulation`** is emitted on the same cadence as `FoodLevel` / `PopulationSnapshot` (`economy_interval`, default 100 ticks) and counts live `WildAnimal` entities by species: `foxes`, `hawks`, `snakes`, `shadow_foxes`.
- **Spatial snapshots for the map overlay** — four additive event kinds, each gated by its own interval in `SnapshotConfig`:
  - `WildlifePositions { positions: [{species, x, y}, ...] }` — every `spatial_interval` ticks (default 500).
  - `PreyPositions { positions: [{species, x, y}, ...] }` — same cadence as wildlife.
  - `DenSnapshot { prey_dens: [...], fox_dens: [...] }` — every `den_snapshot_interval` ticks (default 1000). Dens move rarely.
  - `HuntingBeliefSnapshot { cat, width, height, values }` — colony aggregate, downsampled to at most 32×32 cells; every `hunting_belief_interval` ticks (default 1000). `cat: null` for colony aggregate; per-cat snapshots are reserved for a future extension (would 10× the log).
- Historical sweep comparisons may carry `score_schema_version` tags from the retired `score_track`/`score_diff` pipeline; dashboards should warn on cross-schema comparisons. The current statistical comparison surface is `just sweep-stats --vs`.

## Views (implemented)

- **Overview — Colony timeline:** toggle between welfare, aggregate, population total, food stores, prey-by-species (stacked area), predators-by-species (four lines), and colony-averaged Maslow (checkbox-selected subset of the 10 needs). Season bands appear on the scalar charts when all runs agree on `ticks_per_season`.
- **Cat detail:** picker-driven page with a 10-line Maslow needs chart (default four visible: hunger/energy/warmth/safety), a mood-valence line chart, and an action-distribution bar chart aggregated from `ActionChosen` events. Reachable via the `Cat detail` sub-tab.
- **Map overlay:** canvas with a tick slider and layer toggles (cats, prey, predators, dens, wards, ambushes, kills, deaths, hunting-belief heatmap). Wards are reconstructed by replaying `WardPlaced`/`WardDespawned` events. Event-dot layers (ambushes/kills) use a 500-tick trailing window that fades with age; deaths are persistent. Belief heatmap is the colony aggregate — per-cat is out of v1.
- **System activation panel:** three side-by-side columns (Positive / Negative / Neutral) with per-feature firing counts. Dead positives and neutrals are the system-liveness canary; dead negatives are the opposite (means nothing bad happened — surface that framing in the UI, not as a warning). Overlaying two runs diffs each category independently.
- **Comparison mode:** overlay two runs (e.g., pre/post tuning) to visualize balance changes. Schema-version guard for `SystemActivation` / `ColonyScore` shape mismatches; also surfaces mismatched constants, dirty commits, and missing headers.

## Views (planned)

- **Action heatmap:** cats × time, colored by current action — groom-lock and starvation spirals at a glance.
- **Death autopsy:** per-death final N snapshots with annotated need trajectories and the failure chain.
- **Per-cat hunting-belief heatmap toggle** on the cat detail page (requires enabling per-cat `HuntingBeliefSnapshot` emission).

## Tech

Lives as a new page inside the existing `tools/narrative-editor/` Svelte 5 + Vite app; shares its GitHub Pages deployment (`.github/workflows/pages.yml`). Reachable at `/clowder/#/logs` alongside `#/quiz` and `#/templates`.

- **Fully client-side.** No backend, no storage, no uploads-to-server. Users drag-and-drop (or pick) JSONL files; parsing happens in-tab via `File.stream()` → `TextDecoderStream` → line split → `JSON.parse`. Data never leaves the machine and is discarded on page unload.
- **Charting:** [uPlot](https://github.com/leeoniya/uPlot) — small (~45 KB), fast enough to render 100k+ points from a 15-min deep-soak without noticeable jank. Wrapped in a Svelte component.
- **No new dependency on the sim binary.** The dashboard reads only the JSONL outputs that `just headless` / `just soak` already produce.
- **Sibling CLI tools stay in place:** `just verdict <run-dir>` is the one-call gate, `just sweep-stats <dir> [--vs <baseline>]` is the per-metric statistical surface (with `--charts` for matplotlib output, replacing the retired `balance_report.py`), and `just check-canaries` / `just check-continuity` remain as primitives. The dashboard *complements* them for interactive exploration across many runs.
