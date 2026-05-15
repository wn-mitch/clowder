# Focal-cat Trace Viewer

Interactive browser tool for inspecting the L1 → L2 → L3 activation pipeline
for a single cat at a single tick. Shortens per-tick triage from composing
`just q trace` + `just q cat-timeline` manually to a point-and-click scrubber.

## Quick start

```bash
# 1. Generate a trace sidecar (15-min soak with Simba as focal cat)
just soak-trace 42 Simba

# 2. Open the viewer
just trace
# Opens http://localhost:5173/#/trace in the browser.

# 3. Drop files
# Drag logs/tuned-42/trace-Simba.jsonl onto the drop zone.
# Optionally also drop logs/tuned-42/events.jsonl to keep run metadata.
```

The viewer parses files client-side (browser Streams API, zero upload). No
server-side data leaves the machine.

## Layout

Three panels arranged horizontally:

```
┌─────────────────┬─────────────────────────────┬────────────────────┐
│  L1 · Perception│  L2 · DSE scoring           │  L3 · Decision     │
│                 │                             │                    │
│  per-map scent  │  one card per eligible DSE  │  ranked bar chart  │
│  bars + history │  sorted by final_score      │  + chosen action   │
│  sparkline      │  expand → full pipeline     │  + GOAP plan       │
└─────────────────┴─────────────────────────────┴────────────────────┘
```

Above the panels: a uPlot timeline strip showing all DSE L2 `final_score`
series across the whole run. Click or drag to scrub to any tick.

## Interpreting the panels

### L1 · Perception

Each card is one influence map sampled at the focal cat's current position.

- **base** bar: raw signal at the cat's tile before attenuation.
- **perceived** bar (accent colour): final signal after all attenuation
  factors. This is the value the DSE considerations receive.
- **Attenuation** section: four multiplicative factors
  (`species_sens × role_mod × (1 − injury_deficit) × env_mul`) that
  convert `base` → `perceived`. A factor of 1.0 means no effect.
- **Recent** sparkline: `perceived` over the last 40 decision ticks so you
  can see whether the signal is rising, falling, or stable.
- **Top contributors**: emitters (other cats, prey, predators) ranked by
  how much signal they contribute to the sample.

When you expand an L2 DSE card, any L1 maps that card reads via its
`spatial` considerations are outlined in accent — the L1 → L2 signal
path is highlighted for you.

### L2 · DSE scoring

One card per DSE that passed eligibility at this tick, sorted by
`final_score` descending. The chosen DSE (from L3) is marked `★` and
outlined in accent.

Click any card to expand the full scoring pipeline:

- **Considerations**: per-axis input (muted bar) and post-curve score
  (accent bar). The curve label shortens Rust debug output
  (`Logistic { steepness: 8.0, midpoint: 0.75 }` → `Logistic(8, 0.75)`).
  Weight multiplier shown on the right.
- **Pipeline chain**: `raw → maslow× → modifier → final`. Positive
  modifiers appear in green, negative in red.
- **Intention**: Goal (with target + goal_state) or Activity.
- **Targets**: for multi-candidate DSEs, the ranked candidate list and
  the winner.
- **Top losing axes**: considerations that scored lowest and dragged the
  DSE down.

Ineligible DSEs (marker requirements not met) appear greyed-out in a
collapsed row at the bottom.

### L3 · Decision

- **Ranked dispositions**: horizontal bar chart of L2 `final_score` values,
  normalised to the highest score. Softmax probability shown on the right
  (temperature in the header).
- **Chosen**: the winning disposition after softmax sampling + momentum.
- **Intention**: the resolved goal or activity for the chosen DSE.
- **GOAP plan**: the step sequence the planner will execute.
- **Momentum**: current commitment strength and whether a preemption
  occurred.
- **Commitment gate**: appears when the gate fired this tick — branch
  outcome (achieved / unachievable / dropped_goal / retained) and whether
  the commitment was dropped.
- **Plan failure**: appears when the plan ended outside the commitment gate
  (replan cap or anxiety interrupt).

## Tick navigation

| Action | Keyboard | Button |
|--------|----------|--------|
| Step ±1 decision tick | ← / → | ‹ › |
| Step ±10 decision ticks | Shift+← / Shift+→ | « » |
| Jump to prev/next action change | `[` / `]` | [‹ change] / [change ›] |
| Jump to specific tick | — | type tick number, press Enter |

**Action changes** (`[`/`]` buttons) skip directly to ticks where the
winning disposition changed — the fastest way to find decision-point
boundaries.

The timeline strip marks:
- Amber dashes: action-change ticks (chosen disposition changed).
- Teal rules: commitment-gate ticks.
- Red rules: plan-failure ticks.
- Bright yellow rule: current focal tick.

## Cross-checking with `just q`

To verify that the viewer's L3 winner matches the CLI:

```bash
just q trace logs/tuned-42 Simba --layer=L3 --tick-range=<tick>..<tick+1>
```

The `chosen` field in the L3 record should match the "Chosen:" label in
the L3 panel. If it doesn't, the trace file parsed a different tick than
expected — check the tick input field in the toolbar.

## Generating traces for specific scenarios

```bash
# Focal cat Nala, seed 7, standard 15-min soak
just soak-trace 7 Nala

# Multiple focal cats require separate soak-trace runs —
# each produces its own trace-<name>.jsonl sidecar.
just soak-trace 42 Simba
just soak-trace 42 Nala

# Drop both into the viewer; the run picker in the toolbar lets you switch.
```

## Ticket 163 note

Nine post-L2 bonus modifier layers in `goap.rs::evaluate_and_plan` mutate
per-Action scores after L2 emits. These are not yet recorded in the trace
sidecar (tracked in ticket 163). The L2 `final_score` shown here is the
pre-bonus value. When 163 lands, a "post-L2 modifier delta" row will
appear in the L3 panel showing the aggregate bonus contribution.
