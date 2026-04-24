<script lang="ts">
  import uPlot from 'uplot'
  import 'uplot/dist/uPlot.min.css'
  import { onDestroy } from 'svelte'
  import type { RunModel } from '../../lib/logs/types'

  export interface SeriesDef {
    label: string
    color: string
  }

  interface Props {
    runs: RunModel[]
    /** One row per series definition. Order drives color assignment. */
    seriesDefs: SeriesDef[]
    /** Extracts the x-axis + one row per seriesDefs entry for a given run.
     *  Missing samples should be reported as null (uPlot tolerates gaps). */
    extract: (run: RunModel) => { xs: number[]; ys: (number | null)[][] }
    title: string
    yLabel: string
    height?: number
  }

  let { runs, seriesDefs, extract, title, yLabel, height = 260 }: Props = $props()

  let host = $state<HTMLDivElement | null>(null)
  let charts: uPlot[] = []

  function destroyCharts() {
    for (const c of charts) c.destroy()
    charts = []
  }

  function runLabel(run: RunModel): string {
    const h = run.header
    if (!h) return run.id.slice(0, 6)
    const commit = h.commit_hash_short ?? '—'
    const dirty = h.commit_dirty ? '*' : ''
    return `seed ${h.seed} · ${commit}${dirty}`
  }

  function render() {
    destroyCharts()
    if (!host) return
    host.innerHTML = ''
    if (runs.length === 0 || seriesDefs.length === 0) return

    const width = host.clientWidth || 640
    for (const run of runs) {
      const panel = document.createElement('div')
      panel.style.marginBottom = '8px'
      host.appendChild(panel)

      const { xs, ys } = extract(run)
      if (xs.length === 0) continue

      const data: uPlot.AlignedData = [xs, ...ys] as unknown as uPlot.AlignedData
      const opts: uPlot.Options = {
        title: runs.length > 1 ? `${title} · ${runLabel(run)}` : title,
        width,
        height,
        scales: { x: { time: false } },
        axes: [
          { label: 'tick', stroke: '#8a8477' },
          { label: yLabel, stroke: '#8a8477' },
        ],
        series: [
          { label: 'tick' },
          ...seriesDefs.map(d => ({
            label: d.label,
            stroke: d.color,
            width: 1.5,
            spanGaps: false,
          })),
        ],
      }
      charts.push(new uPlot(opts, data, panel))
    }
  }

  $effect(() => {
    void runs; void seriesDefs; void extract; void title; void yLabel; void height
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
