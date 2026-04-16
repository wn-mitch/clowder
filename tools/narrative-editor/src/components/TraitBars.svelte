<script lang="ts">
  import { TRAIT_KEYS, TRAIT_SECTIONS, TRAIT_COLORS, type TraitKey } from '../lib/quiz-data'

  interface Props {
    traits: Record<TraitKey, number>
  }

  let { traits }: Props = $props()
</script>

<div class="bg-surface border border-border rounded-md p-4">
  {#each TRAIT_SECTIONS as section}
    <div class="text-[0.7rem] uppercase tracking-widest text-accent mt-2 mb-0.5 first:mt-0">{section.label}</div>
    {#each TRAIT_KEYS.slice(section.start, section.end) as trait}
      <div class="flex items-center my-0.5 text-sm">
        <span class="w-[100px] text-right pr-2 text-muted shrink-0">{trait}</span>
        <div class="flex-1 h-3.5 bg-bar-bg rounded-sm relative overflow-hidden">
          <div class="absolute left-1/2 top-0 bottom-0 w-px bg-border"></div>
          <div
            class="h-full rounded-sm transition-all duration-300 min-w-0.5"
            style="width: {traits[trait] * 100}%; background: {TRAIT_COLORS[trait]}"
          ></div>
        </div>
        <span class="w-9 text-right pl-1.5 font-mono text-xs text-muted shrink-0">{traits[trait].toFixed(2)}</span>
      </div>
    {/each}
  {/each}
</div>
