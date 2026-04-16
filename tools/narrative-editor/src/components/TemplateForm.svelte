<script lang="ts">
  import {
    selectedFile, selectedFileName, selectedTemplateIndex, selectedTemplate,
    updateTemplate,
  } from '../stores/templates'
  import {
    TIERS, ACTIONS, DAY_PHASES, SEASONS, WEATHERS, MOODS,
    LIFE_STAGES, TERRAINS, PERSONALITY_AXES, PERSONALITY_BUCKETS,
    NEED_AXES, NEED_LEVELS, TEMPLATE_VARIABLES,
  } from '../lib/schema'
  import type { NarrativeTemplate, PersonalityReq, NeedReq } from '../lib/types'

  let showVarRef = $state(false)

  function update(field: string, value: unknown) {
    if ($selectedFileName && $selectedTemplateIndex !== null) {
      updateTemplate($selectedFileName, $selectedTemplateIndex, { [field]: value })
    }
  }

  function updateText(e: Event) {
    update('text', (e.target as HTMLTextAreaElement).value)
  }

  function updateOptional(field: string, value: string) {
    update(field, value === '' ? undefined : value)
  }

  function updateOptionalBool(field: string, value: string) {
    if (value === '') update(field, undefined)
    else update(field, value === 'true')
  }

  function addPersonality() {
    if (!$selectedTemplate) return
    const reqs = [...$selectedTemplate.personality, { axis: 'Boldness' as const, bucket: 'High' as const }]
    update('personality', reqs)
  }

  function removePersonality(idx: number) {
    if (!$selectedTemplate) return
    const reqs = $selectedTemplate.personality.filter((_, i) => i !== idx)
    update('personality', reqs)
  }

  function updatePersonality(idx: number, field: keyof PersonalityReq, value: string) {
    if (!$selectedTemplate) return
    const reqs = $selectedTemplate.personality.map((r, i) =>
      i === idx ? { ...r, [field]: value } : r
    )
    update('personality', reqs)
  }

  function addNeed() {
    if (!$selectedTemplate) return
    const reqs = [...$selectedTemplate.needs, { axis: 'Hunger' as const, level: 'Critical' as const }]
    update('needs', reqs)
  }

  function removeNeed(idx: number) {
    if (!$selectedTemplate) return
    const reqs = $selectedTemplate.needs.filter((_, i) => i !== idx)
    update('needs', reqs)
  }

  function updateNeed(idx: number, field: keyof NeedReq, value: string) {
    if (!$selectedTemplate) return
    const reqs = $selectedTemplate.needs.map((r, i) =>
      i === idx ? { ...r, [field]: value } : r
    )
    update('needs', reqs)
  }

  function insertVariable(name: string) {
    const textarea = document.querySelector('.template-text') as HTMLTextAreaElement
    if (!textarea || !$selectedTemplate) return
    const start = textarea.selectionStart
    const end = textarea.selectionEnd
    const text = $selectedTemplate.text
    const newText = text.slice(0, start) + `{${name}}` + text.slice(end)
    update('text', newText)
    // Restore cursor position after the inserted variable
    requestAnimationFrame(() => {
      textarea.focus()
      const newPos = start + name.length + 2
      textarea.setSelectionRange(newPos, newPos)
    })
  }

  // Preview sample text
  let previewText = $derived.by(() => {
    if (!$selectedTemplate) return ''
    return $selectedTemplate.text
      .replace(/\{name\}/g, 'Bramble')
      .replace(/\{Subject\}/g, 'She')
      .replace(/\{subject\}/g, 'she')
      .replace(/\{object\}/g, 'her')
      .replace(/\{possessive\}/g, 'her')
      .replace(/\{other\}/g, 'Reed')
      .replace(/\{weather_desc\}/g, $selectedTemplate.weather ?? 'Clear')
      .replace(/\{time_desc\}/g, $selectedTemplate.day_phase ?? 'Day')
      .replace(/\{season\}/g, $selectedTemplate.season ?? 'Summer')
      .replace(/\{life_stage\}/g, $selectedTemplate.life_stage?.toLowerCase() ?? 'cat')
      .replace(/\{fur_color\}/g, 'tortoiseshell')
      .replace(/\{prey\}/g, 'vole')
      .replace(/\{item\}/g, 'berries')
      .replace(/\{quality\}/g, 'fine')
  })
</script>

{#if $selectedTemplate}
  {@const t = $selectedTemplate}

  <div class="bg-surface border border-border rounded-md p-5">
    <!-- Text -->
    <div class="mb-4">
      <div class="flex items-center justify-between mb-1">
        <label for="template-text" class="block text-xs font-bold text-accent mb-0 uppercase tracking-wide">Template Text</label>
        <button class="p-0 border-none bg-transparent text-accent text-sm cursor-pointer underline hover:text-accent-hover hover:bg-transparent" onclick={() => showVarRef = !showVarRef}>
          {showVarRef ? 'Hide' : 'Show'} Variables
        </button>
      </div>

      {#if showVarRef}
        <div class="flex flex-wrap gap-1 mb-2">
          {#each TEMPLATE_VARIABLES as v}
            <button
              class="px-2 py-0.5 text-xs font-mono border border-border bg-bg text-accent rounded-sm cursor-pointer hover:bg-accent hover:text-bg"
              title={v.description}
              onclick={() => insertVariable(v.name)}
            >
              {`{${v.name}}`}
            </button>
          {/each}
        </div>
      {/if}

      <textarea
        id="template-text"
        class="template-text w-full text-base leading-normal resize-y"
        rows="3"
        value={t.text}
        oninput={updateText}
      ></textarea>

      <div class="mt-2 px-3 py-2 bg-bg rounded-md text-sm italic text-txt">
        <span class="text-muted not-italic text-xs uppercase mr-2">Preview:</span>
        {previewText}
      </div>
    </div>

    <!-- Tier + Weight row -->
    <div class="flex gap-4 mb-4">
      <div class="mb-4" style="flex: 2">
        <label class="block text-xs font-bold text-accent mb-1 uppercase tracking-wide">Tier</label>
        <div class="flex gap-3 flex-wrap">
          {#each TIERS as tier}
            <label class="inline-flex items-center gap-1 text-sm cursor-pointer text-txt font-normal normal-case tracking-normal" title={tier.description}>
              <input
                class="accent-accent"
                type="radio"
                name="tier"
                value={tier.value}
                checked={t.tier === tier.value}
                onchange={() => update('tier', tier.value)}
              >
              {tier.label}
            </label>
          {/each}
        </div>
      </div>

      <div class="mb-4" style="flex: 1">
        <label for="weight" class="block text-xs font-bold text-accent mb-1 uppercase tracking-wide">Weight</label>
        <input
          id="weight"
          class="w-full"
          type="number"
          min="0.1"
          max="10"
          step="0.1"
          value={t.weight}
          onchange={(e) => update('weight', parseFloat((e.target as HTMLInputElement).value) || 1.0)}
        >
      </div>
    </div>

    <!-- Condition dropdowns -->
    <h3 class="text-base mt-5 mb-1 pt-3 border-t border-border">Conditions</h3>
    <p class="text-sm text-muted italic -mt-2 mb-3">Leave as "Any" for wildcard matching (template applies regardless of that axis).</p>

    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 mb-4">
      <div class="mb-0">
        <label class="block text-xs font-bold text-accent mb-1 uppercase tracking-wide">Action</label>
        <select
          class="w-full"
          value={t.action ?? ''}
          onchange={(e) => updateOptional('action', (e.target as HTMLSelectElement).value)}
        >
          <option value="">Any</option>
          {#each ACTIONS as a}
            <option value={a.value}>{a.label}</option>
          {/each}
        </select>
      </div>

      <div class="mb-0">
        <label class="block text-xs font-bold text-accent mb-1 uppercase tracking-wide">Event Tag</label>
        <input
          class="w-full"
          type="text"
          placeholder="e.g. catch, miss, scent"
          value={t.event ?? ''}
          oninput={(e) => {
            const v = (e.target as HTMLInputElement).value.trim()
            update('event', v || undefined)
          }}
        >
      </div>

      <div class="mb-0">
        <label class="block text-xs font-bold text-accent mb-1 uppercase tracking-wide">Day Phase</label>
        <select
          class="w-full"
          value={t.day_phase ?? ''}
          onchange={(e) => updateOptional('day_phase', (e.target as HTMLSelectElement).value)}
        >
          <option value="">Any</option>
          {#each DAY_PHASES as dp}
            <option value={dp.value}>{dp.label}</option>
          {/each}
        </select>
      </div>

      <div class="mb-0">
        <label class="block text-xs font-bold text-accent mb-1 uppercase tracking-wide">Season</label>
        <select
          class="w-full"
          value={t.season ?? ''}
          onchange={(e) => updateOptional('season', (e.target as HTMLSelectElement).value)}
        >
          <option value="">Any</option>
          {#each SEASONS as s}
            <option value={s.value}>{s.label}</option>
          {/each}
        </select>
      </div>

      <div class="mb-0">
        <label class="block text-xs font-bold text-accent mb-1 uppercase tracking-wide">Weather</label>
        <select
          class="w-full"
          value={t.weather ?? ''}
          onchange={(e) => updateOptional('weather', (e.target as HTMLSelectElement).value)}
        >
          <option value="">Any</option>
          {#each WEATHERS as w}
            <option value={w.value}>{w.label}</option>
          {/each}
        </select>
      </div>

      <div class="mb-0">
        <label class="block text-xs font-bold text-accent mb-1 uppercase tracking-wide">Mood</label>
        <select
          class="w-full"
          value={t.mood ?? ''}
          onchange={(e) => updateOptional('mood', (e.target as HTMLSelectElement).value)}
        >
          <option value="">Any</option>
          {#each MOODS as m}
            <option value={m.value}>{m.label}</option>
          {/each}
        </select>
      </div>

      <div class="mb-0">
        <label class="block text-xs font-bold text-accent mb-1 uppercase tracking-wide">Life Stage</label>
        <select
          class="w-full"
          value={t.life_stage ?? ''}
          onchange={(e) => updateOptional('life_stage', (e.target as HTMLSelectElement).value)}
        >
          <option value="">Any</option>
          {#each LIFE_STAGES as ls}
            <option value={ls.value}>{ls.label}</option>
          {/each}
        </select>
      </div>

      <div class="mb-0">
        <label class="block text-xs font-bold text-accent mb-1 uppercase tracking-wide">Terrain</label>
        <select
          class="w-full"
          value={t.terrain ?? ''}
          onchange={(e) => updateOptional('terrain', (e.target as HTMLSelectElement).value)}
        >
          <option value="">Any</option>
          {#each TERRAINS as tr}
            <option value={tr.value}>{tr.label}</option>
          {/each}
        </select>
      </div>

      <div class="mb-0">
        <label class="block text-xs font-bold text-accent mb-1 uppercase tracking-wide">Has Target</label>
        <select
          class="w-full"
          value={t.has_target === undefined ? '' : String(t.has_target)}
          onchange={(e) => updateOptionalBool('has_target', (e.target as HTMLSelectElement).value)}
        >
          <option value="">Any</option>
          <option value="true">Yes</option>
          <option value="false">No</option>
        </select>
      </div>
    </div>

    <!-- Personality Requirements -->
    <div class="mb-4">
      <div class="flex items-center justify-between mb-1">
        <label class="block text-xs font-bold text-accent mb-0 uppercase tracking-wide">Personality Requirements</label>
        <button class="p-0 border-none bg-transparent text-accent text-sm cursor-pointer underline hover:text-accent-hover hover:bg-transparent" onclick={addPersonality}>+ Add</button>
      </div>
      {#each t.personality as req, i}
        <div class="flex gap-2 mb-1.5 items-center">
          <select
            class="flex-1"
            value={req.axis}
            onchange={(e) => updatePersonality(i, 'axis', (e.target as HTMLSelectElement).value)}
          >
            {#each PERSONALITY_AXES as axis}
              <option value={axis.value}>{axis.label} ({axis.lowLabel} &#8594; {axis.highLabel})</option>
            {/each}
          </select>
          <select
            class="flex-1"
            value={req.bucket}
            onchange={(e) => updatePersonality(i, 'bucket', (e.target as HTMLSelectElement).value)}
          >
            {#each PERSONALITY_BUCKETS as b}
              <option value={b.value}>{b.label}</option>
            {/each}
          </select>
          <button class="px-2 py-0.5 text-base border-none bg-transparent text-muted rounded-sm cursor-pointer shrink-0 hover:text-negative hover:bg-transparent" onclick={() => removePersonality(i)}>&times;</button>
        </div>
      {/each}
    </div>

    <!-- Needs Requirements -->
    <div class="mb-4">
      <div class="flex items-center justify-between mb-1">
        <label class="block text-xs font-bold text-accent mb-0 uppercase tracking-wide">Needs Requirements</label>
        <button class="p-0 border-none bg-transparent text-accent text-sm cursor-pointer underline hover:text-accent-hover hover:bg-transparent" onclick={addNeed}>+ Add</button>
      </div>
      {#each t.needs as req, i}
        <div class="flex gap-2 mb-1.5 items-center">
          <select
            class="flex-1"
            value={req.axis}
            onchange={(e) => updateNeed(i, 'axis', (e.target as HTMLSelectElement).value)}
          >
            {#each NEED_AXES as axis}
              <option value={axis.value}>{axis.label}</option>
            {/each}
          </select>
          <select
            class="flex-1"
            value={req.level}
            onchange={(e) => updateNeed(i, 'level', (e.target as HTMLSelectElement).value)}
          >
            {#each NEED_LEVELS as l}
              <option value={l.value}>{l.label}</option>
            {/each}
          </select>
          <button class="px-2 py-0.5 text-base border-none bg-transparent text-muted rounded-sm cursor-pointer shrink-0 hover:text-negative hover:bg-transparent" onclick={() => removeNeed(i)}>&times;</button>
        </div>
      {/each}
    </div>
  </div>
{/if}
