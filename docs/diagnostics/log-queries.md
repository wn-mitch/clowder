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
- `logs/tuned-<seed>/...` — convention for tuning-run outputs

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
