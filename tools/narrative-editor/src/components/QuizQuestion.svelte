<script lang="ts">
  import type { QuizQuestion } from '../lib/quiz-data'

  interface Props {
    question: QuizQuestion
    index: number
    selectedOption: number
    onAnswer: (questionIndex: number, optionIndex: number) => void
  }

  let { question, index, selectedOption, onAnswer }: Props = $props()

  const letters = 'abcde'
</script>

<div class="bg-surface border border-border rounded-md p-5 mb-4 {selectedOption >= 0 ? 'border-accent border-l-[3px]' : ''}">
  <h3 class="m-0 mb-1 text-lg"><span class="text-accent font-bold">{index + 1}.</span> {question.title}</h3>
  <p class="text-muted text-sm mb-3">{question.flavor}</p>
  {#each question.options as option, oi}
    <label class="block px-3 py-2 my-1 rounded cursor-pointer transition-colors text-sm hover:bg-surface-alt">
      <input
        type="radio"
        name="q{index}"
        value={oi}
        checked={selectedOption === oi}
        onchange={() => onAnswer(index, oi)}
        class="mr-2 accent-accent"
      >
      <span class="font-bold text-accent mr-1">{letters[oi]})</span> {option}
    </label>
  {/each}
</div>
