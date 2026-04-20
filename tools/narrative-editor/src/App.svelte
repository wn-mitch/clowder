<script lang="ts">
  import Nav from './components/Nav.svelte'
  import CatQuiz from './pages/CatQuiz.svelte'
  import TemplateEditor from './pages/TemplateEditor.svelte'
  import LogsDashboard from './pages/LogsDashboard.svelte'

  const PAGES = ['templates', 'quiz', 'logs'] as const
  type Page = typeof PAGES[number]

  let page = $state<Page>(getPageFromHash())

  function getPageFromHash(): Page {
    const hash = window.location.hash.replace('#/', '')
    return (PAGES as readonly string[]).includes(hash) ? hash as Page : 'templates'
  }

  function navigate(target: string) {
    window.location.hash = `#/${target}`
    page = getPageFromHash()
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
  {:else if page === 'logs'}
    <LogsDashboard />
  {:else}
    <TemplateEditor />
  {/if}
</main>
