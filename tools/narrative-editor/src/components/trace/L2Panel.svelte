<script lang="ts">
  import type { Frame, L2Record, ConsiderationContribution, ModifierApplication } from '../../lib/logs/trace'
  import { selectedDse } from '../../stores/trace'

  interface Props { frame: Frame }
  let { frame }: Props = $props()

  let chosenDisposition = $derived(frame.l3?.chosen ?? null)

  /** L3 emits title-case disposition names (`"Hunt"`) while L2 emits
   *  lowercase DSE ids (`"hunt"`, `"groom_self"`, `"groom_other"`).
   *  Match case-insensitively plus underscore-prefix so disposition
   *  groups like Groom → {groom_self, groom_other} highlight together. */
  function isChosenDse(dseName: string): boolean {
    if (!chosenDisposition) return false
    const c = chosenDisposition.toLowerCase()
    const d = dseName.toLowerCase()
    return d === c || d.startsWith(c + '_')
  }

  let sorted = $derived.by(() => {
    const eligible = frame.l2.filter(r => r.eligibility.passed)
    const ineligible = frame.l2.filter(r => !r.eligibility.passed)
    eligible.sort((a, b) => b.final_score - a.final_score)
    return { eligible, ineligible }
  })

  function fmt(n: number, d = 3) {
    if (!Number.isFinite(n)) return '—'
    return n.toFixed(d)
  }

  function toggleExpand(dse: string) {
    selectedDse.update(cur => cur === dse ? null : dse)
  }

  /** Shorten Rust-debug-style curve strings for display, e.g.
   *  `"Logistic { steepness: 8.0, midpoint: 0.75 }"` →
   *  `"Logistic(8, 0.75)"`. Leaves unrecognised shapes verbatim so we
   *  never lie about the underlying curve. */
  function curveLabel(raw: string): string {
    const logistic = raw.match(/^Logistic\s*\{\s*steepness:\s*([\d.-]+),\s*midpoint:\s*([\d.-]+)\s*\}$/)
    if (logistic) return `Logistic(${+logistic[1]}, ${+logistic[2]})`
    const quad = raw.match(/^Quadratic\s*\{\s*exponent:\s*([\d.-]+),\s*divisor:\s*([\d.-]+),\s*shift:\s*([\d.-]+)\s*\}$/)
    if (quad) return `Quadratic(${+quad[1]}, ${+quad[2]}, ${+quad[3]})`
    const lin = raw.match(/^Linear\s*\{\s*slope:\s*([\d.-]+),\s*intercept:\s*([\d.-]+)\s*\}$/)
    if (lin) return `Linear(${+lin[1]}, ${+lin[2]})`
    const logit = raw.match(/^Logit\s*\{\s*slope:\s*([\d.-]+),\s*inflection:\s*([\d.-]+)\s*\}$/)
    if (logit) return `Logit(${+logit[1]}, ${+logit[2]})`
    return raw
  }

  function modifierText(m: ModifierApplication): string {
    if (m.delta !== undefined) return `${m.name} ${m.delta >= 0 ? '+' : ''}${fmt(m.delta, 3)}`
    if (m.multiplier !== undefined) return `${m.name} ×${fmt(m.multiplier, 3)}`
    return m.name
  }

  function considerationTitle(c: ConsiderationContribution): string {
    return `input=${fmt(c.input, 3)} · curve=${c.curve} · score=${fmt(c.score, 3)} · weight=${fmt(c.weight, 2)}`
  }

  function intentionText(r: L2Record): string {
    const parts: string[] = [r.intention.kind]
    if (r.intention.target) parts.push(`→ ${r.intention.target}`)
    if (r.intention.goal_state) parts.push(`⟨${r.intention.goal_state}⟩`)
    return parts.join(' ')
  }
</script>

<div class="flex flex-col gap-2">
  {#if frame.l2.length === 0}
    <div class="text-xs italic text-muted p-3 bg-surface border border-border rounded">
      No L2 records at this tick.
    </div>
  {:else}
    {#each sorted.eligible as row (row.dse)}
      {@const chosen = isChosenDse(row.dse)}
      {@const expanded = $selectedDse === row.dse}
      <div class="bg-surface border {chosen ? 'border-accent' : 'border-border'} rounded-md p-2.5 text-xs">
        <button
          type="button"
          class="w-full flex items-center gap-2 text-left bg-transparent border-none p-0 cursor-pointer"
          onclick={() => toggleExpand(row.dse)}
        >
          <span class="text-base leading-none w-4 {chosen ? 'text-accent' : 'text-muted'}">{chosen ? '★' : '·'}</span>
          <span class="font-mono font-semibold {chosen ? 'text-accent' : 'text-txt'}">{row.dse}</span>
          <span class="text-muted">· {row.composition.mode}</span>
          {#if row.modifiers.length > 0}
            <span class="text-muted">· {row.modifiers.length} mod{row.modifiers.length === 1 ? '' : 's'}</span>
          {/if}
          <span class="ml-auto flex items-center gap-2">
            <div class="w-24 h-1.5 bg-surface-alt rounded overflow-hidden" title={`final_score=${fmt(row.final_score, 4)}`}>
              <div class="h-full {chosen ? 'bg-accent' : 'bg-accent/50'}"
                   style="width: {Math.min(100, Math.max(0, row.final_score) * 100)}%"></div>
            </div>
            <span class="font-mono tabular-nums w-14 text-right">{fmt(row.final_score, 3)}</span>
            <span class="text-muted w-3">{expanded ? '▾' : '▸'}</span>
          </span>
        </button>

        {#if expanded}
          <div class="mt-2 pt-2 border-t border-border flex flex-col gap-1.5">
            {#if row.eligibility.markers_required.length > 0}
              <div class="text-muted">
                markers: <span class="font-mono">[{row.eligibility.markers_required.join(', ')}]</span>
              </div>
            {/if}

            <div class="text-muted uppercase tracking-wider text-[10px]">Considerations</div>
            {#each row.considerations as c (c.name)}
              <div class="grid grid-cols-[7rem_1fr_4.5rem_3rem] items-center gap-2 font-mono" title={considerationTitle(c)}>
                <span class="truncate text-txt">{c.name}</span>
                <div class="relative h-3 bg-surface-alt rounded overflow-hidden">
                  <!-- input bar, muted -->
                  <div class="absolute left-0 top-0 bottom-0 bg-muted/30"
                       style="width: {Math.min(100, c.input * 100)}%"></div>
                  <!-- score bar, accent, overlaid -->
                  <div class="absolute left-0 top-0 bottom-0 bg-accent/60"
                       style="width: {Math.min(100, c.score * 100)}%"></div>
                </div>
                <span class="text-[10px] text-muted truncate" title={c.curve}>{curveLabel(c.curve)}</span>
                <span class="text-right tabular-nums">×{fmt(c.weight, 2)}</span>
              </div>
            {/each}

            <!-- Pipeline chain: raw → maslow × → modifiers → final -->
            <div class="flex flex-wrap items-center gap-1.5 font-mono mt-1 text-[11px]">
              <span class="text-muted">raw</span>
              <span class="tabular-nums">{fmt(row.composition.raw, 3)}</span>
              <span class="text-muted">→</span>
              <span class="text-muted">maslow×</span>
              <span class="tabular-nums">{fmt(row.maslow_pregate, 2)}</span>
              {#each row.modifiers as m (m.name)}
                <span class="text-muted">→</span>
                <span class="{(m.delta ?? m.multiplier ?? 0) < 0 ? 'text-negative' : 'text-positive'}">
                  {modifierText(m)}
                </span>
              {/each}
              <span class="text-muted">→</span>
              <span class="font-semibold {chosen ? 'text-accent' : 'text-txt'} tabular-nums">
                final {fmt(row.final_score, 3)}
              </span>
            </div>

            <div class="text-muted text-[11px]">
              intention: {intentionText(row)}
            </div>

            {#if row.targets}
              <div class="mt-1 pt-1 border-t border-border">
                <div class="text-muted uppercase tracking-wider text-[10px] mb-1">
                  Targets · {row.targets.aggregation}{row.targets.winner ? ' · winner ' + row.targets.winner : ''}
                </div>
                {#each row.targets.candidates as t (t.name)}
                  <div class="grid grid-cols-[1fr_3rem_3rem] gap-2 font-mono text-[11px]">
                    <span class="truncate {t.contributed ? 'text-txt' : 'text-muted'}">{t.name}</span>
                    <span class="text-right tabular-nums">{fmt(t.score, 3)}</span>
                    <span class="text-right text-muted">{t.contributed ? 'in' : 'out'}</span>
                  </div>
                {/each}
              </div>
            {/if}

            {#if row.top_losing.length > 0}
              <div class="mt-1 pt-1 border-t border-border">
                <div class="text-muted uppercase tracking-wider text-[10px] mb-1">Top losing axes</div>
                {#each row.top_losing as l (l.axis)}
                  <div class="flex gap-2 font-mono text-[11px]">
                    <span>{l.axis}</span>
                    <span class="tabular-nums">{fmt(l.score, 3)}</span>
                    <span class="text-muted tabular-nums">(deficit {fmt(l.deficit, 3)})</span>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {/each}

    {#if sorted.ineligible.length > 0}
      <div class="text-xs text-muted p-2 border border-dashed border-border rounded">
        <span class="uppercase tracking-wider text-[10px]">Filtered by eligibility</span> ·
        <span class="font-mono">
          {sorted.ineligible.map(r => {
            const req = r.eligibility.markers_required.join(',')
            return req ? `${r.dse}[${req}]` : r.dse
          }).join(' · ')}
        </span>
      </div>
    {/if}
  {/if}
</div>
