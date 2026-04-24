<script lang="ts">
  import uPlot from 'uplot'
  import 'uplot/dist/uPlot.min.css'
  import { onDestroy } from 'svelte'
  import type { RunModel } from '../../lib/logs/types'
  import { preyBySpeciesSeries } from '../../lib/logs/metrics'

  interface Props {
    runs: RunModel[]
    height?: number
  }

  let { runs, height = 260 }: Props = $props()

  const SPECIES: { key: 'mice' | 'rats' | 'rabbits' | 'fish' | 'birds'; label: string; color: string }[] = [
    { key: 'mice',    label: 'mice',    color: '#d4a574' },
    { key: 'rats',    label: 'rats',    color: '#a0766c' },
    { key: 'rabbits', label: 'rabbits', color: '#e0b888' },
    { key: 'fish',    label: 'fish',    color: '#74b4d4' },
    { key: 'birds',   label: 'birds',   color: '#7ec87e' },
  ]

  let host = $state<HTMLDivElement | null>(null)
  let charts: uPlot[] = []

  function destroyCharts() {
    for (const c of charts) c.destroy()
    charts = []
  }

  function runLabel(run: RunModel): string {
    const h = run.header
    if (!h) return run.id.slice(0, 6)
    return `seed ${h.seed} · ${h.commit_hash_short ?? '—'}${h.commit_dirty ? '*' : ''}`
  }

  function render() {
    destroyCharts()
    if (!host) return
    host.innerHTML = ''
    if (runs.length === 0) return
    const width = host.clientWidth || 640

    for (const run of runs) {
      const panel = document.createElement('div')
      panel.style.marginBottom = '8px'
      host.appendChild(panel)

      const series = preyBySpeciesSeries(run.events)
      if (series.length === 0) continue

      // Convert per-species counts into a cumulative stack so uPlot's fill
      // mode paints bands rather than overlapping lines. Each layer is
      // species_i summed with all species below it.
      const xs = series.map(s => s.tick)
      const stackOrder = SPECIES.map(s => s.key)
      const cumulativeYs: number[][] = []
      let running = new Array(series.length).fill(0) as number[]
      for (const key of stackOrder) {
        const layer = series.map((s, i) => running[i] + (((s[key] ?? 0) as number)))
        cumulativeYs.push(layer)
        running = layer
      }

      const data: uPlot.AlignedData = [xs, ...cumulativeYs] as unknown as uPlot.AlignedData

      // Fill each band from the layer below to itself. Bands are 1-indexed
      // against the series index (series 0 = x-axis).
      const bands = stackOrder.map((_, i) => ({
        series: [i + 1, i === 0 ? -1 : i] as [number, number],
        fill: SPECIES[i].color + '80',
      }))

      const opts: uPlot.Options = {
        title: runs.length > 1 ? `Prey by species · ${runLabel(run)}` : 'Prey by species',
        width,
        height,
        scales: { x: { time: false } },
        axes: [
          { label: 'tick', stroke: '#8a8477' },
          { label: 'prey count', stroke: '#8a8477' },
        ],
        series: [
          { label: 'tick' },
          ...SPECIES.map(s => ({
            label: s.label,
            stroke: s.color,
            width: 1,
            fill: s.color + '60',
            spanGaps: true,
          })),
        ],
        bands,
      }
      charts.push(new uPlot(opts, data, panel))
    }
  }

  $effect(() => {
    void runs; void height
    render()
  })

  $effect(() => {
    if (!host) return
    const el = host
    const ro = new ResizeObserver(() => {
      const w = el.clientWidth
      for (const c of charts) c.setSize({ width: w, height })
    })
    ro.observe(el)
    return () => ro.disconnect()
  })

  onDestroy(destroyCharts)
</script>

<div bind:this={host} class="bg-surface border border-border rounded-md p-2 w-full"></div>

<style>
  :global(.u-legend) { color: var(--color-muted); }
  :global(.u-title) { color: var(--color-accent); font-weight: 600; }
  :global(.u-axis) { color: var(--color-muted); }
  :global(.uplot) { background: transparent; }
</style>
