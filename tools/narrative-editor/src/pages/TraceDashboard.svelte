<script lang="ts">
  import { runs, selectedTraceRunId, loadError } from '../stores/runs'
  import {
    activeTraceRun, runsWithTraces, frameIndex, currentFrame,
    focalTick, selectedDse,
  } from '../stores/trace'
  import { stepDecisionTick, decisionTickIndex } from '../lib/logs/trace'
  import FileDropZone from '../components/logs/FileDropZone.svelte'
  import TimelineStrip from '../components/trace/TimelineStrip.svelte'
  import L1Panel from '../components/trace/L1Panel.svelte'
  import L2Panel from '../components/trace/L2Panel.svelte'
  import L3Panel from '../components/trace/L3Panel.svelte'

  let tickInput = $state('')
  $effect(() => {
    tickInput = $focalTick !== null ? String($focalTick) : ''
  })

  function pickRun(e: Event) {
    const target = e.target as HTMLSelectElement
    selectedTraceRunId.set(target.value || null)
    // Reset focal tick so the new run snaps to its own first decision tick.
    focalTick.set(null)
    selectedDse.set(null)
  }

  function step(delta: number) {
    if (!$frameIndex) return
    const current = $focalTick ?? $frameIndex.decisionTicks[0]
    if (current === undefined) return
    focalTick.set(stepDecisionTick($frameIndex, current, delta))
  }

  function submitTickInput() {
    if (!$frameIndex) return
    const n = Number.parseInt(tickInput, 10)
    if (Number.isFinite(n)) focalTick.set(n)
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement) return
    if (e.key === 'ArrowLeft')  { e.preventDefault(); step(e.shiftKey ? -10 : -1) }
    if (e.key === 'ArrowRight') { e.preventDefault(); step(e.shiftKey ?  10 :  1) }
  }

  function runLabel(run: typeof $runs[number]): string {
    const h = run.header
    const focal = run.focalCat ?? '?'
    const seed = h?.seed ?? '?'
    const commit = h?.commit_hash_short ?? '—'
    return `seed ${seed} · ${commit} · ${focal}`
  }

  let decisionIndex = $derived(
    $frameIndex && $focalTick !== null
      ? decisionTickIndex($frameIndex, $focalTick)
      : -1,
  )

  // Auto-snap the focal tick to the active run's first decision tick
  // whenever no tick is pinned. Fires on initial mount and after every
  // pickRun() reset. The panels already fall back via currentFrame's
  // derivation, but the toolbar (tickInput, decisionIndex counter)
  // needs focalTick set explicitly to reflect the true state.
  $effect(() => {
    if ($frameIndex && $focalTick === null && $frameIndex.decisionTicks.length > 0) {
      focalTick.set($frameIndex.decisionTicks[0])
    }
  })
</script>

<svelte:window onkeydown={onKeyDown} />

<section class="flex flex-col gap-4">
  <header>
    <h1 class="m-0 text-xl">Focal-cat Trace Scrubber</h1>
    <p class="m-0 mt-1 text-sm text-muted">
      Drop a <code>trace-&lt;name&gt;.jsonl</code> file below (pair with its
      <code>events.jsonl</code> / <code>narrative.jsonl</code> to keep run
      metadata together). Generate one with
      <code>just soak-trace &lt;seed&gt; &lt;focal&gt;</code>. Arrow keys step
      through decision ticks (Shift for ×10).
    </p>
  </header>

  <FileDropZone />

  {#if $loadError}
    <div class="bg-negative/10 border border-negative text-negative rounded-md p-3 text-sm">
      Load failed: {$loadError}
    </div>
  {/if}

  {#if $runsWithTraces.length === 0}
    <div class="text-muted italic text-sm p-4">
      No focal-cat traces loaded yet. Run <code>just soak-trace 42 Simba</code>
      and drop <code>logs/tuned-42/trace-Simba.jsonl</code> above.
    </div>
  {:else}
    <div class="sticky top-0 z-20 bg-bg/95 backdrop-blur-sm -mx-4 px-4 py-2 border-b border-border">
      <div class="flex flex-wrap items-center gap-3 text-sm">
        <label class="flex items-center gap-2">
          <span class="text-muted">Run:</span>
          <select
            value={$activeTraceRun?.id ?? ''}
            onchange={pickRun}
            class="bg-surface border border-border rounded px-2 py-1 text-sm"
          >
            {#each $runsWithTraces as r (r.id)}
              <option value={r.id}>{runLabel(r)}</option>
            {/each}
          </select>
        </label>

        {#if $frameIndex}
          <span class="text-muted">·</span>
          <label class="flex items-center gap-2">
            <span class="text-muted">Tick:</span>
            <input
              type="text"
              inputmode="numeric"
              class="w-28 bg-surface border border-border rounded px-2 py-1 text-sm font-mono"
              bind:value={tickInput}
              onkeydown={e => e.key === 'Enter' && submitTickInput()}
              onblur={submitTickInput}
            />
          </label>
          <div class="flex items-center gap-1">
            <button type="button" class="text-xs px-2 py-1" onclick={() => step(-10)}>«</button>
            <button type="button" class="text-xs px-2 py-1" onclick={() => step(-1)}>‹</button>
            <button type="button" class="text-xs px-2 py-1" onclick={() => step( 1)}>›</button>
            <button type="button" class="text-xs px-2 py-1" onclick={() => step( 10)}>»</button>
          </div>
          <span class="text-xs text-muted">
            {decisionIndex >= 0 ? decisionIndex + 1 : '?'} / {$frameIndex.decisionTicks.length}
            decision ticks
          </span>
          {#if $frameIndex.commitmentTicks.length > 0}
            <span class="text-xs text-muted">· {$frameIndex.commitmentTicks.length} commitment drops</span>
          {/if}
          {#if $frameIndex.planFailureTicks.length > 0}
            <span class="text-xs text-muted">· {$frameIndex.planFailureTicks.length} plan failures</span>
          {/if}
        {/if}
      </div>
    </div>

    {#if $frameIndex && $currentFrame}
      <TimelineStrip index={$frameIndex} focalTick={$currentFrame.tick} onScrub={(t: number) => focalTick.set(t)} />

      <div class="grid gap-3 grid-cols-1 xl:grid-cols-[minmax(0,1fr)_minmax(0,1.6fr)_minmax(0,1fr)]">
        <div class="min-w-0">
          <h2 class="m-0 mb-2 text-sm uppercase tracking-wider text-muted">L1 · Perception</h2>
          <L1Panel frame={$currentFrame} index={$frameIndex} />
        </div>
        <div class="min-w-0">
          <h2 class="m-0 mb-2 text-sm uppercase tracking-wider text-muted">L2 · DSE scoring</h2>
          <L2Panel frame={$currentFrame} />
        </div>
        <div class="min-w-0">
          <h2 class="m-0 mb-2 text-sm uppercase tracking-wider text-muted">L3 · Decision</h2>
          <L3Panel frame={$currentFrame} />
        </div>
      </div>
    {:else}
      <div class="text-muted italic text-sm p-4">
        Selected run has no decision ticks in its trace.
      </div>
    {/if}
  {/if}
</section>
