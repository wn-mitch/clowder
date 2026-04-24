<script lang="ts">
  import uPlot from 'uplot'
  import 'uplot/dist/uPlot.min.css'
  import { onDestroy } from 'svelte'
  import type { FrameIndex } from '../../lib/logs/trace'
  import { nearestDecisionTick } from '../../lib/logs/trace'

  interface Props {
    index: FrameIndex
    focalTick: number
    onScrub: (tick: number) => void
    height?: number
  }

  let props: Props = $props()

  let host = $state<HTMLDivElement | null>(null)
  let chart: uPlot | null = null

  /** Stable HSL colour per DSE name — same DSE keeps its colour across
   *  frames so your eye can track it. Uses a cheap string hash for hue
   *  selection and fixed saturation/lightness to stay legible on the
   *  dark theme. */
  function dseColor(name: string): string {
    let h = 0
    for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) | 0
    const hue = Math.abs(h) % 360
    return `hsl(${hue}, 62%, 62%)`
  }

  /** Draws vertical rules for event ticks and the focal tick. Runs on
   *  every uPlot redraw via the `draw` hook. All input ticks are
   *  absolute values from `props.index` / `props.focalTick`; the scale
   *  operates in normalized (tick - tickBase) space. */
  function markerPlugin() {
    return {
      hooks: {
        draw: (u: uPlot) => {
          const ctx = u.ctx
          const top = u.bbox.top
          const bottom = u.bbox.top + u.bbox.height

          function rule(absTick: number, color: string, width = 1, dash?: number[]) {
            const x = Math.round(u.valToPos(absTick - tickBase, 'x', true))
            if (x < u.bbox.left || x > u.bbox.left + u.bbox.width) return
            ctx.save()
            ctx.strokeStyle = color
            ctx.lineWidth = width
            if (dash) ctx.setLineDash(dash); else ctx.setLineDash([])
            ctx.beginPath()
            ctx.moveTo(x + 0.5, top)
            ctx.lineTo(x + 0.5, bottom)
            ctx.stroke()
            ctx.restore()
          }

          // Chosen-DSE change markers — subtle amber dashes.
          for (const t of props.index.chosenChangeTicks) rule(t, 'rgba(212, 165, 116, 0.45)', 1, [3, 3])
          // Commitment-drop markers — teal solid.
          for (const t of props.index.commitmentTicks) rule(t, 'rgba(116, 212, 180, 0.55)', 1)
          // Plan-failure markers — red solid.
          for (const t of props.index.planFailureTicks) rule(t, 'rgba(212, 116, 116, 0.7)', 1)
          // Focal tick — bright accent, thicker.
          rule(props.focalTick, 'rgba(235, 200, 100, 0.95)', 2)
        },
      },
    }
  }

  /** Offset used to zero-base the x-axis. uPlot has a long tail of
   *  quirks when the data's x magnitude is large relative to its range
   *  (e.g. ticks 1,200,211 → 1,325,859 — absolute ~1.2M, range ~125k).
   *  Subtracting the first tick keeps the internal scale in a sane
   *  range and we format labels back to absolute for display. */
  let tickBase = 0

  function render() {
    if (!host) return
    if (chart) { chart.destroy(); chart = null }
    host.innerHTML = ''

    if (props.index.decisionTicks.length === 0) {
      const empty = document.createElement('div')
      empty.className = 'text-xs italic text-muted p-4'
      empty.textContent = 'No decision ticks in this trace.'
      host.appendChild(empty)
      return
    }

    tickBase = props.index.decisionTicks[0]
    const xs = props.index.decisionTicks.map(t => t - tickBase)
    const ys = props.index.dseSeries.map(s => s.scores)
    const data: uPlot.AlignedData = [xs, ...ys] as unknown as uPlot.AlignedData

    const MIN_W = 640
    const width = Math.max(host.clientWidth, MIN_W)
    const opts: uPlot.Options = {
      width,
      height: props.height ?? 240,
      scales: { x: { time: false } },
      axes: [
        {
          label: 'tick',
          stroke: '#8a8477',
          values: (_u, splits) => splits.map(s => formatTick(s + tickBase)),
        },
        { label: 'L2 final_score', stroke: '#8a8477' },
      ],
      series: [
        { label: 'tick', value: (_u, v) => formatTick(v + tickBase) },
        ...props.index.dseSeries.map(s => ({
          label: s.dse,
          stroke: dseColor(s.dse),
          width: 1.5,
          spanGaps: false,
        })),
      ],
      plugins: [markerPlugin()],
    }

    chart = new uPlot(opts, data, host)
    wireScrub(chart)
    const applyRealSize = () => {
      if (!chart || !host) return
      const w = host.clientWidth
      const h = props.height ?? 240
      if (w > 0) chart.setSize({ width: w, height: h })
    }
    applyRealSize()
    requestAnimationFrame(applyRealSize)

  }

  function formatTick(n: number): string {
    if (!Number.isFinite(n)) return ''
    return n.toLocaleString()
  }

  /** Attach click + drag handlers to the uPlot overlay so any pointer
   *  event inside the plot body snaps the focal tick to the nearest
   *  decision tick. */
  function wireScrub(u: uPlot) {
    const over = u.over
    let dragging = false

    function snapFromEvent(ev: PointerEvent) {
      const rect = over.getBoundingClientRect()
      const x = ev.clientX - rect.left
      const tickVal = u.posToVal(x, 'x')
      // posToVal returns normalized space; snap against absolute ticks.
      const absolute = Math.round(tickVal) + tickBase
      const snapped = nearestDecisionTick(props.index, absolute)
      if (snapped !== null) props.onScrub(snapped)
    }

    over.addEventListener('pointerdown', ev => {
      dragging = true
      ;(ev.target as Element).setPointerCapture?.(ev.pointerId)
      snapFromEvent(ev)
    })
    over.addEventListener('pointermove', ev => {
      if (!dragging) return
      snapFromEvent(ev)
    })
    over.addEventListener('pointerup', ev => {
      dragging = false
      ;(ev.target as Element).releasePointerCapture?.(ev.pointerId)
    })
    over.addEventListener('pointercancel', () => { dragging = false })
    over.style.cursor = 'col-resize'
  }

  // Full re-render on index change (new run / different focal cat).
  // Defer with requestAnimationFrame so the chart construction happens
  // AFTER Svelte's update cycle completes. Constructing inside the
  // effect synchronously causes uPlot's scale auto-range to silently
  // produce null scales and [0,0] series idxs — verified empirically:
  // the exact same data + options work when the construction is
  // deferred or invoked outside the effect.
  let pendingRenderFrame = 0
  $effect(() => {
    void props.index
    if (pendingRenderFrame) cancelAnimationFrame(pendingRenderFrame)
    pendingRenderFrame = requestAnimationFrame(() => {
      pendingRenderFrame = 0
      render()
    })
    return () => {
      if (pendingRenderFrame) {
        cancelAnimationFrame(pendingRenderFrame)
        pendingRenderFrame = 0
      }
    }
  })

  // Lightweight redraw when only the focal tick moves — avoids tearing
  // down the uPlot instance. The marker plugin reads props.focalTick
  // directly, so we just nudge uPlot to redraw.
  $effect(() => {
    void props.focalTick
    chart?.redraw()
  })

  $effect(() => {
    if (!host) return
    const el = host
    const h = props.height ?? 240
    const ro = new ResizeObserver(() => {
      const w = el.clientWidth
      // Guard against transient zero widths — uPlot's setSize(0,h)
      // leaves the chart with null scales and breaks line drawing
      // until re-construction.
      if (w > 0 && chart) chart.setSize({ width: w, height: h })
    })
    ro.observe(el)
    return () => ro.disconnect()
  })

  onDestroy(() => { chart?.destroy(); chart = null })
</script>

<div bind:this={host} class="bg-surface border border-border rounded-md p-2 w-full"></div>

<style>
  :global(.u-legend) { color: var(--color-muted); font-size: 0.75rem; }
  :global(.u-title) { color: var(--color-accent); font-weight: 600; }
  :global(.u-axis) { color: var(--color-muted); }
  :global(.uplot) { background: transparent; }
</style>
