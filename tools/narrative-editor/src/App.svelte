<script lang="ts">
  import Nav from './components/Nav.svelte'
  import CatQuiz from './pages/CatQuiz.svelte'
  import TemplateEditor from './pages/TemplateEditor.svelte'

  let page = $state(getPageFromHash())

  function getPageFromHash(): string {
    const hash = window.location.hash.replace('#/', '')
    return hash === 'quiz' ? 'quiz' : 'templates'
  }

  function navigate(target: string) {
    window.location.hash = `#/${target}`
    page = target
  }

  $effect(() => {
    const onHashChange = () => { page = getPageFromHash() }
    window.addEventListener('hashchange', onHashChange)
    return () => window.removeEventListener('hashchange', onHashChange)
  })
</script>

<Nav currentPage={page} onNavigate={navigate} />

<main class="max-w-6xl mx-auto p-6">
  {#if page === 'quiz'}
    <CatQuiz />
  {:else}
    <TemplateEditor />
  {/if}
</main>
