<script lang="ts">
  import type { Frame } from '../../lib/logs/trace'

  interface Props { frame: Frame }
  let { frame }: Props = $props()

  let l3 = $derived(frame.l3)

  // Pair ranked scores with softmax probabilities by index. Rust side
  // emits them as parallel arrays — indices are stable.
  let rows = $derived.by(() => {
    if (!l3) return []
    const probs = l3.softmax.probabilities
    return l3.ranked.map(([name, score], i) => ({
      name,
      score,
      prob: probs[i] ?? 0,
      chosen: name === l3?.chosen,
    }))
  })

  let maxScore = $derived(Math.max(0.0001, ...rows.map(r => r.score)))

  function fmt(n: number, d = 3) {
    if (!Number.isFinite(n)) return '—'
    return n.toFixed(d)
  }

  function intentionText(): string {
    if (!l3) return ''
    const parts: string[] = [l3.intention.kind]
    if (l3.intention.target) parts.push(`→ ${l3.intention.target}`)
    if (l3.intention.goal_state) parts.push(`⟨${l3.intention.goal_state}⟩`)
    return parts.join(' ')
  }
</script>

<div class="flex flex-col gap-3">
  {#if !l3}
    <div class="text-xs italic text-muted p-3 bg-surface border border-border rounded">
      No L3 record at this tick.
    </div>
  {:else}
    <!-- Ranked dispositions: horizontal bar chart -->
    <div class="bg-surface border border-border rounded-md p-3">
      <div class="flex items-baseline justify-between mb-2">
        <span class="text-sm font-semibold">Ranked dispositions</span>
        <span class="text-xs text-muted">T = {fmt(l3.softmax.temperature, 3)}</span>
      </div>
      <div class="flex flex-col gap-1.5">
        {#each rows as row (row.name)}
          <div class="grid grid-cols-[6rem_1fr_3rem_3rem] items-center gap-2 text-xs font-mono">
            <span class="truncate {row.chosen ? 'text-accent font-semibold' : 'text-txt'}" title={row.name}>
              {row.chosen ? '★' : ' '} {row.name}
            </span>
            <div class="relative h-4 bg-surface-alt rounded overflow-hidden">
              <div
                class="absolute left-0 top-0 bottom-0 {row.chosen ? 'bg-accent/70' : 'bg-accent/30'}"
                style="width: {Math.min(100, (row.score / maxScore) * 100)}%"
              ></div>
            </div>
            <span class="text-right text-muted tabular-nums">{fmt(row.score, 3)}</span>
            <span class="text-right text-muted tabular-nums" title="softmax probability">
              {fmt(row.prob * 100, 1)}%
            </span>
          </div>
        {/each}
      </div>
    </div>

    <!-- Chosen + intention + GOAP plan -->
    <div class="bg-surface border border-border rounded-md p-3">
      <div class="text-sm font-semibold mb-1">Chosen: <span class="text-accent">{l3.chosen}</span></div>
      <div class="text-xs text-muted mb-2">{intentionText()}</div>
      <div class="text-xs uppercase tracking-wider text-muted mb-1">GOAP plan</div>
      {#if l3.goap_plan.length === 0}
        <div class="text-xs italic text-muted">(no steps)</div>
      {:else}
        <ol class="list-decimal list-inside text-xs font-mono text-txt m-0 p-0">
          {#each l3.goap_plan as step, i (i + ':' + step)}
            <li class="py-0.5">{step}</li>
          {/each}
        </ol>
      {/if}
    </div>

    <!-- Momentum / commitment state -->
    <div class="bg-surface border border-border rounded-md p-3 text-xs">
      <div class="uppercase tracking-wider text-muted mb-1">Momentum</div>
      <div class="grid grid-cols-2 gap-1 font-mono">
        <span class="text-muted">active intention</span>
        <span class="text-right">{l3.momentum.active_intention ?? '—'}</span>
        <span class="text-muted">commitment strength</span>
        <span class="text-right tabular-nums">{fmt(l3.momentum.commitment_strength)}</span>
        <span class="text-muted">margin threshold</span>
        <span class="text-right tabular-nums">{fmt(l3.momentum.margin_threshold)}</span>
        <span class="text-muted">preempted</span>
        <span class="text-right">{l3.momentum.preempted ? 'yes' : 'no'}</span>
      </div>
    </div>
  {/if}

  <!-- Commitment-gate callouts -->
  {#each frame.commitment as c, i (i)}
    <div class="border border-border rounded-md p-3 text-xs bg-positive/5">
      <div class="font-semibold mb-1">
        Commitment gate: <span class="text-accent">{c.branch}</span>
        {c.dropped ? '(dropped)' : '(retained)'}
      </div>
      <div class="grid grid-cols-2 gap-1 font-mono">
        <span class="text-muted">disposition</span>
        <span class="text-right">{c.disposition}</span>
        <span class="text-muted">strategy</span>
        <span class="text-right">{c.strategy}</span>
        <span class="text-muted">achievement_believed</span>
        <span class="text-right">{c.proxies.achievement_believed}</span>
        <span class="text-muted">achievable_believed</span>
        <span class="text-right">{c.proxies.achievable_believed}</span>
        <span class="text-muted">still_goal</span>
        <span class="text-right">{c.proxies.still_goal}</span>
        <span class="text-muted">trips_done / target</span>
        <span class="text-right tabular-nums">{c.plan_state.trips_done} / {c.plan_state.target_trips}</span>
        <span class="text-muted">replan / max</span>
        <span class="text-right tabular-nums">{c.plan_state.replan_count} / {c.plan_state.max_replans}</span>
      </div>
    </div>
  {/each}

  <!-- Plan-failure callouts -->
  {#each frame.planFailure as f, i (i)}
    <div class="border border-negative/60 rounded-md p-3 text-xs bg-negative/5">
      <div class="font-semibold mb-1">
        Plan failure: <span class="text-negative">{f.reason}</span>
      </div>
      <div class="grid grid-cols-2 gap-1 font-mono">
        <span class="text-muted">disposition</span>
        <span class="text-right">{f.disposition}</span>
      </div>
      {#if f.detail && typeof f.detail === 'object'}
        <pre class="m-0 mt-1 p-2 bg-surface-alt rounded text-[10px] overflow-x-auto">{JSON.stringify(f.detail, null, 2)}</pre>
      {/if}
    </div>
  {/each}
</div>
