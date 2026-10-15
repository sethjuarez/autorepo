import {isRepositorySource} from './repository-source'
import type {LastCollectionSource} from './types'

export function readLastSelectedOwner() {
  try {
    return window.localStorage.getItem('autorepo:last-selected-owner')
  } catch {
    return null
  }
}

export function saveLastSelectedOwner(owner: string) {
  try {
    window.localStorage.setItem('autorepo:last-selected-owner', owner)
  } catch {
    // Ignore storage failures; the picker still works without persistence.
  }
}

export function readLastCollectionSource(): LastCollectionSource | null {
  try {
    const raw = window.localStorage.getItem('autorepo:last-collection-source')
    if (!raw) return null
    const parsed = JSON.parse(raw) as Partial<LastCollectionSource>
    if (!parsed.source?.trim()) return null
    const kind = parsed.kind === 'repo' || parsed.kind === 'folder' ? parsed.kind : isRepositorySource(parsed.source) ? 'repo' : 'folder'
    return {source: parsed.source, kind}
  } catch {
    return null
  }
}

export function saveLastCollectionSource(source: string) {
  const normalized = source.trim()
  if (!normalized) return
  try {
    window.localStorage.setItem(
      'autorepo:last-collection-source',
      JSON.stringify({
        source: normalized,
        kind: isRepositorySource(normalized) ? 'repo' : 'folder',
      } satisfies LastCollectionSource),
    )
  } catch {
    // Ignore storage failures; collection loading still works without persistence.
  }
}
