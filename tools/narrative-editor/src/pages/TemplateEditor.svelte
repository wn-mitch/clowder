<script lang="ts">
  import FileManager from '../components/FileManager.svelte'
  import TemplateList from '../components/TemplateList.svelte'
  import TemplateForm from '../components/TemplateForm.svelte'
  import CoverageView from '../components/CoverageView.svelte'
  import { selectedFile, selectedTemplate, totalTemplateCount } from '../stores/templates'

  type Tab = 'edit' | 'coverage'
  let activeTab = $state<Tab>('edit')
</script>

<div class="flex flex-col lg:flex-row gap-4 min-h-[calc(100vh-100px)]">
  <aside class="lg:w-60 lg:shrink-0">
    <FileManager />
  </aside>

  <div class="flex-1 min-w-0">
    <div class="flex items-center gap-1 mb-4 border-b border-border pb-2">
      <button
        class={activeTab === 'edit'
          ? 'px-4 py-1.5 border-none bg-surface text-accent text-sm rounded-t-md cursor-pointer transition-all duration-150 border-b-2 border-accent'
          : 'px-4 py-1.5 border-none bg-transparent text-muted text-sm rounded-t-md cursor-pointer transition-all duration-150 hover:text-txt hover:bg-surface-alt'}
        onclick={() => activeTab = 'edit'}
      >
        Edit Templates
      </button>
      <button
        class={activeTab === 'coverage'
          ? 'px-4 py-1.5 border-none bg-surface text-accent text-sm rounded-t-md cursor-pointer transition-all duration-150 border-b-2 border-accent'
          : 'px-4 py-1.5 border-none bg-transparent text-muted text-sm rounded-t-md cursor-pointer transition-all duration-150 hover:text-txt hover:bg-surface-alt'}
        onclick={() => activeTab = 'coverage'}
      >
        Coverage
      </button>
      {#if $totalTemplateCount > 0}
        <span class="ml-auto text-sm text-muted">{$totalTemplateCount} templates loaded</span>
      {/if}
    </div>

    {#if activeTab === 'edit'}
      <div class="flex flex-col lg:flex-row gap-4">
        <div class="lg:w-80 lg:shrink-0 max-h-[300px] lg:max-h-[calc(100vh-160px)] overflow-y-auto">
          <TemplateList />
        </div>
        <div class="flex-1 min-w-0">
          {#if $selectedTemplate}
            <TemplateForm />
          {:else if $selectedFile}
            <div class="flex items-center justify-center min-h-[300px] text-muted italic">
              <p>Select a template from the list, or add a new one</p>
            </div>
          {:else}
            <div class="flex items-center justify-center min-h-[300px] text-muted italic">
              <p>Import <code>.ron</code> files to start editing</p>
            </div>
          {/if}
        </div>
      </div>
    {:else}
      <CoverageView />
    {/if}
  </div>
</div>
