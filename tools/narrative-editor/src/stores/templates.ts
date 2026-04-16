// Svelte store for loaded template files and editor state.

import { writable, derived } from 'svelte/store'
import type { NarrativeTemplate, TemplateFile } from '../lib/types'
import { parseRon } from '../lib/ron-parser'
import { serializeRon } from '../lib/ron-serializer'

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

export const files = writable<Map<string, TemplateFile>>(new Map())
export const selectedFileName = writable<string | null>(null)
export const selectedTemplateIndex = writable<number | null>(null)

// ---------------------------------------------------------------------------
// Derived
// ---------------------------------------------------------------------------

export const selectedFile = derived(
  [files, selectedFileName],
  ([$files, $name]) => $name ? $files.get($name) ?? null : null,
)

export const selectedTemplate = derived(
  [selectedFile, selectedTemplateIndex],
  ([$file, $index]) => {
    if (!$file || $index === null || $index < 0 || $index >= $file.templates.length) return null
    return $file.templates[$index]
  },
)

export const allTemplates = derived(files, ($files) => {
  const all: NarrativeTemplate[] = []
  for (const file of $files.values()) {
    all.push(...file.templates)
  }
  return all
})

export const totalTemplateCount = derived(allTemplates, ($all) => $all.length)

// ---------------------------------------------------------------------------
// GitHub source loading
// ---------------------------------------------------------------------------

const GITHUB_OWNER = 'wn-mitch'
const GITHUB_REPO = 'clowder'
const GITHUB_BRANCH = 'main'
const NARRATIVE_PATH = 'assets/narrative'

export const loadingFromGithub = writable(false)
export const githubError = writable<string | null>(null)

export async function loadFromGithub() {
  loadingFromGithub.set(true)
  githubError.set(null)

  try {
    // List .ron files via GitHub Contents API
    const listUrl = `https://api.github.com/repos/${GITHUB_OWNER}/${GITHUB_REPO}/contents/${NARRATIVE_PATH}?ref=${GITHUB_BRANCH}`
    const listRes = await fetch(listUrl)
    if (!listRes.ok) throw new Error(`GitHub API returned ${listRes.status}`)
    const entries: { name: string; download_url: string }[] = await listRes.json()

    const ronEntries = entries.filter(e => e.name.endsWith('.ron'))

    // Fetch all .ron files in parallel via raw URLs (no rate limit)
    const results = await Promise.allSettled(
      ronEntries.map(async (entry) => {
        const rawUrl = `https://raw.githubusercontent.com/${GITHUB_OWNER}/${GITHUB_REPO}/${GITHUB_BRANCH}/${NARRATIVE_PATH}/${entry.name}`
        const res = await fetch(rawUrl)
        if (!res.ok) throw new Error(`${entry.name}: ${res.status}`)
        const text = await res.text()
        const { templates, headerComment } = parseRon(text)
        return {
          name: entry.name,
          templates,
          dirty: false,
          _headerComment: headerComment,
        } as TemplateFile
      })
    )

    const loaded = new Map<string, TemplateFile>()
    const errors: string[] = []
    for (const result of results) {
      if (result.status === 'fulfilled') {
        loaded.set(result.value.name, result.value)
      } else {
        errors.push(result.reason?.message ?? 'Unknown error')
      }
    }

    if (errors.length > 0) {
      console.warn('Some files failed to load:', errors)
    }

    files.set(loaded)

    // Auto-select first file
    if (loaded.size > 0) {
      selectedFileName.set(Array.from(loaded.keys()).sort()[0])
    }
  } catch (e) {
    githubError.set((e as Error).message)
    console.error('Failed to load from GitHub:', e)
  } finally {
    loadingFromGithub.set(false)
  }
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

export async function importFiles(fileList: FileList) {
  const newFiles = new Map<string, TemplateFile>()

  for (const file of Array.from(fileList)) {
    if (!file.name.endsWith('.ron')) continue
    const text = await file.text()
    try {
      const { templates, headerComment } = parseRon(text)
      newFiles.set(file.name, {
        name: file.name,
        templates,
        dirty: false,
        _headerComment: headerComment,
      })
    } catch (e) {
      console.error(`Failed to parse ${file.name}:`, e)
      alert(`Failed to parse ${file.name}: ${(e as Error).message}`)
    }
  }

  files.update($files => {
    for (const [name, file] of newFiles) {
      $files.set(name, file)
    }
    return new Map($files)
  })

  // Auto-select first imported file if none selected
  selectedFileName.update($name => {
    if ($name === null && newFiles.size > 0) {
      return Array.from(newFiles.keys())[0]
    }
    return $name
  })
}

export function exportFile(name: string) {
  let file: TemplateFile | undefined
  files.subscribe($files => { file = $files.get(name) })()

  if (!file) return

  const ron = serializeRon(file.templates, file._headerComment)
  const blob = new Blob([ron], { type: 'text/plain' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = name
  a.click()
  URL.revokeObjectURL(url)

  // Mark as not dirty after export
  files.update($files => {
    const f = $files.get(name)
    if (f) f.dirty = false
    return new Map($files)
  })
}

export function addTemplate(fileName: string) {
  const template: NarrativeTemplate = {
    text: '{name} ',
    tier: 'Micro',
    weight: 1.0,
    personality: [],
    needs: [],
  }

  files.update($files => {
    const file = $files.get(fileName)
    if (file) {
      file.templates.push(template)
      file.dirty = true
    }
    return new Map($files)
  })

  // Select the new template
  files.subscribe($files => {
    const file = $files.get(fileName)
    if (file) {
      selectedTemplateIndex.set(file.templates.length - 1)
    }
  })()
}

export function updateTemplate(fileName: string, index: number, updates: Partial<NarrativeTemplate>) {
  files.update($files => {
    const file = $files.get(fileName)
    if (file && index >= 0 && index < file.templates.length) {
      file.templates[index] = { ...file.templates[index], ...updates }
      file.dirty = true
    }
    return new Map($files)
  })
}

export function deleteTemplate(fileName: string, index: number) {
  files.update($files => {
    const file = $files.get(fileName)
    if (file) {
      file.templates.splice(index, 1)
      file.dirty = true
    }
    return new Map($files)
  })

  selectedTemplateIndex.update($index => {
    if ($index === index) return null
    if ($index !== null && $index > index) return $index - 1
    return $index
  })
}

export function duplicateTemplate(fileName: string, index: number) {
  files.update($files => {
    const file = $files.get(fileName)
    if (file && index >= 0 && index < file.templates.length) {
      const copy = structuredClone(file.templates[index])
      delete copy._comment // Don't duplicate comments
      file.templates.splice(index + 1, 0, copy)
      file.dirty = true
    }
    return new Map($files)
  })

  selectedTemplateIndex.set(index + 1)
}

export function removeFile(name: string) {
  files.update($files => {
    $files.delete(name)
    return new Map($files)
  })

  selectedFileName.update($name => {
    if ($name === name) return null
    return $name
  })

  selectedTemplateIndex.set(null)
}
