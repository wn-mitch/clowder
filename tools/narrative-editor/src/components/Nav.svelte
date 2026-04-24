<script lang="ts">
  interface Props {
    currentPage: string
    onNavigate: (page: string) => void
  }

  let { currentPage, onNavigate }: Props = $props()
</script>

<nav class="flex items-center gap-8 px-6 py-3 bg-surface border-b border-border">
  <span class="text-lg font-bold text-accent whitespace-nowrap">Clowder Writer's Toolkit</span>
  <div class="flex gap-1">
    <button
      class="px-4 py-1.5 border-none bg-transparent text-muted text-sm rounded-md cursor-pointer transition-all duration-150 hover:text-txt hover:bg-surface-alt {currentPage === 'templates' ? 'text-accent bg-bg' : ''}"
      onclick={() => onNavigate('templates')}
    >
      Template Editor
    </button>
    <button
      class="px-4 py-1.5 border-none bg-transparent text-muted text-sm rounded-md cursor-pointer transition-all duration-150 hover:text-txt hover:bg-surface-alt {currentPage === 'quiz' ? 'text-accent bg-bg' : ''}"
      onclick={() => onNavigate('quiz')}
    >
      Know Thy Cat
    </button>
    <button
      class="px-4 py-1.5 border-none bg-transparent text-muted text-sm rounded-md cursor-pointer transition-all duration-150 hover:text-txt hover:bg-surface-alt {currentPage === 'logs' ? 'text-accent bg-bg' : ''}"
      onclick={() => onNavigate('logs')}
    >
      Logs
    </button>
    <button
      class="px-4 py-1.5 border-none bg-transparent text-muted text-sm rounded-md cursor-pointer transition-all duration-150 hover:text-txt hover:bg-surface-alt {currentPage === 'trace' ? 'text-accent bg-bg' : ''}"
      onclick={() => onNavigate('trace')}
    >
      Focal trace
    </button>
  </div>
</nav>
