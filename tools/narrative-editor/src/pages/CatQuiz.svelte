<script lang="ts">
  import QuizQuestion from '../components/QuizQuestion.svelte'
  import TraitBars from '../components/TraitBars.svelte'
  import { QUESTIONS, SCORING, TRAIT_KEYS, type TraitKey } from '../lib/quiz-data'

  // Identity fields
  let catName = $state('')
  let gender = $state('')
  let furColor = $state('')
  let pattern = $state('')
  let eyeColor = $state('')
  let marks = $state('')

  // Quiz answers (-1 = unanswered)
  let answers: number[] = $state(Array(15).fill(-1))

  // Derived trait values
  let traits = $derived.by(() => {
    const result = {} as Record<TraitKey, number>
    for (const t of TRAIT_KEYS) result[t] = 0.5

    for (let q = 0; q < 15; q++) {
      if (answers[q] < 0) continue
      const deltas = SCORING[q][answers[q]]
      for (const [trait, delta] of Object.entries(deltas)) {
        result[trait as TraitKey] += delta as number
      }
    }

    // Clamp to [0, 1] and round
    for (const t of TRAIT_KEYS) {
      result[t] = Math.round(Math.max(0, Math.min(1, result[t])) * 100) / 100
    }
    return result
  })

  let answeredCount = $derived(answers.filter(a => a >= 0).length)
  let ready = $derived(answeredCount === 15 && catName.trim().length > 0 && gender.length > 0)
  let showPreview = $state(false)
  let toastMessage = $state('')
  let toastVisible = $state(false)

  function onAnswer(qi: number, oi: number) {
    answers[qi] = oi
  }

  function buildJSON() {
    const marksArr = marks.split(',').map(s => s.trim()).filter(s => s.length > 0)
    return {
      name: catName.trim() || 'Unnamed',
      gender: gender || 'Nonbinary',
      appearance: {
        fur_color: furColor.trim() || 'tabby brown',
        pattern: pattern.trim() || 'tabby',
        eye_color: eyeColor.trim() || 'amber',
        distinguishing_marks: marksArr,
      },
      personality: { ...traits },
    }
  }

  function jsonString() {
    return JSON.stringify(buildJSON(), null, 2)
  }

  function showToast(msg: string) {
    toastMessage = msg
    toastVisible = true
    setTimeout(() => { toastVisible = false }, 2000)
  }

  function download() {
    const data = buildJSON()
    const json = JSON.stringify(data, null, 2)
    const blob = new Blob([json], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `${data.name.toLowerCase().replace(/\s+/g, '_')}.json`
    a.click()
    URL.revokeObjectURL(url)
    showToast(`Downloaded ${a.download}`)
  }

  function copyToClipboard() {
    navigator.clipboard.writeText(jsonString()).then(() => {
      showToast('Copied to clipboard')
    })
  }

  function reset() {
    catName = ''
    gender = ''
    furColor = ''
    pattern = ''
    eyeColor = ''
    marks = ''
    answers = Array(15).fill(-1)
    showPreview = false
  }
</script>

<h1>Know Thy Cat</h1>
<p class="text-muted italic mb-6 text-base">
  A questionnaire for immortalizing your real-life feline as a colonist in Clowder.
  Answer honestly. Your cat already knows what you're going to pick.
</p>

<h2>Your Cat</h2>

<div class="mb-4">
  <label for="cat-name" class="block font-bold mb-1 text-accent text-sm">Name</label>
  <input type="text" id="cat-name" class="w-full" bind:value={catName}
    placeholder="e.g. Biscuit, Chairman Meow, Lord Fluffington">
</div>

<div class="mb-4">
  <!-- svelte-ignore a11y_label_has_associated_control -->
  <label class="block font-bold mb-1 text-accent text-sm">Gender</label>
  <div class="flex gap-4 flex-wrap">
    <label class="font-normal text-txt cursor-pointer"><input type="radio" name="gender" value="Tom" bind:group={gender} class="accent-accent"> Tom (he/him)</label>
    <label class="font-normal text-txt cursor-pointer"><input type="radio" name="gender" value="Queen" bind:group={gender} class="accent-accent"> Queen (she/her)</label>
    <label class="font-normal text-txt cursor-pointer"><input type="radio" name="gender" value="Nonbinary" bind:group={gender} class="accent-accent"> Nonbinary (they/them)</label>
  </div>
</div>

<div class="mb-4">
  <label for="fur-color" class="block font-bold mb-1 text-accent text-sm">Fur Color</label>
  <input type="text" id="fur-color" class="w-full" bind:value={furColor}
    placeholder='e.g. "orange tabby," "tuxedo black-and-white," "void"'>
</div>

<div class="mb-4">
  <label for="pattern" class="block font-bold mb-1 text-accent text-sm">Pattern</label>
  <input type="text" id="pattern" class="w-full" bind:value={pattern}
    placeholder='e.g. "mackerel," "solid," "calico"'>
</div>

<div class="mb-4">
  <label for="eye-color" class="block font-bold mb-1 text-accent text-sm">Eye Color</label>
  <input type="text" id="eye-color" class="w-full" bind:value={eyeColor}
    placeholder='e.g. "green," "amber," "heterochromia blue/gold"'>
</div>

<div class="mb-4">
  <label for="marks" class="block font-bold mb-1 text-accent text-sm">Distinguishing Marks <span class="text-sm text-muted font-normal">(comma-separated)</span></label>
  <input type="text" id="marks" class="w-full" bind:value={marks}
    placeholder='e.g. "notched left ear, one white toe, perpetual judgment"'>
</div>

<hr class="border-border my-8">

<h2>The Questions</h2>
<p class="text-muted italic mb-6 text-base">
  Answer each scenario with the option that best describes your cat.
  If multiple apply, pick the one that happens most often.
</p>

<div class="flex flex-col lg:flex-row gap-6">
  <div class="flex-1 min-w-0">
    <p class="text-muted text-sm mb-2">{answeredCount} of 15 answered</p>

    {#each QUESTIONS as question, qi}
      <QuizQuestion {question} index={qi} selectedOption={answers[qi]} {onAnswer} />
    {/each}

    <div class="flex gap-3 mt-6 flex-wrap">
      <button class="bg-accent text-bg font-bold hover:bg-accent-hover" disabled={!ready} onclick={download}>Download JSON</button>
      <button disabled={!ready} onclick={copyToClipboard}>Copy to Clipboard</button>
      <button disabled={!ready} onclick={() => showPreview = !showPreview}>
        {showPreview ? 'Hide' : 'Preview'} JSON
      </button>
      <button onclick={reset}>Reset All</button>
    </div>

    {#if showPreview && ready}
      <pre class="bg-surface border border-border rounded-md p-4 mt-4 font-mono text-sm whitespace-pre-wrap break-all max-h-[400px] overflow-y-auto">{jsonString()}</pre>
    {/if}

    <div class="mt-6 p-4 bg-surface border border-border rounded-md text-sm text-muted">
      <strong class="text-accent">What to do with the file</strong>
      <ol class="mt-2 ml-5 p-0">
        <li>Fill out the identity fields and all 15 questions above.</li>
        <li>Click <strong class="text-accent">Download JSON</strong> to save your cat's file.</li>
        <li>Drop the <code>.json</code> file into <code>assets/data/cats/</code> in your Clowder directory.</li>
        <li>Launch the game. Your cat will join the colony, and any remaining slots (up to 8) will be filled with random cats.</li>
      </ol>
    </div>
  </div>

  <div class="lg:w-[280px] lg:shrink-0">
    <div class="lg:sticky lg:top-4">
      <h3 class="text-base text-accent m-0 mb-2">Personality</h3>
      <TraitBars {traits} />
    </div>
  </div>
</div>

<div class="fixed bottom-6 left-1/2 -translate-x-1/2 bg-accent text-bg px-5 py-2 rounded-md text-sm transition-opacity pointer-events-none z-50 {toastVisible ? 'opacity-100' : 'opacity-0'}">{toastMessage}</div>
