<script lang="ts">
  import type { Mismatch } from '../../lib/logs/diff'

  interface Props {
    mismatches: Mismatch[]
  }

  let { mismatches }: Props = $props()

  function iconFor(kind: string): string {
    if (kind === 'dirty_tree') return '⚠'
    if (kind === 'commit_hash') return '⚡'
    if (kind === 'constants') return '⚙'
    if (kind === 'missing_header') return '?'
    return '·'
  }
</script>

{#if mismatches.length > 0}
  <div class="bg-warning/10 border border-warning/60 rounded-md p-3 text-sm flex flex-col gap-1">
    <div class="font-medium text-warning">Reproducibility check</div>
    {#each mismatches as m}
      <div class="text-txt/90 flex items-baseline gap-2">
        <span class="text-warning font-mono shrink-0">{iconFor(m.kind)}</span>
        <span>{m.message}</span>
      </div>
    {/each}
  </div>
{/if}
