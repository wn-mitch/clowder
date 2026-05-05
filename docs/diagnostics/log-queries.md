# Log queries — diagnostic jq recipes

Canonical source for jq recipes that probe `logs/*/events.jsonl` and
`logs/*/narrative.jsonl`. Runtime entry points live in `justfile`
(`just check-canaries`, `just diff-constants`) and call into the named
queries below. The doc is the source of truth; the scripts are the
runtime.

Each block is headed `### Query: <name>` so tooling can grep them.
Every query ends with a one-line shape note describing the output.

Default logfile locations:
- `logs/events.jsonl` — structured events (seed header, per-event JSON, footer)
- `logs/narrative.jsonl` — human-facing narrative lines with tiers
- `logs/trace-<focal>.jsonl` — focal-cat per-tick L1/L2/L3 trace records (headless only, emitted when `--focal-cat NAME` is supplied). See §11.
- `logs/tuned-<seed>/...` — convention for tuning-run outputs. `just soak-trace` also writes `logs/tuned-<seed>/trace-<focal>.jsonl` in this tree.

---

## 1. Quickstart

### Query: footer
End-of-run summary: deaths_by_cause, feature totals, ward counts.

```bash
jq -c 'select(._footer)' logs/events.jsonl
```

Shape: one JSON object. Keys include `deaths_by_cause`, `wards_placed_total`,
`wards_despawned_total`, `ward_count_final`, `anxiety_interrupt_total`,
`plan_failures_by_reason`, `interrupts_by_reason`, `shadow_fox_spawn_total`,
`ward_siege_started_total`.

### Query: header
Seed + constants snapshot. Two runs are only comparable if their
headers match on `constants`.

```bash
jq -c 'select(._header)' logs/events.jsonl
```

Shape: one JSON object with `seed`, `duration_secs`, `constants`.

---

## 2. Header & constants

### Query: constants
Extract only the tuning constants.

```bash
jq -c 'select(._header) | .constants' logs/events.jsonl
```

Shape: one JSON object with every SimConstants field.

### Query: diff-constants
Diff tuning between two runs. Any diff here explains outcome differences.

```bash
diff <(jq -c 'select(._header) | .constants' BASE_LOG) \
     <(jq -c 'select(._header) | .constants' NEW_LOG)
```

Shape: unified-diff output. Empty diff means the runs are comparable.

---

## 3. Canaries

The four canaries from `CLAUDE.md`. `just check-canaries LOGFILE` runs
all of them and exits non-zero on any failure.

### Query: canary-starvation
Zero starvation deaths is the target on seed 42.

```bash
jq -c 'select(._footer) | .deaths_by_cause.Starvation // 0' logs/events.jsonl
```

Shape: integer (expected 0).

### Query: canary-shadowfox
Target: ≤ 5 shadowfox ambush deaths on a 15-min seed-42 soak.

```bash
jq -c 'select(._footer) | .deaths_by_cause.ShadowFoxAmbush // 0' logs/events.jsonl
```

Shape: integer.

### Query: canary-activation-zeros
Features that activated 0 times — likely dead systems. Compare to a
known-good baseline footer to spot regressions.

```bash
jq -c 'select(._footer) | .features_activated | to_entries | map(select(.value == 0)) | map(.key)' logs/events.jsonl
```

Shape: array of feature names.

### Query: canary-wipeout
`stderr` from the runner prints `All cats dead at tick N. Ending early.`
if the colony wiped. Inspect the footer's final tick to confirm.

```bash
jq -c 'select(._footer) | .final_tick' logs/events.jsonl
```

Shape: integer. Expect ≥ `duration_secs * ticks_per_second` when the
colony survived the full soak.

---

## 4. Deaths

### Query: deaths-by-cause
Every death grouped by cause.

```bash
jq -r 'select(.type=="Death") | .cause' logs/events.jsonl | sort | uniq -c | sort -rn
```

Shape: `count cause` lines.

### Query: deaths-by-cat
Which cats died, with tick and cause.

```bash
jq -c 'select(.type=="Death") | {tick, cat, cause, injury_source}' logs/events.jsonl
```

Shape: one JSON object per death.

### Query: deaths-by-location
Cluster deaths by tile to spot dangerous zones.

```bash
jq -c 'select(.type=="Death") | .location' logs/events.jsonl | sort | uniq -c | sort -rn
```

Shape: `count (x,y)` lines.

### Query: wipeout-forensics
Last 20 events before the final tick — useful when the colony wipes.

```bash
jq -c '.' logs/events.jsonl | grep -v '"_header"\|"_footer"' | tail -20
```

Shape: 20 JSON lines.

---

## 4b. Hunt success (per-discrete-attempt)

Ticket 149 — `EventKind::HuntAttempt` is emitted at every outcome
resolution of `resolve_engage_prey` (kill, lost, abandoned). Lets the
per-discrete-attempt success rate be computed against the 30–50% real-
cat-biology target without conflating per-Hunt-action retargeting.

The skill-surface wrapper is `just q hunt-success <run-dir>` (with
optional `--cat`, `--species`, `--tick-range`); the recipes below are
the underlying jq formulas.

### Query: hunt-success-rate
Overall per-discrete-attempt success rate. The three `Killed*` outcomes
all count toward the kill numerator; everything else is a loss.

```bash
jq -r 'select(.type=="HuntAttempt") | .outcome' logs/events.jsonl \
  | awk '
      /^killed/ { kills++ }
      { total++ }
      END {
        if (total > 0) printf "%.2f%% (%d kills / %d attempts)\n",
          100*kills/total, kills, total
        else print "no HuntAttempt events"
      }'
```

Shape: one summary line. Compare to the 30–50% band.

### Query: hunt-outcomes-by-count
Distribution across all 7 outcome variants.

```bash
jq -r 'select(.type=="HuntAttempt") | .outcome' logs/events.jsonl \
  | sort | uniq -c | sort -rn
```

Shape: `count outcome` lines.

### Query: hunt-failure-reasons
Among losses, which `StepResult::Fail` reason dominates? Distinguishes
"prey-targeting is sub-optimal" from "spook/timeout overhead too high".

```bash
jq -r 'select(.type=="HuntAttempt" and .failure_reason != null)
       | .failure_reason' logs/events.jsonl \
  | sort | uniq -c | sort -rn
```

Shape: `count reason` lines.

### Query: hunt-success-by-cat
Per-cat breakdown — useful for spotting one cat with markedly worse
hit-rate (e.g., a high-anxiety cat spooking prey at the stalk phase).

```bash
jq -c 'select(.type=="HuntAttempt") | {cat, outcome}' logs/events.jsonl \
  | jq -s 'group_by(.cat) | map({
      cat: .[0].cat,
      total: length,
      kills: ([.[] | select(.outcome | startswith("killed"))] | length)
    } | .success_rate_pct = (100 * .kills / .total))'
```

Shape: JSON array of `{cat, total, kills, success_rate_pct}` rows.

### Query: hunt-success-by-species
Per-prey-species breakdown — birds/fish/mice/rabbits often have very
different catch difficulties. Surfaces ecology imbalance.

```bash
jq -c 'select(.type=="HuntAttempt") | {prey_species, outcome}' \
       logs/events.jsonl \
  | jq -s 'group_by(.prey_species) | map({
      prey_species: .[0].prey_species,
      total: length,
      kills: ([.[] | select(.outcome | startswith("killed"))] | length)
    } | .success_rate_pct = (100 * .kills / .total))'
```

Shape: JSON array of per-species rows.

---

## 5. Wards

### Query: wards-placed
All ward placements with kind and location.

```bash
jq -c 'select(.type=="WardPlaced") | {tick, ward_kind, location}' logs/events.jsonl
```

Shape: one JSON object per placement.

### Query: wards-despawned-sieged-ratio
Sieged vs. natural despawn count. Sieged ratio falling is the signal
that the siege-wave retune is working.

```bash
jq -c 'select(.type=="WardDespawned") | {sieged, ward_kind}' logs/events.jsonl \
  | sort | uniq -c | sort -rn
```

Shape: `count {sieged, ward_kind}` lines.

### Query: ward-lifespan
Per-ward lifetime in ticks by pairing WardPlaced and WardDespawned on
(location, ward_kind). Approximate — the first unused pair wins if a
location is reused. Good enough for soak-level diagnostics.

```bash
jq -c 'select(.type=="WardPlaced" or .type=="WardDespawned") | {tick, type, location, ward_kind}' logs/events.jsonl \
  | jq -s 'group_by([.location, .ward_kind]) | map(select(length >= 2) | {location:.[0].location, ward_kind:.[0].ward_kind, lifespan:(.[1].tick - .[0].tick), sieged:(.[1].sieged // false)})'
```

Shape: array of `{location, ward_kind, lifespan, sieged}`. Target
post-retune: sieged-ward lifespans ≥ 500 ticks (was ~250).

---

## 6. Posses & banishments

### Query: banishments-by-posse
Who banished which fox, in what party.

```bash
jq -c 'select(.type=="ShadowFoxBanished") | {tick, posse, location}' logs/events.jsonl
```

Shape: one JSON object per banishment.

### Query: banishments-per-cat
Count banishments per participant. After the hero-skew cap, no single
cat should dominate.

```bash
jq -r 'select(.type=="ShadowFoxBanished") | .posse[]' logs/events.jsonl \
  | sort | uniq -c | sort -rn
```

Shape: `count cat_name` lines. Target post-cap: top cat ≤ 5, top-2
within 2× of top-1.

### Query: posse-candidate-excluded-starving
Fires every time dispatch_urgent_directives skipped a starving cat for
a Fight directive. Zero means the path never triggered; non-zero means
the starvation guard did its job.

```bash
jq -c 'select(.type=="FeatureActivated" and .feature=="PosseCandidateExcludedStarving")' logs/events.jsonl \
  | wc -l
```

Shape: integer.

---

## 7. Features

### Query: feature-totals
Every feature activation count. Use for dead-system detection.

```bash
jq -r 'select(.type=="FeatureActivated") | .feature' logs/events.jsonl \
  | sort | uniq -c | sort -rn
```

Shape: `count feature` lines.

### Query: feature-diff
Compare feature totals between two runs.

```bash
diff <(jq -r 'select(.type=="FeatureActivated") | .feature' BASE_LOG | sort | uniq -c) \
     <(jq -r 'select(.type=="FeatureActivated") | .feature' NEW_LOG | sort | uniq -c)
```

Shape: unified diff of `count feature` lines.

---

## 8. Narrative tier

### Query: legend-entries
All Legend-tier narrative lines. Post-template-wire-in, these should
rotate between 4 variants from `banishment.ron`.

```bash
jq -c 'select(.tier=="Legend")' logs/narrative.jsonl
```

Shape: one JSON object per Legend line. `.text` is the rendered string.

### Query: legend-variety
Distinct Legend-tier strings in the run. If only one variant fires,
either the registry isn't loaded or the context is over-constrained.

```bash
jq -r 'select(.tier=="Legend") | .text' logs/narrative.jsonl \
  | sort -u | wc -l
```

Shape: integer. Target: ≥ 2 distinct strings when ≥ 2 banishments
occurred (variant selection is weighted-random).

### Query: narrative-tier-totals
Count lines per tier.

```bash
jq -r '.tier' logs/narrative.jsonl | sort | uniq -c | sort -rn
```

Shape: `count tier` lines.

---

## 9. Plan failures

### Query: plan-failures-by-reason
Grouped failure reasons. Spikes in one reason often point at a broken
action resolver.

```bash
jq -r 'select(.type=="PlanStepFailed") | .reason' logs/events.jsonl \
  | sort | uniq -c | sort -rn
```

Shape: `count reason` lines.

### Query: plan-failures-by-cat
Per-cat failure counts.

```bash
jq -r 'select(.type=="PlanStepFailed") | .cat' logs/events.jsonl \
  | sort | uniq -c | sort -rn
```

Shape: `count cat_name` lines.

---

## 10. Session-specific (2026-04 polish pass)

Queries added with the starvation / hero-skew / template / siege retune
pass. Keep them here so subsequent runs can re-verify those behaviors.

### Query: session-starvation-filter-fires
Same as `posse-candidate-excluded-starving` under section 6 — pointer.

### Query: session-banishment-spread
Banishment counts per cat AND the implied accumulated skill gain under
the soft-cap formula
(`gain = 0.25 / (1 + N * 0.25)` for the Nth prior banishment).

```bash
jq -r 'select(.type=="ShadowFoxBanished") | .posse[]' logs/events.jsonl \
  | sort | uniq -c | sort -rn \
  | awk '{ total = 0; for (i = 0; i < $1; i++) { total += 0.25 / (1 + i * 0.25); }; printf "%3d  %s  skill_gain=%.2f\n", $1, $2, total }'
```

Shape: `count cat_name skill_gain=V` lines. Target: no cat exceeds
~1.1 implied skill gain (vs. 1.75 under the old linear formula).

### Query: session-siege-wave-shape
Average lifespan of sieged vs. natural ward deaths. Post-retune, sieged
wards should average ≥ 500 ticks.

```bash
jq -c 'select(.type=="WardPlaced" or .type=="WardDespawned") | {tick, type, location, ward_kind, sieged}' logs/events.jsonl \
  | jq -s 'group_by([.location, .ward_kind]) | map(select(length >= 2) | {lifespan:(.[1].tick - .[0].tick), sieged:(.[1].sieged // false)}) | group_by(.sieged) | map({sieged:.[0].sieged, count:length, avg_lifespan:(map(.lifespan) | add / length)})'
```

Shape: array of `{sieged, count, avg_lifespan}`.

### Query: session-banishment-template-variety
Distinct Legend-tier strings from the banishment.ron pool.

```bash
jq -r 'select(.tier=="Legend") | .text' logs/narrative.jsonl \
  | sort -u
```

Shape: one text per line. Target: ≥ 2 distinct lines on a run with ≥ 2
banishments.

---

## 11. Focal-cat trace queries

Queries over `logs/trace-<focal>.jsonl` — the headless-only per-tick L1/L2/L3
sidecar emitted when `just soak-trace SEED FOCAL_CAT` (or `--headless
--focal-cat NAME`) is run. See `CLAUDE.md` §"Focal-cat trace" and
`docs/systems/ai-substrate-refactor.md` §11 for record shapes. The header line
shares commit / seed / constants fields with `events.jsonl` so the two files
diff-lock as a pair.

All non-header records carry `{tick, cat, layer, …variant fields}`; `layer` is
one of `"L1"`, `"L2"`, `"L3"`.

### Query: trace-header
Seed, focal-cat name, and constants snapshot for a trace file. Must match the
paired `events.jsonl` header on `constants` + `commit_hash` for the two to be
comparable.

```bash
jq -c 'select(._header)' logs/tuned-42/trace-Simba.jsonl
```

Shape: one JSON object with `focal_cat`, `seed`, `duration_secs`,
`commit_hash`, `sim_config`, `constants`.

### Query: trace-l3-chosen
Per-tick `(tick, chosen_dse, intention_kind)` — the winning-action timeline
for the focal cat.

```bash
jq -c 'select(.layer=="L3") | {tick, chosen, kind: .intention.kind}' \
  logs/tuned-42/trace-Simba.jsonl
```

Shape: one JSON object per tick.

### Query: trace-l3-ranked-top3
Top-3 ranked DSEs per tick — useful for spotting close-call ticks where the
softmax almost flipped the pick.

```bash
jq -c 'select(.layer=="L3") | {tick, top3: (.ranked[0:3])}' \
  logs/tuned-42/trace-Simba.jsonl
```

Shape: one JSON per tick with `top3` as an array of `[dse_name, score]`
2-tuples.

### Query: trace-dse-trajectory
`(tick, final_score)` series for one DSE across the run. Useful for plotting a
single DSE's score curve over time — drift, flat lines, or phase shifts show
up immediately.

```bash
jq -c 'select(.layer=="L2" and .dse=="hunt") | {tick, score: .final_score}' \
  logs/tuned-42/trace-Simba.jsonl
```

Shape: one JSON object per tick. Replace `hunt` with any DSE id.

### Query: trace-l2-considerations
Per-consideration breakdown for one DSE on one tick — the "why did Hunt beat
Eat" decomposition. Use `trace-l3-chosen` to pick the tick first.

```bash
jq -c 'select(.layer=="L2" and .dse=="hunt" and .tick==8432) |
       {tick, final: .final_score, considerations}' \
  logs/tuned-42/trace-Simba.jsonl
```

Shape: one JSON with `final` score + `considerations` array of `{name, input,
curve, score, weight, spatial?}`.

### Query: trace-l1-map-samples
All L1 samples for one influence map, across the run. The `top_contributors`
array answers "which emitter drove the perceived value."

```bash
jq -c 'select(.layer=="L1" and .map=="fox_scent") |
       {tick, pos, perceived, top: (.top_contributors[0:3])}' \
  logs/tuned-42/trace-Simba.jsonl
```

Shape: one JSON per sample. Replace `fox_scent` with any map id (see
`src/resources/trace_log.rs` `TraceRecord::L1.map`).

### Query: trace-frame-at-tick
All three layers for a single tick — full frame reconstruction. Mirrors what
`scripts/replay_frame.py` does; useful for ad-hoc sanity checks.

```bash
jq -c 'select(.tick==8432)' logs/tuned-42/trace-Simba.jsonl
```

Shape: ordered JSON lines (L1 samples, then L2 rows, then one L3).

### Query: trace-softmax-temperature
Per-tick softmax temperature from L3 records. Flat temperature across the run
suggests the momentum/commitment system isn't varying — expected at Phase 1
entry (shim emits constants); Phase 3+ should show variation.

```bash
jq -c 'select(.layer=="L3") | {tick, temp: .softmax.temperature,
       committed: .momentum.active_intention, preempted: .momentum.preempted}' \
  logs/tuned-42/trace-Simba.jsonl
```

Shape: one JSON per tick.

### Query: trace-eligibility-misses
DSEs that were evaluated but failed their marker-eligibility gate. Non-empty
output on a cat you expected to be *fully* eligible means you're on the wrong
focal cat for the behavior you're investigating — re-read the "Picking a
focal cat" note in `CLAUDE.md` §Focal-cat trace.

```bash
jq -c 'select(.layer=="L2" and .eligibility.passed==false) |
       {tick, dse, markers: .eligibility.markers_required}' \
  logs/tuned-42/trace-Simba.jsonl \
  | head -20
```

Shape: one JSON per failed eligibility check, first 20 rows.

### Query: trace-dse-catalog
Which DSEs appear in the trace at all — the set of behaviors this focal cat
actually exercised. Gaps here compared to the full catalog drive your choice
of next focal cat for multi-focal coverage.

```bash
jq -r 'select(.layer=="L2") | .dse' logs/tuned-42/trace-Simba.jsonl \
  | sort -u
```

Shape: sorted unique list of DSE ids. Compare across `trace-*.jsonl` files to
see which focal covers which slice of the catalog.
