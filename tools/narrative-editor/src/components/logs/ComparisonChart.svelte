<script lang="ts">
  import uPlot from 'uplot'
  import 'uplot/dist/uPlot.min.css'
  import { onDestroy } from 'svelte'
  import type {
    ColonyScoreEvent, FoodLevelEvent, PopulationSnapshotEvent, RunModel,
  } from '../../lib/logs/types'

  type ScalarMetric = 'welfare' | 'aggregate' | 'population' | 'food-stores'

  interface Props {
    runs: RunModel[]
    metric: ScalarMetric
    title: string
    yLabel: string
  }

  let { runs, metric, title, yLabel }: Props = $props()

  let container = $state<HTMLDivElement | null>(null)
  let chart: uPlot | null = null

  // Colors for up to 12 concurrent series; palette inspired by the app's
  // accent + warning + positive tones to avoid jarring reds.
  const SERIES_COLORS = [
    '#d4a574', '#7ec87e', '#74b4d4', '#d474b4', '#b4d474', '#74d4b4',
    '#e0b888', '#a0c8a0', '#a0a0d4', '#d4a0a0', '#d4d4a0', '#a0d4d4',
  ]

  // Season tints painted as a background behind the series in the
  // population view. Colors are intentionally low-alpha so they read as
  // bands, not as data. Order matches Season::from_tick in
  // src/resources/time.rs (Spring → Summer → Autumn → Winter).
  const SEASONS = [
    { name: 'Spring', fill: 'rgba(126, 200, 126, 0.10)', label: '#a8c8a0' },
    { name: 'Summer', fill: 'rgba(212, 196, 116, 0.10)', label: '#c8c078' },
    { name: 'Autumn', fill: 'rgba(212, 165, 116, 0.13)', label: '#c89860' },
    { name: 'Winter', fill: 'rgba(120, 150, 200, 0.10)', label: '#90a4c0' },
  ] as const

  /** Returns ticks_per_season iff every run agrees; null otherwise. Runs
   *  without a sim_config block are ignored (they predate the field). */
  function commonTicksPerSeason(runs: RunModel[]): number | null {
    const values = runs
      .map(r => r.header?.sim_config?.ticks_per_season)
      .filter((n): n is number => typeof n === 'number' && n > 0)
    if (values.length === 0) return null
    const first = values[0]
    return values.every(v => v === first) ? first : null
  }

  function extractSeries(run: RunModel): { xs: number[]; ys: (number | null)[] } {
    const xs: number[] = []
    const ys: (number | null)[] = []
    if (metric === 'welfare' || metric === 'aggregate') {
      for (const e of run.events) {
        if (e.type !== 'ColonyScore') continue
        const cs = e as ColonyScoreEvent
        if (cs.tick === undefined) continue
        const v = metric === 'welfare' ? cs.welfare : cs.aggregate
        if (v === undefined || v === null) continue
        xs.push(cs.tick)
        ys.push(v)
      }
    } else if (metric === 'food-stores') {
      for (const e of run.events) {
        if (e.type !== 'FoodLevel') continue
        const f = e as FoodLevelEvent
        if (f.tick === undefined) continue
        xs.push(f.tick)
        ys.push(f.current)
      }
    } else {
      for (const e of run.events) {
        if (e.type !== 'PopulationSnapshot') continue
        const ps = e as PopulationSnapshotEvent
        if (ps.tick === undefined) continue
        xs.push(ps.tick)
        ys.push(ps.mice + ps.rats + ps.rabbits + ps.fish + ps.birds)
      }
    }
    return { xs, ys }
  }

  function buildData(runs: RunModel[]): uPlot.AlignedData {
    const series = runs.map(extractSeries)
    const unionTicks = Array.from(
      new Set(series.flatMap(s => s.xs)),
    ).sort((a, b) => a - b)
    if (unionTicks.length === 0) return [[]] as unknown as uPlot.AlignedData
    const columns: (number | null)[][] = [unionTicks]
    for (const s of series) {
      const map = new Map<number, number>()
      for (let i = 0; i < s.xs.length; i++) map.set(s.xs[i], s.ys[i] as number)
      columns.push(unionTicks.map(t => map.get(t) ?? null))
    }
    return columns as unknown as uPlot.AlignedData
  }

  function shortLabel(run: RunModel): string {
    const h = run.header
    if (!h) return run.id.slice(0, 6)
    const commit = h.commit_hash_short ?? '—'
    const dirty = h.commit_dirty ? '*' : ''
    return `seed ${h.seed} · ${commit}${dirty}`
  }

  function destroyChart() {
    if (chart) {
      chart.destroy()
      chart = null
    }
  }

  function render() {
    destroyChart()
    if (!container || runs.length === 0) return
    const data = buildData(runs)
    if (data[0].length === 0) return
    const width = container.clientWidth || 640

    const ticksPerSeason = metric === 'population' ? commonTicksPerSeason(runs) : null
    const drawBands = buildSeasonBandHook(ticksPerSeason)

    const opts: uPlot.Options = {
      title,
      width,
      height: 260,
      scales: { x: { time: false } },
      axes: [
        { label: 'tick', stroke: '#8a8477' },
        { label: yLabel, stroke: '#8a8477' },
      ],
      series: [
        { label: 'tick' },
        ...runs.map((r, i) => ({
          label: shortLabel(r),
          stroke: SERIES_COLORS[i % SERIES_COLORS.length],
          width: 1.5,
          spanGaps: false,
        })),
      ],
      hooks: drawBands ? { drawClear: [drawBands] } : undefined,
    }
    chart = new uPlot(opts, data, container)
  }

  /** Builds a uPlot drawClear hook that paints Spring/Summer/Autumn/Winter
   *  bands across the visible x-range. Returns null when bands shouldn't
   *  render (no ticks_per_season available or runs disagree). */
  function buildSeasonBandHook(ticksPerSeason: number | null): ((u: uPlot) => void) | null {
    if (!ticksPerSeason || ticksPerSeason <= 0) return null
    return (u: uPlot) => {
      const xMin = u.scales.x.min
      const xMax = u.scales.x.max
      if (xMin === undefined || xMax === undefined || xMax <= xMin) return
      const ctx = u.ctx
      const plotTop = u.bbox.top
      const plotHeight = u.bbox.height
      const plotLeft = u.bbox.left
      const plotRight = plotLeft + u.bbox.width

      const firstSeason = Math.floor(xMin / ticksPerSeason)
      const lastSeason = Math.ceil(xMax / ticksPerSeason)

      // Too many seasons to label legibly — just paint bands.
      const labelEvery = Math.max(1, Math.ceil((lastSeason - firstSeason) / 16))

      ctx.save()
      for (let i = firstSeason; i < lastSeason; i++) {
        const seasonStart = i * ticksPerSeason
        const seasonEnd = (i + 1) * ticksPerSeason
        const drawStart = Math.max(seasonStart, xMin)
        const drawEnd = Math.min(seasonEnd, xMax)
        if (drawEnd <= drawStart) continue

        const x0 = Math.max(plotLeft, u.valToPos(drawStart, 'x', true))
        const x1 = Math.min(plotRight, u.valToPos(drawEnd, 'x', true))
        if (x1 <= x0) continue

        const season = SEASONS[((i % 4) + 4) % 4]
        ctx.fillStyle = season.fill
        ctx.fillRect(x0, plotTop, x1 - x0, plotHeight)

        // Label the season near the top of the band when there's room.
        if ((i - firstSeason) % labelEvery === 0 && x1 - x0 > 36) {
          ctx.fillStyle = season.label
          ctx.font = '10px system-ui, sans-serif'
          ctx.textBaseline = 'top'
          ctx.fillText(season.name, x0 + 4, plotTop + 4)
        }
      }
      ctx.restore()
    }
  }

  $effect(() => {
    // Re-render whenever inputs change.
    void runs; void metric; void title; void yLabel
    render()
  })

  $effect(() => {
    if (!container) return
    const el = container
    const ro = new ResizeObserver(() => {
      if (chart) chart.setSize({ width: el.clientWidth, height: 260 })
    })
    ro.observe(el)
    return () => ro.disconnect()
  })

  onDestroy(destroyChart)
</script>

<div bind:this={container} class="bg-surface border border-border rounded-md p-2 w-full min-h-[280px]"></div>

<style>
  /* uPlot's stylesheet assumes white backgrounds — nudge lines/text toward
     the app's palette without forking the CSS. */
  :global(.u-legend) { color: var(--color-muted); }
  :global(.u-title) { color: var(--color-accent); font-weight: 600; }
  :global(.u-axis) { color: var(--color-muted); }
  :global(.uplot) { background: transparent; }
</style>
